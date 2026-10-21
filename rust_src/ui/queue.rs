use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use crate::app::App;
use crate::models::Focus;

pub fn render_queue(f: &mut Frame, app: &App, area: Rect) {
    // The queue list owns focus in its ring (QueueList <-> TabBar); dim it
    // when focus sits on the tab bar.
    let list_focused = app.focus == Focus::QueueList;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if list_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(if list_focused {
            app.accent()
        } else {
            Color::DarkGray
        }))
        .title(" 📋 Playback Queue & History ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Now Playing card
            Constraint::Min(6),    // Up next queue
        ])
        .split(inner);

    // 1. Now Playing card
    if let Some(track) = &app.audio.current_track {
        let lines = vec![
            Line::from(vec![
                Span::styled("NOW PLAYING: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(&track.title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(" by ", Style::default().fg(Color::DarkGray)),
                Span::styled(track.artist_display(), Style::default().fg(app.accent())),
            ]),
            Line::from(vec![
                Span::styled("Album: ", Style::default().fg(Color::DarkGray)),
                Span::styled(track.album_display(), Style::default().fg(Color::Gray)),
                Span::styled("  │  Format: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&track.codec, Style::default().fg(Color::Yellow)),
                {
                    let dr = track.depth_rate_str();
                    if dr.is_empty() {
                        Span::raw("")
                    } else {
                        Span::styled(format!(" {} ", dr), Style::default().fg(Color::Green))
                    }
                },
                Span::styled("  │  Time: ", Style::default().fg(Color::DarkGray)),
                Span::styled(track.duration_formatted(), Style::default().fg(Color::Green)),
            ]),
        ];
        f.render_widget(Paragraph::new(lines), chunks[0]);
    } else {
        f.render_widget(
            Paragraph::new("No track currently playing.")
                .style(Style::default().fg(Color::DarkGray)),
            chunks[0],
        );
    }

    // 2. Up Next Queue
    let mut list_items = Vec::new();
    let cur_id = app.audio.current_track.as_ref().map(|t| t.id.as_str());

    for (pos, &idx) in app.active_playlist_tracks.iter().enumerate() {
        if let Some(track) = app.library.tracks.get(idx) {
            let is_now = cur_id == Some(track.id.as_str());
            let line = Line::from(vec![
                Span::styled(if is_now { "▶ " } else { "  " }, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:02}. ", pos + 1), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<32} ", track.title), if is_now { Style::default().fg(app.accent()).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                Span::styled(format!("{:<20} ", track.artist_display()), Style::default().fg(Color::Gray)),
                Span::styled(track.duration_formatted(), Style::default().fg(Color::DarkGray)),
            ]);
            list_items.push(ListItem::new(line));
        }
    }

    let up_next_block = Block::default()
        .borders(Borders::TOP)
        .title(" Up Next in Queue ");
    let up_next_inner = up_next_block.inner(chunks[1]);
    f.render_widget(up_next_block, chunks[1]);
    f.render_widget(List::new(list_items), up_next_inner);
}
