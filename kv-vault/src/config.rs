//! Configuration for kv-vault capability provider
//!
use std::{collections::HashMap, env};
use wasmbus_rpc::error::{RpcError, RpcResult};

/// KV-Vault configuration
#[derive(Clone, Debug)]
pub struct Config {
    /// Token for connecting to vault, can be set in environment with VAULT_TOKEN.
    /// Required
    pub token: String,
    /// Url for connecting to vault, can be set in environment with VAULT_ADDR.
    /// Defaults to 'http://127.0.0.1:8200'
    pub addr: String,
    /// Vault mount point, can be set with in environment with VAULT_MOUNT.
    /// Efaults to "secret/"
    pub mount: String,
    /// certificate files - path to CA certificate file(s). Setting this enables TLS
    /// The linkdef value `certs` and the environment variable `VAULT_CERTS`
    /// are parsed as a comma-separated string of file paths to generate this list.
    pub certs: Vec<String>,
}

impl Config {
    /// initialize from linkdef values, environment, and defaults
    pub fn from_values(values: &HashMap<String, String>) -> RpcResult<Config> {
        let config = Config {
            addr: env::var("VAULT_ADDR")
                .ok()
                .or_else(|| values.get("addr").cloned())
                .or_else(|| values.get("ADDR").cloned())
                .unwrap_or_else(|| "http://127.0.0.1:8200".to_string()),
            token: env::var("VAULT_TOKEN")
                .ok()
                .or_else(|| values.get("token").cloned())
                .or_else(|| values.get("TOKEN").cloned())
                .ok_or_else(|| {
                    RpcError::ProviderInit("missing setting for 'token' or VAULT_TOKEN".to_string())
                })?,
            mount: env::var("VAULT_MOUNT")
                .ok()
                .or_else(|| values.get("mount").cloned())
                .or_else(|| values.get("mount").cloned())
                .unwrap_or_else(|| "secret".to_string()),
            certs: match env::var("VAULT_CERTS")
                .ok()
                .or_else(|| values.get("certs").cloned())
                .or_else(|| values.get("CERTS").cloned())
            {
                Some(certs) => certs.split(',').map(|s| s.trim().to_string()).collect(),
                _ => Vec::new(),
            },
        };
        Ok(config)
    }
}
