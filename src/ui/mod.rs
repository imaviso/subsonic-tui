//! Main UI layout and rendering.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::action::Tab;
use crate::app::App;

pub mod components;

pub use components::*;

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Main layout: [tabs] [content + queue] [now playing]
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(5), // Now playing
        ])
        .split(area);

    // Store layout areas for mouse detection
    app.layout.tabs = main_chunks[0];
    app.layout.now_playing = main_chunks[2];

    // Calculate album art offset for controls positioning
    // Album art takes up space on the left when present
    let now_playing_inner_height = main_chunks[2].height.saturating_sub(2); // minus borders
    let has_album_art = app.now_playing.album_art.is_some() && app.now_playing.picker.is_some();
    let art_width = if has_album_art {
        (now_playing_inner_height * 2).min(8) // Same calculation as in now_playing.rs
    } else {
        0
    };
    let info_area_x = main_chunks[2].x + 1 + art_width; // +1 for border, +art_width for album art

    // Progress bar is at the bottom of now_playing area (row 3 = last content row)
    // New layout: row 0 = title, row 1 = controls, row 2 = progress bar
    // With border, progress bar is at y + 3
    app.layout.progress_bar = Rect {
        x: info_area_x + 6,      // Skip time display (6 chars)
        y: main_chunks[2].y + 3, // Row 2 within now_playing (after top border)
        width: main_chunks[2].width.saturating_sub(16 + art_width), // Minus borders, time displays, and art
        height: 1,
    };
    // Volume bar is at the right side of row 1 (controls row)
    // Layout: controls(14) + album(min 10) + volume(18)
    // Volume content (right-aligned): "icon  ━━━━━━━━━━  XX%"
    // The bar is 10 chars, followed by space + 3-4 char percentage
    // So bar ends at (width - 1 border - 5 for " XXX%") and starts 10 chars before that
    let volume_section_end = main_chunks[2].x + main_chunks[2].width - 1; // -1 for right border
    let bar_end = volume_section_end - 5; // " XXX%" is 5 chars
    let bar_start = bar_end - 10; // bar is 10 chars
    app.layout.volume_bar = Rect {
        x: bar_start,
        y: main_chunks[2].y + 2, // Row 1 within now_playing (controls row)
        width: 10,               // "━━━━━━━━━━" is 10 chars
        height: 1,
    };
    // Playback controls area: "󰒮 ▶ 󰒭 󰒟 󰑖" in first 14 chars of controls row
    // Layout: prev(2) + space(1) + play(1-2) + space(1) + next(2) + space(1) + shuffle(2) + space(1) + repeat(2)
    app.layout.controls = Rect {
        x: info_area_x,          // Start after album art
        y: main_chunks[2].y + 2, // Row 1 within now_playing (controls row)
        width: 14,
        height: 1,
    };

    // Render tabs
    render_tabs(frame, main_chunks[0], app.library.tab);

    // Content area: [library] [queue/lyrics]
    let content_chunks = if app.lyrics.visible {
        // Show lyrics panel instead of queue
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_chunks[1])
    } else if app.queue.visible {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(main_chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(main_chunks[1])
    };

    // Store library area
    app.layout.library = content_chunks[0];

    // Store queue area if visible
    if app.queue.visible && content_chunks.len() > 1 && !app.lyrics.visible {
        app.layout.queue = Some(content_chunks[1]);
    } else {
        app.layout.queue = None;
    }

    // Render library with focus indicator
    render_library(frame, content_chunks[0], &mut app.library, app.focus == 0);

    // Render queue or lyrics (if visible)
    if app.lyrics.visible && content_chunks.len() > 1 {
        render_lyrics(frame, content_chunks[1], &mut app.lyrics);
    } else if app.queue.visible && content_chunks.len() > 1 {
        render_queue(frame, content_chunks[1], &mut app.queue, app.focus == 1);
    }

    // Render now playing bar
    render_now_playing(frame, main_chunks[2], &mut app.now_playing);

    // Render search overlay if active
    if app.search.active {
        render_search(frame, area, &mut app.search);
    }

    // Render help overlay if active
    if app.show_help {
        render_help(frame, area);
    }

    // Render track info popup if active
    if app.show_track_info {
        render_track_info(frame, area, &app.now_playing);
    }

    // Render error message if present
    if let Some(error) = &app.error_message {
        render_error(frame, area, error);
    }
}

