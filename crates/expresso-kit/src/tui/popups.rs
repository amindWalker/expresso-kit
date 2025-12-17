//! TUI popup widgets
//!
//! Popup dialogs for confirmation, error display, and help.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::Widget,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::icons;

// =============================================================================
// Confirm Dialog
// =============================================================================

/// Widget for confirmation dialogs
pub struct ConfirmDialog<'a> {
    pub title: &'a str,
    pub summary: &'a str,
    pub details: &'a str,
    pub expanded: bool,
    pub scroll_offset: usize,
    pub actions: &'a [(&'static str, &'static str)],
}

impl ConfirmDialog<'_> {
    pub fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        // Calculate flexible dimensions based on viewport size
        let detail_lines = self.details.lines().count();
        let max_line_width = self.details.lines().map(str::len).max().unwrap_or(40);

        // Width: use most of the screen width for better readability
        let content_width = (max_line_width + 8) as u16; // +8 for padding and borders
        let min_width = 60_u16;
        let max_width = area.width.saturating_sub(6).min(120);
        let width = content_width.clamp(min_width, max_width);

        // Height: use most of available height when expanded
        let base_height = 8_u16; // summary + actions + borders
        let expanded_height = if self.expanded {
            // Use 80% of screen height for expanded dialog
            let available = area.height * 4 / 5; // 80% without floating point
            available.max(base_height + 10)
        } else {
            base_height
        };
        let height = expanded_height.min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let dialog_area = Rect::new(x, y, width, height);

        Clear.render(dialog_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(self.title)
            .title_alignment(Alignment::Center);
        block.clone().render(dialog_area, buf);

        let inner = block.inner(dialog_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Summary
                Constraint::Min(1),    // Details (if expanded) - flexible
                Constraint::Length(2), // Actions
            ])
            .split(inner);

        let expand_icon = if self.expanded { icons::EXPANDED } else { icons::COLLAPSED };
        let summary_line = Line::from(vec![
            Span::styled(expand_icon, Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::raw(self.summary),
        ]);
        Paragraph::new(summary_line).wrap(Wrap { trim: true }).render(chunks[0], buf);

        if self.expanded {
            // Calculate scroll bounds
            let visible_height = chunks[1].height.saturating_sub(2) as usize; // -2 for borders
            let max_scroll = detail_lines.saturating_sub(visible_height);
            let scroll = self.scroll_offset.min(max_scroll);

            let scroll_indicator = if detail_lines > visible_height {
                format!(" Details [{}/{} ↑↓j/k PgUp/Dn] ", scroll + 1, detail_lines.saturating_sub(visible_height) + 1)
            } else {
                " Details ".to_string()
            };

            let details_block = Block::default()
                .borders(Borders::ALL)
                .title(scroll_indicator)
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::DarkGray));
            let details_inner = details_block.inner(chunks[1]);
            details_block.render(chunks[1], buf);

            // Render details with scroll support
            let details_text = Text::from(self.details);
            Paragraph::new(details_text)
                .wrap(Wrap { trim: false })
                .scroll((scroll as u16, 0))
                .render(details_inner, buf);
        }

        let action_text = self
            .actions
            .iter()
            .map(|(key, label)| format!("[{}] {}", key, label))
            .collect::<Vec<_>>()
            .join("  ");
        Paragraph::new(action_text).alignment(Alignment::Center).render(chunks[2], buf);
    }
}

// =============================================================================
// Error Popup
// =============================================================================

/// Widget for error display popups
pub struct ErrorPopup<'a> {
    pub message: &'a str,
}

impl ErrorPopup<'_> {
    pub fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let width = area.width.min(50);
        let height = 8;
        let x = area.x + (area.width - width) / 2;
        let y = area.y + (area.height - height) / 2;
        let popup_area = Rect::new(x, y, width, height);

        Clear.render(popup_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(" Error ")
            .title_alignment(Alignment::Center);
        block.clone().render(popup_area, buf);

        let inner = block.inner(popup_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(inner);

        Paragraph::new(self.message)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Red))
            .render(chunks[0], buf);

        Paragraph::new("Press ESC to close")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray))
            .render(chunks[1], buf);
    }
}

// =============================================================================
// Help Popup
// =============================================================================

/// Widget for help display
pub struct HelpPopup;

impl HelpPopup {
    pub fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let width = area.width.min(70);
        let height = 30;
        let x = area.x + (area.width - width) / 2;
        let y = area.y + (area.height - height) / 2;
        let help_area = Rect::new(x, y, width, height);

        Clear.render(help_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Help - Key Bindings ")
            .title_alignment(Alignment::Center);
        block.clone().render(help_area, buf);

        let inner = block.inner(help_area);
        let help_text = vec![
            ("Navigation", vec![
                ("↑ ↓", "Move selection up/down"),
                ("← →", "Switch tabs"),
                ("Tab", "Next element"),
                ("Shift+Tab", "Previous element"),
            ]),
            ("Repository Operations", vec![
                ("a", "Add repositories"),
                ("SPACE", "Toggle selection"),
                ("ENTER", "Confirm/Execute"),
                ("c", "Clone (configure path)"),
                ("v", "Validate environment"),
                ("r", "Refresh status"),
                ("p", "Edit repo path"),
                ("b", "Edit global base path"),
                ("B", "Edit repo base path"),
                ("Del/Ctrl+D", "Remove selected repo"),
            ]),
            ("Environment Tab", vec![
                ("e", "Edit .env file (multiline)"),
                ("f", "Edit required files list"),
                ("c", "Copy .env.sample to .env"),
                ("v", "Validate .env against sample"),
            ]),
            ("UI Controls", vec![
                ("ESC", "Go back/Close/Dismiss toasts"),
                ("s", "Save configuration"),
                ("h", "Show this help"),
                ("q", "Quit application"),
                ("+/-", "Expand/collapse details"),
            ]),
            ("Global", vec![
                ("F5", "Force refresh"),
                ("Ctrl+S", "Save configuration"),
                ("Ctrl+C", "Emergency quit"),
            ]),
        ];

        let mut y_offset = 0;
        for (section, items) in help_text {
            let title_line = Line::from(Span::styled(section, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            Paragraph::new(title_line).render(Rect::new(inner.x + 2, inner.y + y_offset, inner.width - 4, 1), buf);
            y_offset += 1;

            for (key, desc) in items {
                let line = Line::from(vec![
                    Span::styled(key, Style::default().fg(Color::Green)),
                    Span::raw(" - "),
                    Span::styled(desc, Style::default().fg(Color::White)),
                ]);
                Paragraph::new(line).render(Rect::new(inner.x + 4, inner.y + y_offset, inner.width - 8, 1), buf);
                y_offset += 1;
            }
            y_offset += 1; // Spacing between sections
        }
    }
}
