//! TUI widgets
//!
//! Reusable widget components for the TUI.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::Widget,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, TableState},
};

use super::types::{DashboardStats, LogEntry, RepoStatus, Repository};

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a centered popup area within a given area
pub fn centered_popup(
    size: Rect,
    width_percent: u16,
    height_percent: u16,
    min_width: u16,
    min_height: u16,
    max_width: u16,
    max_height: u16,
) -> Rect {
    let popup_width = ((size.width as u32 * width_percent as u32) / 100) as u16;
    let popup_width = popup_width.clamp(min_width.min(size.width), max_width.min(size.width));
    let popup_height = ((size.height as u32 * height_percent as u32) / 100) as u16;
    let popup_height = popup_height.clamp(min_height.min(size.height), max_height.min(size.height));
    let popup_x = size.width.saturating_sub(popup_width) / 2;
    let popup_y = size.height.saturating_sub(popup_height) / 2;
    Rect::new(popup_x, popup_y, popup_width, popup_height)
}

/// Create a progress bar string with percentage display
pub fn make_progress_bar(percent: u8, width: usize) -> String {
    let filled = (percent as usize * width) / 100;
    let percent_str = format!("{:>3}%", percent);
    let percent_start = (width - percent_str.len()) / 2;
    let bar: String = (0..width).map(|i| if i < filled { '█' } else { '░' }).collect();
    let mut chars: Vec<char> = bar.chars().collect();
    percent_str.chars().enumerate().for_each(|(i, c)| {
        if let Some(slot) = chars.get_mut(percent_start + i) {
            *slot = c;
        }
    });
    chars.into_iter().collect()
}

/// Create a centered cell for tables
pub fn centered_cell(content: impl Into<String>) -> Cell<'static> {
    Cell::from(Line::from(content.into()).alignment(Alignment::Center))
}

/// Create a styled centered cell for tables
pub fn styled_centered_cell(content: impl Into<String>, style: Style) -> Cell<'static> {
    Cell::from(Line::from(content.into()).alignment(Alignment::Center)).style(style)
}

// =============================================================================
// Repository Table Widget
// =============================================================================

/// Widget for displaying the repository table
pub struct RepoTable<'a> {
    pub repos: &'a [Repository],
    pub state: &'a mut TableState,
}

impl RepoTable<'_> {
    pub fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let header = Row::new(vec![
            Cell::from(Line::from("Select").alignment(Alignment::Center)).style(Style::default().fg(Color::Green)),
            Cell::from(Line::from("Cloned").alignment(Alignment::Center)).style(Style::default().fg(Color::Cyan)),
            Cell::from(Line::from("Repository").alignment(Alignment::Center)).style(Style::default().fg(Color::Magenta)),
            Cell::from(Line::from("Progress").alignment(Alignment::Center)).style(Style::default().fg(Color::Yellow)),
            Cell::from(Line::from("Updated").alignment(Alignment::Center)).style(Style::default().fg(Color::Blue)),
            Cell::from(Line::from("Issues").alignment(Alignment::Center)).style(Style::default().fg(Color::LightRed)),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1);

        let rows = self.repos.iter().map(|repo| {
            let selection_indicator = if repo.selected { "🔖" } else { "  " };
            let status_icon = repo.status.icon();
            let progress = repo.status.progress().unwrap_or_else(|| if repo.is_ready() { 100 } else { 0 });
            let progress_bar = make_progress_bar(progress, 20);
            let issue_display = if !repo.is_cloned() {
                "—".to_string()
            } else if repo.has_issues() {
                format!("{} ⚠", repo.total_issues())
            } else {
                "✓".to_string()
            };
            let last_update = repo.last_updated.format("%H:%M:%S").to_string();
            let issue_style = match (repo.is_cloned(), repo.has_issues()) {
                (false, _) => Style::default().fg(Color::DarkGray),
                (true, true) => Style::default().fg(Color::Red),
                (true, false) => Style::default().fg(Color::Green),
            };
            Row::new(vec![
                centered_cell(selection_indicator),
                styled_centered_cell(status_icon, Style::default().fg(repo.status.color())),
                centered_cell(repo.name.clone()),
                centered_cell(progress_bar),
                centered_cell(last_update),
                styled_centered_cell(issue_display, issue_style),
            ])
            .height(1)
        });

        let table = Table::new(rows, &[
            Constraint::Ratio(1, 8), // Select
            Constraint::Ratio(1, 8), // Cloned
            Constraint::Ratio(2, 8), // Repository (2x)
            Constraint::Ratio(2, 8), // Progress (2x)
            Constraint::Ratio(1, 8), // Updated
            Constraint::Ratio(1, 8), // Issues
        ])
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Repositories ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .column_spacing(0);

        ratatui::widgets::StatefulWidget::render(table, area, buf, self.state);
    }
}

