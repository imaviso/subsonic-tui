//! Library browser component for artists, albums, and songs.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, ListState, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::action::Tab;
use crate::client::models::{Album, Artist, Genre, Playlist, Song};

/// Library view state.
#[derive(Debug, Default)]
pub struct LibraryState {
    /// Currently selected tab
    pub tab: Tab,

    /// Artists list
    pub artists: Vec<Artist>,
    pub artists_state: ListState,

    /// Albums list
    pub albums: Vec<Album>,
    pub albums_state: ListState,

    /// Songs list (from album or random)
    pub songs: Vec<Song>,
    pub songs_state: ListState,

    /// Playlists list
    pub playlists: Vec<Playlist>,
    pub playlists_state: ListState,

    /// Genres list
    pub genres: Vec<Genre>,
    pub genres_state: ListState,

    /// Currently selected genre (for drill-down)
    pub selected_genre: Option<Genre>,
    pub genre_albums: Vec<Album>,
    pub genre_albums_state: ListState,

    /// Favorites (starred items)
    pub favorites_artists: Vec<Artist>,
    pub favorites_artists_state: ListState,
    pub favorites_albums: Vec<Album>,
    pub favorites_albums_state: ListState,
    pub favorites_songs: Vec<Song>,
    pub favorites_songs_state: ListState,
    /// Current section in favorites view (0=artists, 1=albums, 2=songs)
    pub favorites_section: u8,

    /// Currently selected artist (for drill-down)
    pub selected_artist: Option<Artist>,
    pub artist_albums: Vec<Album>,
    pub artist_albums_state: ListState,

    /// Currently selected album (for drill-down)
    pub selected_album: Option<Album>,
    pub album_songs: Vec<Song>,
    pub album_songs_state: ListState,

    /// View depth (0 = list, 1 = artist/album detail)
    pub view_depth: u8,

    /// Loading state
    pub loading: bool,
}

