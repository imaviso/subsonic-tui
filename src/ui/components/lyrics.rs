//! Lyrics display component.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::client::models::{LyricLine, StructuredLyrics};

/// Lyrics display state.
pub struct LyricsState {
    /// Whether lyrics panel is visible
    pub visible: bool,

    /// Current lyrics data
    pub lyrics: Option<StructuredLyrics>,

    /// Song ID for currently loaded lyrics
    pub song_id: Option<String>,

    /// Whether currently loading
    pub loading: bool,

    /// Current line index (for synced lyrics)
    pub current_line: usize,

    /// Scroll state for unsynced lyrics
    pub scroll_state: ListState,
}

impl Default for LyricsState {
    fn default() -> Self {
        Self::new()
    }
}

impl LyricsState {
    pub fn new() -> Self {
        Self {
            visible: false,
            lyrics: None,
            song_id: None,
            loading: false,
            current_line: 0,
            scroll_state: ListState::default(),
        }
    }

    /// Toggle lyrics visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Set lyrics for a song (picks the best from available options).
    pub fn set_lyrics(&mut self, song_id: String, lyrics_list: Vec<StructuredLyrics>) {
        self.song_id = Some(song_id);
        // Prefer synced lyrics over unsynced, pick first available
        self.lyrics = lyrics_list
            .into_iter()
            .max_by_key(|l| if l.synced { 1 } else { 0 });
        self.loading = false;
        self.current_line = 0;
        self.scroll_state.select(Some(0));
    }

    /// Clear lyrics.
    pub fn clear(&mut self) {
        self.lyrics = None;
        self.song_id = None;
        self.current_line = 0;
    }

    /// Update current line based on playback position (in milliseconds).
    pub fn update_position(&mut self, position_ms: u64) {
        if let Some(lyrics) = &self.lyrics {
            if !lyrics.synced {
                return;
            }

            let offset = lyrics.offset;
            let adjusted_pos = position_ms as i64 + offset;

            // Find the current line
            let mut new_line = 0;
            for (i, line) in lyrics.line.iter().enumerate() {
                if let Some(start) = line.start {
                    if start <= adjusted_pos {
                        new_line = i;
                    } else {
                        break;
                    }
                }
            }

            if new_line != self.current_line {
                self.current_line = new_line;
                self.scroll_state.select(Some(new_line));
            }
        }
    }

    /// Scroll up.
    #[allow(dead_code)]
    pub fn scroll_up(&mut self) {
        if let Some(lyrics) = &self.lyrics {
            if !lyrics.line.is_empty() {
                let current = self.scroll_state.selected().unwrap_or(0);
                let new = current.saturating_sub(1);
                self.scroll_state.select(Some(new));
            }
        }
    }

    /// Scroll down.
    #[allow(dead_code)]
    pub fn scroll_down(&mut self) {
        if let Some(lyrics) = &self.lyrics {
            if !lyrics.line.is_empty() {
                let current = self.scroll_state.selected().unwrap_or(0);
                let new = (current + 1).min(lyrics.line.len().saturating_sub(1));
                self.scroll_state.select(Some(new));
            }
        }
    }
}

/// Render the lyrics panel.
pub fn render_lyrics(frame: &mut Frame, area: Rect, state: &mut LyricsState) {
    // Clear background
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Lyrics [L to close]")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.loading {
        let loading =
            Paragraph::new("Loading lyrics...").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(loading, inner);
        return;
    }

    match &state.lyrics {
        None => {
            let no_lyrics =
                Paragraph::new("No lyrics available").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(no_lyrics, inner);
        }
        Some(lyrics) => {
            if lyrics.synced {
                render_synced_lyrics(
                    frame,
                    inner,
                    &lyrics.line,
                    state.current_line,
                    &mut state.scroll_state,
                );
            } else {
                render_unsynced_lyrics(frame, inner, &lyrics.line, &mut state.scroll_state);
            }
        }
    }
}

/// Render synced lyrics with current line highlighted.
fn render_synced_lyrics(
    frame: &mut Frame,
    area: Rect,
    lines: &[LyricLine],
    current_line: usize,
    scroll_state: &mut ListState,
) {
    let items: Vec<ListItem> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let style = if i == current_line {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if i < current_line {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(&line.value, style)))
        })
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(Color::DarkGray));

    // Center the current line in view
    scroll_state.select(Some(current_line));

    frame.render_stateful_widget(list, area, scroll_state);
}

/// Render unsynced lyrics (plain text).
fn render_unsynced_lyrics(
    frame: &mut Frame,
    area: Rect,
    lines: &[LyricLine],
    scroll_state: &mut ListState,
) {
    let items: Vec<ListItem> = lines
        .iter()
        .map(|line| {
            ListItem::new(Line::from(Span::styled(
                &line.value,
                Style::default().fg(Color::White),
            )))
        })
        .collect();

    let list = List::new(items).highlight_style(Style::default().fg(Color::Yellow));

    frame.render_stateful_widget(list, area, scroll_state);
}
