//! Nats implementation for wasmcloud:messaging.
//!
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use tracing::{error, info, instrument, warn};
use tracing_futures::Instrument;
use wascap::prelude::KeyPair;
use wasmbus_rpc::{core::LinkDefinition, otel::OtelHeaderInjector, provider::prelude::*};
use wasmcloud_interface_messaging::{
    MessageSubscriber, MessageSubscriberSender, Messaging, MessagingReceiver, PubMessage,
    ReplyMessage, RequestMessage, SubMessage,
};

const DEFAULT_NATS_URI: &str = "0.0.0.0:4222";
const ENV_NATS_SUBSCRIPTION: &str = "SUBSCRIPTION";
const ENV_NATS_URI: &str = "URI";
const ENV_NATS_CLIENT_JWT: &str = "CLIENT_JWT";
const ENV_NATS_CLIENT_SEED: &str = "CLIENT_SEED";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // handle lattice control messages and forward rpc to the provider dispatch
    // returns when provider receives a shutdown control message
    let host_data = load_host_data()?;
    let provider = if let Some(c) = host_data.config_json.as_ref() {
        let config: ConnectionConfig = serde_json::from_str(c)?;
        NatsMessagingProvider {
            default_config: config,
            ..Default::default()
        }
    } else {
        NatsMessagingProvider::default()
    };
    provider_main(provider, Some("Nats Messaging Provider".to_string()))?;

    eprintln!("Nats-messaging provider exiting");
    Ok(())
}

/// Configuration for connecting a nats client.
/// More options are available if you use the json than variables in the values string map.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConnectionConfig {
    /// list of topics to subscribe to
    #[serde(default)]
    subscriptions: Vec<String>,
    #[serde(default)]
    cluster_uris: Vec<String>,
    #[serde(default)]
    auth_jwt: Option<String>,
    #[serde(default)]
    auth_seed: Option<String>,

    /// ping interval in seconds
    #[serde(default)]
    ping_interval_sec: Option<u16>,
}

impl Default for ConnectionConfig {
    fn default() -> ConnectionConfig {
        ConnectionConfig {
            subscriptions: vec![],
            cluster_uris: vec!["nats://127.0.0.1:4222".to_string()],
            auth_jwt: None,
            auth_seed: None,
            ping_interval_sec: None,
        }
    }
}

impl ConnectionConfig {
    fn new_from(values: &HashMap<String, String>) -> RpcResult<ConnectionConfig> {
        let mut config = if let Some(config_b64) = values.get("config_b64") {
            let bytes = base64::decode(config_b64.as_bytes()).map_err(|e| {
                RpcError::InvalidParameter(format!("invalid base64 encoding: {}", e))
            })?;
            serde_json::from_slice::<ConnectionConfig>(&bytes)
                .map_err(|e| RpcError::InvalidParameter(format!("corrupt config_b64: {}", e)))?
        } else if let Some(config) = values.get("config_json") {
            serde_json::from_str::<ConnectionConfig>(config)
                .map_err(|e| RpcError::InvalidParameter(format!("corrupt config_json: {}", e)))?
        } else {
            ConnectionConfig::default()
        };
        if let Some(sub) = values.get(ENV_NATS_SUBSCRIPTION) {
            config
                .subscriptions
                .extend(sub.split(',').map(|s| s.to_string()));
        }
        if let Some(url) = values.get(ENV_NATS_URI) {
            config.cluster_uris.push(url.clone());
        }
        if let Some(jwt) = values.get(ENV_NATS_CLIENT_JWT) {
            config.auth_jwt = Some(jwt.clone());
        }
        if let Some(seed) = values.get(ENV_NATS_CLIENT_SEED) {
            config.auth_seed = Some(seed.clone());
        }
        if config.auth_jwt.is_some() && config.auth_seed.is_none() {
            return Err(RpcError::InvalidParameter(
                "if you specify jwt, you must also specify a seed".to_string(),
            ));
        }
        if config.cluster_uris.is_empty() {
            config.cluster_uris.push(DEFAULT_NATS_URI.to_string());
        }
        Ok(config)
    }
}

/// Nats implementation for wasmcloud:messaging
#[derive(Default, Clone, Provider)]
#[services(Messaging)]
struct NatsMessagingProvider {
    // store nats connection client per actor
    actors: Arc<RwLock<HashMap<String, async_nats::Client>>>,
    default_config: ConnectionConfig,
}

// use default implementations of provider message handlers
impl ProviderDispatch for NatsMessagingProvider {}

