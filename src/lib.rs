pub mod automation;
pub mod cache;
pub mod cli;
pub mod error;
pub mod models;
pub mod platform;
pub mod selector;

pub use automation::{Peekaboo, PeekabooConfig};
pub use error::{PeekabooError, Result};
pub use models::*;
pub use selector::Selector;
