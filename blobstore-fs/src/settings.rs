//! Configuration settings for blobstore-fs.
//! The "values" map in the actor link definition may contain
//! one or more of the following keys,
//! which determine how the configuration is parsed.
//!
//! For the key...
///   config_file:       load configuration from file name.
///                      Interprets file as json.
///   config_b64:        Configuration is a base64-encoded json string
///   config_json:       Configuration is a raw json string
///
/// If no configuration is provided, the default settings below will be used:
/// - ROOT for where blobs are stored is /tmp
/// - CHUNK_SIZE = 129
///
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use wasmbus_rpc::provider::prelude::RpcError;

const DEFAULT_ROOT: &str = "/tmp";
const DEFAULT_CHUNK_SIZE: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceSettings {
    /// root path
    #[serde(default)]
    pub root: PathBuf,

    /// chunk size
    #[serde(default)]
    pub chunk_size: usize,
}

impl Default for ServiceSettings {
    fn default() -> ServiceSettings {
        ServiceSettings {
            root: PathBuf::from(DEFAULT_ROOT),
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }
}

impl ServiceSettings {
    // if current value of a field is default, replace it with the new.
    // This will allow partial settings of new values.
    fn merge(&mut self, other: ServiceSettings) {
        if self.root == PathBuf::from(DEFAULT_ROOT) {
            self.root = other.root;
        }
        if self.chunk_size == DEFAULT_CHUNK_SIZE {
            self.chunk_size = other.chunk_size;
        }
    }

    /// load Settings from a file with  .json extension
    fn from_file<P: Into<PathBuf>>(fpath: P) -> Result<Self, RpcError> {
        let fpath: PathBuf = fpath.into();
        let data = std::fs::read(&fpath).map_err(|e| {
            RpcError::InvalidParameter(format!(
                "Settings error reading file {}: {}",
                &fpath.display(),
                e
            ))
        })?;
        if let Some(ext) = fpath.extension() {
            let ext = ext.to_string_lossy();
            match ext.as_ref() {
                "json" => ServiceSettings::from_json(&data),
                _ => Err(RpcError::InvalidParameter(format!(
                    "Settings error unrecognized extension {}",
                    ext
                ))),
            }
        } else {
            Err(RpcError::InvalidParameter(format!(
                "Settings error unrecognized file type {}",
                &fpath.display()
            )))
        }
    }

    /// load settings from json
    fn from_json(data: &[u8]) -> Result<Self, RpcError> {
        serde_json::from_slice(data)
            .map_err(|e| RpcError::InvalidParameter(format!("Settings error invalid json: {}", e)))
    }
}

/// Load settings provides a flexible means for loading configuration.
/// Return value is any structure with Deserialize, or for example, HashMap<String,String>
///   config_file: load from file name. Interprets file as json, toml, yaml, based on file extension.
///   config_b64:  base64-encoded json string
///   config_json: raw json string
/// Also accept "address" (a string representing SocketAddr) and "port", a localhost port
/// If more than one key is provided, they are processed in the order above.
///   (later names override earlier names in the list)
///
pub fn load_settings(values: &HashMap<String, String>) -> Result<ServiceSettings, RpcError> {
    // Allow keys to be UPPERCASE, as an accommodation
    // for the lost souls who prefer ugly all-caps variable names.
    let values = crate::make_case_insensitive(values).ok_or_else(|| RpcError::InvalidParameter(
            "Key collision: httpserver settings (from linkdef.values) has one or more keys that are not unique based on case-insensitivity"
                .to_string(),
        ))?;

    let mut settings = ServiceSettings::default();

    if let Some(fpath) = values.get("config_file") {
        settings.merge(ServiceSettings::from_file(fpath)?);
    }

    if let Some(str) = values.get("config_b64") {
        let bytes = base64::decode(str.as_bytes()).map_err(|e| {
            RpcError::InvalidParameter(format!("Settings error invalid base64 encoding: {}", e))
        })?;
        settings.merge(ServiceSettings::from_json(&bytes)?);
    }

    if let Some(str) = values.get("config_json") {
        settings.merge(ServiceSettings::from_json(str.as_bytes())?);
    }

    // accept root as value parameter
    if let Some(root) = values.get("root") {
        settings.root = PathBuf::from(root);
    }

    // accept chunk_size as value parameter
    if let Some(chunk_size) = values.get("port") {
        settings.chunk_size = chunk_size.parse::<usize>().unwrap();
    }

    Ok(settings)
}