impl LibraryState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently active list state based on tab and depth.
    pub fn active_list_state(&mut self) -> &mut ListState {
        match self.tab {
            Tab::Artists => {
                if self.view_depth == 0 {
                    &mut self.artists_state
                } else if self.view_depth == 1 {
                    &mut self.artist_albums_state
                } else {
                    &mut self.album_songs_state
                }
            }
            Tab::Albums => {
                if self.view_depth == 0 {
                    &mut self.albums_state
                } else {
                    &mut self.album_songs_state
                }
            }
            Tab::Songs => &mut self.songs_state,
            Tab::Playlists => {
                if self.view_depth == 0 {
                    &mut self.playlists_state
                } else {
                    &mut self.album_songs_state
                }
            }
            Tab::Genres => {
                if self.view_depth == 0 {
                    &mut self.genres_state
                } else if self.view_depth == 1 {
                    &mut self.genre_albums_state
                } else {
                    &mut self.album_songs_state
                }
            }
            Tab::Favorites => {
                if self.view_depth == 0 {
                    // Top-level favorites view - depends on section
                    match self.favorites_section {
                        0 => &mut self.favorites_artists_state,
                        1 => &mut self.favorites_albums_state,
                        _ => &mut self.favorites_songs_state,
                    }
                } else if self.view_depth == 1 {
                    // Drill-down into artist -> albums
                    &mut self.artist_albums_state
                } else {
                    // Drill-down into album -> songs
                    &mut self.album_songs_state
                }
            }
        }
    }

    /// Get the length of the currently active list.
    pub fn active_list_len(&self) -> usize {
        match self.tab {
            Tab::Artists => {
                if self.view_depth == 0 {
                    self.artists.len()
                } else if self.view_depth == 1 {
                    self.artist_albums.len()
                } else {
                    self.album_songs.len()
                }
            }
            Tab::Albums => {
                if self.view_depth == 0 {
                    self.albums.len()
                } else {
                    self.album_songs.len()
                }
            }
            Tab::Songs => self.songs.len(),
            Tab::Playlists => {
                if self.view_depth == 0 {
                    self.playlists.len()
                } else {
                    self.album_songs.len()
                }
            }
            Tab::Genres => {
                if self.view_depth == 0 {
                    self.genres.len()
                } else if self.view_depth == 1 {
                    self.genre_albums.len()
                } else {
                    self.album_songs.len()
                }
            }
            Tab::Favorites => {
                if self.view_depth == 0 {
                    match self.favorites_section {
                        0 => self.favorites_artists.len(),
                        1 => self.favorites_albums.len(),
                        _ => self.favorites_songs.len(),
                    }
                } else if self.view_depth == 1 {
                    self.artist_albums.len()
                } else {
                    self.album_songs.len()
                }
            }
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }

        let selected = self.active_list_state().selected();
        let i = match selected {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.active_list_state().select(Some(i));
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }

        let selected = self.active_list_state().selected();
        let i = match selected {
            Some(i) => {
                if i >= len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.active_list_state().select(Some(i));
    }

    /// Get selected artist.
    pub fn selected_artist_item(&self) -> Option<&Artist> {
        self.artists_state
            .selected()
            .and_then(|i| self.artists.get(i))
    }

    /// Get selected album.
    pub fn selected_album_item(&self) -> Option<&Album> {
        if self.view_depth == 0 {
            self.albums_state
                .selected()
                .and_then(|i| self.albums.get(i))
        } else {
            self.artist_albums_state
                .selected()
                .and_then(|i| self.artist_albums.get(i))
        }
    }

    /// Get selected song.
    pub fn selected_song_item(&self) -> Option<&Song> {
        if self.view_depth == 0 {
            self.songs_state.selected().and_then(|i| self.songs.get(i))
        } else {
            self.album_songs_state
                .selected()
                .and_then(|i| self.album_songs.get(i))
        }
    }

    /// Get selected playlist.
    pub fn selected_playlist_item(&self) -> Option<&Playlist> {
        self.playlists_state
            .selected()
            .and_then(|i| self.playlists.get(i))
    }

    /// Set artists and reset selection.
    pub fn set_artists(&mut self, artists: Vec<Artist>) {
        self.artists = artists;
        if self.artists.is_empty() {
            self.artists_state.select(None);
        } else {
            self.artists_state.select(Some(0));
        }
    }

    /// Set albums and reset selection.
    pub fn set_albums(&mut self, albums: Vec<Album>) {
        self.albums = albums;
        if self.albums.is_empty() {
            self.albums_state.select(None);
        } else {
            self.albums_state.select(Some(0));
        }
    }

    /// Set songs and reset selection.
    pub fn set_songs(&mut self, songs: Vec<Song>) {
        self.songs = songs;
        if self.songs.is_empty() {
            self.songs_state.select(None);
        } else {
            self.songs_state.select(Some(0));
        }
    }

    /// Set playlists and reset selection.
    pub fn set_playlists(&mut self, playlists: Vec<Playlist>) {
        self.playlists = playlists;
        if self.playlists.is_empty() {
            self.playlists_state.select(None);
        } else {
            self.playlists_state.select(Some(0));
        }
    }

    /// Set genres and reset selection.
    pub fn set_genres(&mut self, genres: Vec<Genre>) {
        self.genres = genres;
        if self.genres.is_empty() {
            self.genres_state.select(None);
        } else {
            self.genres_state.select(Some(0));
        }
    }

    /// Get selected genre.
    pub fn selected_genre_item(&self) -> Option<&Genre> {
        self.genres_state
            .selected()
            .and_then(|i| self.genres.get(i))
    }

    /// Get selected genre album (when in genre detail view).
    pub fn selected_genre_album_item(&self) -> Option<&Album> {
        self.genre_albums_state
            .selected()
            .and_then(|i| self.genre_albums.get(i))
    }

    /// Enter genre detail view.
    pub fn enter_genre(&mut self, genre: Genre, albums: Vec<Album>) {
        self.selected_genre = Some(genre);
        self.genre_albums = albums;
        self.view_depth = 1;
        if self.genre_albums.is_empty() {
            self.genre_albums_state.select(None);
        } else {
            self.genre_albums_state.select(Some(0));
        }
    }

    /// Set favorites and reset selection.
    pub fn set_favorites(&mut self, artists: Vec<Artist>, albums: Vec<Album>, songs: Vec<Song>) {
        self.favorites_artists = artists;
        self.favorites_albums = albums;
        self.favorites_songs = songs;
        // Reset selections - clear if empty, select first if not
        if self.favorites_artists.is_empty() {
            self.favorites_artists_state.select(None);
        } else {
            self.favorites_artists_state.select(Some(0));
        }
        if self.favorites_albums.is_empty() {
            self.favorites_albums_state.select(None);
        } else {
            self.favorites_albums_state.select(Some(0));
        }
        if self.favorites_songs.is_empty() {
            self.favorites_songs_state.select(None);
        } else {
            self.favorites_songs_state.select(Some(0));
        }
    }

    /// Get selected favorite artist.
    pub fn selected_favorite_artist(&self) -> Option<&Artist> {
        self.favorites_artists_state
            .selected()
            .and_then(|i| self.favorites_artists.get(i))
    }

    /// Get selected favorite album.
    pub fn selected_favorite_album(&self) -> Option<&Album> {
        self.favorites_albums_state
            .selected()
            .and_then(|i| self.favorites_albums.get(i))
    }

    /// Get selected favorite song.
    pub fn selected_favorite_song(&self) -> Option<&Song> {
        self.favorites_songs_state
            .selected()
            .and_then(|i| self.favorites_songs.get(i))
    }

    /// Move to next favorites section. Returns true if moved, false if at rightmost section.
    /// Skips empty sections.
    pub fn next_favorites_section(&mut self) -> bool {
        let mut next = self.favorites_section + 1;
        while next <= 2 {
            let len = match next {
                0 => self.favorites_artists.len(),
                1 => self.favorites_albums.len(),
                _ => self.favorites_songs.len(),
            };
            if len > 0 {
                self.favorites_section = next;
                return true;
            }
            next += 1;
        }
        false
    }

    /// Move to previous favorites section. Returns true if moved, false if at leftmost section.
    /// Skips empty sections.
    pub fn prev_favorites_section(&mut self) -> bool {
        if self.favorites_section == 0 {
            return false;
        }
        let mut prev = self.favorites_section - 1;
        loop {
            let len = match prev {
                0 => self.favorites_artists.len(),
                1 => self.favorites_albums.len(),
                _ => self.favorites_songs.len(),
            };
            if len > 0 {
                self.favorites_section = prev;
                return true;
            }
            if prev == 0 {
                break;
            }
            prev -= 1;
        }
        false
    }

    /// Enter artist detail view.
    pub fn enter_artist(&mut self, artist: Artist, albums: Vec<Album>) {
        self.selected_artist = Some(artist);
        self.artist_albums = albums;
        self.view_depth = 1;
        if self.artist_albums.is_empty() {
            self.artist_albums_state.select(None);
        } else {
            self.artist_albums_state.select(Some(0));
        }
    }

    /// Enter album detail view.
    pub fn enter_album(&mut self, album: Album, songs: Vec<Song>) {
        self.selected_album = Some(album);
        self.album_songs = songs;
        self.view_depth = if self.tab == Tab::Albums { 1 } else { 2 };
        if self.album_songs.is_empty() {
            self.album_songs_state.select(None);
        } else {
            self.album_songs_state.select(Some(0));
        }
    }

    /// Go back to previous view.
    pub fn go_back(&mut self) {
        if self.view_depth > 0 {
            self.view_depth -= 1;
            if self.view_depth == 0 {
                self.selected_artist = None;
                self.selected_album = None;
                self.selected_genre = None;
            } else if self.view_depth == 1 {
                self.selected_album = None;
            }
        }
    }

    /// Jump to the top of the current list.
    pub fn jump_to_top(&mut self) {
        if self.active_list_len() > 0 {
            self.active_list_state().select(Some(0));
        }
    }

    /// Jump to the bottom of the current list.
    pub fn jump_to_bottom(&mut self) {
        let len = self.active_list_len();
        if len > 0 {
            self.active_list_state().select(Some(len - 1));
        }
    }

    /// Scroll half a page down.
    pub fn scroll_half_page_down(&mut self, page_size: usize) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }

        let half_page = page_size / 2;
        let current = self.active_list_state().selected().unwrap_or(0);
        let new_index = (current + half_page).min(len - 1);
        self.active_list_state().select(Some(new_index));
    }

    /// Scroll half a page up.
    pub fn scroll_half_page_up(&mut self, page_size: usize) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }

        let half_page = page_size / 2;
        let current = self.active_list_state().selected().unwrap_or(0);
        let new_index = current.saturating_sub(half_page);
        self.active_list_state().select(Some(new_index));
    }
}

