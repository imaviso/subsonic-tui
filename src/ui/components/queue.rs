//! Play queue component.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::client::models::Song;

/// Queue state.
#[derive(Debug, Default)]
pub struct QueueState {
    /// Songs in the queue
    pub songs: Vec<Song>,

    /// Currently playing index
    pub current_index: Option<usize>,

    /// Selection state for UI
    pub list_state: ListState,

    /// Whether the queue is visible
    pub visible: bool,
}

impl QueueState {
    pub fn new() -> Self {
        Self {
            visible: true,
            ..Default::default()
        }
    }

    /// Add a song to the queue.
    pub fn add(&mut self, song: Song) {
        self.songs.push(song);
    }

    /// Add multiple songs to the queue.
    pub fn add_all(&mut self, songs: Vec<Song>) {
        self.songs.extend(songs);
    }

    /// Clear the queue.
    pub fn clear(&mut self) {
        self.songs.clear();
        self.current_index = None;
        self.list_state.select(None);
    }

    /// Remove a song from the queue.
    pub fn remove(&mut self, index: usize) {
        if index < self.songs.len() {
            self.songs.remove(index);

            // Adjust current index if needed
            if let Some(current) = self.current_index {
                if index < current {
                    self.current_index = Some(current - 1);
                } else if index == current {
                    // Currently playing song was removed
                    self.current_index = None;
                }
            }

            // Adjust selection to stay valid
            if self.songs.is_empty() {
                self.list_state.select(None);
            } else if let Some(selected) = self.list_state.selected() {
                if selected >= self.songs.len() {
                    // Selection was at end, move to new last item
                    self.list_state.select(Some(self.songs.len() - 1));
                }
            }
        }
    }

    /// Remove the currently selected song from the queue.
    /// Returns true if a song was removed.
    pub fn remove_selected(&mut self) -> bool {
        if let Some(index) = self.list_state.selected() {
            self.remove(index);
            true
        } else {
            false
        }
    }

    /// Get the current song.
    pub fn current_song(&self) -> Option<&Song> {
        self.current_index.and_then(|i| self.songs.get(i))
    }

    /// Get the next song.
    pub fn next_song(&self) -> Option<(usize, &Song)> {
        match self.current_index {
            Some(i) if i + 1 < self.songs.len() => Some((i + 1, &self.songs[i + 1])),
            None if !self.songs.is_empty() => Some((0, &self.songs[0])),
            _ => None,
        }
    }

    /// Get the previous song.
    pub fn previous_song(&self) -> Option<(usize, &Song)> {
        match self.current_index {
            Some(i) if i > 0 => Some((i - 1, &self.songs[i - 1])),
            _ => None,
        }
    }

    /// Move to the next song.
    pub fn advance(&mut self) -> Option<&Song> {
        if let Some((i, _)) = self.next_song() {
            self.current_index = Some(i);
            self.current_song()
        } else {
            None
        }
    }

    /// Move to the previous song.
    pub fn go_back(&mut self) -> Option<&Song> {
        if let Some((i, _)) = self.previous_song() {
            self.current_index = Some(i);
            self.current_song()
        } else {
            None
        }
    }

    /// Play a specific song from the queue.
    pub fn play_index(&mut self, index: usize) -> Option<&Song> {
        if index < self.songs.len() {
            self.current_index = Some(index);
            self.current_song()
        } else {
            None
        }
    }

    /// Shuffle the queue (keeping current song if any).
    pub fn shuffle(&mut self) {
        use rand::seq::SliceRandom;

        if self.songs.len() <= 1 {
            return;
        }

        let mut rng = rand::thread_rng();

        if let Some(current_idx) = self.current_index {
            // Keep current song, shuffle the rest
            let current = self.songs.remove(current_idx);
            self.songs.shuffle(&mut rng);
            self.songs.insert(0, current);
            self.current_index = Some(0);
        } else {
            self.songs.shuffle(&mut rng);
        }
    }

    /// Get queue length.
    pub fn len(&self) -> usize {
        self.songs.len()
    }

    /// Check if queue is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        if self.songs.is_empty() {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.songs.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.songs.is_empty() {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.songs.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Get selected index.
    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }

    /// Get the selected song.
    pub fn selected_song(&self) -> Option<&Song> {
        self.list_state.selected().and_then(|i| self.songs.get(i))
    }

    /// Jump to the top of the queue.
    pub fn jump_to_top(&mut self) {
        if !self.songs.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Jump to the bottom of the queue.
    pub fn jump_to_bottom(&mut self) {
        if !self.songs.is_empty() {
            self.list_state.select(Some(self.songs.len() - 1));
        }
    }

    /// Jump to the currently playing track.
    pub fn jump_to_current(&mut self) {
        if let Some(idx) = self.current_index {
            self.list_state.select(Some(idx));
        }
    }

    /// Scroll half a page down.
    pub fn scroll_half_page_down(&mut self, page_size: usize) {
        if self.songs.is_empty() {
            return;
        }

        let half_page = page_size / 2;
        let current = self.list_state.selected().unwrap_or(0);
        let new_index = (current + half_page).min(self.songs.len() - 1);
        self.list_state.select(Some(new_index));
    }

    /// Scroll half a page up.
    pub fn scroll_half_page_up(&mut self, page_size: usize) {
        if self.songs.is_empty() {
            return;
        }

        let half_page = page_size / 2;
        let current = self.list_state.selected().unwrap_or(0);
        let new_index = current.saturating_sub(half_page);
        self.list_state.select(Some(new_index));
    }
}

