//! Search component.

use std::time::Instant;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::client::models::{Album, Artist, Song};

/// Debounce delay in milliseconds.
const DEBOUNCE_MS: u128 = 300;

/// Minimum query length to trigger search.
const MIN_QUERY_LENGTH: usize = 2;

/// Search state.
#[derive(Debug, Default)]
pub struct SearchState {
    /// Whether search is active
    pub active: bool,

    /// Current search query
    pub query: String,

    /// Search results - artists
    pub artists: Vec<Artist>,

    /// Search results - albums
    pub albums: Vec<Album>,

    /// Search results - songs
    pub songs: Vec<Song>,

    /// Currently focused section (0=artists, 1=albums, 2=songs)
    pub focus: usize,

    /// List states for each section
    pub artists_state: ListState,
    pub albums_state: ListState,
    pub songs_state: ListState,

    /// Is searching (loading)
    pub searching: bool,

    /// Last time the query was modified (for debouncing)
    last_query_change: Option<Instant>,

    /// The query that was last searched (to avoid duplicate searches)
    last_searched_query: String,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open search.
    pub fn open(&mut self) {
        self.active = true;
        self.query.clear();
        self.last_query_change = None;
        self.last_searched_query.clear();
        self.clear_results();
    }

    /// Close search.
    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.last_query_change = None;
        self.last_searched_query.clear();
        self.clear_results();
    }

    /// Clear search results.
    pub fn clear_results(&mut self) {
        self.artists.clear();
        self.albums.clear();
        self.songs.clear();
        self.artists_state.select(None);
        self.albums_state.select(None);
        self.songs_state.select(None);
        self.focus = 0;
    }

    /// Set search results.
    pub fn set_results(&mut self, artists: Vec<Artist>, albums: Vec<Album>, songs: Vec<Song>) {
        self.artists = artists;
        self.albums = albums;
        self.songs = songs;
        self.searching = false;

        // Select first item in first non-empty section, or clear all if empty
        if !self.artists.is_empty() {
            self.artists_state.select(Some(0));
            self.albums_state.select(None);
            self.songs_state.select(None);
            self.focus = 0;
        } else if !self.albums.is_empty() {
            self.artists_state.select(None);
            self.albums_state.select(Some(0));
            self.songs_state.select(None);
            self.focus = 1;
        } else if !self.songs.is_empty() {
            self.artists_state.select(None);
            self.albums_state.select(None);
            self.songs_state.select(Some(0));
            self.focus = 2;
        } else {
            // All results are empty - clear all selection states
            self.artists_state.select(None);
            self.albums_state.select(None);
            self.songs_state.select(None);
            self.focus = 0;
        }
    }

    /// Add character to query and mark as changed.
    pub fn input(&mut self, c: char) {
        self.query.push(c);
        self.last_query_change = Some(Instant::now());
    }

    /// Remove last character from query and mark as changed.
    pub fn backspace(&mut self) {
        if self.query.pop().is_some() {
            self.last_query_change = Some(Instant::now());
        }
    }

    /// Check if a debounced search should be triggered.
    /// Returns true if we should search now.
    pub fn should_search(&self) -> bool {
        // Don't search if query is too short
        if self.query.len() < MIN_QUERY_LENGTH {
            return false;
        }

        // Don't search if already searching
        if self.searching {
            return false;
        }

        // Don't search if query hasn't changed since last search
        if self.query == self.last_searched_query {
            return false;
        }

        // Check if debounce time has passed
        if let Some(last_change) = self.last_query_change {
            last_change.elapsed().as_millis() >= DEBOUNCE_MS
        } else {
            false
        }
    }

    /// Mark that a search has been initiated.
    pub fn mark_search_started(&mut self) {
        self.last_searched_query = self.query.clone();
        self.searching = true;
    }

    /// Force an immediate search (e.g., when Enter is pressed).
    #[allow(dead_code)]
    pub fn should_force_search(&self) -> bool {
        self.query.len() >= MIN_QUERY_LENGTH
            && !self.searching
            && self.query != self.last_searched_query
    }

    /// Get current list state based on focus.
    pub fn active_list_state(&mut self) -> &mut ListState {
        match self.focus {
            0 => &mut self.artists_state,
            1 => &mut self.albums_state,
            _ => &mut self.songs_state,
        }
    }

    /// Get current list length based on focus.
    fn active_list_len(&self) -> usize {
        match self.focus {
            0 => self.artists.len(),
            1 => self.albums.len(),
            _ => self.songs.len(),
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }

        let state = self.active_list_state();
        let i = match state.selected() {
            Some(i) if i > 0 => i - 1,
            Some(_) => len - 1,
            None => 0,
        };
        state.select(Some(i));
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }

        let state = self.active_list_state();
        let i = match state.selected() {
            Some(i) if i < len - 1 => i + 1,
            Some(_) => 0,
            None => 0,
        };
        state.select(Some(i));
    }

    /// Switch to next section.
    pub fn next_section(&mut self) {
        self.focus = (self.focus + 1) % 3;

        // Select first item in new section, or clear selection if empty
        let len = self.active_list_len();
        if len > 0 {
            self.active_list_state().select(Some(0));
        } else {
            self.active_list_state().select(None);
        }
    }

    /// Switch to previous section.
    pub fn prev_section(&mut self) {
        self.focus = if self.focus == 0 { 2 } else { self.focus - 1 };

        // Select first item in new section, or clear selection if empty
        let len = self.active_list_len();
        if len > 0 {
            self.active_list_state().select(Some(0));
        } else {
            self.active_list_state().select(None);
        }
    }

    /// Get selected artist.
    pub fn selected_artist(&self) -> Option<&Artist> {
        if self.focus == 0 {
            self.artists_state
                .selected()
                .and_then(|i| self.artists.get(i))
        } else {
            None
        }
    }

    /// Get selected album.
    pub fn selected_album(&self) -> Option<&Album> {
        if self.focus == 1 {
            self.albums_state
                .selected()
                .and_then(|i| self.albums.get(i))
        } else {
            None
        }
    }

    /// Get selected song.
    pub fn selected_song(&self) -> Option<&Song> {
        if self.focus == 2 {
            self.songs_state.selected().and_then(|i| self.songs.get(i))
        } else {
            None
        }
    }

    /// Check if there are any results.
    pub fn has_results(&self) -> bool {
        !self.artists.is_empty() || !self.albums.is_empty() || !self.songs.is_empty()
    }
}

