//! Profile-generation-aware ownership of multiple independent client runtimes.
//!
//! Each [`ClientRuntime`] still owns exactly one relay connection. This crate
//! resolves stable node IDs and aliases, eagerly starts the default node, and
//! starts other referenced nodes only when policy selects them.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::{Mutex, RwLock};
use velum_client_runtime::{
    ClientConfig, ClientRuntime, RuntimeError, RuntimePhase, RuntimeStream,
};

type StartFuture =
    Pin<Box<dyn Future<Output = Result<Arc<ClientRuntime>, NodePoolError>> + Send + 'static>>;

/// Starts one runtime and returns only after it is online.
pub trait RuntimeFactory: Send + Sync + 'static {
    fn start(&self, configuration: ClientConfig) -> StartFuture;
}

/// Production runtime factory backed by the existing single-node lifecycle.
#[derive(Default)]
pub struct ClientRuntimeFactory;

impl RuntimeFactory for ClientRuntimeFactory {
    fn start(&self, configuration: ClientConfig) -> StartFuture {
        Box::pin(async move {
            let runtime = Arc::new(ClientRuntime::new());
            let mut snapshots = runtime.subscribe();
            runtime
                .start(configuration)
                .await
                .map_err(NodePoolError::Runtime)?;
            loop {
                let snapshot = *snapshots.borrow_and_update();
                match snapshot.phase {
                    RuntimePhase::Online => return Ok(runtime),
                    RuntimePhase::Failed | RuntimePhase::Stopped => {
                        return Err(NodePoolError::ConnectionFailed);
                    }
                    RuntimePhase::Connecting | RuntimePhase::Stopping => {}
                }
                snapshots
                    .changed()
                    .await
                    .map_err(|_| NodePoolError::ConnectionFailed)?;
            }
        })
    }
}

/// One fully resolved node. Secret references are resolved before this boundary.
pub struct ResolvedNode {
    pub id: String,
    pub alias: String,
    pub configuration: ClientConfig,
}

struct NodeEntry {
    configuration: ClientConfig,
    runtime: Mutex<Option<Arc<ClientRuntime>>>,
}

#[derive(Default)]
struct PoolState {
    generation: u64,
    default_node: String,
    nodes: BTreeMap<String, Arc<NodeEntry>>,
    aliases: BTreeMap<String, String>,
}

/// Stable, payload-free failures for node selection and connection ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodePoolError {
    Empty,
    DuplicateNode,
    MissingDefault,
    UnknownNode,
    Superseded,
    ConnectionFailed,
    Runtime(RuntimeError),
}

/// A pool of independent runtimes owned by one immutable profile generation.
pub struct NodePool {
    factory: Arc<dyn RuntimeFactory>,
    state: RwLock<PoolState>,
    next_generation: AtomicU64,
}

impl Default for NodePool {
    fn default() -> Self {
        Self::new(Arc::new(ClientRuntimeFactory))
    }
}

impl NodePool {
    pub fn new(factory: Arc<dyn RuntimeFactory>) -> Self {
        Self {
            factory,
            state: RwLock::new(PoolState::default()),
            next_generation: AtomicU64::new(1),
        }
    }

    /// Atomically replaces the profile and eagerly connects its default node.
    pub async fn activate(
        &self,
        nodes: Vec<ResolvedNode>,
        default_node: &str,
    ) -> Result<u64, NodePoolError> {
        let prepared = Self::prepare(nodes, default_node)?;
        let default_entry = prepared
            .nodes
            .get(&prepared.default_node)
            .cloned()
            .ok_or(NodePoolError::MissingDefault)?;
        let default_runtime = self
            .factory
            .start(default_entry.configuration.clone())
            .await?;
        *default_entry.runtime.lock().await = Some(default_runtime);
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let previous = {
            let mut state = self.state.write().await;
            std::mem::replace(
                &mut *state,
                PoolState {
                    generation,
                    ..prepared
                },
            )
        };
        Self::stop_state(previous).await;
        Ok(generation)
    }

    pub async fn generation(&self) -> u64 {
        self.state.read().await.generation
    }

    /// Resolves an ID or alias and starts that node once for this generation.
    pub async fn runtime_for(&self, reference: &str) -> Result<Arc<ClientRuntime>, NodePoolError> {
        let generation = self.generation().await;
        self.runtime_for_generation(reference, generation).await
    }

    pub async fn open_stream(
        &self,
        reference: &str,
        target: SocketAddr,
    ) -> Result<RuntimeStream, NodePoolError> {
        let generation = self.generation().await;
        let runtime = self.runtime_for_generation(reference, generation).await?;
        let stream = runtime
            .open_stream(target)
            .await
            .map_err(NodePoolError::Runtime)?;
        if self.generation().await != generation {
            return Err(NodePoolError::Superseded);
        }
        Ok(stream)
    }

