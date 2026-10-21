use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use crate::app::App;

pub fn render_folder_dialog(f: &mut Frame, app: &App) {
    let screen = f.area();

    // Modal size: 74 chars wide, 11 rows high
    let dialog_w = 74.min(screen.width.saturating_sub(4));
    let dialog_h = 11.min(screen.height.saturating_sub(2));

    let dialog_x = (screen.width.saturating_sub(dialog_w)) / 2;
    let dialog_y = (screen.height.saturating_sub(dialog_h)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_w,
        height: dialog_h,
    };

    // 1. Clear underlying text in dialog area
    f.render_widget(Clear, dialog_area);

    // 2. Render Modal Block Frame
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.accent()))
        .title(" 📁 Connect Music Folder ");

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Prompt line
            Constraint::Length(1), // Gap
            Constraint::Length(3), // Input box
            Constraint::Min(1),    // Error or feedback line
            Constraint::Length(1), // Action hotkeys hint
        ])
        .split(inner);

    // Prompt Line
    let prompt_line = Line::from(vec![
        Span::styled("Paste or type ", Style::default().fg(Color::Gray)),
        Span::styled("the path to your music folder, then press ", Style::default().fg(Color::White)),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(":", Style::default().fg(Color::White)),
    ]);
    f.render_widget(Paragraph::new(prompt_line), chunks[0]);

    // Input Box
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    let input_inner = input_block.inner(chunks[2]);
    f.render_widget(input_block, chunks[2]);

    let input_display = if app.folder_dialog_input.is_empty() {
        Line::from(vec![
            Span::styled("e.g. ~/Music or ./songs", Style::default().fg(Color::DarkGray)),
            Span::styled("█", Style::default().fg(app.accent())),
        ])
    } else {
        Line::from(vec![
            Span::styled(&app.folder_dialog_input, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(app.accent())),
        ])
    };
    f.render_widget(Paragraph::new(input_display), input_inner);

    // Error or Feedback Message
    if let Some(err) = &app.folder_dialog_error {
        let err_line = Line::from(vec![
            Span::styled("⚠️  ", Style::default().fg(Color::Red)),
            Span::styled(err, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]);
        f.render_widget(Paragraph::new(err_line), chunks[3]);
    } else {
        let current_folder_str = app
            .library
            .folder_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Built-in catalog".to_string());

        let hint_line = Line::from(vec![
            Span::styled("Current: ", Style::default().fg(Color::DarkGray)),
            Span::styled(current_folder_str, Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(hint_line), chunks[3]);
    }

    // Hotkeys Footer
    let footer_line = Line::from(vec![
        Span::styled(" [Enter] Scan & Load ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(" [Ctrl+U] Clear ", Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50))),
        Span::raw("   "),
        Span::styled(" [Esc] Cancel ", Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50))),
    ]);
    f.render_widget(Paragraph::new(footer_line).alignment(Alignment::Center), chunks[4]);
}
