//! Cross-platform icons for consistent TUI rendering
//!
//! This module provides ASCII-based icons that render consistently across
//! macOS, Linux, and Windows terminals. Using emojis causes layout issues
//! because they have different display widths on different platforms.
//!
//! All icons are designed to be 1-3 characters wide for predictable layout.

// =============================================================================
// Status Icons (1-2 chars)
// =============================================================================

/// Pending/waiting status
pub const PENDING: &str = "-";
/// Cloning/downloading status
pub const CLONING: &str = "~";
/// Validating/checking status
pub const VALIDATING: &str = "*";
/// Ready/success status
pub const READY: &str = "+";
/// Error status
pub const ERROR: &str = "!";
/// Warning status
pub const WARNING: &str = "?";

// =============================================================================
// Selection Icons (2 chars for alignment)
// =============================================================================

/// Selected item marker
pub const SELECTED: &str = "> ";
/// Unselected item marker (same width as SELECTED)
pub const UNSELECTED: &str = "  ";
/// Bookmark/pinned marker
pub const BOOKMARKED: &str = "*";
/// Not bookmarked (same width)
pub const NOT_BOOKMARKED: &str = " ";

// =============================================================================
// Expand/Collapse Icons (1-2 chars)
// =============================================================================

/// Expanded state arrow
pub const EXPANDED: &str = "v";
/// Collapsed state arrow
pub const COLLAPSED: &str = ">";
/// Collapsed state arrow (alternative)
pub const COLLAPSED_ALT: &str = ">";
/// Right arrow for selection
pub const ARROW_RIGHT: &str = "> ";
/// Down arrow
pub const ARROW_DOWN: &str = "v ";

// =============================================================================
// Check/Cross Icons (1-3 chars)
// =============================================================================

/// Checkmark for success/found
pub const CHECK: &str = "[+]";
/// Cross for failure/not found
pub const CROSS: &str = "[x]";
/// OK checkmark (single char)
pub const OK: &str = "+";
/// Dash for N/A
pub const DASH: &str = "--";
/// Simple checkmark
pub const CHECKMARK: &str = "+";
/// Warning indicator
pub const WARN: &str = "!";

// =============================================================================
// Log Level Icons (1-2 chars)
// =============================================================================

/// Info log icon
pub const LOG_INFO: &str = "i";
/// Success log icon
pub const LOG_SUCCESS: &str = "+";
/// Warning log icon
pub const LOG_WARNING: &str = "!";
/// Error log icon
pub const LOG_ERROR: &str = "X";
/// Debug log icon
pub const LOG_DEBUG: &str = "#";

// =============================================================================
// Stats/Dashboard Icons (1-3 chars)
// =============================================================================

/// Total/count icon
pub const STATS_TOTAL: &str = "#";
/// Ready count icon
pub const STATS_READY: &str = "+";
/// Issues count icon
pub const STATS_ISSUES: &str = "!";

// =============================================================================
// Tab Icons (replaced with text labels)
// =============================================================================

/// Repositories tab
pub const TAB_REPOS: &str = "[R]";
/// Environment tab
pub const TAB_ENV: &str = "[E]";
/// Workflows tab
pub const TAB_WORKFLOWS: &str = "[W]";
/// Logs tab
pub const TAB_LOGS: &str = "[L]";

// =============================================================================
// File/Folder Icons (1-3 chars)
// =============================================================================

/// Folder icon
pub const FOLDER: &str = "[D]";
/// File icon
pub const FILE: &str = "[F]";
/// Config/gear icon
pub const CONFIG: &str = "[C]";
/// Docker icon
pub const DOCKER: &str = "[D]";

// =============================================================================
// Status Strings (for display)
// =============================================================================

/// Found status string
pub const FOUND: &str = "[+] Found";
/// Not found status string
pub const NOT_FOUND: &str = "[x] Not found";
/// Yes status string
pub const YES: &str = "[+] Yes";
/// No status string
pub const NO: &str = "[x] No";