impl NatsMessagingProvider {
    /// attempt to connect to nats url (with jwt credentials, if provided)
    async fn connect(
        &self,
        cfg: ConnectionConfig,
        ld: &LinkDefinition,
    ) -> Result<async_nats::Client, RpcError> {
        let opts = match (cfg.auth_jwt, cfg.auth_seed) {
            (Some(jwt), Some(seed)) => {
                let key_pair = std::sync::Arc::new(
                    KeyPair::from_seed(&seed)
                        .map_err(|e| RpcError::ProviderInit(format!("key init: {}", e)))?,
                );
                async_nats::ConnectOptions::with_jwt(jwt, move |nonce| {
                    let key_pair = key_pair.clone();
                    async move { key_pair.sign(&nonce).map_err(async_nats::AuthError::new) }
                })
            }
            (None, None) => async_nats::ConnectOptions::default(),
            _ => {
                return Err(RpcError::InvalidParameter(
                    "must provide both jwt and seed for jwt authentication".into(),
                ));
            }
        };
        let url = cfg.cluster_uris.get(0).unwrap();
        let conn = opts
            .connect(url)
            .await
            .map_err(|e| RpcError::ProviderInit(format!("Nats connection to {}: {}", url, e)))?;

        for sub in cfg.subscriptions.iter().filter(|s| !s.is_empty()) {
            let (sub, queue) = match sub.split_once('|') {
                Some((sub, queue)) => (sub, Some(queue.to_string())),
                None => (sub.as_str(), None),
            };
            self.subscribe(&conn, ld, sub.to_string(), queue).await?;
        }
        Ok(conn)
    }

    /// Add a regular or queue subscription
    async fn subscribe(
        &self,
        conn: &async_nats::Client,
        ld: &LinkDefinition,
        sub: String,
        queue: Option<String>,
    ) -> RpcResult<()> {
        let mut subscription = match queue {
            Some(queue) => conn.queue_subscribe(sub.clone(), queue).await,
            None => conn.subscribe(sub.clone()).await,
        }
        .map_err(|e| {
            error!(subject = %sub, error = %e, "error subscribing subscribing");
            RpcError::Nats(format!("subscription to {}: {}", sub, e))
        })?;
        let link_def = ld.to_owned();
        let _join_handle = tokio::spawn(async move {
            // MAGIC NUMBER: Based on our benchmark testing, this seems to be a good upper limit
            // where we start to get diminishing returns. We can consider making this
            // configurable down the line.
            // NOTE (thomastaylor312): It may be better to have a semaphore pool on the
            // NatsMessagingProvider struct that has a global limit of permits so that we don't end
            // up with 20 subscriptions all getting slammed with up to 75 tasks, but we should wait
            // to do anything until we see what happens with real world usage and benchmarking
            let semaphore = Arc::new(Semaphore::new(75));
            while let Some(msg) = subscription.next().await {
                let span = tracing::debug_span!("handle_message", actor_id = %link_def.actor_id);
                span.in_scope(|| {
                    wasmbus_rpc::otel::attach_span_context(&msg);
                });
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        warn!("Work pool has been closed, exiting queue subscribe");
                        break;
                    }
                };
                tokio::spawn(dispatch_msg(link_def.clone(), msg, permit).instrument(span));
            }
        });
        Ok(())
    }
}

#[instrument(level = "debug", skip_all, fields(actor_id = %link_def.actor_id, subject = %nats_msg.subject, reply_to = ?nats_msg.reply))]
async fn dispatch_msg(
    link_def: LinkDefinition,
    nats_msg: async_nats::Message,
    _permit: OwnedSemaphorePermit,
) {
    let msg = SubMessage {
        body: nats_msg.payload.into(),
        reply_to: nats_msg.reply,
        subject: nats_msg.subject,
    };
    let actor = MessageSubscriberSender::for_actor(&link_def);
    if let Err(e) = actor.handle_message(&Context::default(), &msg).await {
        error!(
            error = %e,
            "Unable to send subscription"
        );
    }
}

/// Handle provider control commands
/// put_link (new actor link command), del_link (remove link command), and shutdown
#[async_trait]
impl ProviderHandler for NatsMessagingProvider {
    /// Provider should perform any operations needed for a new link,
    /// including setting up per-actor resources, and checking authorization.
    /// If the link is allowed, return true, otherwise return false to deny the link.
    #[instrument(level = "debug", skip(self, ld), fields(actor_id = %ld.actor_id))]
    async fn put_link(&self, ld: &LinkDefinition) -> RpcResult<bool> {
        // If the link definition values are empty, use the default connection configuration
        let config = if ld.values.is_empty() {
            self.default_config.clone()
        } else {
            ConnectionConfig::new_from(&ld.values)?
        };
        let conn = self.connect(config, ld).await?;

        let mut update_map = self.actors.write().await;
        update_map.insert(ld.actor_id.to_string(), conn);

        Ok(true)
    }

