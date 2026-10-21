use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use crate::app::App;

pub fn render_metadata_editor(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // Track Selector
            Constraint::Percentage(65), // Form Editor
        ])
        .split(area);

    render_track_selector(f, app, chunks[0]);
    render_editor_form(f, app, chunks[1]);
}

fn render_track_selector(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" 📁 Select Track to Edit (↑/↓) ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.library.tracks.is_empty() {
        let empty_msg = Paragraph::new("No tracks in library.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, inner);
        return;
    }

    let mut items = Vec::new();
    let selected_idx = app.tag_editor.track_index.unwrap_or(0);

    for (idx, track) in app.library.tracks.iter().enumerate() {
        let is_selected = idx == selected_idx;
        let prefix = if is_selected { "▶ " } else { "  " };

        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(app.accent())),
            Span::styled(
                format!("{:<20} ", truncate(track.title.as_str(), 20)),
                if is_selected { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) },
            ),
            Span::styled(
                truncate(track.artist_display(), 14),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let item_style = if is_selected {
            Style::default().bg(Color::Rgb(30, 45, 65)).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        items.push(ListItem::new(line).style(item_style));
    }

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn render_editor_form(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.accent()))
        .title(" 🏷️  Metadata Tag Editor ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // File path header
            Constraint::Min(12),    // Form fields
            Constraint::Length(4),  // Save button & instructions
        ])
        .split(inner);

    // 1. File Path Header
    let path_str = app
        .tag_editor
        .track_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "No track loaded".to_string());

    let header_line = Line::from(vec![
        Span::styled("Target File: ", Style::default().fg(Color::DarkGray)),
        Span::styled(path_str, Style::default().fg(app.accent())),
    ]);
    f.render_widget(Paragraph::new(header_line), layout[0]);

    // 2. Form Fields
    let mut field_lines = Vec::new();

    for (idx, field) in app.tag_editor.fields.iter().enumerate() {
        let is_focused = idx == app.tag_editor.focused_field;

        let cursor = if is_focused { "█" } else { " " };
        let label_style = if is_focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let val_style = if is_focused {
            Style::default().fg(Color::White).bg(Color::Rgb(25, 45, 70)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).bg(Color::Rgb(20, 20, 28))
        };

        let val_display = if field.value.is_empty() && !is_focused {
            " <empty> "
        } else {
            &field.value
        };

        field_lines.push(Line::from(vec![
            Span::styled(format!(" {:<14}: ", field.label), label_style),
            Span::styled(format!(" {} ", val_display), val_style),
            Span::styled(cursor, Style::default().fg(app.accent())),
        ]));
        field_lines.push(Line::raw("")); // Spacing row
    }

    f.render_widget(Paragraph::new(field_lines), layout[1]);

    // 3. Status and Save Button
    let mut bottom_lines = Vec::new();

    if let Some((msg, is_success)) = &app.tag_editor.status_message {
        let status_color = if *is_success { Color::Green } else { Color::Red };
        bottom_lines.push(Line::from(vec![
            Span::styled(msg, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        bottom_lines.push(Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+S]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" to save tags to audio file.  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Tab/↓]", Style::default().fg(app.accent())),
            Span::styled(" Next field  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Shift+Tab/↑]", Style::default().fg(app.accent())),
            Span::styled(" Prev field", Style::default().fg(Color::DarkGray)),
        ]));
    }

    bottom_lines.push(Line::raw(""));
    bottom_lines.push(Line::from(vec![
        Span::styled(" [ Ctrl+S / Enter: Save to File ] ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(" [ c: Clear Field ] ", Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50))),
        Span::raw("   "),
        Span::styled(" [ 1: Back to Now Playing ] ", Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50))),
    ]));

    f.render_widget(Paragraph::new(bottom_lines), layout[2]);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let mut out: String = s.chars().take(max_len.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}