/// Render the library view.
pub fn render_library(frame: &mut Frame, area: Rect, state: &mut LibraryState, focused: bool) {
    let title: String = match state.tab {
        Tab::Artists => {
            if state.view_depth == 0 {
                String::from("Artists")
            } else if state.view_depth == 1 {
                state
                    .selected_artist
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| String::from("Artist"))
            } else {
                // Depth 2: show album name
                state
                    .selected_album
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| String::from("Album"))
            }
        }
        Tab::Albums => {
            if state.view_depth == 0 {
                String::from("Albums")
            } else {
                state
                    .selected_album
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| String::from("Album"))
            }
        }
        Tab::Songs => String::from("Songs"),
        Tab::Playlists => {
            if state.view_depth == 0 {
                String::from("Playlists")
            } else {
                String::from("Playlist")
            }
        }
        Tab::Genres => {
            if state.view_depth == 0 {
                String::from("Genres")
            } else if state.view_depth == 1 {
                state
                    .selected_genre
                    .as_ref()
                    .map(|g| g.value.clone())
                    .unwrap_or_else(|| String::from("Genre"))
            } else {
                state
                    .selected_album
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| String::from("Album"))
            }
        }
        Tab::Favorites => {
            if state.view_depth == 0 {
                String::from("Favorites")
            } else if state.view_depth == 1 {
                state
                    .selected_artist
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| String::from("Artist"))
            } else {
                state
                    .selected_album
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| String::from("Album"))
            }
        }
    };

    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    if state.loading {
        let loading = Paragraph::new("Loading...")
            .style(Style::default().fg(Color::Yellow))
            .block(block);
        frame.render_widget(loading, area);
        return;
    }

    match state.tab {
        Tab::Artists => render_artists_view(frame, area, state, block),
        Tab::Albums => render_albums_view(frame, area, state, block),
        Tab::Songs => render_songs_view(frame, area, state, block),
        Tab::Playlists => render_playlists_view(frame, area, state, block),
        Tab::Genres => render_genres_view(frame, area, state, block),
        Tab::Favorites => render_favorites_view(frame, area, state, block),
    }
}

