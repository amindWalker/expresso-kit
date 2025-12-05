//! TUI module for the interactive dashboard
//!
//! This module provides a terminal user interface for managing repositories,
//! environments, and workflows.

pub mod popups;
pub mod state;
pub mod types;
pub mod widgets;

// TODO: Add remaining modules as they are created
// pub mod app;
// pub mod render;
// pub mod handlers;

pub use popups::*;
pub use state::*;
pub use types::*;
pub use widgets::*;
