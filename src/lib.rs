#![doc = include_str!("../README.md")]

pub mod automation;
pub mod cache;
pub mod cli;
pub mod compat;
pub mod error;
pub mod mcp;
pub mod models;
pub mod platform;
pub mod selector;

pub use automation::{Peekaboo, PeekabooConfig};
pub use error::{PeekabooError, Result};
pub use models::*;
pub use selector::Selector;
