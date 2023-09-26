use std::{collections::HashMap, sync::Arc};

use bytes::Bytes;
use lazy_static::lazy_static;
use tokio::sync::{oneshot, RwLock};
use tracing::warn;

lazy_static! {
    static ref REQUEST_WAITERS: Arc<RwLock<HashMap<String, oneshot::Sender<Bytes>>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

pub(crate) async fn add_request_waiter(subject: &str, sender: oneshot::Sender<Bytes>) {
    let mut waiters = REQUEST_WAITERS.write().await;
    waiters.insert(subject.to_string(), sender);
}

pub(crate) async fn dispatch_request_waiter(subject: &str, bytes: Bytes) {
    let mut waiters = REQUEST_WAITERS.write().await;
    if let Some(sender) = waiters.remove(subject) {
        if let Err(_) = sender.send(bytes) {
            warn!("Receiver side of request waiter dropped");
        }
    }
}

pub(crate) async fn is_request_waiting(subject: &str) -> bool {
    let waits = REQUEST_WAITERS.read().await;
    waits.contains_key(subject)
}
