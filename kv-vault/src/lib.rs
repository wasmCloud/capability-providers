pub mod client;
pub mod config;
pub mod error;

// generate wasmcloud_interface_keyvaule here (created by build.rs and codegen.toml)
#[allow(dead_code)]
pub mod wasmcloud_interface_keyvalue {
    include!(concat!(env!("OUT_DIR"), "/keyvalue.rs"));
}