/// Render the queue panel.
pub fn render_queue(frame: &mut Frame, area: Rect, state: &mut QueueState, focused: bool) {
    let title = format!("Queue ({})", state.songs.len());

    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    // Calculate available width for content (subtract borders only)
    // 2 for borders
    let content_width = area.width.saturating_sub(2) as usize;
    // Prefix takes 2 chars (playing indicator)
    let text_width = content_width;

    // Get the currently selected index
    let selected_index = state.list_state.selected();

    let items: Vec<ListItem> = state
        .songs
        .iter()
        .enumerate()
        .map(|(i, song)| {
            let is_current = state.current_index == Some(i);
            let is_selected = selected_index == Some(i);

            let prefix = if is_current { "▶ " } else { "  " };

            // Determine styles based on selection and current playing state
            let (title_style, duration_style) = if is_selected {
                // Selected item - use brighter colors for contrast on dark gray background
                if is_current {
                    (
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::White),
                    )
                } else {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Gray),
                    )
                }
            } else {
                // Not selected
                if is_current {
                    (
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::DarkGray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::DarkGray),
                    )
                }
            };

            let duration = song.duration_string();
            let duration_len = duration.chars().count();

            // Use char count for proper width calculation
            let title_char_count = song.title.chars().count();
            let prefix_len = 2; // "▶ " or "  "

            // Space needed: prefix + title + at least 1 space + duration
            // Available: text_width
            // Title can use: text_width - duration_len - 1 (for spacing)
            let title_max_width = text_width.saturating_sub(duration_len + 1 + prefix_len);

            if title_char_count <= title_max_width && title_max_width > 0 {
                // Title fits on one line with duration
                let padding =
                    text_width.saturating_sub(prefix_len + title_char_count + duration_len);
                let spaces = " ".repeat(padding);
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, title_style),
                    Span::styled(song.title.clone(), title_style),
                    Span::raw(spaces),
                    Span::styled(duration, duration_style),
                ]))
            } else {
                // Title needs to wrap - create multiple lines
                let mut lines = Vec::new();
                let title_chars: Vec<char> = song.title.chars().collect();

                // First line width: what's available after prefix
                let first_line_max = text_width.saturating_sub(prefix_len);
                let first_chunk: String = title_chars.iter().take(first_line_max).collect();
                let mut pos = first_chunk.chars().count();

                lines.push(Line::from(vec![
                    Span::styled(prefix, title_style),
                    Span::styled(first_chunk, title_style),
                ]));

                // Continuation lines (indented with same prefix width)
                let continuation_indent = "  "; // Same as prefix width
                let continuation_width = text_width.saturating_sub(2);

                while pos < title_chars.len() {
                    let remaining_chars = title_chars.len() - pos;
                    let is_last_chunk = remaining_chars <= continuation_width;

                    if is_last_chunk {
                        // Last chunk - try to fit duration on same line
                        let chunk: String = title_chars.iter().skip(pos).collect();
                        let chunk_len = chunk.chars().count();
                        let space_for_duration = continuation_width.saturating_sub(chunk_len);

                        if space_for_duration > duration_len {
                            // Duration fits on this line
                            let padding = space_for_duration.saturating_sub(duration_len);
                            let spaces = " ".repeat(padding);
                            lines.push(Line::from(vec![
                                Span::raw(continuation_indent),
                                Span::styled(chunk, title_style),
                                Span::raw(spaces),
                                Span::styled(duration.clone(), duration_style),
                            ]));
                        } else {
                            // Duration doesn't fit, put it on next line
                            lines.push(Line::from(vec![
                                Span::raw(continuation_indent),
                                Span::styled(chunk, title_style),
                            ]));
                            let duration_padding = text_width.saturating_sub(duration_len);
                            let duration_spaces = " ".repeat(duration_padding);
                            lines.push(Line::from(vec![
                                Span::raw(duration_spaces),
                                Span::styled(duration.clone(), duration_style),
                            ]));
                        }
                        break;
                    } else {
                        let chunk: String = title_chars
                            .iter()
                            .skip(pos)
                            .take(continuation_width)
                            .collect();
                        pos += chunk.chars().count();

                        lines.push(Line::from(vec![
                            Span::raw(continuation_indent),
                            Span::styled(chunk, title_style),
                        ]));
                    }
                }

                ListItem::new(lines)
            }
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(list, area, &mut state.list_state);
}