/// Render the tab bar.
fn render_tabs(frame: &mut Frame, area: Rect, current_tab: Tab) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .map(|t| {
            let style = if *t == current_tab {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(t.title(), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("subsonic-tui")
                .border_style(Style::default().fg(Color::Blue)),
        )
        .select(current_tab.index())
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

/// Render the help overlay.
fn render_help(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(70, 80, area);
    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  j/k or ↑/↓    Move up/down"),
        Line::from("  h/l or ←/→    Switch focus / navigate"),
        Line::from("  Enter         Select item"),
        Line::from("  Esc/Backspace Go back"),
        Line::from("  g/G           Jump to top/bottom"),
        Line::from("  Ctrl+d/u      Scroll half page down/up"),
        Line::from("  1-6           Switch tabs (Artists/Albums/Songs/Playlists/Genres/Favorites)"),
        Line::from("  Tab/Shift+Tab Cycle through tabs"),
        Line::from(""),
        Line::from(Span::styled(
            "Playback",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Space         Play/Pause"),
        Line::from("  n/p           Next/Previous track"),
        Line::from("  ,/.           Seek backward/forward (10s)"),
        Line::from("  +/-           Volume up/down"),
        Line::from("  s             Toggle shuffle"),
        Line::from("  r             Cycle repeat mode"),
        Line::from(""),
        Line::from(Span::styled(
            "Queue & Library",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  a             Add to queue (without playing)"),
        Line::from("  c             Clear queue"),
        Line::from("  d/Delete      Remove selected from queue"),
        Line::from("  o             Jump to current track in queue"),
        Line::from("  J/K           Move queue item down/up"),
        Line::from("  *             Toggle star on current song"),
        Line::from("  R             Refresh library"),
        Line::from(""),
        Line::from(Span::styled(
            "Other",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  /             Search"),
        Line::from("  L             Toggle lyrics panel"),
        Line::from("  i             Show track info"),
        Line::from("  ?             Show this help"),
        Line::from("  x             Clear error message"),
        Line::from("  q             Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Mouse",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Click         Select item / Switch focus"),
        Line::from("  Double-click  Play item"),
        Line::from("  Click tab     Switch to tab"),
        Line::from("  Click prog    Seek in track"),
        Line::from("  Click vol     Set volume"),
        Line::from("  Click ctrl    Playback controls"),
        Line::from("  Scroll        Navigate list"),
        Line::from("  Scroll vol    Adjust volume"),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc or ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Help")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}

/// Render the track info popup.
fn render_track_info(frame: &mut Frame, area: Rect, now_playing: &NowPlayingState) {
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let info_lines = if let Some(song) = &now_playing.current_song {
        vec![
            Line::from(Span::styled(
                "Track Information",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Title: ", Style::default().fg(Color::Cyan)),
                Span::raw(&song.title),
            ]),
            Line::from(vec![
                Span::styled("Artist: ", Style::default().fg(Color::Cyan)),
                Span::raw(song.display_artist()),
            ]),
            Line::from(vec![
                Span::styled("Album: ", Style::default().fg(Color::Cyan)),
                Span::raw(song.album.as_deref().unwrap_or("Unknown")),
            ]),
            Line::from(vec![
                Span::styled("Duration: ", Style::default().fg(Color::Cyan)),
                Span::raw(song.duration_string()),
            ]),
            Line::from(vec![
                Span::styled("Track: ", Style::default().fg(Color::Cyan)),
                Span::raw(
                    song.track
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]),
            Line::from(vec![
                Span::styled("Year: ", Style::default().fg(Color::Cyan)),
                Span::raw(
                    song.year
                        .map(|y| y.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]),
            Line::from(vec![
                Span::styled("Genre: ", Style::default().fg(Color::Cyan)),
                Span::raw(song.genre.as_deref().unwrap_or("-")),
            ]),
            Line::from(vec![
                Span::styled("Bitrate: ", Style::default().fg(Color::Cyan)),
                Span::raw(
                    song.bit_rate
                        .map(|b| format!("{} kbps", b))
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]),
            Line::from(vec![
                Span::styled("Format: ", Style::default().fg(Color::Cyan)),
                Span::raw(song.suffix.as_deref().unwrap_or("-")),
            ]),
            Line::from(vec![
                Span::styled("Size: ", Style::default().fg(Color::Cyan)),
                Span::raw(
                    song.size
                        .map(|s| format_size(s as u64))
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]),
            Line::from(vec![
                Span::styled("Play Count: ", Style::default().fg(Color::Cyan)),
                Span::raw(
                    song.play_count
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Press Esc or i to close",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "No track playing",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Esc or i to close",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Track Info")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(info_lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}

/// Format file size in human-readable format.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Render an error message overlay.
fn render_error(frame: &mut Frame, area: Rect, message: &str) {
    // Create a centered popup
    let popup_area = centered_rect(60, 20, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Error")
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::Red))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, popup_area);
}

/// Create a centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
