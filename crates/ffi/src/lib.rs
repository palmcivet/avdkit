//! Foreign-function interface boundary.
//!
//! The UniFFI surface is intentionally not generated yet. Keeping this crate
//! in the workspace establishes the boundary without leaking binding-specific
//! types into the core API.

pub use kit::{Kit, KitConfig};
