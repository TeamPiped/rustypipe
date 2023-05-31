#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::todo, clippy::dbg_macro, clippy::pedantic)]
#![allow(
    clippy::doc_markdown,
    clippy::similar_names,
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::single_match_else,
    clippy::missing_errors_doc
)]

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
