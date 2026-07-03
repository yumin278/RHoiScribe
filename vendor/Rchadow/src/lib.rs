//! Embeddable playset and launcher primitives.

pub mod error;
pub mod games;
pub mod launch;
pub mod paths;
pub mod playsets;

#[cfg(feature = "rnmdb")]
pub mod rnmdb;

pub use error::{Error, Result};
