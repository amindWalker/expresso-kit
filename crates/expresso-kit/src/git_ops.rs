//! Git operations module
//!
//! Provides git repository cloning with progress tracking and URL parsing.

use std::process::Stdio;

use tokio::sync::mpsc::UnboundedSender;

use crate::platform;

// =============================================================================
// URL Parsing
// =============================================================================

/// Parse and validate a git URL from user input
pub fn parse_git_url(input: &str) -> Result<String, String> {
    let input = input.trim();

    // Try to extract URL from pasted text
    if let Some(url) = extract_git_url_from_text(input) {
        return Ok(url);
    }

    // Handle SSH URLs (git@...)
    if input.starts_with("git@") && input.contains(':') {
        return input
            .ends_with(".git")
            .then(|| input.to_string())
            .ok_or_else(|| format!("Invalid git URL: '{}'. SSH URLs should end with .git", input));
    }

    // Handle HTTP(S) URLs
    if input.starts_with("http://") || input.starts_with("https://") {
        let git_hosts = ["github.com", "gitlab.com", "bitbucket.org", "git."];
        if git_hosts.iter().any(|host| input.contains(host)) {
            let url = if input.ends_with(".git") {
                input.to_string()
            } else {
                format!("{}.git", input.trim_end_matches('/'))
            };
            return Ok(url);
        }
    }

    Err(format!(
        "Invalid git URL: '{}'. Expected http(s):// or git@ URL ending with .git",
        input
    ))
}

/// Extract a git URL from mixed text content
pub fn extract_git_url_from_text(text: &str) -> Option<String> {
    ["https://", "http://"].iter().find_map(|prefix| {
        text.find(prefix).and_then(|start| {
            let rest = &text[start..];
            rest.find(".git")
                .map(|git_idx| &rest[..git_idx + 4])
                .filter(|url| !url.contains(' '))
                .map(String::from)
        })
    })
}

/// Extract the repository name from a git URL
pub fn extract_repo_name(url: &str) -> String {
    let url = url.trim_end_matches('/').trim_end_matches(".git");
    url.rsplit('/')
        .next()
        .or_else(|| url.rsplit(':').next())
        .unwrap_or("unknown-repo")
        .to_string()
}

// =============================================================================
// Clone Progress Tracking
// =============================================================================

/// Result of a clone operation
#[derive(Debug, Clone)]
pub struct CloneResult {
    /// Whether the clone succeeded
    pub success: bool,
    /// Human-readable result message
    pub message: String,
}

/// Progress updates during cloning
#[derive(Debug, Clone)]
pub enum CloneProgress {
    /// Clone operation started
    Started,
    /// Progress update (percentage 0-100)
    Progress(u8),
    /// Clone completed (success or failure)
    Completed(CloneResult),
}