    /// Handle notification that a link is dropped: close the connection
    #[instrument(level = "info", skip(self))]
    async fn delete_link(&self, actor_id: &str) {
        let mut aw = self.actors.write().await;
        if aw.remove(actor_id).is_some() {
            info!("nats closing connection for actor {}", actor_id);
            // close and drop the connection
            // dropping the client should close it
        } // else ignore: it's already been dropped
    }

    /// Handle shutdown request by closing all connections
    async fn shutdown(&self) -> Result<(), Infallible> {
        let mut aw = self.actors.write().await;
        // empty the actor link data and stop all servers
        aw.clear();
        // dropping all connections should send unsubscribes and close the connections.
        Ok(())
    }
}

/// Handle Messaging methods that interact with redis
#[async_trait]
impl Messaging for NatsMessagingProvider {
    #[instrument(level = "debug", skip(self, ctx, msg), fields(actor_id = ?ctx.actor, subject = %msg.subject, reply_to = ?msg.reply_to, body_len = %msg.body.len()))]
    async fn publish(&self, ctx: &Context, msg: &PubMessage) -> RpcResult<()> {
        let actor_id = ctx
            .actor
            .as_ref()
            .ok_or_else(|| RpcError::InvalidParameter("no actor in request".to_string()))?;
        // get read lock on actor-client hashmap to get the connection, then drop it
        let _rd = self.actors.read().await;
        let conn = _rd
            .get(actor_id)
            .ok_or_else(|| RpcError::InvalidParameter(format!("actor not linked:{}", actor_id)))?
            .clone();
        drop(_rd);
        let headers = OtelHeaderInjector::default_with_span().into();
        match msg.reply_to.clone() {
            Some(reply_to) => conn
                .publish_with_reply_and_headers(
                    msg.subject.to_string(),
                    reply_to,
                    headers,
                    msg.body.clone().into(),
                )
                .await
                .map_err(|e| RpcError::Nats(e.to_string())),
            None => conn
                .publish_with_headers(msg.subject.to_string(), headers, msg.body.clone().into())
                .await
                .map_err(|e| RpcError::Nats(e.to_string())),
        }
    }

    #[instrument(level = "debug", skip(self, ctx, msg), fields(actor_id = ?ctx.actor, subject = %msg.subject))]
    async fn request(&self, ctx: &Context, msg: &RequestMessage) -> RpcResult<ReplyMessage> {
        let actor_id = ctx
            .actor
            .as_ref()
            .ok_or_else(|| RpcError::InvalidParameter("no actor in request".to_string()))?;
        // get read lock on actor-client hashmap
        let _rd = self.actors.read().await;
        let conn = _rd
            .get(actor_id)
            .ok_or_else(|| RpcError::InvalidParameter(format!("actor not linked:{}", actor_id)))?
            .clone();
        drop(_rd);
        let headers = OtelHeaderInjector::default_with_span().into();
        match tokio::time::timeout(
            Duration::from_millis(msg.timeout_ms as u64),
            conn.request_with_headers(msg.subject.to_string(), headers, msg.body.clone().into()),
        )
        .await
        {
            Err(_timeout_err) => Err(RpcError::Timeout("nats request timed out".to_string())),
            Ok(Err(send_err)) => Err(RpcError::Nats(format!("nats send error: {}", send_err))),
            Ok(Ok(resp)) => Ok(ReplyMessage {
                body: resp.payload.to_vec(),
                reply_to: resp.reply,
                subject: resp.subject,
            }),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::ConnectionConfig;

    #[test]
    fn test_default_connection_serialize() {
        // test to verify that we can default a config with partial input
        let input = r#"
{
    "cluster_uris": ["nats://soyvuh"],
    "auth_jwt": "authy",
    "auth_seed": "seedy"
}        
"#;

        let config: ConnectionConfig = serde_json::from_str(&input).unwrap();
        assert_eq!(config.auth_jwt.unwrap(), "authy");
        assert_eq!(config.auth_seed.unwrap(), "seedy");
        assert_eq!(config.cluster_uris, ["nats://soyvuh"]);
        assert!(config.subscriptions.is_empty());
        assert!(config.ping_interval_sec.is_none());
    }
}
