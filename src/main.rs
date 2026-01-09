//! subsonic-tui - A TUI music player for OpenSubsonic-compatible servers.

use std::time::Duration;

use clap::Parser;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use tokio::sync::mpsc;

mod action;
mod app;
mod client;
mod config;
mod mpris;
mod player;
mod tui;
mod ui;

use action::{Action, PlayerState, RepeatMode, Tab};
use app::App;
use config::Config;

/// State tracked for MPRIS synchronization.
#[derive(Default, Clone)]
struct MprisState {
    track_id: Option<String>,
    player_state: PlayerState,
    position: u32,
    volume: u8,
    shuffle: bool,
    repeat: RepeatMode,
}

/// Command-line arguments.
#[derive(Parser, Debug)]
#[command(name = "subsonic-tui")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Server URL (overrides config)
    #[arg(short, long)]
    server: Option<String>,

    /// Username (overrides config)
    #[arg(short, long)]
    username: Option<String>,

    /// Password (overrides config)
    #[arg(short, long)]
    password: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hooks
    tui::install_hooks()?;

    // Initialize logging
    let log_file = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("subsonic-tui")
        .join("subsonic-tui.log");

    if let Some(parent) = log_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_appender = tracing_subscriber::fmt::layer()
        .with_writer(std::fs::File::create(&log_file)?)
        .with_ansi(false);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::sink) // Don't write to stdout in TUI mode
        .finish()
        .with(file_appender)
        .try_init()
        .ok();

    // Parse command-line arguments
    let args = Args::parse();

    // Load configuration
    let mut config = Config::load().unwrap_or_default();

    // Apply command-line overrides
    if let Some(server) = args.server {
        config.server.url = server;
    }
    if let Some(username) = args.username {
        config.server.username = username;
    }
    if let Some(password) = args.password {
        config.server.password = Some(password);
    }

    // Create action channel
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Create application
    let mut app = App::new(config, action_tx.clone());

    // Initialize MPRIS server (runs on a dedicated thread)
    let mut mpris_handle = match mpris::MprisHandle::new() {
        Ok(handle) => {
            tracing::info!("MPRIS server initialized");
            Some(handle)
        }
        Err(e) => {
            tracing::warn!("Failed to initialize MPRIS server: {}", e);
            None
        }
    };

    // Initialize terminal
    let mut terminal = tui::init()?;

    // Initialize application
    app.init().await?;

    // Track state for MPRIS synchronization
    let mut mpris_state = MprisState::default();

    // Main event loop
    let tick_rate = Duration::from_millis(100);

    loop {
        // Render UI
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        // Handle MPRIS events
        if let Some(ref mut handle) = mpris_handle {
            while let Some(mpris_event) = handle.try_recv() {
                let action = match mpris_event {
                    mpris::MprisEvent::PlayPause => Action::PlayPause,
                    mpris::MprisEvent::Play => Action::PlayPause, // Will resume if paused
                    mpris::MprisEvent::Pause => Action::PlayPause, // Will pause if playing
                    mpris::MprisEvent::Stop => Action::Stop,
                    mpris::MprisEvent::Next => Action::NextTrack,
                    mpris::MprisEvent::Previous => Action::PreviousTrack,
                    mpris::MprisEvent::Seek(offset_us) => {
                        // Convert microseconds to seconds
                        let offset_secs = (offset_us / 1_000_000) as i32;
                        if offset_secs >= 0 {
                            Action::SeekForward
                        } else {
                            Action::SeekBackward
                        }
                    }
                    mpris::MprisEvent::SetPosition(pos_us) => {
                        // Convert to seconds and create a seek action
                        let pos_secs = (pos_us / 1_000_000) as u32;
                        Action::SeekTo(pos_secs)
                    }
                    mpris::MprisEvent::SetVolume(vol) => {
                        // Convert 0.0-1.0 to 0-100
                        let vol_percent = (vol * 100.0) as u8;
                        Action::SetVolume(vol_percent)
                    }
                    mpris::MprisEvent::SetLoopStatus(status) => {
                        Action::SetRepeat(mpris::loop_status_to_repeat(status))
                    }
                    mpris::MprisEvent::SetShuffle(shuffle) => {
                        if shuffle != app.now_playing.shuffle {
                            Action::ToggleShuffle
                        } else {
                            Action::None
                        }
                    }
                    mpris::MprisEvent::Raise => Action::None,
                    mpris::MprisEvent::Quit => Action::Quit,
                };
                if action != Action::None {
                    action_tx.send(action)?;
                }
            }
        }

        // Handle events with timeout
        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        let action = handle_key_event(key.code, key.modifiers, &app);
                        if action != Action::None {
                            action_tx.send(action)?;
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    let action = handle_mouse_event(mouse);
                    if action != Action::None {
                        action_tx.send(action)?;
                    }
                }
                Event::Resize(width, height) => {
                    action_tx.send(Action::Resize(width, height))?;
                }
                _ => {}
            }
        }

        // Send tick action
        action_tx.send(Action::Tick)?;

        // Process all pending actions
        while let Ok(action) = action_rx.try_recv() {
            app.handle_action(action).await?;
        }

        // Sync state to MPRIS
        if let Some(ref handle) = mpris_handle {
            sync_mpris_state(&app, &mut mpris_state, handle);
        }

        // Check if we should quit
        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    tui::restore()?;

    Ok(())
}

