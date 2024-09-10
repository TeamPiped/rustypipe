#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::todo, clippy::dbg_macro)]

//! ## Go to
//!
//! - Client ([`rustypipe::client::Rustypipe`](crate::client::RustyPipe))
//! - Query ([`rustypipe::client::RustypipeQuery`](crate::client::RustyPipeQuery))

mod deobfuscate;
mod serializer;
mod util;

pub mod cache;
pub mod client;
pub mod error;
pub mod model;
pub mod param;
pub mod report;
pub mod validate;

/// Version of the RustyPipe crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
