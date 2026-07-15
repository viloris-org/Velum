//! Client lifecycle authority above the experimental direct QUIC API.
//!
//! The runtime owns connection state and the active direct client. Platform
//! hosts consume immutable snapshots and bounded flow operations; presentation
//! code must not infer transport state independently.

mod state;

use std::{future::Future, net::SocketAddr, sync::Arc};

use state::Lifecycle;
use tokio::{
    runtime::Handle,
    sync::{Mutex, watch},
    task::JoinHandle,
};
use velum_client_api::Client;

pub use state::{RuntimeError, RuntimeFailure, RuntimePhase, RuntimeSnapshot};
pub use velum_client_api::{
    ClientConfig, ClientConfigError, ClientDatagram, ClientError, ClientTrust, DatagramSessionId,
};

struct LifecycleTask {
    generation: u64,
    handle: JoinHandle<()>,
}

#[derive(Default)]
struct RuntimeInner {
    lifecycle: Lifecycle,
    client: Option<Arc<Client>>,
    lifecycle_task: Option<LifecycleTask>,
}

/// A reliable flow tied to the runtime generation that opened it.
pub struct RuntimeStream {
    generation: u64,
    inner: velum_client_api::ClientStream,
}

impl RuntimeStream {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        self.inner.write_all(bytes).await
    }

    pub async fn read(&mut self, bytes: &mut [u8]) -> Result<Option<usize>, ClientError> {
        self.inner.read(bytes).await
    }

    pub fn finish(&mut self) -> Result<(), ClientError> {
        self.inner.finish()
    }

    pub fn into_parts(self) -> (u64, RuntimeSendStream, RuntimeReceiveStream) {
        let (send, receive) = self.inner.split();
        (
            self.generation,
            RuntimeSendStream { inner: send },
            RuntimeReceiveStream { inner: receive },
        )
    }
}

/// Independently synchronized send half of a runtime stream.
pub struct RuntimeSendStream {
    inner: velum_client_api::ClientSendStream,
}

impl RuntimeSendStream {
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        self.inner.write_all(bytes).await
    }

    pub fn finish(&mut self) -> Result<(), ClientError> {
        self.inner.finish()
    }
}

/// Independently synchronized receive half of a runtime stream.
pub struct RuntimeReceiveStream {
    inner: velum_client_api::ClientReceiveStream,
}

impl RuntimeReceiveStream {
    pub async fn read(&mut self, bytes: &mut [u8]) -> Result<Option<usize>, ClientError> {
        self.inner.read(bytes).await
    }
}

/// Owns one direct client and publishes its lifecycle as latest-value state.
pub struct ClientRuntime {
    inner: Mutex<RuntimeInner>,
    snapshots: watch::Sender<RuntimeSnapshot>,
}

impl Default for ClientRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientRuntime {
    pub fn new() -> Self {
        let (snapshots, _) = watch::channel(RuntimeSnapshot::default());
        Self {
            inner: Mutex::new(RuntimeInner::default()),
            snapshots,
        }
    }

    /// Returns the current immutable snapshot without waiting for a transition.
    pub fn snapshot(&self) -> RuntimeSnapshot {
        *self.snapshots.borrow()
    }

    /// Subscribes to latest-value lifecycle snapshots.
    pub fn subscribe(&self) -> watch::Receiver<RuntimeSnapshot> {
        self.snapshots.subscribe()
    }

    /// Accepts a connection command and returns before network establishment.
    ///
    /// The returned generation is already visible as `Connecting`. This method
    /// must run inside a Tokio runtime because the runtime owns the resulting
    /// connection task.
    pub async fn start(self: &Arc<Self>, configuration: ClientConfig) -> Result<u64, RuntimeError> {
        self.start_with(Client::connect(configuration)).await
    }

    async fn start_with<F>(self: &Arc<Self>, operation: F) -> Result<u64, RuntimeError>
    where
        F: Future<Output = Result<Client, ClientError>> + Send + 'static,
    {
        let executor = Handle::try_current().map_err(|_| RuntimeError::ExecutorUnavailable)?;
        let mut inner = self.inner.lock().await;
        let generation = inner.lifecycle.begin_connect()?;
        self.publish(inner.lifecycle.snapshot);
        let runtime = Arc::downgrade(self);
        let handle = executor.spawn(async move {
            let result = operation.await;
            if let Some(runtime) = runtime.upgrade() {
                if let Some(client) = runtime.complete_connect(generation, result).await {
                    let failure = client.closed().await.into();
                    runtime
                        .complete_connection_closed(generation, failure)
                        .await;
                }
            } else if let Ok(client) = result {
                client.close();
            }
        });
        inner.lifecycle_task = Some(LifecycleTask { generation, handle });
        Ok(generation)
    }

