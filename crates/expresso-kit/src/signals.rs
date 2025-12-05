//! Signal handling for graceful shutdown
//!
//! Cross-platform signal handling for Unix and Windows

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// =============================================================================
// Unix Signal Handler
// =============================================================================
#[cfg(unix)]
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGTERM};
#[cfg(unix)]
use signal_hook::iterator::Signals;

/// Set up signal handlers for graceful shutdown (Unix)
#[cfg(unix)]
pub fn setup_signal_handler(should_quit: Arc<AtomicBool>) -> std::io::Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP])?;

    std::thread::spawn(move || {
        for sig in signals.forever() {
            if matches!(sig, SIGINT | SIGTERM | SIGHUP) {
                should_quit.store(true, Ordering::SeqCst);
                break;
            }
        }
    });

    Ok(())
}

// =============================================================================
// Windows Signal Handler
// =============================================================================

/// Set up signal handlers for graceful shutdown (Windows - no-op)
#[cfg(windows)]
pub fn setup_signal_handler(should_quit: Arc<AtomicBool>) -> std::io::Result<()> {
    let _ = should_quit;
    Ok(())
}