fn render_artists_view(frame: &mut Frame, area: Rect, state: &mut LibraryState, block: Block) {
    if state.view_depth == 0 {
        // Artist list with columns: Artist Name | Album Count
        let mut table_state = TableState::default();
        table_state.select(state.artists_state.selected());
        let selected_idx = table_state.selected();

        let rows: Vec<Row> = state
            .artists
            .iter()
            .enumerate()
            .map(|(i, artist)| {
                let is_selected = selected_idx == Some(i);
                let album_count = artist
                    .album_count
                    .map(|c| format!("{} albums", c))
                    .unwrap_or_default();

                let (name_style, count_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                Row::new(vec![
                    Cell::from(artist.name.clone()).style(name_style),
                    Cell::from(album_count).style(count_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(75), // Artist name
                Constraint::Percentage(25), // Album count
            ],
        )
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut table_state);
        *state.artists_state.selected_mut() = table_state.selected();
    } else if state.view_depth == 1 {
        // Artist albums with columns: Album Name | Year
        let mut table_state = TableState::default();
        table_state.select(state.artist_albums_state.selected());
        let selected_idx = table_state.selected();

        let rows: Vec<Row> = state
            .artist_albums
            .iter()
            .enumerate()
            .map(|(i, album)| {
                let is_selected = selected_idx == Some(i);
                let year = album.year.map(|y| y.to_string()).unwrap_or_default();

                let (name_style, year_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                Row::new(vec![
                    Cell::from(album.name.clone()).style(name_style),
                    Cell::from(year).style(year_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(85), // Album name
                Constraint::Length(6),      // Year
            ],
        )
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut table_state);
        *state.artist_albums_state.selected_mut() = table_state.selected();
    } else {
        // Album songs (depth 2)
        render_song_list(
            frame,
            area,
            &state.album_songs,
            &mut state.album_songs_state,
            block,
        );
    }
}

fn render_albums_view(frame: &mut Frame, area: Rect, state: &mut LibraryState, block: Block) {
    if state.view_depth == 0 {
        // Album list with columns: Album Name | Artist | Year
        let mut table_state = TableState::default();
        table_state.select(state.albums_state.selected());
        let selected_idx = table_state.selected();

        let rows: Vec<Row> = state
            .albums
            .iter()
            .enumerate()
            .map(|(i, album)| {
                let is_selected = selected_idx == Some(i);
                let artist = album.artist.as_deref().unwrap_or("Unknown Artist");
                let year = album.year.map(|y| y.to_string()).unwrap_or_default();

                let (name_style, artist_style, year_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::LightCyan),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::Cyan),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                Row::new(vec![
                    Cell::from(album.name.clone()).style(name_style),
                    Cell::from(artist.to_string()).style(artist_style),
                    Cell::from(year).style(year_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(50), // Album name
                Constraint::Percentage(40), // Artist
                Constraint::Length(6),      // Year
            ],
        )
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut table_state);
        *state.albums_state.selected_mut() = table_state.selected();
    } else {
        // Album songs
        render_song_list(
            frame,
            area,
            &state.album_songs,
            &mut state.album_songs_state,
            block,
        );
    }
}

fn render_songs_view(frame: &mut Frame, area: Rect, state: &mut LibraryState, block: Block) {
    render_song_list(frame, area, &state.songs, &mut state.songs_state, block);
}

fn render_playlists_view(frame: &mut Frame, area: Rect, state: &mut LibraryState, block: Block) {
    if state.view_depth == 0 {
        // Playlist list with columns: Playlist Name | Song Count
        let mut table_state = TableState::default();
        table_state.select(state.playlists_state.selected());
        let selected_idx = table_state.selected();

        let rows: Vec<Row> = state
            .playlists
            .iter()
            .enumerate()
            .map(|(i, playlist)| {
                let is_selected = selected_idx == Some(i);
                let count = playlist
                    .song_count
                    .map(|c| format!("{} songs", c))
                    .unwrap_or_default();

                let (name_style, count_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                Row::new(vec![
                    Cell::from(playlist.name.clone()).style(name_style),
                    Cell::from(count).style(count_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(75), // Playlist name
                Constraint::Percentage(25), // Song count
            ],
        )
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut table_state);
        *state.playlists_state.selected_mut() = table_state.selected();
    } else {
        // Playlist songs
        render_song_list(
            frame,
            area,
            &state.album_songs,
            &mut state.album_songs_state,
            block,
        );
    }
}

fn render_song_list(
    frame: &mut Frame,
    area: Rect,
    songs: &[Song],
    list_state: &mut ListState,
    block: Block,
) {
    // Convert ListState to TableState
    let mut table_state = TableState::default();
    table_state.select(list_state.selected());

    let selected_idx = table_state.selected();

    let rows: Vec<Row> = songs
        .iter()
        .enumerate()
        .map(|(i, song)| {
            let is_selected = selected_idx == Some(i);

            let track = song
                .track
                .map(|t| format!("{:02}", t))
                .unwrap_or_else(|| format!("{:02}", i + 1));
            let duration = song.duration_string();
            let artist = song.display_artist();

            // Use brighter colors for selected row
            let (track_style, title_style, artist_style, duration_style) = if is_selected {
                (
                    Style::default().fg(Color::Gray),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::LightCyan),
                    Style::default().fg(Color::Gray),
                )
            } else {
                (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::White),
                    Style::default().fg(Color::Cyan),
                    Style::default().fg(Color::DarkGray),
                )
            };

            Row::new(vec![
                Cell::from(track).style(track_style),
                Cell::from(song.title.clone()).style(title_style),
                Cell::from(artist).style(artist_style),
                Cell::from(duration).style(duration_style),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),      // Track number
            Constraint::Percentage(50), // Title
            Constraint::Percentage(35), // Artist
            Constraint::Length(6),      // Duration
        ],
    )
    .block(block)
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(table, area, &mut table_state);

    // Sync selection back to ListState
    *list_state.selected_mut() = table_state.selected();
}

fn render_genres_view(frame: &mut Frame, area: Rect, state: &mut LibraryState, block: Block) {
    if state.view_depth == 0 {
        // Genre list with columns: Genre | Albums | Songs
        let mut table_state = TableState::default();
        table_state.select(state.genres_state.selected());
        let selected_idx = table_state.selected();

        let rows: Vec<Row> = state
            .genres
            .iter()
            .enumerate()
            .map(|(i, genre)| {
                let is_selected = selected_idx == Some(i);
                let album_count = genre
                    .album_count
                    .map(|c| format!("{} albums", c))
                    .unwrap_or_default();
                let song_count = genre
                    .song_count
                    .map(|c| format!("{} songs", c))
                    .unwrap_or_default();

                let (name_style, count_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                Row::new(vec![
                    Cell::from(genre.value.clone()).style(name_style),
                    Cell::from(album_count).style(count_style),
                    Cell::from(song_count).style(count_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(50), // Genre name
                Constraint::Percentage(25), // Album count
                Constraint::Percentage(25), // Song count
            ],
        )
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut table_state);
        *state.genres_state.selected_mut() = table_state.selected();
    } else if state.view_depth == 1 {
        // Genre albums with columns: Album | Artist
        let mut table_state = TableState::default();
        table_state.select(state.genre_albums_state.selected());
        let selected_idx = table_state.selected();

        let rows: Vec<Row> = state
            .genre_albums
            .iter()
            .enumerate()
            .map(|(i, album)| {
                let is_selected = selected_idx == Some(i);
                let artist = album.artist.as_deref().unwrap_or("Unknown Artist");

                let (name_style, artist_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::LightCyan),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::Cyan),
                    )
                };

                Row::new(vec![
                    Cell::from(album.name.clone()).style(name_style),
                    Cell::from(artist.to_string()).style(artist_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(60), // Album name
                Constraint::Percentage(40), // Artist
            ],
        )
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut table_state);
        *state.genre_albums_state.selected_mut() = table_state.selected();
    } else {
        // Album songs (depth 2)
        render_song_list(
            frame,
            area,
            &state.album_songs,
            &mut state.album_songs_state,
            block,
        );
    }
}

fn render_favorites_view(frame: &mut Frame, area: Rect, state: &mut LibraryState, block: Block) {
    if state.view_depth == 0 {
        // Top-level favorites view - show three columns for artists, albums, songs
        // First render the outer block
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Split into three columns
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(inner);

        // Render artists column
        let artists_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Artists ({})", state.favorites_artists.len()))
            .border_style(Style::default().fg(if state.favorites_section == 0 {
                Color::Cyan
            } else {
                Color::DarkGray
            }));

        let mut artists_table_state = TableState::default();
        artists_table_state.select(state.favorites_artists_state.selected());
        let artists_selected_idx = artists_table_state.selected();

        let artist_rows: Vec<Row> = state
            .favorites_artists
            .iter()
            .enumerate()
            .map(|(i, artist)| {
                let is_selected = artists_selected_idx == Some(i);
                let style = if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                Row::new(vec![Cell::from(artist.name.clone()).style(style)])
            })
            .collect();

        let artists_table = Table::new(artist_rows, [Constraint::Percentage(100)])
            .block(artists_block)
            .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(artists_table, columns[0], &mut artists_table_state);
        *state.favorites_artists_state.selected_mut() = artists_table_state.selected();

        // Render albums column
        let albums_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Albums ({})", state.favorites_albums.len()))
            .border_style(Style::default().fg(if state.favorites_section == 1 {
                Color::Cyan
            } else {
                Color::DarkGray
            }));

        let mut albums_table_state = TableState::default();
        albums_table_state.select(state.favorites_albums_state.selected());
        let albums_selected_idx = albums_table_state.selected();

        let album_rows: Vec<Row> = state
            .favorites_albums
            .iter()
            .enumerate()
            .map(|(i, album)| {
                let is_selected = albums_selected_idx == Some(i);
                let artist = album.artist.as_deref().unwrap_or("Unknown");

                let (name_style, artist_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::LightCyan),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::Cyan),
                    )
                };

                Row::new(vec![
                    Cell::from(album.name.clone()).style(name_style),
                    Cell::from(artist.to_string()).style(artist_style),
                ])
            })
            .collect();

        let albums_table = Table::new(
            album_rows,
            [Constraint::Percentage(60), Constraint::Percentage(40)],
        )
        .block(albums_block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(albums_table, columns[1], &mut albums_table_state);
        *state.favorites_albums_state.selected_mut() = albums_table_state.selected();

        // Render songs column
        let songs_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Songs ({})", state.favorites_songs.len()))
            .border_style(Style::default().fg(if state.favorites_section == 2 {
                Color::Cyan
            } else {
                Color::DarkGray
            }));

        let mut songs_table_state = TableState::default();
        songs_table_state.select(state.favorites_songs_state.selected());
        let songs_selected_idx = songs_table_state.selected();

        let song_rows: Vec<Row> = state
            .favorites_songs
            .iter()
            .enumerate()
            .map(|(i, song)| {
                let is_selected = songs_selected_idx == Some(i);
                let artist = song.display_artist();
                let duration = song.duration_string();

                let (title_style, artist_style, duration_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::LightCyan),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::Cyan),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                Row::new(vec![
                    Cell::from(song.title.clone()).style(title_style),
                    Cell::from(artist).style(artist_style),
                    Cell::from(duration).style(duration_style),
                ])
            })
            .collect();

        let songs_table = Table::new(
            song_rows,
            [
                Constraint::Percentage(50),
                Constraint::Percentage(35),
                Constraint::Length(6),
            ],
        )
        .block(songs_block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(songs_table, columns[2], &mut songs_table_state);
        *state.favorites_songs_state.selected_mut() = songs_table_state.selected();
    } else if state.view_depth == 1 {
        // Drill-down into artist -> albums with columns: Album | Year
        let mut table_state = TableState::default();
        table_state.select(state.artist_albums_state.selected());
        let selected_idx = table_state.selected();

        let rows: Vec<Row> = state
            .artist_albums
            .iter()
            .enumerate()
            .map(|(i, album)| {
                let is_selected = selected_idx == Some(i);
                let year = album.year.map(|y| y.to_string()).unwrap_or_default();

                let (name_style, year_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                Row::new(vec![
                    Cell::from(album.name.clone()).style(name_style),
                    Cell::from(year).style(year_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(85), // Album name
                Constraint::Length(6),      // Year
            ],
        )
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut table_state);
        *state.artist_albums_state.selected_mut() = table_state.selected();
    } else {
        // Drill-down into album -> songs (depth 2)
        render_song_list(
            frame,
            area,
            &state.album_songs,
            &mut state.album_songs_state,
            block,
        );
    }
}
