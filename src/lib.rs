#![allow(dead_code)]
#![warn(clippy::todo)]

#[macro_use]
mod macros;

// #[cfg(test)]
// mod codegen;

mod deobfuscate;
mod dictionary;
mod serializer;
mod timeago;
mod util;

// pub mod client;
pub mod cache;
pub mod client;
pub mod download;
pub mod model;
pub mod report;
