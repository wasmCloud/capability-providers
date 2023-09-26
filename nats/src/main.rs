//! Nats implementation for wasmcloud:messaging.
//!

use async_nats::service::{Service, ServiceExt};
use bytes::Bytes;
use futures::StreamExt;
use services::is_request_waiting;
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::{oneshot, OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, instrument, warn};
use tracing_futures::Instrument;
use wascap::prelude::KeyPair;
use wasmbus_rpc::{
    core::{HostData, LinkDefinition},
    otel::OtelHeaderInjector,
    provider::prelude::*,
};
use wasmcloud_interface_messaging::{
    MessageSubscriber, MessageSubscriberSender, Messaging, MessagingReceiver, PubMessage,
    ReplyMessage, RequestMessage, SubMessage,
};

mod config;
mod services;

use config::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // handle lattice control messages and forward rpc to the provider dispatch
    // returns when provider receives a shutdown control message
    let host_data = load_host_data()?;
    let provider = generate_provider(host_data);
    provider_main(provider, Some("NATS Messaging Provider".to_string()))?;

    eprintln!("NATS messaging provider exiting");
    Ok(())
}

fn generate_provider(host_data: HostData) -> NatsMessagingProvider {
    if let Some(c) = host_data.config_json.as_ref() {
        // empty string becomes the default configuration
        if c.trim().is_empty() {
            NatsMessagingProvider::default()
        } else {
            let config: ConnectionConfig = serde_json::from_str(c)
                .expect("JSON deserialization from connection config should have worked");
            NatsMessagingProvider {
                default_config: config,
                ..Default::default()
            }
        }
    } else {
        NatsMessagingProvider::default()
    }
}

/// NatsClientBundles hold a NATS client and information (subscriptions)
/// related to it.
///
/// This struct is necssary because subscriptions are *not* automatically removed on client drop,
/// meaning that we must keep track of all subscriptions to close once the client is done
#[derive(Debug)]
struct NatsClientBundle {
    pub client: async_nats::Client,
    pub sub_handles: Vec<(String, JoinHandle<()>)>,
}

impl Drop for NatsClientBundle {
    fn drop(&mut self) {
        for handle in &self.sub_handles {
            handle.1.abort()
        }
    }
}

/// Nats implementation for wasmcloud:messaging
#[derive(Default, Clone, Provider)]
#[services(Messaging)]
struct NatsMessagingProvider {
    // store nats connection client per actor
    actors: Arc<RwLock<HashMap<String, NatsClientBundle>>>,
    default_config: ConnectionConfig,
}

// use default implementations of provider message handlers
impl ProviderDispatch for NatsMessagingProvider {}

impl NatsMessagingProvider {
    /// Attempt to connect to nats url (with jwt credentials, if provided)
    async fn connect(
        &self,
        cfg: ConnectionConfig,
        link_def: &LinkDefinition,
    ) -> Result<NatsClientBundle, RpcError> {
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

        // Use the first visible cluster_uri
        let url = cfg.cluster_uris.get(0).unwrap();

        let client = opts
            .name("NATS Messaging Provider") // allow this to show up uniquely in a NATS connection list
            .connect(url)
            .await
            .map_err(|e| RpcError::ProviderInit(format!("NATS connection to {}: {}", url, e)))?;

        let mut sub_handles = Vec::new();

        // Every service subscribes on {service}.{endpoint}
        if cfg.service_name.is_some() {
            let service_name = cfg.service_name.unwrap_or("default".to_string());
            let mut svc = client
                .service_builder()
                .description(cfg.service_description.unwrap_or("Unknown".to_string()))
                .start(
                    service_name.clone(),
                    cfg.service_version.unwrap_or("0.0.1".to_string()),
                )
                .await
                .map_err(|e| RpcError::ProviderInit(format!("service start failed: {}", e)))?;
            if let Some(ref eps) = cfg.service_endpoints {
                for ep in eps {
                    let subject = format!("{}.{}", service_name, ep);
                    sub_handles.push((
                        subject.to_string(),
                        self.service_subscribe(&mut svc, link_def, subject, ep.to_string())
                            .await?,
                    ));
                }
            }
        }

        // Connections

        for sub in cfg.subscriptions.iter().filter(|s| !s.is_empty()) {
            let (sub, queue) = match sub.split_once('|') {
                Some((sub, queue)) => (sub, Some(queue.to_string())),
                None => (sub.as_str(), None),
            };

            sub_handles.push((
                sub.to_string(),
                self.subscribe(&client, link_def, sub.to_string(), queue)
                    .await?,
            ));
        }

        Ok(NatsClientBundle {
            client,
            sub_handles,
        })
    }