// =============================================================================
// Stats Panel Widget
// =============================================================================

/// Widget for displaying dashboard statistics
pub struct StatsPanel<'a> {
    pub stats: &'a DashboardStats,
}

impl StatsPanel<'_> {
    pub fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 3), Constraint::Ratio(1, 3), Constraint::Ratio(1, 3)])
            .split(area);

        let stats_blocks = [
            ("Total", self.stats.total_repos.to_string(), Color::Blue, "📊"),
            ("Ready", self.stats.ready.to_string(), Color::Green, "✅"),
            ("Issues", format!("{}/{}", self.stats.errors, self.stats.warnings), Color::Red, "⚠️"),
        ];

        for (i, (title, value, color, icon)) in stats_blocks.iter().enumerate() {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(*color))
                .title(format!(" {} ", title))
                .title_alignment(Alignment::Center);
            let inner = block.inner(chunks[i]);
            block.render(chunks[i], buf);
            let content = Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(*color).add_modifier(Modifier::BOLD)),
                Span::styled(value.clone(), Style::default().fg(*color).add_modifier(Modifier::BOLD)),
            ]);
            Paragraph::new(content).alignment(Alignment::Center).render(inner, buf);
        }
    }
}

// =============================================================================
// Progress Gauges Widget
// =============================================================================

/// Widget for displaying active operation progress
pub struct ProgressGauges<'a> {
    pub repos: &'a [Repository],
}

impl ProgressGauges<'_> {
    pub fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let cloning_repos: Vec<_> = self
            .repos
            .iter()
            .filter(|r| matches!(r.status, RepoStatus::Cloning(_) | RepoStatus::Validating(_)))
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Active Operations ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(if cloning_repos.is_empty() { Color::DarkGray } else { Color::Yellow }));

        let inner = block.inner(area);
        block.render(area, buf);

        if cloning_repos.is_empty() {
            let content_y = inner.y + inner.height.saturating_sub(1) / 2;
            let centered_area = Rect::new(inner.x, content_y, inner.width, 1);
            Paragraph::new("No active operations")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray))
                .render(centered_area, buf);
            return;
        }

        let constraints: Vec<Constraint> = std::iter::repeat_n(Constraint::Length(3), cloning_repos.len()).collect();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        for (i, repo) in cloning_repos.iter().enumerate() {
            let progress = repo.status.progress().unwrap_or(0) as u16;
            let label = match repo.status {
                RepoStatus::Cloning(_) => format!(" Cloning {}... ", repo.name),
                RepoStatus::Validating(_) => format!(" Validating {}... ", repo.name),
                _ => continue,
            };
            let gauge = Gauge::default()
                .block(Block::default().title(label).title_alignment(Alignment::Center))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(progress)
                .label(format!("{}%", progress));
            gauge.render(chunks[i], buf);
        }
    }
}

// =============================================================================
// Log Viewer Widget
// =============================================================================

/// Widget for displaying recent logs
pub struct LogViewer<'a> {
    pub logs: &'a [LogEntry],
}

impl LogViewer<'_> {
    pub fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Recent Logs ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Gray));

        let inner = block.inner(area);
        block.render(area, buf);

        let items: Vec<ListItem> = self
            .logs
            .iter()
            .rev()
            .take(inner.height as usize - 1)
            .map(|log| {
                let time = log.timestamp.format("%H:%M:%S").to_string();
                let level_icon = log.level.icon();
                let message = log
                    .repo_name
                    .as_ref()
                    .map_or_else(|| log.message.clone(), |repo_name| format!("[{}] {}", repo_name, log.message));
                let spans = vec![
                    Span::styled(format!(" {} ", time), Style::default().fg(Color::DarkGray)),
                    Span::styled(level_icon, Style::default().fg(log.level.color())),
                    Span::raw(" "),
                    Span::styled(message, Style::default().fg(Color::White)),
                ];
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items).block(Block::default()).style(Style::default());
        list.render(inner, buf);
    }
}