/// Clone a repository with progress reporting
///
/// This spawns a thread to perform the clone operation and sends
/// progress updates through the provided channel.
pub fn clone_repository_with_progress(
    url: String,
    target_path: std::path::PathBuf,
    progress_tx: UnboundedSender<(usize, CloneProgress)>,
    repo_id: usize,
) {
    std::thread::spawn(move || {
        let _ = progress_tx.send((repo_id, CloneProgress::Started));

        // Create parent directory if needed
        if let Some(parent) = target_path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            let _ = progress_tx.send((
                repo_id,
                CloneProgress::Completed(CloneResult {
                    success: false,
                    message: format!("Failed to create directory: {}", e),
                }),
            ));
            return;
        }

        // Check if already cloned
        if target_path.exists() {
            let git_dir = target_path.join(".git");
            if git_dir.exists() {
                let _ = progress_tx.send((
                    repo_id,
                    CloneProgress::Completed(CloneResult {
                        success: true,
                        message: "Repository already exists".to_string(),
                    }),
                ));
                return;
            }
            let _ = progress_tx.send((
                repo_id,
                CloneProgress::Completed(CloneResult {
                    success: false,
                    message: "Target directory exists but is not a git repository".to_string(),
                }),
            ));
            return;
        }

        // Execute git clone
        let mut cmd = platform::new_git_command();
        let args = platform::git_args(&["clone", "--progress", &url, target_path.to_str().unwrap_or("")]);
        cmd.args(&args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                // Read stderr for progress updates
                if let Some(stderr) = child.stderr.take() {
                    use std::io::Read;
                    let mut reader = stderr;
                    let mut buffer = [0u8; 1];
                    let mut line_buffer = String::new();
                    let mut last_progress: u8 = 0;

                    loop {
                        match reader.read(&mut buffer) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                let ch = buffer[0] as char;
                                if ch == '\r' || ch == '\n' {
                                    if !line_buffer.is_empty()
                                        && let Some(progress) = parse_git_progress(&line_buffer)
                                        && progress != last_progress
                                    {
                                        last_progress = progress;
                                        let _ = progress_tx.send((repo_id, CloneProgress::Progress(progress)));
                                    }
                                    line_buffer.clear();
                                } else {
                                    line_buffer.push(ch);
                                }
                            }
                        }
                    }

                    // Process any remaining content
                    if !line_buffer.is_empty()
                        && let Some(progress) = parse_git_progress(&line_buffer)
                        && progress != last_progress
                    {
                        let _ = progress_tx.send((repo_id, CloneProgress::Progress(progress)));
                    }
                }

                // Wait for completion
                match child.wait() {
                    Ok(status) => {
                        if status.success() {
                            let _ = progress_tx.send((
                                repo_id,
                                CloneProgress::Completed(CloneResult {
                                    success: true,
                                    message: "Repository cloned successfully".to_string(),
                                }),
                            ));
                        } else {
                            let _ = progress_tx.send((
                                repo_id,
                                CloneProgress::Completed(CloneResult {
                                    success: false,
                                    message: format!("Clone failed with exit code: {:?}", status.code()),
                                }),
                            ));
                        }
                    }
                    Err(e) => {
                        let _ = progress_tx.send((
                            repo_id,
                            CloneProgress::Completed(CloneResult {
                                success: false,
                                message: format!("Failed to wait for git: {}", e),
                            }),
                        ));
                    }
                }
            }
            Err(e) => {
                let _ = progress_tx.send((
                    repo_id,
                    CloneProgress::Completed(CloneResult {
                        success: false,
                        message: format!("Failed to execute git: {}", e),
                    }),
                ));
            }
        }
    });
}

/// Parse git clone progress output to percentage
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_git_progress(line: &str) -> Option<u8> {
    let percent_idx = line.find('%')?;
    let before_percent = &line[..percent_idx];
    let number_start = before_percent
        .rfind(|c: char| !c.is_ascii_digit() && c != ' ')
        .map_or(0, |i| i + 1);
    let progress = before_percent[number_start..].trim().parse::<u8>().ok()?;

    // Weight progress based on operation phase
    let weighted = if line.contains("Counting") || line.contains("Compressing") {
        u16::from(progress) / 10
    } else if line.contains("Receiving") {
        10 + u16::from(progress) * 8 / 10
    } else if line.contains("Resolving") {
        90 + u16::from(progress) / 10
    } else {
        u16::from(progress)
    };

    Some(weighted.min(100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_url_https_should_pass() {
        let url = parse_git_url("https://github.com/user/repo").unwrap();
        assert_eq!(url, "https://github.com/user/repo.git");
    }

    #[test]
    fn test_parse_git_url_https_with_git_extension_should_pass() {
        let url = parse_git_url("https://github.com/user/repo.git").unwrap();
        assert_eq!(url, "https://github.com/user/repo.git");
    }

    #[test]
    fn test_parse_git_url_ssh_should_pass() {
        let url = parse_git_url("git@github.com:user/repo.git").unwrap();
        assert_eq!(url, "git@github.com:user/repo.git");
    }

    #[test]
    fn test_extract_repo_name_should_pass() {
        assert_eq!(extract_repo_name("https://github.com/user/repo.git"), "repo");
        assert_eq!(extract_repo_name("git@github.com:user/repo.git"), "repo");
        assert_eq!(extract_repo_name("https://github.com/user/repo"), "repo");
    }

    #[test]
    fn test_extract_git_url_from_text_should_pass() {
        let text = "Check out https://github.com/user/repo.git for more info";
        assert_eq!(
            extract_git_url_from_text(text),
            Some("https://github.com/user/repo.git".to_string())
        );
    }
}