    /// Add a regular or queue subscription
    async fn subscribe(
        &self,
        client: &async_nats::Client,
        ld: &LinkDefinition,
        sub: String,
        queue: Option<String>,
    ) -> RpcResult<JoinHandle<()>> {
        let mut subscriber = match queue {
            Some(queue) => client.queue_subscribe(sub.clone(), queue).await,
            None => client.subscribe(sub.clone()).await,
        }
        .map_err(|e| {
            error!(subject = %sub, error = %e, "error subscribing subscribing");
            RpcError::Nats(format!("subscription to {}: {}", sub, e))
        })?;

        let link_def = ld.to_owned();

        // Spawn a thread that listens for messages coming from NATS
        // this thread is expected to run the full duration that the provider is available
        let join_handle = tokio::spawn(async move {
            // MAGIC NUMBER: Based on our benchmark testing, this seems to be a good upper limit
            // where we start to get diminishing returns. We can consider making this
            // configurable down the line.
            // NOTE (thomastaylor312): It may be better to have a semaphore pool on the
            // NatsMessagingProvider struct that has a global limit of permits so that we don't end
            // up with 20 subscriptions all getting slammed with up to 75 tasks, but we should wait
            // to do anything until we see what happens with real world usage and benchmarking
            let semaphore = Arc::new(Semaphore::new(75));

            // Listen for NATS message(s)
            while let Some(msg) = subscriber.next().await {
                // Set up tracing context for the NATS message
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

        Ok(join_handle)
    }

    async fn service_subscribe(
        &self,
        svc: &mut Service,
        ld: &LinkDefinition,
        subject: String,
        endpoint: String,
    ) -> RpcResult<JoinHandle<()>> {
        let mut endpoint = svc
            .endpoint_builder()
            .name(endpoint.to_string())
            .add(subject)
            .await
            .map_err(|e| RpcError::ProviderInit(format!("service start failed: {}", e)))?;

        let ld = ld.clone();
        let join_handle = tokio::spawn(async move {
            let semaphore = Arc::new(Semaphore::new(75));
            while let Some(req) = endpoint.next().await {
                let msg = req.message.clone();

                let (tx, rx) = oneshot::channel::<Bytes>();
                services::add_request_waiter(
                    &req.message.reply.clone().unwrap_or("default".to_string()),
                    tx,
                )
                .await;

                //Set up tracing context for the NATS message
                let span = tracing::debug_span!("handle_service_request", actor_id = %ld.actor_id);
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

                tokio::spawn(dispatch_msg(ld.clone(), msg, permit).instrument(span));
                if let Ok(raw) = rx.await {
                    let _ = req.respond(Ok(raw)).await;
                } else {
                    warn!("Sender for service request dropped without sending.");
                }
            }
        });

        Ok(join_handle)
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
        eprintln!("CONFIG: {ld:?}");
        let config = if ld.values.is_empty() {
            self.default_config.clone()
        } else {
            // create a config from the supplied values and merge that with the existing default
            match ConnectionConfig::new_from(&ld.values) {
                Ok(cc) => self.default_config.merge(&cc),
                Err(e) => {
                    error!("Failed to build connection configuration: {e:?}");
                    return Ok(false);
                }
            }
        };

        let mut update_map = self.actors.write().await;
        update_map.insert(ld.actor_id.to_string(), self.connect(config, ld).await?);

        Ok(true)
    }

    /// Handle notification that a link is dropped: close the connection
    #[instrument(level = "info", skip(self))]
    async fn delete_link(&self, actor_id: &str) {
        let mut aw = self.actors.write().await;

        if let Some(bundle) = aw.remove(actor_id) {
            // Note: subscriptions will be closed via Drop on the NatsClientBundle
            debug!(
                "closing [{}] NATS subscriptions for actor [{}]...",
                &bundle.sub_handles.len(),
                actor_id,
            );
        }

        debug!("finished processing delete link for actor [{}]", actor_id);
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

        let nats_bundle = _rd
            .get(actor_id)
            .ok_or_else(|| RpcError::InvalidParameter(format!("actor not linked:{}", actor_id)))?;
        let nats_client = nats_bundle.client.clone();
        drop(_rd);

        let headers = OtelHeaderInjector::default_with_span().into();
        // Is this publish actually a reply to a request?
        if is_request_waiting(&msg.subject).await {
            let _ = services::dispatch_request_waiter(&msg.subject, msg.body.clone().into()).await;
            return Ok(());
        }

        let res = match msg.reply_to.clone() {
            Some(reply_to) => if should_strip_headers(&msg.subject) {
                nats_client
                    .publish_with_reply(msg.subject.to_string(), reply_to, msg.body.clone().into())
                    .await
            } else {
                nats_client
                    .publish_with_reply_and_headers(
                        msg.subject.to_string(),
                        reply_to,
                        headers,
                        msg.body.clone().into(),
                    )
                    .await
            }
            .map_err(|e| RpcError::Nats(e.to_string())),
            None => nats_client
                .publish_with_headers(msg.subject.to_string(), headers, msg.body.clone().into())
                .await
                .map_err(|e| RpcError::Nats(e.to_string())),
        };
        let _ = nats_client.flush().await;
        res
    }

    #[instrument(level = "debug", skip(self, ctx, msg), fields(actor_id = ?ctx.actor, subject = %msg.subject))]
    async fn request(&self, ctx: &Context, msg: &RequestMessage) -> RpcResult<ReplyMessage> {
        let actor_id = ctx
            .actor
            .as_ref()
            .ok_or_else(|| RpcError::InvalidParameter("no actor in request".to_string()))?;
        // Obtain read lock on actor-client hashmap
        let _rd = self.actors.read().await;

        // Extract NATS client from bundle
        let nats_client_bundle = _rd
            .get(actor_id)
            .ok_or_else(|| RpcError::InvalidParameter(format!("actor not linked:{}", actor_id)))?;
        let nats_client = nats_client_bundle.client.clone();
        drop(_rd); // early release of actor-client map

        // Inject OTEL headers
        let headers = OtelHeaderInjector::default_with_span().into();

        // Perform the request with a timeout
        let request_with_timeout = if should_strip_headers(&msg.subject) {
            tokio::time::timeout(
                Duration::from_millis(msg.timeout_ms as u64),
                nats_client.request(msg.subject.to_string(), msg.body.clone().into()),
            )
            .await
        } else {
            tokio::time::timeout(
                Duration::from_millis(msg.timeout_ms as u64),
                nats_client.request_with_headers(
                    msg.subject.to_string(),
                    headers,
                    msg.body.clone().into(),
                ),
            )
            .await
        };

        // Process results of request
        match request_with_timeout {
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

// In the current version of the NATS server, using headers on certain $SYS.REQ topics will cause server-side
// parse failures
fn should_strip_headers(topic: &str) -> bool {
    topic.starts_with("$SYS")
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::{generate_provider, ConnectionConfig, NatsMessagingProvider};
    use wasmbus_rpc::{
        core::{HostData, LinkDefinition},
        error::RpcError,
        provider::ProviderHandler,
    };

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

    #[test]
    fn test_generate_provider_works_with_empty_string() {
        let mut host_data = HostData::default();
        host_data.config_json = Some("".to_string());
        let prov = generate_provider(host_data);
        assert_eq!(prov.default_config, ConnectionConfig::default());
    }

    #[test]
    fn test_generate_provider_works_with_none() {
        let mut host_data = HostData::default();
        host_data.config_json = None;
        let prov = generate_provider(host_data);
        assert_eq!(prov.default_config, ConnectionConfig::default());
    }

    #[test]
    fn test_connectionconfig_merge() {
        // second > original, individual vec fields are replace not extend
        let mut cc1 = ConnectionConfig::default();
        cc1.cluster_uris = vec!["old_server".to_string()];
        cc1.subscriptions = vec!["topic1".to_string()];
        let mut cc2 = ConnectionConfig::default();
        cc2.cluster_uris = vec!["server1".to_string(), "server2".to_string()];
        cc2.auth_jwt = Some("jawty".to_string());
        let cc3 = cc1.merge(&cc2);
        assert_eq!(cc3.cluster_uris, cc2.cluster_uris);
        assert_eq!(cc3.subscriptions, cc1.subscriptions);
        assert_eq!(cc3.auth_jwt, Some("jawty".to_string()))
    }

    /// Ensure that unlink triggers subscription removal
    /// https://github.com/wasmCloud/capability-providers/issues/196
    ///
    /// NOTE: this is tested here for easy access to put_link/del_link without
    /// the fuss of loading/managing individual actors in the lattice
    #[tokio::test]
    async fn test_link_unsub() -> anyhow::Result<()> {
        // Build a nats messaging provider
        let prov = NatsMessagingProvider::default();

        // Actor should have no clients and no subs before hand
        let actor_map = prov.actors.write().await;
        assert_eq!(actor_map.len(), 0);
        drop(actor_map);

        // Add a provider
        let mut ld = LinkDefinition::default();
        ld.actor_id = String::from("???");
        ld.link_name = String::from("test");
        ld.contract_id = String::from("test");
        ld.values = HashMap::<String, String>::from([
            (
                String::from("SUBSCRIPTION"),
                String::from("test.wasmcloud.unlink"),
            ),
            (String::from("URI"), String::from("127.0.0.1:4222")),
        ]);
        prov.put_link(&ld).await?;

        // After putting a link there should be one sub
        let actor_map = prov.actors.write().await;
        assert_eq!(actor_map.len(), 1);
        assert_eq!(actor_map.get("???").unwrap().sub_handles.len(), 1);
        drop(actor_map);

        // Remove link (this should kill the subscription)
        let _ = prov.delete_link(&ld.actor_id).await;

        // After removing a link there should be no subs
        let actor_map = prov.actors.write().await;
        assert_eq!(actor_map.len(), 0);
        drop(actor_map);

        let _ = prov.shutdown().await;
        Ok(())
    }

    /// Ensure that provided URIs are honored by NATS provider
    /// https://github.com/wasmCloud/capability-providers/issues/231
    ///
    /// NOTE: This test can't be rolled into the put_link test because
    /// NATS does not store the URL you fed it to connect -- it stores the host's view in
    /// [async_nats::ServerInfo]
    #[tokio::test]
    async fn test_link_value_uri_usage() -> anyhow::Result<()> {
        // Build a nats messaging provider
        let prov = NatsMessagingProvider::default();

        // Actor should have no clients and no subs before hand
        let actor_map = prov.actors.write().await;
        assert_eq!(actor_map.len(), 0);
        drop(actor_map);

        // Add a provider
        let mut ld = LinkDefinition::default();
        ld.actor_id = String::from("???");
        ld.link_name = String::from("test");
        ld.contract_id = String::from("test");
        ld.values = HashMap::<String, String>::from([
            (
                String::from("SUBSCRIPTION"),
                String::from("test.wasmcloud.unlink"),
            ),
            (String::from("URI"), String::from("99.99.99.99:4222")),
        ]);
        let result = prov.put_link(&ld).await;

        // Expect the result to fail, connecting to an IP that (should) not exist
        assert!(result.is_err(), "put_link failed");
        assert!(
            matches!(result, Err(RpcError::ProviderInit(msg)) if msg == "NATS connection to 99.99.99.99:4222: timed out")
        );

        let _ = prov.shutdown().await;
        Ok(())
    }
}
