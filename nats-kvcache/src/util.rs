//! subscription utilities
//!

use keyvalue::deserialize;
use std::time::Duration;

/// Result of next_with_timeout
pub enum SubscriptionNextResult<T: serde::de::DeserializeOwned> {
    /// Item received and deserialized
    Item(T),
    /// Timeout
    Timeout,
    /// Subscription cancelled or connection closed
    Cancelled,
    /// Deserialization error
    Err(String),
}

/// Wait for next subscription result and attempt to deserialize
pub async fn next_with_timeout<T: serde::de::DeserializeOwned>(
    sub: &async_nats::Subscription,
    timeout: Duration,
) -> SubscriptionNextResult<T> {
    match tokio::time::timeout(timeout, sub.next()).await {
        Err(_) => SubscriptionNextResult::Timeout,
        Ok(None) => SubscriptionNextResult::Cancelled,
        Ok(Some(msg)) => match deserialize::<T>(&msg.data) {
            Ok(item) => SubscriptionNextResult::Item(item),
            Err(e) => SubscriptionNextResult::Err(e.to_string()),
        },
    }
}