    async fn complete_connect(
        &self,
        generation: u64,
        result: Result<Client, ClientError>,
    ) -> Option<Arc<Client>> {
        let mut inner = self.inner.lock().await;
        if generation != inner.lifecycle.snapshot.generation
            || inner.lifecycle.snapshot.phase != RuntimePhase::Connecting
        {
            drop(inner);
            if let Ok(client) = result {
                client.close();
            }
            return None;
        }
        let client = match result {
            Ok(client) => {
                let applied = inner.lifecycle.complete_connect(generation, Ok(()));
                debug_assert!(applied);
                let client = Arc::new(client);
                inner.client = Some(Arc::clone(&client));
                Some(client)
            }
            Err(error) => {
                let applied = inner
                    .lifecycle
                    .complete_connect(generation, Err(error.into()));
                debug_assert!(applied);
                None
            }
        };
        if client.is_none() {
            Self::clear_lifecycle_task(&mut inner, generation);
        }
        self.publish(inner.lifecycle.snapshot);
        client
    }

    async fn complete_connection_closed(&self, generation: u64, failure: RuntimeFailure) {
        let mut inner = self.inner.lock().await;
        if !inner.lifecycle.fail_online(generation, failure) {
            return;
        }
        inner.client = None;
        Self::clear_lifecycle_task(&mut inner, generation);
        self.publish(inner.lifecycle.snapshot);
    }

    /// Invalidates and joins in-flight work, closes the client, and ends stopped.
    pub async fn stop(&self) -> u64 {
        let (generation, task) = {
            let mut inner = self.inner.lock().await;
            let Some(generation) = inner.lifecycle.begin_stop() else {
                return inner.lifecycle.snapshot.generation;
            };
            self.publish(inner.lifecycle.snapshot);
            let task = inner.lifecycle_task.take();
            if let Some(task) = &task {
                task.handle.abort();
            }
            if let Some(client) = inner.client.take() {
                client.close();
            }
            let completed = inner.lifecycle.complete_stop(generation);
            debug_assert!(completed);
            self.publish(inner.lifecycle.snapshot);
            (generation, task)
        };
        if let Some(task) = task {
            let _ = task.handle.await;
        }
        generation
    }

    /// Opens a reliable flow through the active direct client.
    pub async fn open_stream(&self, target: SocketAddr) -> Result<RuntimeStream, RuntimeError> {
        let (generation, client) = self.active_client().await?;
        let inner = client
            .open_stream(target)
            .await
            .map_err(RuntimeError::Client)?;
        if !self.is_generation_online(generation) {
            return Err(RuntimeError::Superseded);
        }
        Ok(RuntimeStream { generation, inner })
    }

    /// Sends one explicitly unreliable datagram through the active client.
    pub async fn send_datagram(
        &self,
        session_id: DatagramSessionId,
        destination: SocketAddr,
        payload: &[u8],
    ) -> Result<(), RuntimeError> {
        self.active_client()
            .await?
            .1
            .send_datagram(session_id, destination, payload)
            .map_err(RuntimeError::Client)
    }

    /// Receives one authenticated datagram from the active client.
    pub async fn receive_datagram(&self) -> Result<ClientDatagram, RuntimeError> {
        let (generation, client) = self.active_client().await?;
        let datagram = client
            .receive_datagram()
            .await
            .map_err(RuntimeError::Client)?;
        if !self.is_generation_online(generation) {
            return Err(RuntimeError::Superseded);
        }
        Ok(datagram)
    }

    /// Reports whether a flow generation may still publish native handles.
    pub fn is_generation_online(&self, generation: u64) -> bool {
        let snapshot = self.snapshot();
        snapshot.generation == generation && snapshot.phase == RuntimePhase::Online
    }

    async fn active_client(&self) -> Result<(u64, Arc<Client>), RuntimeError> {
        let inner = self.inner.lock().await;
        if inner.lifecycle.snapshot.phase != RuntimePhase::Online {
            return Err(RuntimeError::NotOnline);
        }
        inner
            .client
            .clone()
            .map(|client| (inner.lifecycle.snapshot.generation, client))
            .ok_or(RuntimeError::NotOnline)
    }

    fn publish(&self, snapshot: RuntimeSnapshot) {
        self.snapshots.send_replace(snapshot);
    }

