use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use crate::app::App;
use crate::models::AppTab;

pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),  // App title
            Constraint::Min(45),     // Tabs
            Constraint::Length(24),  // Status / Audio info
        ])
        .split(area);

    // 1. App Title
    let title_line = Line::from(vec![
        Span::styled("♫ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("Walkz TUI Player", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(title_line), chunks[0]);

    // 2. Navigation Tabs. When the tab bar owns focus (Tab ring), the digit
    // prefixes light up to show that 1-6 will switch tabs from here.
    let tabs = [
        (AppTab::NowPlaying, "1", "Now Playing"),
        (AppTab::Categories, "2", "Categories"),
        (AppTab::Queue, "3", "Queue"),
        (AppTab::MetadataEditor, "4", "Metadata"),
        (AppTab::AudioOutput, "5", "Audio"),
        (AppTab::Settings, "6", "Settings"),
    ];

    let tabbar_focused = app.focus == crate::models::Focus::TabBar;
    let mut tab_spans = Vec::new();
    for (tab, num, name) in tabs {
        let is_active = app.active_tab == tab;
        if is_active {
            tab_spans.push(Span::styled(
                format!(" [{} {}] ", num, name),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if tabbar_focused {
            tab_spans.push(Span::styled(
                format!(" {}", num),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
            tab_spans.push(Span::styled(
                format!(" {}  ", name),
                Style::default().fg(Color::White),
            ));
        } else {
            tab_spans.push(Span::styled(
                format!(" {} {} ", num, name),
                Style::default().fg(Color::Gray),
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[1]);

    // 3. Audio & Folder Status
    let vol = app.audio.volume;
    let vol_icon = if vol == 0 { "🔇" } else if vol < 50 { "🔉" } else { "🔊" };
    let folder_name = app
        .library
        .folder_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Built-in Library");

    // Open-folder and rescan keys follow the user's Settings bindings.
    let open_key = app.settings.hotkey_char(crate::settings::HotkeyAction::OpenFolder);
    let rescan_key = app.settings.hotkey_char(crate::settings::HotkeyAction::Rescan);

    let mut spans = vec![
        Span::styled(
            format!("[{}] 📁 ", open_key),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{} ", folder_name), Style::default().fg(Color::Cyan)),
        Span::styled("│ ", Style::default().fg(Color::Rgb(50, 50, 60))),
        Span::styled(format!("{} {}% ", vol_icon, vol), Style::default().fg(Color::Yellow)),
        Span::styled("│ ", Style::default().fg(Color::Rgb(50, 50, 60))),
    ];

    // Scan status chip: spinner + progress while scanning, [key] hint otherwise
    if let Some(scan) = app.scan_state.as_ref() {
        if scan.listed && scan.files_total > 0 {
            spans.push(Span::styled(
                format!("[{}] ⠿ {}% ", rescan_key, (scan.ratio() * 100.0).round() as u64),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("[{}] ⠿ Scanning… ", rescan_key),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ));
        }
    } else {
        spans.push(Span::styled(
            format!("[{}] Scan ", rescan_key),
            Style::default().fg(Color::Yellow),
        ));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
        chunks[2],
    );
}
