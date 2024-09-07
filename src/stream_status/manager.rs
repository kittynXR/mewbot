use tokio::sync::{broadcast, RwLock};
use std::sync::Arc;

pub struct StreamStatusManager {
    status: RwLock<bool>,
    status_change_sender: broadcast::Sender<bool>,
}

impl StreamStatusManager {
    pub fn new() -> Arc<Self> {
        let (status_change_sender, _) = broadcast::channel(100);
        Arc::new(Self {
            status: RwLock::new(false),
            status_change_sender,
        })
    }

    pub async fn set_stream_live(&self, is_live: bool) {
        let mut status = self.status.write().await;
        if *status != is_live {
            *status = is_live;
            let _ = self.status_change_sender.send(is_live);
        }
    }

    pub async fn is_stream_live(&self) -> bool {
        *self.status.read().await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<bool> {
        self.status_change_sender.subscribe()
    }
}