    fn clear_lifecycle_task(inner: &mut RuntimeInner, generation: u64) {
        if inner
            .lifecycle_task
            .as_ref()
            .is_some_and(|task| task.generation == generation)
        {
            inner.lifecycle_task = None;
        }
    }
}

impl Drop for ClientRuntime {
    fn drop(&mut self) {
        let inner = self.inner.get_mut();
        if let Some(task) = inner.lifecycle_task.take() {
            task.handle.abort();
        }
        if let Some(client) = inner.client.take() {
            client.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use tokio::sync::oneshot;

    use super::*;

    struct DropSignal(Option<oneshot::Sender<()>>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[test]
    fn subscribers_observe_the_latest_snapshot() {
        let runtime = ClientRuntime::new();
        let receiver = runtime.subscribe();
        let snapshot = RuntimeSnapshot {
            revision: 1,
            generation: 1,
            phase: RuntimePhase::Connecting,
            failure: None,
        };

        runtime.publish(snapshot);

        assert_eq!(*receiver.borrow(), snapshot);
        assert_eq!(runtime.snapshot(), snapshot);
    }

    #[tokio::test]
    async fn start_publishes_connecting_before_returning() {
        let runtime = Arc::new(ClientRuntime::new());
        let generation = runtime
            .start_with(pending::<Result<Client, ClientError>>())
            .await
            .expect("start accepted");

        assert_eq!(runtime.snapshot().generation, generation);
        assert_eq!(runtime.snapshot().phase, RuntimePhase::Connecting);

        runtime.stop().await;
    }

    #[tokio::test]
    async fn stop_aborts_and_joins_pending_connection_work() {
        let runtime = Arc::new(ClientRuntime::new());
        let (sender, receiver) = oneshot::channel();
        let signal = DropSignal(Some(sender));
        runtime
            .start_with(async move {
                let _signal = signal;
                pending::<Result<Client, ClientError>>().await
            })
            .await
            .expect("start accepted");

        let generation = runtime.stop().await;

        receiver.await.expect("connection future dropped");
        assert_eq!(runtime.snapshot().generation, generation);
        assert_eq!(runtime.snapshot().phase, RuntimePhase::Stopped);
        assert_eq!(runtime.snapshot().failure, None);
    }

    #[tokio::test]
    async fn parallel_start_is_rejected_while_connection_work_is_active() {
        let runtime = Arc::new(ClientRuntime::new());
        runtime
            .start_with(pending::<Result<Client, ClientError>>())
            .await
            .expect("first start accepted");

        assert_eq!(
            runtime
                .start_with(pending::<Result<Client, ClientError>>())
                .await,
            Err(RuntimeError::Busy)
        );

        runtime.stop().await;
    }

    #[tokio::test]
    async fn dropping_runtime_aborts_pending_connection_work() {
        let runtime = Arc::new(ClientRuntime::new());
        let (sender, receiver) = oneshot::channel();
        let signal = DropSignal(Some(sender));
        runtime
            .start_with(async move {
                let _signal = signal;
                pending::<Result<Client, ClientError>>().await
            })
            .await
            .expect("start accepted");

        drop(runtime);

        receiver.await.expect("connection future dropped");
    }

    #[tokio::test]
    async fn asynchronous_failure_is_published_without_retaining_a_task() {
        let runtime = Arc::new(ClientRuntime::new());
        let mut snapshots = runtime.subscribe();
        runtime
            .start_with(async { Err(ClientError::Connection) })
            .await
            .expect("start accepted");

        while snapshots.borrow().phase == RuntimePhase::Connecting {
            snapshots.changed().await.expect("snapshot channel open");
        }

        assert_eq!(snapshots.borrow().phase, RuntimePhase::Failed);
        assert_eq!(snapshots.borrow().failure, Some(RuntimeFailure::Connection));
    }

    #[tokio::test]
    async fn transport_closure_retires_the_active_generation() {
        let runtime = ClientRuntime::new();
        let generation = {
            let mut inner = runtime.inner.lock().await;
            let generation = inner.lifecycle.begin_connect().expect("begin connect");
            assert!(inner.lifecycle.complete_connect(generation, Ok(())));
            runtime.publish(inner.lifecycle.snapshot);
            generation
        };

        runtime
            .complete_connection_closed(generation, RuntimeFailure::Connection)
            .await;

        assert_eq!(runtime.snapshot().phase, RuntimePhase::Failed);
        assert_eq!(runtime.snapshot().failure, Some(RuntimeFailure::Connection));
        assert!(!runtime.is_generation_online(generation));
    }
}
