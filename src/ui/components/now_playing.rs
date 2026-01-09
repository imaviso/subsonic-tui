//! Now playing bar component.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};

use crate::action::{PlayerState, RepeatMode};
use crate::client::models::Song;

/// Now playing state.
pub struct NowPlayingState {
    /// Currently playing song
    pub current_song: Option<Song>,

    /// Player state
    pub state: PlayerState,

    /// Current position in seconds
    pub position: u32,

    /// Total duration in seconds
    pub duration: u32,

    /// Volume (0-100)
    pub volume: u8,

    /// Shuffle enabled
    pub shuffle: bool,

    /// Repeat mode
    pub repeat: RepeatMode,

    /// Album art image protocol (for Sixel/Kitty/etc.)
    pub album_art: Option<StatefulProtocol>,

    /// Album art ID currently loaded
    pub album_art_id: Option<String>,

    /// Image picker for terminal graphics
    pub picker: Option<Picker>,

    /// Whether scrobble was sent for current track
    pub scrobbled: bool,
}

impl NowPlayingState {
    pub fn new() -> Self {
        // Try to create a picker for terminal graphics
        let picker = Picker::from_query_stdio().ok();

        Self {
            current_song: None,
            state: PlayerState::default(),
            position: 0,
            duration: 0,
            volume: 80,
            shuffle: false,
            repeat: RepeatMode::default(),
            album_art: None,
            album_art_id: None,
            picker,
            scrobbled: false,
        }
    }

    /// Get progress as a ratio (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.duration == 0 {
            0.0
        } else {
            (self.position as f64) / (self.duration as f64)
        }
    }

    /// Format position as MM:SS.
    pub fn position_string(&self) -> String {
        let mins = self.position / 60;
        let secs = self.position % 60;
        format!("{mins}:{secs:02}")
    }

    /// Format duration as MM:SS.
    pub fn duration_string(&self) -> String {
        let mins = self.duration / 60;
        let secs = self.duration % 60;
        format!("{mins}:{secs:02}")
    }

    /// Get play/pause symbol.
    pub fn state_symbol(&self) -> &'static str {
        match self.state {
            PlayerState::Playing => " ",
            PlayerState::Paused => " ",
            PlayerState::Stopped => " ",
            PlayerState::Buffering => "󰔟 ",
        }
    }

    /// Get shuffle symbol.
    pub fn shuffle_symbol(&self) -> &'static str {
        if self.shuffle {
            "󰒟 "
        } else {
            "  "
        }
    }

    /// Get volume symbol based on level.
    pub fn volume_symbol(&self) -> &'static str {
        if self.volume == 0 {
            "󰝟 "
        } else if self.volume < 30 {
            "󰕿 "
        } else if self.volume < 70 {
            "󰖀 "
        } else {
            "󰕾 "
        }
    }

    /// Set the current song and update duration.
    pub fn set_song(&mut self, song: Song) {
        self.duration = song.duration.unwrap_or(0) as u32;
        self.position = 0;
        self.scrobbled = false;
        // Clear album art if it's a different album
        let new_art_id = song.cover_art.clone();
        if self.album_art_id != new_art_id {
            self.album_art = None;
            self.album_art_id = new_art_id;
        }
        self.current_song = Some(song);
    }

    /// Set the album art image data.
    pub fn set_album_art(&mut self, image_data: &[u8]) {
        if let Some(picker) = &self.picker {
            if let Ok(dyn_image) = image::load_from_memory(image_data) {
                self.album_art = Some(picker.new_resize_protocol(dyn_image));
            }
        }
    }

    /// Check if we should scrobble (played > 50% or > 4 minutes).
    pub fn should_scrobble(&self) -> bool {
        if self.scrobbled {
            return false;
        }
        let half_duration = self.duration / 2;
        let four_minutes = 240;
        self.position >= half_duration.min(four_minutes) && self.position > 30
    }

    /// Mark as scrobbled.
    pub fn mark_scrobbled(&mut self) {
        self.scrobbled = true;
    }

    /// Clear the current song.
    pub fn clear(&mut self) {
        self.current_song = None;
        self.position = 0;
        self.duration = 0;
        self.state = PlayerState::Stopped;
        self.album_art = None;
        self.album_art_id = None;
        self.scrobbled = false;
    }
}

/// Render the now playing bar.
pub fn render_now_playing(frame: &mut Frame, area: Rect, state: &mut NowPlayingState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if area.height < 4 {
        return;
    }

    // Layout: [album art] [info + progress]
    let has_album_art = state.album_art.is_some() && state.picker.is_some();
    let art_width = if has_album_art { inner.height * 2 } else { 0 }; // Approximate square

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if has_album_art {
            vec![Constraint::Length(art_width.min(8)), Constraint::Min(20)]
        } else {
            vec![Constraint::Min(20)]
        })
        .split(inner);

    let info_area = if has_album_art {
        main_chunks[1]
    } else {
        main_chunks[0]
    };

    // Render album art if available
    if has_album_art {
        if let Some(ref mut protocol) = state.album_art {
            let art_area = main_chunks[0];
            let image = StatefulImage::default();
            frame.render_stateful_widget(image, art_area, protocol);
        }
    }

    // Layout for info area: [info] [spacer] [progress bar]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Song info
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Progress bar
        ])
        .split(info_area);

    // Song info line
    let info_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(3),  // Play state
            Constraint::Min(20),    // Song info
            Constraint::Length(28), // Time + controls + volume
        ])
        .split(chunks[0]);

    // Play state symbol
    let state_symbol =
        Paragraph::new(state.state_symbol()).style(Style::default().fg(Color::Green));
    frame.render_widget(state_symbol, info_chunks[0]);

    // Song info
    if let Some(song) = &state.current_song {
        let info = Line::from(vec![
            Span::styled(
                &song.title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" - ", Style::default().fg(Color::DarkGray)),
            Span::styled(song.display_artist(), Style::default().fg(Color::Cyan)),
            Span::styled(" • ", Style::default().fg(Color::DarkGray)),
            Span::styled(song.display_album(), Style::default().fg(Color::Yellow)),
        ]);
        let info_para = Paragraph::new(info);
        frame.render_widget(info_para, info_chunks[1]);
    } else {
        let no_song = Paragraph::new("No song playing").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_song, info_chunks[1]);
    }

    // Time and controls with volume bar
    let vol_bar = render_volume_bar(state.volume);
    let time_str = format!(
        "{} / {}  {} {} {}{}",
        state.position_string(),
        state.duration_string(),
        state.shuffle_symbol(),
        state.repeat.symbol(),
        state.volume_symbol(),
        vol_bar
    );
    let time = Paragraph::new(time_str).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(time, info_chunks[2]);

    // Progress bar
    let progress = (state.progress() * 100.0) as u16;
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
        .percent(progress)
        .label("");

    frame.render_widget(gauge, chunks[2]);
}

/// Render a small volume bar.
fn render_volume_bar(volume: u8) -> String {
    let filled = (volume as usize) / 10;
    let empty = 10 - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
