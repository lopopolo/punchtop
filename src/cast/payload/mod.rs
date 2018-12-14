#![allow(clippy::large_enum_variant)]
#![allow(dead_code)]

//! The cast protocol splits messages across namespaces, which act like distinct
//! communication channels. Each channel defines its own request and response
//! messages.

pub mod connection;
pub mod heartbeat;
pub mod media;
pub mod receiver;
