#![doc(html_logo_url = "https://avatars0.githubusercontent.com/u/52050279?s=200&v=4")]
//! # wasmCloud Graph Database Actor API
//!
//! The WebAssembly Secure Capabilities Connector (wasmCloud) API for Graph Database actors
//! enables actors to communicate with graph capability providers in a secure, loosely-coupled
//! fashion.
//!
//! For examples and tutorials on using the actor APIs, check out [wasmcloud.dev](https://wasmcloud.dev).
mod results;
pub use results::FromTable;
mod conversions;
mod errors;

#[doc(hidden)]
#[macro_export]
macro_rules! client_type_error {
    ($($arg:tt)*) => {
        Err($crate::errors::GraphError::ClientTypeError(format!($($arg)*)))
    };
}

#[macro_use]
extern crate serde_derive;

pub mod graph;
