//! Fresnica production core.
//!
//! The Rust core ports stable semantics from the Python reference. It should not
//! invent parallel wallet behavior.

pub mod account;

pub use account::{AccountError, AccountIdentity, AccountKind};
