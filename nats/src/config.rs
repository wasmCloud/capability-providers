use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasmbus_rpc::error::{RpcError, RpcResult};

const DEFAULT_NATS_URI: &str = "0.0.0.0:4222";
const ENV_NATS_SUBSCRIPTION: &str = "SUBSCRIPTION";
const ENV_NATS_URI: &str = "URI";
const ENV_NATS_CLIENT_JWT: &str = "CLIENT_JWT";
const ENV_NATS_CLIENT_SEED: &str = "CLIENT_SEED";
const ENV_SERVICE_NAME: &str = "SERVICE_NAME";
const ENV_SERVICE_ENDPOINTS: &str = "SERVICE_ENDPOINTS";
const ENV_SERVICE_DESCRIPTION: &str = "SERVICE_DESCRIPTION";
const ENV_SERVICE_VERSION: &str = "SERVICE_VERSION";

/// Configuration for connecting a nats client.
/// More options are available if you use the json than variables in the values string map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ConnectionConfig {
    /// list of topics to subscribe to
    #[serde(default)]
    pub subscriptions: Vec<String>,
    #[serde(default)]
    pub cluster_uris: Vec<String>,
    #[serde(default)]
    pub auth_jwt: Option<String>,
    #[serde(default)]
    pub auth_seed: Option<String>,

    #[serde(default)]
    pub service_name: Option<String>,
    #[serde(default)]
    pub service_description: Option<String>,
    #[serde(default)]
    pub service_endpoints: Option<Vec<String>>,
    #[serde(default)]
    pub service_version: Option<String>,

    /// ping interval in seconds
    #[serde(default)]
    pub ping_interval_sec: Option<u16>,
}

impl ConnectionConfig {
    pub fn merge(&self, extra: &ConnectionConfig) -> ConnectionConfig {
        let mut out = self.clone();
        if !extra.subscriptions.is_empty() {
            out.subscriptions = extra.subscriptions.clone();
        }
        // If the default configuration has a URL in it, and then the link definition
        // also provides a URL, the assumption is to replace/override rather than combine
        // the two into a potentially incompatible set of URIs
        if !extra.cluster_uris.is_empty() {
            out.cluster_uris = extra.cluster_uris.clone();
        }
        if extra.auth_jwt.is_some() {
            out.auth_jwt = extra.auth_jwt.clone()
        }
        if extra.auth_seed.is_some() {
            out.auth_seed = extra.auth_seed.clone()
        }
        if extra.ping_interval_sec.is_some() {
            out.ping_interval_sec = extra.ping_interval_sec.clone()
        }
        if extra.service_name.is_some() {
            out.service_name = extra.service_name.clone();
            out.service_description = extra.service_description.clone();
            out.service_endpoints = extra.service_endpoints.clone();
            out.service_version = extra.service_version.clone();
        }
        out
    }
}

impl Default for ConnectionConfig {
    fn default() -> ConnectionConfig {
        ConnectionConfig {
            subscriptions: vec![],
            cluster_uris: vec![DEFAULT_NATS_URI.to_string()],
            auth_jwt: None,
            auth_seed: None,
            ping_interval_sec: None,
            service_description: None,
            service_endpoints: None,
            service_name: None,
            service_version: None,
        }
    }
}

impl ConnectionConfig {
    pub fn new_from(vs: &HashMap<String, String>) -> RpcResult<ConnectionConfig> {
        let mut values = HashMap::<String, String>::new();
        for (k, v) in vs {
            values.insert(k.to_ascii_uppercase(), v.to_string());
        }

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
            config.cluster_uris = url.split(',').map(String::from).collect();
        }
        if let Some(jwt) = values.get(ENV_NATS_CLIENT_JWT) {
            config.auth_jwt = Some(jwt.clone());
        }
        if let Some(seed) = values.get(ENV_NATS_CLIENT_SEED) {
            config.auth_seed = Some(seed.clone());
        }
        config.service_name = values.get(ENV_SERVICE_NAME).cloned();
        config.service_description = values.get(ENV_SERVICE_DESCRIPTION).cloned();
        config.service_version = values.get(ENV_SERVICE_VERSION).cloned();
        config.service_endpoints = values
            .get(ENV_SERVICE_ENDPOINTS)
            .map(|es| es.split(',').map(|s| s.to_string()).collect());

        if config.auth_jwt.is_some() && config.auth_seed.is_none() {
            return Err(RpcError::InvalidParameter(
                "if you specify jwt, you must also specify a seed".to_string(),
            ));
        }

        if config.cluster_uris.is_empty() {
            config.cluster_uris.push(DEFAULT_NATS_URI.to_string());
        }

        eprintln!("{config:?}");

        Ok(config)
    }
}
