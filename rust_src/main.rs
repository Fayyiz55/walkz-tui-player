mod ambient;
mod app;
mod audio;
mod cover;
mod library;
mod lyrics;
mod media_probe;
mod models;
mod playlist;
mod scanner;
mod settings;
mod tag_editor;
mod ui;
mod viz;

use std::env;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;

fn main() -> io::Result<()> {
    // 1. Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let mut folder: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-f" | "--folder" => {
                if i + 1 < args.len() {
                    folder = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--version" => {
                println!("Walkz TUI Player v0.3.1 - Aesthetic Rust Terminal Music Player");
                return Ok(());
            }
            arg if !arg.starts_with('-') && folder.is_none() => {
                folder = Some(PathBuf::from(arg));
            }
            _ => {}
        }
        i += 1;
    }

    // 2. Album-art protocol picker.
    //
    // Halfblocks directly, with the font size measured via TIOCGWINSZ when
    // the terminal reports pixel dimensions.
    //
    // We deliberately do NOT use `Picker::from_query_stdio()` here: it
    // spawns a thread that blocks on stdin reads and can outlive the query
    // (mute/slow terminals, Windows conpty), which (a) leaves stray
    // terminal-response bytes that wedge crossterm's input parser (the
    // original "frozen at 0/0" bug), and (b) holds Rust's global Stdin lock
    // while blocked, deadlocking any later stdin read until a keypress
    // arrives. Windows Terminal/WSL supports neither kitty nor sixel, so
    // the query bought nothing here anyway.
    let font_size =
        query_font_size_via_ioctl().unwrap_or(ratatui_image::FontSize::new(10, 20));
    #[allow(deprecated)]
    let picker = ratatui_image::picker::Picker::from_fontsize(font_size);
    // The graphics protocol (sniff or the Settings "image protocol" row) is
    // applied after App initialization, once the settings file is loaded —
    // before the first frame renders.

    // Unsolicited terminal bytes (late conpty responses, typed-ahead input)
    // must not leak into the event loop: a leftover complete sequence shows
    // up as phantom key presses, and a leftover *partial* CSI sequence wedges
    // crossterm's parser — every later keypress merges into the unterminated
    // sequence until the terminator byte arrives, and the app appears frozen.
    drain_pending_input();

    // 3. Setup terminal raw mode
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Initialize App
    let mut app = App::new(folder);
    app.picker = Some(picker);
    // Settings "image protocol" choice: Auto re-runs the env sniff, manual
    // choices override — applied before the first frame.
    app.apply_protocol_setting();

    // 5. Main Event Loop
    let tick_rate = Duration::from_millis(50);
    let res = run_event_loop(&mut terminal, &mut app, tick_rate);

    // 6. Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableBracketedPaste, LeaveAlternateScreen, Show)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Application Error: {:?}", err);
    }

    Ok(())
}

/// Read and discard any bytes already pending on stdin (unsolicited
/// terminal responses, typed-ahead input). Uses a raw `read(2)` on the fd:
/// going through `io::stdin()` would take Rust's global Stdin lock, which
/// another thread could hold while itself blocked on a tty read.
fn drain_pending_input() {
    use std::os::unix::io::AsRawFd;

    let fd = io::stdin().as_raw_fd();
    // SAFETY: fcntl/read on our own stdin fd; flags restored before return.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 {
            return;
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return;
        }
        let mut buf = [0u8; 4096];
        loop {
            let n = libc::read(fd, buf.as_mut_ptr().cast(), buf.len());
            if n <= 0 {
                break; // drained (0) or would-block / error (<0)
            }
        }
        libc::fcntl(fd, libc::F_SETFL, flags);
    }
}

/// Cell size in pixels from TIOCGWINSZ, when the terminal reports it.
/// Mirrors ratatui-image's own `font_size_fallback` without the stdio query.
fn query_font_size_via_ioctl() -> Option<ratatui_image::FontSize> {
    use std::os::unix::io::AsRawFd;

    #[repr(C)]
    struct Winsize {
        ws_row: u16,
        ws_col: u16,
        ws_xpixel: u16,
        ws_ypixel: u16,
    }
    const TIOCGWINSZ: libc::c_ulong = libc::TIOCGWINSZ;

    let fd = io::stdout().as_raw_fd();
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: standard terminal ioctl with a valid Winsize pointer.
    let rc = unsafe { libc::ioctl(fd, TIOCGWINSZ, &mut ws) };
    if rc != 0 || ws.ws_col == 0 || ws.ws_row == 0 || ws.ws_xpixel == 0 || ws.ws_ypixel == 0 {
        return None;
    }
    Some(ratatui_image::FontSize::new(
        ws.ws_xpixel / ws.ws_col,
        ws.ws_ypixel / ws.ws_row,
    ))
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    tick_rate: Duration,
) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui::render_app(f, app))?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key.code, key.modifiers);
                }
                Event::Paste(text) => {
                    app.paste_text(&text);
                }
                _ => {}
            }
        }

        app.tick();
    }
    Ok(())
}

fn print_help() {
    println!(
        r#"Walkz TUI Player - Aesthetic Rust Terminal Music Player

USAGE:
    walkz-tui [OPTIONS] [FOLDER]

OPTIONS:
    -f, --folder <PATH>    Connect directly to an audio folder
    -h, --help             Show this help message
    -v, --version          Show version

KEYBOARD SHORTCUTS:
    [1] - [6]              Switch tabs (Now Playing, Categories, Queue, Metadata, Output, Settings);
                           inside Categories, [←]/[→] switch the sub-tab (Album Artist / Song /
                           Artists / Album / Playlist), digits always switch main tabs
    [Tab]                  Switch focus between panes
    [Space]                Toggle Play / Pause
    [n] / [p]              Next / Previous Track
    [←] / [→]              Seek backward / forward (step set in Settings, default 5s)
    [+] / [-]              Volume Up / Down (step set in Settings, default 5%)
    [r]                    Cycle Repeat mode (Off / All / One)
    [o] / [f]              Open folder dialog to paste or change music folder path
                           (also: Settings tab → "Music folder" row → Enter)
    [e]                    Edit metadata tags of highlighted track
    [Ctrl+S]               Save edited metadata tags to audio file
    [a]                    Song menu (add to playlist, credits, favourite)
    [b]                    Toggle the ambient album-art background (Now Playing);
                           style, dimming & the cava spectrum visualizer live in
                           the Settings tab (visualizer needs `cava` installed)
    [Shift+S]              Re-scan the music folder in the background
    [c]                    New playlist (from the Categories tab)
    [Esc]                  Back: close menus/dialogs; cancel a running scan
    [q] / [Ctrl+C]         Quit
"#
    );
}
