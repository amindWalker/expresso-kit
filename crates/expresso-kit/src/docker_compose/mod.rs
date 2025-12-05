//! Docker Compose module
//!
//! Provides parsing, validation, and migration utilities for docker-compose files.

mod compose;
mod migration;

pub use compose::*;
pub use migration::*;

// Re-export file pattern functions for convenience
pub use crate::file_patterns::{find_compose_file, find_compose_sample_file};
