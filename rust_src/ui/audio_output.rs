use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use crate::app::App;

pub fn render_audio_output(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" 🔊 Audio Output & PC Speaker Configuration ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),  // Current Driver Status
            Constraint::Min(12),    // How to connect to PC speaker guide
        ])
        .split(inner);

    // 1. Current Driver Status
    let driver_lines = vec![
        Line::from(vec![
            Span::styled("● Audio Backend: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(&app.audio.output_driver, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("● Playback Engine: ", Style::default().fg(Color::Cyan)),
            Span::styled("/usr/bin/ffplay (Lossless PCM / FLAC / MP3 / Opus / AAC)", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("● Current Output Volume: ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{}%", app.audio.volume), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let status_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Active Output Device ");
    let status_inner = status_block.inner(chunks[0]);
    f.render_widget(status_block, chunks[0]);
    f.render_widget(Paragraph::new(driver_lines), status_inner);

    // 2. Speaker Connection Guide
    let guide_lines = vec![
        Line::from(vec![
            Span::styled("How to Connect this TUI Music Player to your PC Speakers / Bluetooth:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("1. Automatic Soundcard Binding (PipeWire / PulseAudio / ALSA):", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("   The player directly invokes "),
            Span::styled("ffplay", Style::default().fg(Color::Green)),
            Span::raw(" which automatically selects your active default PC output sink."),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("2. Selecting a Specific Audio Output Device (Headphones / USB / HDMI / Bluetooth):", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("   • View available audio sinks:  "),
            Span::styled("wpctl status", Style::default().fg(Color::Cyan)),
            Span::raw("  (or "),
            Span::styled("pactl list short sinks", Style::default().fg(Color::Cyan)),
            Span::raw(")"),
        ]),
        Line::from(vec![
            Span::raw("   • Set default PC speaker:     "),
            Span::styled("wpctl set-default <ID>", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("   • Interactive Volume & Mixer: "),
            Span::styled("pavucontrol", Style::default().fg(Color::Cyan)),
            Span::raw("  or  "),
            Span::styled("alsamixer", Style::default().fg(Color::Cyan)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("3. Volume Control inside the TUI:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("   Use "),
            Span::styled("[+]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" or "),
            Span::styled("[=]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" to increase volume by 5%. Use "),
            Span::styled("[-]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" to decrease volume by 5%."),
        ]),
    ];

    let guide_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Sound Routing & Troubleshooting ");
    let guide_inner = guide_block.inner(chunks[1]);
    f.render_widget(guide_block, chunks[1]);
    f.render_widget(Paragraph::new(guide_lines), guide_inner);
}
