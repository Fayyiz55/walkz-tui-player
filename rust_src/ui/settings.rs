//! Settings tab: toggles, launch defaults, appearance and hotkey bindings.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::models::Focus;
use crate::settings::{settings_rows, SettingsRow};

pub fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == Focus::SettingsList;
    let border_style = if is_focused {
        Style::default().fg(app.accent())
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if is_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(border_style)
        .title(" ⚙ Settings ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Hint (or rebind prompt) pinned to the bottom row.
    let rows_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let rows_area = rows_chunks[0];
    let hint_area = rows_chunks[1];

    let rows = settings_rows();
    let selected = app.settings_selected.min(rows.len().saturating_sub(1));

    // Keep the selected row inside a scrolling window.
    let visible = rows_area.height as usize;
    let start = crate::ui::now_playing::scroll_start(selected, visible);

    let mut items = Vec::new();
    for (pos, row) in rows.iter().enumerate().skip(start).take(visible) {
        let is_selected = is_focused && pos == selected;
        items.push(row_to_item(app, row, is_selected, rows_area.width));
    }

    let list = List::new(items);
    f.render_widget(list, rows_area);

    // Bottom hint / rebind prompt.
    let hint = if let Some(action) = app.rebinding {
        Line::from(Span::styled(
            format!(
                " ⌨ Press a new key for {} — Esc to cancel ",
                action.label()
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(Span::styled(
            " [↑/↓] Select  [Enter] Toggle/Rebind  [←/→] Adjust  [Esc] Back ",
            Style::default().fg(Color::DarkGray),
        ))
    };
    f.render_widget(
        Paragraph::new(hint).alignment(Alignment::Left),
        hint_area,
    );
}

/// Display form of a hotkey char (space is not printable).
fn key_display(c: char) -> String {
    if c == ' ' {
        "Space".to_string()
    } else {
        c.to_string()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let take = max.saturating_sub(1);
        let cut: String = s.chars().take(take).collect();
        format!("{}…", cut)
    }
}

fn row_to_item(app: &App, row: &SettingsRow, is_selected: bool, width: u16) -> ListItem<'static> {
    let (label, value) = match row {
        SettingsRow::MusicFolder => {
            // Show the folder the app would use right now: the connected
            // library folder, or the launch fallback when none is set yet.
            let folder = app
                .library
                .folder_path
                .clone()
                .unwrap_or_else(App::fallback_music_dir);
            (
                "Music folder".to_string(),
                truncate(&folder.display().to_string(), 42),
            )
        }
        SettingsRow::StartupCache => (
            "Load cache on launch".to_string(),
            if app.settings.load_library_cache_on_launch {
                "On".to_string()
            } else {
                "Off".to_string()
            },
        ),
        SettingsRow::DefaultVolume => (
            "Default volume (launch)".to_string(),
            format!("{}%", app.settings.default_volume),
        ),
        SettingsRow::RememberState => (
            "Remember repeat & shuffle".to_string(),
            if app.settings.remember_repeat_shuffle {
                "On".to_string()
            } else {
                "Off".to_string()
            },
        ),
        SettingsRow::ShowLyrics => (
            "Show synchronized lyrics".to_string(),
            if app.settings.show_lyrics_pane {
                "Shown".to_string()
            } else {
                "Hidden".to_string()
            },
        ),
        SettingsRow::ArtSize => (
            "Album art size".to_string(),
            app.settings.art_size.label().to_string(),
        ),
        SettingsRow::VolumeStep => (
            "Volume step".to_string(),
            format!("±{}%", app.settings.volume_step),
        ),
        SettingsRow::SeekStep => (
            "Seek step".to_string(),
            format!("±{}s", app.settings.seek_step_secs),
        ),
        SettingsRow::Accent => (
            "Accent color".to_string(),
            app.settings.accent.label().to_string(),
        ),
        SettingsRow::AmbientEnabled => (
            "Ambient background".to_string(),
            if app.settings.ambient_enabled {
                "On".to_string()
            } else {
                "Off".to_string()
            },
        ),
        SettingsRow::AmbientStyle => (
            "Ambient style".to_string(),
            app.settings.ambient_style.label().to_string(),
        ),
        SettingsRow::AmbientDimming => (
            "Background dimming".to_string(),
            format!("{}%", app.settings.ambient_dimming),
        ),
        SettingsRow::Visualizer => {
            let base = if app.settings.visualizer_enabled {
                "On".to_string()
            } else {
                "Off".to_string()
            };
            let value = if app.viz_missing() {
                format!("{base} · needs cava")
            } else {
                base
            };
            ("Spectrum visualizer (cava)".to_string(), value)
        }
        SettingsRow::Hotkey(action) => (
            format!("Key · {}", action.label()),
            format!(
                "[{}]",
                key_display(app.settings.hotkey_char(*action))
            ),
        ),
    };

    // Pad the label so the value lands on the right edge.
    let value_width = value.chars().count();
    let pad = (width as usize).saturating_sub(value_width + 2);
    let label = truncate(&label, pad);
    let label_width = label.chars().count();
    let filler = " ".repeat(pad.saturating_sub(label_width));

    let label_style = if is_selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let value_style = if is_selected {
        Style::default().fg(app.accent()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let bg_style = if is_selected {
        Style::default().bg(Color::Rgb(30, 45, 65))
    } else {
        Style::default()
    };

    ListItem::new(Line::from(vec![
        Span::styled(format!(" {}", label), label_style.patch(bg_style)),
        Span::styled(filler, bg_style),
        Span::styled(value, value_style.patch(bg_style)),
    ]))
    .style(bg_style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::models::{AppTab, Focus};
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_to_text(app: &App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_settings(f, app, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        let area = buf.area;
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    fn settings_app() -> App {
        let tmp = std::env::temp_dir().join(format!("walkz-ui-set-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let mut app = App::new_in(tmp, None);
        app.scan_state = None;
        app.active_tab = AppTab::Settings;
        app.focus = Focus::SettingsList;
        app
    }

    #[test]
    fn settings_rows_render_with_values() {
        let app = settings_app();
        let text = render_to_text(&app, 100, 40);
        assert!(text.contains("⚙ Settings"), "{text}");
        assert!(text.contains("Music folder"), "{text}");
        assert!(text.contains("Load cache on launch"), "{text}");
        assert!(text.contains("On"), "{text}");
        assert!(text.contains("Default volume (launch)"), "{text}");
        assert!(text.contains("85%"), "{text}");
        assert!(text.contains("Album art size"), "{text}");
        assert!(text.contains("Medium"), "{text}");
        assert!(text.contains("Key · Next track"), "{text}");
        assert!(text.contains("Key · Rescan library"), "{text}");
        assert!(text.contains("[n]"), "{text}");
        assert!(text.contains("[S]"), "{text}");
        assert!(text.contains("[Space]"), "{text}");
        assert!(text.contains("Enter] Toggle/Rebind"), "{text}");
    }

    #[test]
    fn music_folder_row_shows_current_path() {
        let mut app = settings_app();
        app.library.folder_path = Some(std::path::PathBuf::from("/media/music"));
        let text = render_to_text(&app, 100, 40);
        assert!(text.contains("Music folder"), "{text}");
        assert!(text.contains("/media/music"), "{text}");
    }

    #[test]
    fn music_folder_row_opens_dialog_on_enter() {
        let mut app = settings_app(); // already on Settings with the list focused
        app.settings_selected = 0; // Music folder is the first row
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert!(app.is_folder_dialog_open);
    }

    #[test]
    fn rebind_prompt_replaces_hint() {
        let mut app = settings_app();
        app.rebinding = Some(crate::settings::HotkeyAction::NextTrack);
        let text = render_to_text(&app, 100, 40);
        assert!(text.contains("Press a new key for Next track"), "{text}");
    }

    #[test]
    fn ambient_rows_render_with_values() {
        let mut app = settings_app();
        let text = render_to_text(&app, 100, 40);
        assert!(text.contains("Ambient background"), "{text}");
        assert!(text.contains("Ambient style"), "{text}");
        assert!(text.contains("Album art (auto)"), "{text}");
        assert!(text.contains("Background dimming"), "{text}");
        assert!(text.contains("45%"), "{text}");
        assert!(text.contains("Spectrum visualizer (cava)"), "{text}");
        assert!(text.contains("Key · Toggle ambient background"), "{text}");
        assert!(text.contains("[b]"), "{text}");

        // Missing cava annotates the visualizer value.
        app.viz_unavailable = true;
        let text = render_to_text(&app, 100, 40);
        assert!(text.contains("needs cava"), "{text}");
    }
}