/// Map key events to actions.
fn handle_key_event(code: KeyCode, modifiers: KeyModifiers, app: &App) -> Action {
    // Handle search mode separately
    if app.search.active {
        return handle_search_key(code, modifiers);
    }

    // Handle help overlay
    if app.show_help {
        return match code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Action::HideHelp,
            _ => Action::None,
        };
    }

    // Handle track info popup
    if app.show_track_info {
        return match code {
            KeyCode::Esc | KeyCode::Char('i') | KeyCode::Char('q') => Action::HideTrackInfo,
            _ => Action::None,
        };
    }

    // Handle lyrics panel navigation
    if app.lyrics.visible {
        match code {
            KeyCode::Char('L') | KeyCode::Esc => return Action::ToggleLyrics,
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return Action::Quit,
            // Allow playback controls while lyrics are open
            KeyCode::Char(' ') => return Action::PlayPause,
            KeyCode::Char('n') => return Action::NextTrack,
            KeyCode::Char('p') => return Action::PreviousTrack,
            KeyCode::Char('.') | KeyCode::Char('>') => return Action::SeekForward,
            KeyCode::Char(',') | KeyCode::Char('<') => return Action::SeekBackward,
            KeyCode::Char(']') => return Action::SeekForwardLarge,
            KeyCode::Char('[') => return Action::SeekBackwardLarge,
            KeyCode::Char('+') | KeyCode::Char('=') => return Action::VolumeUp,
            KeyCode::Char('-') => return Action::VolumeDown,
            _ => return Action::None,
        }
    }

    // Global keys
    match code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return Action::Quit,
        _ => {}
    }

    // Normal mode keys
    match code {
        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Action::NavigateUp,
        KeyCode::Down | KeyCode::Char('j') => Action::NavigateDown,
        KeyCode::Left | KeyCode::Char('h') => Action::NavigateLeft,
        KeyCode::Right | KeyCode::Char('l') => Action::NavigateRight,
        KeyCode::Enter => Action::Select,
        KeyCode::Esc | KeyCode::Backspace => Action::Back,

        // Vim-style jump navigation
        KeyCode::Char('g') => Action::JumpToTop,
        KeyCode::Char('G') => Action::JumpToBottom,
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            Action::ScrollHalfPageDown
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => Action::ScrollHalfPageUp,

        // Tab switching
        KeyCode::Char('1') => Action::SwitchTab(Tab::Artists),
        KeyCode::Char('2') => Action::SwitchTab(Tab::Albums),
        KeyCode::Char('3') => Action::SwitchTab(Tab::Songs),
        KeyCode::Char('4') => Action::SwitchTab(Tab::Playlists),
        KeyCode::Char('5') => Action::SwitchTab(Tab::Genres),
        KeyCode::Char('6') => Action::SwitchTab(Tab::Favorites),

        // Search
        KeyCode::Char('/') => Action::OpenSearch,

        // Playback
        KeyCode::Char(' ') => Action::PlayPause,
        KeyCode::Char('n') => Action::NextTrack,
        KeyCode::Char('p') => Action::PreviousTrack,
        KeyCode::Char('s') => Action::ToggleShuffle,
        KeyCode::Char('r') => Action::CycleRepeat,
        KeyCode::Char('.') | KeyCode::Char('>') => Action::SeekForward,
        KeyCode::Char(',') | KeyCode::Char('<') => Action::SeekBackward,
        KeyCode::Char(']') => Action::SeekForwardLarge,
        KeyCode::Char('[') => Action::SeekBackwardLarge,

        // Volume
        KeyCode::Char('+') | KeyCode::Char('=') => Action::VolumeUp,
        KeyCode::Char('-') => Action::VolumeDown,

        // Queue
        KeyCode::Char('a') => Action::AppendToQueue,
        KeyCode::Char('c') => Action::ClearQueue,
        KeyCode::Char('d') | KeyCode::Delete => Action::RemoveSelectedFromQueue,
        KeyCode::Char('o') => Action::JumpToCurrentTrack,
        KeyCode::Char('J') => Action::MoveQueueItem(0, 1), // Move down (index set in app.rs)
        KeyCode::Char('K') => Action::MoveQueueItem(0, -1), // Move up (index set in app.rs)

        // Star
        KeyCode::Char('*') => Action::ToggleStar,

        // Lyrics
        KeyCode::Char('L') => Action::ToggleLyrics,

        // Help
        KeyCode::Char('?') => Action::ShowHelp,

        // Track info
        KeyCode::Char('i') => Action::ShowTrackInfo,

        // Refresh
        KeyCode::Char('R') => Action::RefreshLibrary,

        // Clear error
        KeyCode::Char('x') => Action::ClearError,

        _ => Action::None,
    }
}

