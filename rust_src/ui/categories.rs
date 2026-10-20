use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use crate::app::App;
use crate::models::CategoryType;

pub fn render_categories(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Category list
            Constraint::Percentage(60), // Tracks in selected category
        ])
        .split(area);

    render_category_column(f, app, chunks[0]);
    render_tracks_column(f, app, chunks[1]);
}

fn render_category_column(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" 📂 Categories: [1] Artist  [2] Playlist  [3] Album  [4] Album Artist ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // category tabs
            Constraint::Min(4),    // items
        ])
        .split(inner);

    // Tab buttons
    let mut spans = Vec::new();
    for cat in CategoryType::all() {
        let is_active = app.category_type == cat;
        if is_active {
            spans.push(Span::styled(
                format!(" [{}] ", cat.title()),
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("  {}  ", cat.title()),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), rows[0]);

    // Items list (with a footer hint row underneath)
    let items = app.library.get_category_items(app.category_type);
    let list_hints = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(rows[1]);
    let mut list_items = Vec::new();

    for (idx, (name, count)) in items.iter().enumerate() {
        let is_sel = idx == app.selected_category_idx;
        let prefix = if is_sel { "▶ " } else { "  " };

        // User playlists and favourites carry ♪ / ♥ markers; tint them.
        let name_color = if name.starts_with('♥') {
            Color::Rgb(255, 120, 160)
        } else if name.starts_with('♪') {
            Color::Green
        } else if is_sel {
            Color::Yellow
        } else {
            Color::White
        };

        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:<26} ", name), if is_sel { Style::default().fg(name_color).add_modifier(Modifier::BOLD) } else { Style::default().fg(name_color) }),
            Span::styled(format!("({} tracks)", count), Style::default().fg(Color::DarkGray)),
        ]);

        let style = if is_sel {
            Style::default().bg(Color::Rgb(30, 45, 65)).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        list_items.push(ListItem::new(line).style(style));
    }

    f.render_widget(List::new(list_items), list_hints[0]);

    // Footer hint: new-playlist shortcut (creates a persistent user playlist)
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " [c] New Playlist",
            Style::default().fg(Color::DarkGray),
        ))),
        list_hints[1],
    );
}

fn render_tracks_column(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" 🎶 Tracks in '{}' ", app.active_category_name));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.active_playlist_tracks.is_empty() {
        let msg = Paragraph::new("No tracks found.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    let mut list_items = Vec::new();
    let current_id = app.audio.current_track.as_ref().map(|t| t.id.as_str());

    for (pos, &idx) in app.active_playlist_tracks.iter().enumerate() {
        if let Some(track) = app.library.tracks.get(idx) {
            let is_now_playing = current_id == Some(track.id.as_str());
            let icon = if is_now_playing { "▶ " } else { "  " };

            let line = Line::from(vec![
                Span::styled(icon, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:02}. ", pos + 1), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<30} ", track.title), if is_now_playing { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                Span::styled(format!("{:<20} ", track.artist_display()), Style::default().fg(Color::Gray)),
                Span::styled(track.duration_formatted(), Style::default().fg(Color::DarkGray)),
            ]);

            list_items.push(ListItem::new(line));
        }
    }

    f.render_widget(List::new(list_items), inner);
}
