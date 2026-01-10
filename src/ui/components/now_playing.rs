//! Now playing bar component.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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
            PlayerState::Playing => "",
            PlayerState::Paused => "",
            PlayerState::Stopped => "",
            PlayerState::Buffering => "󰔟",
        }
    }

    /// Get shuffle symbol.
    pub fn shuffle_symbol(&self) -> &'static str {
        if self.shuffle {
            "󰒟"
        } else {
            "󰒞"
        }
    }

    /// Get volume symbol based on level.
    pub fn volume_symbol(&self) -> &'static str {
        if self.volume == 0 {
            "󰝟"
        } else if self.volume < 30 {
            "󰕿"
        } else if self.volume < 70 {
            "󰖀"
        } else {
            "󰕾"
        }
    }

    /// Get repeat symbol.
    pub fn repeat_symbol(&self) -> &'static str {
        match self.repeat {
            RepeatMode::Off => "󰑗",
            RepeatMode::All => "󰑖",
            RepeatMode::One => "󰑘",
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
        .border_style(Style::default().fg(Color::Rgb(80, 80, 80)));

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

    // Layout for info area: 3 rows
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Song title + artist
            Constraint::Length(1), // Controls + metadata
            Constraint::Length(1), // Progress bar with time
        ])
        .split(info_area);

    // Row 1: Song title and artist (centered feel with left alignment)
    if let Some(song) = &state.current_song {
        let star = if song.starred.is_some() { "󰓎 " } else { "" };

        let title_line = Line::from(vec![
            Span::styled(star, Style::default().fg(Color::Rgb(255, 215, 0))), // Gold star
            Span::styled(
                &song.title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                song.display_artist(),
                Style::default().fg(Color::Rgb(180, 180, 180)),
            ),
        ]);
        frame.render_widget(Paragraph::new(title_line), chunks[0]);
    } else {
        let no_song = Line::from(vec![Span::styled(
            "No track playing",
            Style::default().fg(Color::Rgb(100, 100, 100)),
        )]);
        frame.render_widget(Paragraph::new(no_song), chunks[0]);
    }

    // Row 2: Playback controls (left) + album/metadata (center) + volume (right)
    let controls_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(14), // Controls: ⏮ ▶ ⏭ 󰒟 󰑖
            Constraint::Min(10),    // Album + metadata
            Constraint::Length(18), // Volume
        ])
        .split(chunks[1]);

    // Playback controls
    let prev_color = Color::Rgb(180, 180, 180);
    let play_color = if state.state == PlayerState::Playing {
        Color::Rgb(30, 215, 96) // Spotify green
    } else {
        Color::White
    };
    let next_color = Color::Rgb(180, 180, 180);
    let shuffle_color = if state.shuffle {
        Color::Rgb(30, 215, 96)
    } else {
        Color::Rgb(100, 100, 100)
    };
    let repeat_color = match state.repeat {
        RepeatMode::Off => Color::Rgb(100, 100, 100),
        RepeatMode::All | RepeatMode::One => Color::Rgb(30, 215, 96),
    };

    let controls = Line::from(vec![
        Span::styled("󰒮 ", Style::default().fg(prev_color)),
        Span::styled(state.state_symbol(), Style::default().fg(play_color)),
        Span::styled(" 󰒭 ", Style::default().fg(next_color)),
        Span::styled(state.shuffle_symbol(), Style::default().fg(shuffle_color)),
        Span::styled(" ", Style::default()),
        Span::styled(state.repeat_symbol(), Style::default().fg(repeat_color)),
    ]);
    frame.render_widget(Paragraph::new(controls), controls_chunks[0]);

    // Album + metadata
    if let Some(song) = &state.current_song {
        let mut meta_spans = vec![Span::styled(
            song.display_album(),
            Style::default().fg(Color::Rgb(150, 150, 150)),
        )];

        // Add year if available
        if let Some(year) = song.year {
            meta_spans.push(Span::styled(
                format!(" ({})", year),
                Style::default().fg(Color::Rgb(100, 100, 100)),
            ));
        }

        // Add separator and extra metadata
        let mut extra: Vec<String> = Vec::new();
        if let Some(track) = song.track {
            extra.push(format!("#{}", track));
        }
        if let Some(genre) = &song.genre {
            extra.push(genre.clone());
        }
        if let Some(bitrate) = song.bit_rate {
            extra.push(format!("{}kbps", bitrate));
        }

        if !extra.is_empty() {
            meta_spans.push(Span::styled(
                format!("  ·  {}", extra.join(" · ")),
                Style::default().fg(Color::Rgb(80, 80, 80)),
            ));
        }

        frame.render_widget(Paragraph::new(Line::from(meta_spans)), controls_chunks[1]);
    }

    // Volume bar (right side)
    let vol_bar = render_volume_bar(state.volume);
    let volume_line = Line::from(vec![
        Span::styled(
            state.volume_symbol(),
            Style::default().fg(if state.volume == 0 {
                Color::Rgb(100, 100, 100)
            } else {
                Color::Rgb(180, 180, 180)
            }),
        ),
        Span::styled(" ", Style::default()),
        vol_bar,
        Span::styled(
            format!(" {:>3}%", state.volume),
            Style::default().fg(Color::Rgb(100, 100, 100)),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(volume_line).alignment(Alignment::Right),
        controls_chunks[2],
    );

    // Row 3: Progress bar with timestamps
    render_progress_bar(frame, chunks[2], state);
}

/// Render a modern progress bar with timestamps.
fn render_progress_bar(frame: &mut Frame, area: Rect, state: &NowPlayingState) {
    let time_width = 6; // "MM:SS" + space
    let bar_width = area.width.saturating_sub(time_width * 2 + 2);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(time_width),
            Constraint::Length(bar_width),
            Constraint::Length(time_width + 1),
        ])
        .split(area);

    // Current time (left)
    let current_time = Paragraph::new(state.position_string())
        .style(Style::default().fg(Color::Rgb(150, 150, 150)));
    frame.render_widget(current_time, chunks[0]);

    // Progress bar (center)
    let progress = state.progress();
    let filled_width = ((bar_width as f64) * progress) as usize;
    let empty_width = bar_width as usize - filled_width;

    // Use smooth block characters for gradient effect
    let filled_char = "━";
    let empty_char = "─";
    let handle = "●";

    let bar_spans = if filled_width > 0 {
        vec![
            Span::styled(
                filled_char.repeat(filled_width.saturating_sub(1)),
                Style::default().fg(Color::Rgb(30, 215, 96)), // Spotify green
            ),
            Span::styled(handle, Style::default().fg(Color::White)),
            Span::styled(
                empty_char.repeat(empty_width),
                Style::default().fg(Color::Rgb(60, 60, 60)),
            ),
        ]
    } else {
        vec![Span::styled(
            empty_char.repeat(bar_width as usize),
            Style::default().fg(Color::Rgb(60, 60, 60)),
        )]
    };

    frame.render_widget(Paragraph::new(Line::from(bar_spans)), chunks[1]);

    // Total time (right)
    let total_time = Paragraph::new(format!(" {}", state.duration_string()))
        .style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .alignment(Alignment::Right);
    frame.render_widget(total_time, chunks[2]);
}

/// Render a modern volume bar with gradient.
fn render_volume_bar(volume: u8) -> Span<'static> {
    let bar_width = 10;
    let filled = (volume as usize * bar_width) / 100;
    let empty = bar_width - filled;

    // Use different block characters for a smoother look
    let filled_str = "━".repeat(filled);
    let empty_str = "─".repeat(empty);

    Span::styled(
        format!("{}{}", filled_str, empty_str),
        Style::default().fg(Color::Rgb(30, 215, 96)),
    )
}