    pub async fn stop(&self) {
        let previous = {
            let mut state = self.state.write().await;
            let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
            std::mem::replace(
                &mut *state,
                PoolState {
                    generation,
                    ..PoolState::default()
                },
            )
        };
        Self::stop_state(previous).await;
    }

    fn prepare(nodes: Vec<ResolvedNode>, default_node: &str) -> Result<PoolState, NodePoolError> {
        if nodes.is_empty() {
            return Err(NodePoolError::Empty);
        }
        let mut ids = BTreeSet::new();
        let mut aliases = BTreeMap::new();
        let mut entries = BTreeMap::new();
        for node in nodes {
            if node.id.is_empty()
                || node.alias.is_empty()
                || !ids.insert(node.id.clone())
                || aliases.contains_key(&node.alias)
            {
                return Err(NodePoolError::DuplicateNode);
            }
            aliases.insert(node.alias, node.id.clone());
            entries.insert(
                node.id,
                Arc::new(NodeEntry {
                    configuration: node.configuration,
                    runtime: Mutex::new(None),
                }),
            );
        }
        let resolved_default = if entries.contains_key(default_node) {
            default_node.to_owned()
        } else {
            aliases
                .get(default_node)
                .cloned()
                .ok_or(NodePoolError::MissingDefault)?
        };
        Ok(PoolState {
            generation: 0,
            default_node: resolved_default,
            nodes: entries,
            aliases,
        })
    }

    async fn runtime_for_generation(
        &self,
        reference: &str,
        generation: u64,
    ) -> Result<Arc<ClientRuntime>, NodePoolError> {
        let entry = {
            let state = self.state.read().await;
            if state.generation != generation {
                return Err(NodePoolError::Superseded);
            }
            let id = if reference == "PROXY" {
                &state.default_node
            } else if state.nodes.contains_key(reference) {
                reference
            } else {
                state
                    .aliases
                    .get(reference)
                    .map(String::as_str)
                    .ok_or(NodePoolError::UnknownNode)?
            };
            state
                .nodes
                .get(id)
                .cloned()
                .ok_or(NodePoolError::UnknownNode)?
        };
        let mut active = entry.runtime.lock().await;
        if let Some(runtime) = active.as_ref() {
            return Ok(Arc::clone(runtime));
        }
        let runtime = self.factory.start(entry.configuration.clone()).await?;
        if self.generation().await != generation {
            runtime.stop().await;
            return Err(NodePoolError::Superseded);
        }
        *active = Some(Arc::clone(&runtime));
        Ok(runtime)
    }

    async fn stop_state(state: PoolState) {
        for entry in state.nodes.into_values() {
            if let Some(runtime) = entry.runtime.lock().await.take() {
                runtime.stop().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::atomic::AtomicUsize, time::Duration};

    use velum_client_runtime::ClientTrust;

    use super::*;

    struct FakeFactory(Arc<AtomicUsize>);

    impl RuntimeFactory for FakeFactory {
        fn start(&self, _: ClientConfig) -> StartFuture {
            let starts = Arc::clone(&self.0);
            Box::pin(async move {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(Arc::new(ClientRuntime::new()))
            })
        }
    }

    fn node(id: &str, alias: &str) -> ResolvedNode {
        ResolvedNode {
            id: id.into(),
            alias: alias.into(),
            configuration: ClientConfig::new(
                "192.0.2.1:443".parse().expect("relay"),
                "relay.example".into(),
                vec![7],
                ClientTrust::System,
                Duration::from_secs(1),
            )
            .expect("configuration"),
        }
    }

    #[tokio::test]
    async fn eagerly_starts_default_and_lazily_starts_alias_target() {
        let starts = Arc::new(AtomicUsize::new(0));
        let pool = NodePool::new(Arc::new(FakeFactory(Arc::clone(&starts))));

        let generation = pool
            .activate(
                vec![node("one", "primary"), node("two", "backup")],
                "primary",
            )
            .await
            .expect("activate");

        assert_eq!(generation, pool.generation().await);
        assert_eq!(starts.load(Ordering::Relaxed), 1);
        pool.runtime_for("backup").await.expect("lazy node");
        pool.runtime_for("two").await.expect("same node by id");
        assert_eq!(starts.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn rejects_duplicate_aliases_and_missing_defaults() {
        let pool = NodePool::new(Arc::new(FakeFactory(Arc::new(AtomicUsize::new(0)))));
        assert_eq!(
            pool.activate(vec![node("one", "same"), node("two", "same")], "one")
                .await,
            Err(NodePoolError::DuplicateNode)
        );
        assert_eq!(
            pool.activate(vec![node("one", "primary")], "missing").await,
            Err(NodePoolError::MissingDefault)
        );
    }
}