/// Render the search overlay.
pub fn render_search(frame: &mut Frame, area: Rect, state: &mut SearchState) {
    // Create a centered popup
    let popup_area = centered_rect(80, 80, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Search")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Layout: [search input] [results]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(5),    // Results
        ])
        .split(inner);

    // Search input
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title("Query")
        .border_style(Style::default().fg(Color::Cyan));

    let cursor_symbol = if state.searching { "..." } else { "_" };
    let input_text = format!("{}{}", state.query, cursor_symbol);
    let input = Paragraph::new(input_text)
        .style(Style::default().fg(Color::White))
        .block(input_block);

    frame.render_widget(input, chunks[0]);

    // Results (3 columns)
    if state.has_results() || state.searching {
        let result_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(chunks[1]);

        // Artists column
        render_search_column(
            frame,
            result_chunks[0],
            "Artists",
            &state.artists,
            &mut state.artists_state,
            state.focus == 0,
            |a| a.name.clone(),
        );

        // Albums column
        render_search_column(
            frame,
            result_chunks[1],
            "Albums",
            &state.albums,
            &mut state.albums_state,
            state.focus == 1,
            |a| format!("{} - {}", a.name, a.artist.as_deref().unwrap_or("Unknown")),
        );

        // Songs column
        render_search_column(
            frame,
            result_chunks[2],
            "Songs",
            &state.songs,
            &mut state.songs_state,
            state.focus == 2,
            |s| format!("{} - {}", s.title, s.artist.as_deref().unwrap_or("Unknown")),
        );
    } else if !state.query.is_empty() {
        let hint = if state.query.len() < MIN_QUERY_LENGTH {
            Paragraph::new(format!(
                "Type at least {} characters to search...",
                MIN_QUERY_LENGTH
            ))
            .style(Style::default().fg(Color::DarkGray))
        } else {
            Paragraph::new("No results found").style(Style::default().fg(Color::DarkGray))
        };
        frame.render_widget(hint, chunks[1]);
    } else {
        let hint = Paragraph::new("Type to search (auto-searches after 300ms)...")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[1]);
    }
}

fn render_search_column<T, F>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[T],
    state: &mut ListState,
    focused: bool,
    format_fn: F,
) where
    F: Fn(&T) -> String,
{
    let border_color = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("{} ({})", title, items.len()))
        .border_style(Style::default().fg(border_color));

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| ListItem::new(format_fn(item)))
        .collect();

    let list = List::new(list_items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, state);
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
