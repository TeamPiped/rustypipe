#![allow(dead_code)]

#[macro_use]
mod macros;

#[cfg(test)]
mod codegen;

mod deobfuscate;
mod dictionary;
mod serializer;
mod timeago;
mod util;

pub mod client;
pub mod client2;
pub mod download;
pub mod model;
pub mod cache;
pub mod report;