/// Handle key events in search mode.
fn handle_search_key(code: KeyCode, _modifiers: KeyModifiers) -> Action {
    match code {
        KeyCode::Esc => Action::CloseSearch,
        KeyCode::Enter => Action::Select, // Select result item (or submit search if no results)
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Up => Action::NavigateUp,
        KeyCode::Down => Action::NavigateDown,
        KeyCode::Left => Action::NavigateLeft,
        KeyCode::Right => Action::NavigateRight,
        KeyCode::Tab => Action::NavigateRight,
        KeyCode::BackTab => Action::NavigateLeft,
        KeyCode::Char(c) => Action::SearchInput(c),
        _ => Action::None,
    }
}

/// Handle mouse events.
fn handle_mouse_event(mouse: crossterm::event::MouseEvent) -> Action {
    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            Action::MouseClick(mouse.column, mouse.row)
        }
        MouseEventKind::ScrollUp => Action::MouseScroll(-3),
        MouseEventKind::ScrollDown => Action::MouseScroll(3),
        _ => Action::None,
    }
}

use tracing_subscriber::prelude::*;

/// Synchronize application state to MPRIS.
fn sync_mpris_state(app: &App, state: &mut MprisState, handle: &mpris::MprisHandle) {
    let now_playing = &app.now_playing;

    // Check if track changed
    let current_track_id = now_playing.current_song.as_ref().map(|s| s.id.clone());
    if current_track_id != state.track_id {
        state.track_id = current_track_id.clone();

        if let Some(song) = &now_playing.current_song {
            // Update metadata
            let duration = song.duration.map(|d| d.max(0) as u32);
            let _ = handle.set_metadata(
                &song.id,
                &song.title,
                song.artist.as_deref(),
                song.album.as_deref(),
                duration,
                None, // TODO: cover art URL
            );
        }
    }

    // Check if playback state changed
    if now_playing.state != state.player_state {
        state.player_state = now_playing.state;

        let status = match now_playing.state {
            PlayerState::Playing => mpris_server::PlaybackStatus::Playing,
            PlayerState::Paused => mpris_server::PlaybackStatus::Paused,
            PlayerState::Stopped => mpris_server::PlaybackStatus::Stopped,
            PlayerState::Buffering => mpris_server::PlaybackStatus::Playing, // Treat buffering as playing
        };
        let _ = handle.set_playback_status(status);
    }

    // Update position periodically (every second is fine)
    if now_playing.position != state.position {
        state.position = now_playing.position;
        let _ = handle.set_position(Duration::from_secs(now_playing.position as u64));
    }

    // Check if volume changed
    if now_playing.volume != state.volume {
        state.volume = now_playing.volume;
        let _ = handle.set_volume(now_playing.volume as f64 / 100.0);
    }

    // Check if shuffle changed
    if now_playing.shuffle != state.shuffle {
        state.shuffle = now_playing.shuffle;
        let _ = handle.set_shuffle(now_playing.shuffle);
    }

    // Check if repeat changed
    if now_playing.repeat != state.repeat {
        state.repeat = now_playing.repeat;
        let _ = handle.set_loop_status(mpris::repeat_to_loop_status(now_playing.repeat));
    }

    // Update can_go_next and can_go_previous based on queue
    let can_go_next = !app.queue.songs.is_empty();
    let can_go_previous = !app.queue.songs.is_empty();
    let _ = handle.set_can_go_next(can_go_next);
    let _ = handle.set_can_go_previous(can_go_previous);
}
