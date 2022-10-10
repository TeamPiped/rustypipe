//! # RustyPipe
//!
//! Client for the public YouTube / YouTube Music API (Innertube),
//! inspired by [NewPipe](https://github.com/TeamNewPipe/NewPipeExtractor).

#![warn(clippy::todo)]

#[macro_use]
mod macros;

mod deobfuscate;
mod serializer;
mod util;

pub mod cache;
pub mod client;
pub mod download;
pub mod error;
pub mod model;
pub mod param;
pub mod report;
pub mod timeago;
