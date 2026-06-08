pub mod automation;
pub mod cache;
pub mod cli;
pub mod error;
pub mod models;
pub mod platform;

pub use automation::Peekaboo;
pub use error::{PeekabooError, Result};
pub use models::*;
