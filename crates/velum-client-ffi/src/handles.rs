use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

use tokio::sync::watch;
use velum_adapter_proxy::ProxyAdapter;
use velum_client_engine::NodePool;
use velum_client_runtime::{ClientRuntime, RuntimeReceiveStream, RuntimeSendStream, RuntimeStream};

pub(crate) struct ClientCommandState {
    pub(crate) destroyed: bool,
}

pub(crate) struct ClientEntry {
    pub(crate) runtime: Arc<ClientRuntime>,
    pub(crate) command: Mutex<ClientCommandState>,
    pub(crate) proxy: Mutex<Option<ProxyAdapter>>,
}

pub(crate) struct EngineEntry {
    pub(crate) pool: Arc<NodePool>,
    pub(crate) command: Mutex<ClientCommandState>,
    pub(crate) proxy: Mutex<Option<ProxyAdapter>>,
}

impl EngineEntry {
    pub(crate) fn new(pool: Arc<NodePool>) -> Self {
        Self {
            pool,
            command: Mutex::new(ClientCommandState { destroyed: false }),
            proxy: Mutex::new(None),
        }
    }
}

impl ClientEntry {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self {
            runtime,
            command: Mutex::new(ClientCommandState { destroyed: false }),
            proxy: Mutex::new(None),
        }
    }
}

pub(crate) struct StreamEntry {
    pub(crate) client_handle: u64,
    pub(crate) send: Mutex<RuntimeSendStream>,
    pub(crate) receive: Mutex<RuntimeReceiveStream>,
    cancellation: StreamCancellation,
}

impl StreamEntry {
    fn new(client_handle: u64, stream: RuntimeStream) -> Self {
        let (_, send, receive) = stream.into_parts();
        Self {
            client_handle,
            send: Mutex::new(send),
            receive: Mutex::new(receive),
            cancellation: StreamCancellation::new(),
        }
    }

    pub(crate) fn cancellation(&self) -> watch::Receiver<bool> {
        self.cancellation.subscribe()
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub(crate) fn cancel(&self) {
        self.cancellation.cancel();
    }
}

struct StreamCancellation {
    closed: watch::Sender<bool>,
}

impl StreamCancellation {
    fn new() -> Self {
        let (closed, _) = watch::channel(false);
        Self { closed }
    }

    fn subscribe(&self) -> watch::Receiver<bool> {
        self.closed.subscribe()
    }

    fn is_cancelled(&self) -> bool {
        *self.closed.borrow()
    }

    fn cancel(&self) {
        self.closed.send_replace(true);
    }
}

#[derive(Default)]
pub(crate) struct HandleTable {
    next_handle: u64,
    pub(crate) clients: BTreeMap<u64, Arc<ClientEntry>>,
    pub(crate) streams: BTreeMap<u64, Arc<StreamEntry>>,
    pub(crate) engines: BTreeMap<u64, Arc<EngineEntry>>,
}

impl HandleTable {
    pub(crate) fn insert_client(&mut self, client: Arc<ClientEntry>) -> Option<u64> {
        let handle = self.next()?;
        self.clients.insert(handle, client);
        Some(handle)
    }

    pub(crate) fn insert_stream(
        &mut self,
        client_handle: u64,
        stream: RuntimeStream,
    ) -> Option<u64> {
        let handle = self.next()?;
        self.streams
            .insert(handle, Arc::new(StreamEntry::new(client_handle, stream)));
        Some(handle)
    }

    pub(crate) fn insert_engine(&mut self, engine: Arc<EngineEntry>) -> Option<u64> {
        let handle = self.next()?;
        self.engines.insert(handle, engine);
        Some(handle)
    }

    pub(crate) fn invalidate_streams(&mut self, client_handle: u64) {
        self.streams.retain(|_, stream| {
            if stream.client_handle == client_handle {
                stream.cancel();
                false
            } else {
                true
            }
        });
    }

    fn next(&mut self) -> Option<u64> {
        self.next_handle = self.next_handle.checked_add(1)?;
        Some(self.next_handle)
    }
}

pub(crate) fn handles() -> &'static Mutex<HandleTable> {
    static HANDLES: OnceLock<Mutex<HandleTable>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(HandleTable::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_exhaustion_returns_none_instead_of_panicking() {
        let mut table = HandleTable {
            next_handle: u64::MAX,
            ..Default::default()
        };

        assert_eq!(table.next(), None);
    }

    #[tokio::test]
    async fn stream_cancellation_wakes_existing_subscribers() {
        let cancellation = StreamCancellation::new();
        let mut receiver = cancellation.subscribe();

        cancellation.cancel();

        receiver
            .wait_for(|closed| *closed)
            .await
            .expect("sender remains open");
        assert!(cancellation.is_cancelled());
    }
}
