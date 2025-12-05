//! TUI state management
//!
//! Input modes, popup states, and other UI state.

use std::{collections::HashSet, path::PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::types::WorkflowTemplate;
use crate::workflow_templates::DetectedProjects;

// =============================================================================
// Input Mode
// =============================================================================

/// Current input mode for the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    AddingRepos,
    AddingEnvSample,
    ConfiguringClone,
    Editing,
    EditingEnvVars,
    EditingRequiredFiles,
    EditingDockerCompose,
    EditingRepoPath,
    EditingBasePath,
    EditingRepoBasePath,
    SelectingWorkflow,
    EditingWorkflow,
}

// =============================================================================
// Clone Configuration
// =============================================================================

/// Clone configuration state
#[derive(Debug, Clone, Default)]
pub struct CloneConfig {
    pub dir_name: String,
    pub base_path: String,
    pub selected_repo_index: usize,
    pub cursor: usize,
    pub editing: bool,
}

// =============================================================================
// Docker Compose Popup State
// =============================================================================

/// State for the docker-compose popup
#[derive(Debug, Clone)]
pub struct DockerComposePopupState {
    pub selected_service: usize,
    pub expanded_services: HashSet<usize>,
    pub editing_env: bool,
    pub scroll_offset: usize,
    pub env_folder: String,
    pub editing_path: Option<PathBuf>,
}

impl Default for DockerComposePopupState {
    fn default() -> Self {
        Self {
            selected_service: 0,
            expanded_services: HashSet::new(),
            editing_env: false,
            scroll_offset: 0,
            env_folder: "container-env".to_string(),
            editing_path: None,
        }
    }
}

impl DockerComposePopupState {
    pub fn toggle_expanded(&mut self, idx: usize) {
        if self.expanded_services.contains(&idx) {
            self.expanded_services.remove(&idx);
        } else {
            self.expanded_services.insert(idx);
        }
    }

    pub fn is_expanded(&self, idx: usize) -> bool {
        self.expanded_services.contains(&idx)
    }

    pub fn reset(&mut self) {
        self.selected_service = 0;
        self.expanded_services.clear();
        self.editing_env = false;
        self.editing_path = None;
        self.scroll_offset = 0;
    }
}

// =============================================================================
// Workflow Popup State
// =============================================================================

/// State for the workflow popup
#[derive(Debug, Clone, Default)]
pub struct WorkflowPopupState {
    pub visible: bool,
    pub selected_template: usize,
    pub detected: Option<DetectedProjects>,
    pub target_path: Option<PathBuf>,
    pub target_name: String,
    pub workflow_content: String,
    pub cursor_line: usize,
    pub scroll_offset: usize,
    pub editing: bool,
}

impl WorkflowPopupState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn select_next(&mut self) {
        self.selected_template = (self.selected_template + 1) % WorkflowTemplate::ALL.len();
    }

    pub fn select_prev(&mut self) {
        self.selected_template = (self.selected_template + WorkflowTemplate::ALL.len() - 1) % WorkflowTemplate::ALL.len();
    }

    pub fn selected_template(&self) -> WorkflowTemplate {
        WorkflowTemplate::ALL[self.selected_template]
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_line = self.workflow_content.lines().count().saturating_sub(1);
        self.cursor_line = (self.cursor_line + lines).min(max_line);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.cursor_line = self.cursor_line.saturating_sub(lines);
    }
}

// =============================================================================
// Input State
// =============================================================================

/// Text input state for editing
#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub buffer: String,
    pub cursor: usize,
    pub lines: Vec<String>,
    pub placeholder: String,
}

impl InputState {
    pub fn new(placeholder: &str) -> Self {
        Self { placeholder: placeholder.to_string(), ..Default::default() }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.lines.clear();
    }

    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buffer.remove(self.cursor);
        }
    }

    pub fn delete_char_forward(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
        }
    }

    pub fn move_cursor_start(&mut self) {
        self.cursor = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    pub fn commit_line(&mut self) {
        if !self.buffer.trim().is_empty() {
            self.lines.push(self.buffer.clone());
        }
        self.buffer.clear();
        self.cursor = 0;
    }

    pub fn handle_paste(&mut self, text: &str) {
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                self.lines.push(trimmed.to_string());
            }
        }
    }

    pub fn all_lines(&self) -> Vec<String> {
        let mut result = self.lines.clone();
        if !self.buffer.trim().is_empty() {
            result.push(self.buffer.trim().to_string());
        }
        result
    }

    pub fn insert_newline(&mut self) {
        self.buffer.insert(self.cursor, '\n');
        self.cursor += 1;
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor = self.buffer[..self.cursor].rfind('\n').map_or(0, |p| p + 1);
    }

    pub fn move_to_line_end(&mut self) {
        self.cursor = self.buffer[self.cursor..]
            .find('\n')
            .map_or(self.buffer.len(), |p| self.cursor + p);
    }

    pub fn move_to_prev_line(&mut self) {
        let before = &self.buffer[..self.cursor];
        if let Some(line_start) = before.rfind('\n') {
            let col = self.cursor - line_start - 1;
            let prev_end = line_start;
            let prev_start = before[..prev_end].rfind('\n').map_or(0, |p| p + 1);
            self.cursor = (prev_start + col).min(prev_end);
        }
    }

    pub fn move_to_next_line(&mut self) {
        let after = &self.buffer[self.cursor..];
        if let Some(next_nl) = after.find('\n') {
            let line_start = self.buffer[..self.cursor].rfind('\n').map_or(0, |p| p + 1);
            let col = self.cursor - line_start;
            let next_start = self.cursor + next_nl + 1;
            let next_end = self.buffer[next_start..]
                .find('\n')
                .map_or(self.buffer.len(), |p| next_start + p);
            self.cursor = (next_start + col).min(next_end);
        }
    }

    pub fn handle_edit_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Backspace => self.delete_char(),
            KeyCode::Delete => self.delete_char_forward(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Char(c) => self.insert_char(c),
            _ => return false,
        }
        true
    }

    pub fn handle_multiline_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Home => self.move_to_line_start(),
            KeyCode::End => self.move_to_line_end(),
            KeyCode::Up => self.move_to_prev_line(),
            KeyCode::Down => self.move_to_next_line(),
            _ => return self.handle_edit_key(key),
        }
        true
    }
}
