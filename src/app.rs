//! Main application state and logic.

use std::time::Duration;

use color_eyre::Result;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::action::{Action, PlayerState, RepeatMode, Tab};
use crate::client::models::Song;
use crate::client::SubsonicClient;
use crate::config::Config;
use crate::player::{Player, PlayerEvent};
use crate::ui::{LibraryState, LyricsState, NowPlayingState, QueueState, SearchState};

/// UI layout areas for mouse click detection.
#[derive(Debug, Default, Clone)]
pub struct UiLayout {
    /// Tab bar area
    pub tabs: Rect,
    /// Library panel area
    pub library: Rect,
    /// Queue panel area (if visible)
    pub queue: Option<Rect>,
    /// Now playing bar area
    pub now_playing: Rect,
    /// Progress bar area within now playing
    pub progress_bar: Rect,
    /// Volume bar area within now playing
    pub volume_bar: Rect,
}

/// Main application state.
pub struct App {
    /// Whether the app should quit
    pub should_quit: bool,

    /// Configuration
    pub config: Config,

    /// API client
    pub client: Option<SubsonicClient>,

    /// Audio player
    pub player: Option<Player>,

    /// Library state
    pub library: LibraryState,

    /// Queue state
    pub queue: QueueState,

    /// Now playing state
    pub now_playing: NowPlayingState,

    /// Search state
    pub search: SearchState,

    /// Lyrics state
    pub lyrics: LyricsState,

    /// Help overlay visible
    pub show_help: bool,

    /// Track info popup visible
    pub show_track_info: bool,

    /// Error message to display
    pub error_message: Option<String>,

    /// Action sender for async operations
    pub action_tx: mpsc::UnboundedSender<Action>,

    /// Focus mode (0 = library, 1 = queue)
    pub focus: u8,

    /// Terminal width for mouse click detection
    pub terminal_width: Option<u16>,

    /// Terminal height for mouse click detection
    pub terminal_height: Option<u16>,

    /// UI layout areas for mouse detection
    pub layout: UiLayout,
}

impl App {
    /// Create a new application instance.
    pub fn new(config: Config, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            should_quit: false,
            config,
            client: None,
            player: None,
            library: LibraryState::new(),
            queue: QueueState::new(),
            now_playing: NowPlayingState::new(),
            search: SearchState::new(),
            lyrics: LyricsState::new(),
            show_help: false,
            show_track_info: false,
            error_message: None,
            action_tx,
            focus: 0,
            terminal_width: Some(width),
            terminal_height: Some(height),
            layout: UiLayout::default(),
        }
    }

    /// Initialize the application.
    pub async fn init(&mut self) -> Result<()> {
        // Initialize the API client
        if self.config.is_valid() {
            let auth = if let Some(api_key) = &self.config.server.api_key {
                crate::client::Auth::from_api_key(api_key)
            } else if let Some(password) = &self.config.server.password {
                crate::client::Auth::from_password(&self.config.server.username, password)
            } else {
                return Err(color_eyre::eyre::eyre!("No password or API key configured"));
            };

            let mut client = SubsonicClient::new(&self.config.server.url, auth);

            // Test connection
            match client.ping().await {
                Ok(_) => {
                    tracing::info!("Connected to server: {}", self.config.server.url);

                    // Check for OpenSubsonic extensions
                    if let Ok(extensions) = client.get_open_subsonic_extensions().await {
                        tracing::info!("OpenSubsonic extensions: {:?}", extensions);
                    }

                    self.client = Some(client);
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to connect: {}", e));
                    tracing::error!("Failed to connect to server: {}", e);
                }
            }
        } else {
            self.error_message = Some(String::from(
                "Invalid configuration. Please configure server URL and credentials.",
            ));
        }

        // Initialize the audio player
        match Player::new() {
            Ok(player) => {
                self.player = Some(player);
            }
            Err(e) => {
                tracing::error!("Failed to initialize audio player: {}", e);
                self.error_message = Some(format!("Audio player error: {}", e));
            }
        }

        // Set initial volume
        if let Some(player) = &self.player {
            let _ = player.set_volume(self.config.player.volume as f32 / 100.0);
        }

        // Load initial data
        self.action_tx.send(Action::LoadArtists)?;
        self.action_tx.send(Action::LoadAlbums)?;
        self.action_tx.send(Action::LoadPlaylists)?;
        self.action_tx.send(Action::LoadSongs)?;
        self.action_tx.send(Action::LoadGenres)?;
        self.action_tx.send(Action::LoadFavorites)?;

        Ok(())
    }

    /// Handle an action and update state.
    pub async fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => {
                self.should_quit = true;
            }

            Action::Tick => {
                // Update player progress - collect events first to avoid borrow issues
                let events: Vec<_> = if let Some(player) = &mut self.player {
                    let mut events = Vec::new();
                    while let Some(event) = player.try_recv_event() {
                        events.push(event);
                    }
                    events
                } else {
                    Vec::new()
                };

                for event in events {
                    self.handle_player_event(event).await?;
                }

                // Check if we should scrobble
                if self.now_playing.should_scrobble() {
                    self.now_playing.mark_scrobbled();
                    self.action_tx.send(Action::Scrobble)?;
                }

                // Update lyrics position if visible
                if self.lyrics.visible {
                    let position_ms = (self.now_playing.position as u64) * 1000;
                    self.lyrics.update_position(position_ms);
                }

                // Check for debounced search
                if self.search.active && self.search.should_search() {
                    self.perform_search().await?;
                }
            }

            Action::Render => {
                // Rendering is handled in the main loop
            }

            Action::Resize(width, height) => {
                // Update terminal dimensions for mouse click detection
                self.terminal_width = Some(width);
                self.terminal_height = Some(height);
            }

            // Navigation
            Action::NavigateUp => {
                if self.search.active {
                    self.search.select_previous();
                } else if self.focus == 0 {
                    self.library.select_previous();
                } else {
                    self.queue.select_previous();
                }
            }

            Action::NavigateDown => {
                if self.search.active {
                    self.search.select_next();
                } else if self.focus == 0 {
                    self.library.select_next();
                } else {
                    self.queue.select_next();
                }
            }

            Action::NavigateLeft => {
                if self.search.active {
                    self.search.prev_section();
                } else if self.focus == 1 {
                    // Moving from queue to library
                    self.focus = 0;
                    // If in favorites, start at rightmost section
                    if self.library.tab == Tab::Favorites && self.library.view_depth == 0 {
                        self.library.favorites_section = 2;
                    }
                } else if self.library.tab == Tab::Favorites && self.library.view_depth == 0 {
                    // In favorites, try to move to previous section
                    self.library.prev_favorites_section();
                    // If already at leftmost, stay there (no wrap)
                }
            }

            Action::NavigateRight => {
                if self.search.active {
                    self.search.next_section();
                } else if self.library.tab == Tab::Favorites && self.library.view_depth == 0 {
                    // In favorites, try to move to next section
                    if !self.library.next_favorites_section() && self.queue.visible {
                        // At rightmost section, move to queue
                        self.focus = 1;
                    }
                } else if self.queue.visible {
                    self.focus = 1;
                }
            }

            Action::MouseClick(x, y) => {
                // Don't handle mouse clicks when overlays are active
                if self.search.active || self.show_help || self.show_track_info {
                    return Ok(());
                }

                // Check if click is on tabs (inside the border, row 1 of the tab area)
                if y == self.layout.tabs.y + 1 {
                    // Account for left border (1 char) and calculate based on tab title positions
                    // Tab format: " Title1 | Title2 | Title3 ..." with divider " | " (3 chars)
                    let click_x = x.saturating_sub(self.layout.tabs.x + 1); // +1 for left border

                    let tabs = Tab::all();
                    let mut pos: u16 = 0;
                    for &tab in tabs {
                        let title_len = tab.title().len() as u16;
                        // Each tab takes: space + title + space = title_len + 2, then divider "|" + space
                        let tab_width = title_len + 2; // " Title "

                        if click_x >= pos && click_x < pos + tab_width {
                            self.library.tab = tab;
                            self.library.view_depth = 0;
                            self.focus = 0;
                            if tab == Tab::Favorites {
                                self.library.favorites_section = 0;
                            }
                            break;
                        }
                        pos += tab_width + 1; // +1 for the "|" divider
                    }
                }
                // Check if click is on volume bar
                else if y == self.layout.volume_bar.y
                    && x >= self.layout.volume_bar.x
                    && x < self.layout.volume_bar.x + self.layout.volume_bar.width
                {
                    // Calculate volume based on click position within the bar
                    // Volume bar is "━━━━━━━━━━" - 10 chars directly
                    let bar_width = self.layout.volume_bar.width;
                    let click_offset = x.saturating_sub(self.layout.volume_bar.x);
                    // Map click position to 0-100%
                    let new_volume =
                        (((click_offset as u32 + 1) * 100) / bar_width as u32).min(100) as u8;
                    self.now_playing.volume = new_volume;
                    if let Some(player) = &self.player {
                        player.set_volume(new_volume as f32 / 100.0)?;
                    }
                }
                // Check if click is on progress bar (for seeking)
                else if y >= self.layout.progress_bar.y
                    && y < self.layout.progress_bar.y + self.layout.progress_bar.height
                    && x >= self.layout.progress_bar.x
                    && x < self.layout.progress_bar.x + self.layout.progress_bar.width
                {
                    // Calculate seek position based on click position
                    let click_offset = x.saturating_sub(self.layout.progress_bar.x);
                    let ratio = click_offset as f64 / self.layout.progress_bar.width as f64;
                    let seek_pos = (ratio * self.now_playing.duration as f64) as u32;
                    if let Some(player) = &self.player {
                        player.seek(std::time::Duration::from_secs(seek_pos as u64))?;
                        self.now_playing.position = seek_pos;
                    }
                }
                // Check if click is on library
                else if y >= self.layout.library.y
                    && y < self.layout.library.y + self.layout.library.height
                    && x >= self.layout.library.x
                    && x < self.layout.library.x + self.layout.library.width
                {
                    self.focus = 0;
                    // Calculate which item was clicked (accounting for border and title)
                    let item_y = y.saturating_sub(self.layout.library.y + 1); // +1 for border
                    self.library
                        .active_list_state()
                        .select(Some(item_y as usize));
                }
                // Check if click is on queue
                else if let Some(queue_area) = self.layout.queue {
                    if y >= queue_area.y
                        && y < queue_area.y + queue_area.height
                        && x >= queue_area.x
                        && x < queue_area.x + queue_area.width
                    {
                        self.focus = 1;
                        // Calculate which item was clicked (accounting for border and title)
                        let item_y = y.saturating_sub(queue_area.y + 1); // +1 for border
                        self.queue.list_state.select(Some(item_y as usize));
                    }
                }
            }

            Action::MouseDoubleClick(x, y) => {
                // Don't handle mouse clicks when overlays are active
                if self.search.active || self.show_help || self.show_track_info {
                    return Ok(());
                }

                // Double-click on library item -> select and play
                if y >= self.layout.library.y
                    && y < self.layout.library.y + self.layout.library.height
                    && x >= self.layout.library.x
                    && x < self.layout.library.x + self.layout.library.width
                {
                    self.focus = 0;
                    let item_y = y.saturating_sub(self.layout.library.y + 1);
                    self.library
                        .active_list_state()
                        .select(Some(item_y as usize));
                    self.handle_library_select().await?;
                }
                // Double-click on queue item -> play that item
                else if let Some(queue_area) = self.layout.queue {
                    if y >= queue_area.y
                        && y < queue_area.y + queue_area.height
                        && x >= queue_area.x
                        && x < queue_area.x + queue_area.width
                    {
                        self.focus = 1;
                        let item_y = y.saturating_sub(queue_area.y + 1);
                        let idx = item_y as usize;
                        if idx < self.queue.len() {
                            self.queue.list_state.select(Some(idx));
                            self.play_from_queue(idx)?;
                        }
                    }
                }
            }

            Action::MouseScroll(delta, x, y) => {
                // Check if scrolling on volume bar
                if y == self.layout.volume_bar.y
                    && x >= self.layout.volume_bar.x
                    && x < self.layout.volume_bar.x + self.layout.volume_bar.width
                {
                    // Adjust volume: scroll up = increase, scroll down = decrease (5% per scroll)
                    let change = if delta < 0 { 5i16 } else { -5i16 };
                    let new_volume = (self.now_playing.volume as i16 + change).clamp(0, 100) as u8;
                    self.now_playing.volume = new_volume;
                    if let Some(player) = &self.player {
                        player.set_volume(new_volume as f32 / 100.0)?;
                    }
                } else if !self.search.active {
                    // Scroll the focused panel (3 items per scroll event)
                    let scroll_amount = 3;
                    if delta > 0 {
                        // Scroll down
                        for _ in 0..scroll_amount {
                            if self.focus == 0 {
                                self.library.select_next();
                            } else {
                                self.queue.select_next();
                            }
                        }
                    } else {
                        // Scroll up
                        for _ in 0..scroll_amount {
                            if self.focus == 0 {
                                self.library.select_previous();
                            } else {
                                self.queue.select_previous();
                            }
                        }
                    }
                }
            }

            Action::Select => {
                if self.search.active {
                    self.handle_search_select().await?;
                } else if self.focus == 0 {
                    self.handle_library_select().await?;
                } else {
                    self.handle_queue_select()?;
                }
            }

            Action::Back => {
                if self.search.active {
                    self.search.close();
                } else if self.library.view_depth > 0 {
                    self.library.go_back();
                }
            }

            Action::SwitchTab(tab) => {
                self.library.tab = tab;
                self.library.view_depth = 0;
                self.focus = 0; // Always focus library when switching tabs
                                // Reset favorites section to artists when switching to favorites
                if tab == Tab::Favorites {
                    self.library.favorites_section = 0;
                }
            }

            Action::NextTab => {
                let next = self.library.tab.next();
                self.library.tab = next;
                self.library.view_depth = 0;
                self.focus = 0;
                if next == Tab::Favorites {
                    self.library.favorites_section = 0;
                }
            }

            Action::PrevTab => {
                let prev = self.library.tab.prev();
                self.library.tab = prev;
                self.library.view_depth = 0;
                self.focus = 0;
                if prev == Tab::Favorites {
                    self.library.favorites_section = 0;
                }
            }

            // Search
            Action::OpenSearch => {
                self.search.open();
            }

            Action::CloseSearch => {
                self.search.close();
            }

            Action::SearchInput(c) => {
                self.search.input(c);
            }

            Action::SearchBackspace => {
                self.search.backspace();
            }

            Action::SearchSubmit => {
                self.perform_search().await?;
            }

            // Playback controls
            Action::PlayPause => {
                self.toggle_play_pause()?;
            }

            Action::Stop => {
                if let Some(player) = &self.player {
                    player.stop()?;
                }
                self.now_playing.clear();
            }

            Action::NextTrack => {
                self.play_next()?;
            }

            Action::PreviousTrack => {
                self.play_previous()?;
            }

            Action::SeekForward => {
                self.seek_relative(10)?;
            }

            Action::SeekBackward => {
                self.seek_relative(-10)?;
            }

            Action::SeekForwardLarge => {
                self.seek_relative(60)?;
            }

            Action::SeekBackwardLarge => {
                self.seek_relative(-60)?;
            }

            Action::VolumeUp => {
                let new_vol = (self.now_playing.volume as i32 + 5).min(100) as u8;
                self.now_playing.volume = new_vol;
                if let Some(player) = &self.player {
                    player.set_volume(new_vol as f32 / 100.0)?;
                }
            }

            Action::VolumeDown => {
                let new_vol = (self.now_playing.volume as i32 - 5).max(0) as u8;
                self.now_playing.volume = new_vol;
                if let Some(player) = &self.player {
                    player.set_volume(new_vol as f32 / 100.0)?;
                }
            }

            Action::ToggleShuffle => {
                self.now_playing.shuffle = !self.now_playing.shuffle;
                if self.now_playing.shuffle {
                    self.queue.shuffle();
                }
            }

            Action::CycleRepeat => {
                self.now_playing.repeat = self.now_playing.repeat.next();
            }

            Action::SetRepeat(mode) => {
                self.now_playing.repeat = mode;
            }

            Action::SetVolume(vol) => {
                self.now_playing.volume = vol.min(100);
                if let Some(player) = &self.player {
                    player.set_volume(vol as f32 / 100.0)?;
                }
            }

            Action::SeekTo(pos_secs) => {
                let duration = self.now_playing.duration;
                let new_pos = pos_secs.min(duration);
                self.now_playing.position = new_pos;
                if let Some(player) = &self.player {
                    player.seek(Duration::from_secs(new_pos as u64))?;
                }
            }

            // Queue management
            Action::AddToQueue(song) => {
                self.queue.add(song);
            }

            Action::AddAlbumToQueue(songs) => {
                self.queue.add_all(songs);
            }

            Action::ClearQueue => {
                self.queue.clear();
            }

            Action::RemoveSelectedFromQueue => {
                // Only remove if queue is focused
                if self.focus == 1 {
                    self.queue.remove_selected();
                }
            }

            Action::AppendToQueue => {
                self.append_selected_to_queue().await?;
            }

            Action::MoveQueueItem(_index, direction) => {
                // Use current selection instead of passed index
                if self.focus == 1 {
                    if let Some(idx) = self.queue.selected() {
                        self.move_queue_item(idx, direction);
                    }
                }
            }

            Action::RemoveFromQueue(index) => {
                self.queue.remove(index);
            }

            Action::PlayFromQueue(index) => {
                self.play_from_queue(index)?;
            }

            // Library loading
            Action::LoadArtists => {
                self.load_artists().await?;
            }

            Action::LoadAlbums => {
                self.load_albums().await?;
            }

            Action::LoadAlbum(id) => {
                self.load_album(&id).await?;
            }

            Action::LoadArtist(id) => {
                self.load_artist(&id).await?;
            }

            Action::LoadPlaylists => {
                self.load_playlists().await?;
            }

            Action::LoadPlaylist(id) => {
                self.load_playlist(&id).await?;
            }

            Action::LoadSongs => {
                self.load_songs().await?;
            }

            Action::LoadGenres => {
                self.load_genres().await?;
            }

            Action::LoadGenreAlbums(genre) => {
                self.load_genre_albums(&genre).await?;
            }

            Action::LoadFavorites => {
                self.load_favorites().await?;
            }

            Action::RefreshLibrary => {
                self.action_tx.send(Action::LoadArtists)?;
                self.action_tx.send(Action::LoadAlbums)?;
                self.action_tx.send(Action::LoadPlaylists)?;
                self.action_tx.send(Action::LoadSongs)?;
                self.action_tx.send(Action::LoadGenres)?;
                self.action_tx.send(Action::LoadFavorites)?;
            }

            // API responses (these are typically sent from async tasks)
            Action::ArtistsLoaded(artists) => {
                self.library.set_artists(artists);
                self.library.loading = false;
            }

            Action::AlbumsLoaded(albums) => {
                self.library.set_albums(albums);
                self.library.loading = false;
            }

            Action::AlbumLoaded(album, songs) => {
                self.library.enter_album(album, songs);
                self.library.loading = false;
            }

            Action::ArtistLoaded(artist, albums) => {
                self.library.enter_artist(artist, albums);
                self.library.loading = false;
            }

            Action::PlaylistsLoaded(playlists) => {
                self.library.set_playlists(playlists);
                self.library.loading = false;
            }

            Action::PlaylistLoaded(playlist, songs) => {
                self.library.enter_album(
                    crate::client::models::Album {
                        id: playlist.id,
                        name: playlist.name,
                        artist: playlist.owner,
                        artist_id: None,
                        cover_art: playlist.cover_art,
                        song_count: playlist.song_count,
                        duration: playlist.duration,
                        play_count: None,
                        created: playlist.created,
                        starred: None,
                        year: None,
                        genre: None,
                        music_brainz_id: None,
                        genres: vec![],
                        release_date: None,
                        is_compilation: None,
                        sort_name: None,
                        display_artist: None,
                    },
                    songs,
                );
                self.library.loading = false;
            }

            Action::SongsLoaded(songs) => {
                self.library.set_songs(songs);
                self.library.loading = false;
            }

            Action::GenresLoaded(genres) => {
                self.library.set_genres(genres);
                self.library.loading = false;
            }

            Action::GenreAlbumsLoaded(genre_name, albums) => {
                // Find the genre from our list to pass to enter_genre
                let genre = self
                    .library
                    .genres
                    .iter()
                    .find(|g| g.value == genre_name)
                    .cloned()
                    .unwrap_or(crate::client::models::Genre {
                        value: genre_name,
                        song_count: None,
                        album_count: None,
                    });
                self.library.enter_genre(genre, albums);
                self.library.loading = false;
            }

            Action::FavoritesLoaded {
                artists,
                albums,
                songs,
            } => {
                self.library.set_favorites(artists, albums, songs);
                self.library.loading = false;
            }

            Action::SearchResults {
                artists,
                albums,
                songs,
            } => {
                self.search.set_results(artists, albums, songs);
            }

            // Media annotation
            Action::ToggleStar => {
                self.toggle_star().await?;
            }

            Action::Scrobble => {
                self.scrobble().await?;
            }

            // Lyrics
            Action::ToggleLyrics => {
                self.lyrics.toggle();
                // Load lyrics if becoming visible and we have a current song
                if self.lyrics.visible {
                    if let Some(song) = &self.now_playing.current_song {
                        let song_id = song.id.clone();
                        // Only load if we don't already have lyrics for this song
                        if self.lyrics.song_id.as_ref() != Some(&song_id) {
                            self.lyrics.loading = true;
                            self.action_tx.send(Action::LoadLyrics(song_id))?;
                        }
                    }
                }
            }

            Action::LoadLyrics(song_id) => {
                self.load_lyrics(&song_id).await?;
            }

            Action::LyricsLoaded(song_id, lyrics) => {
                self.lyrics.set_lyrics(song_id, lyrics);
            }

            // Navigation enhancements
            Action::JumpToTop => {
                if self.focus == 0 {
                    self.library.jump_to_top();
                } else {
                    self.queue.jump_to_top();
                }
            }

            Action::JumpToBottom => {
                if self.focus == 0 {
                    self.library.jump_to_bottom();
                } else {
                    self.queue.jump_to_bottom();
                }
            }

            Action::JumpToCurrentTrack => {
                self.queue.jump_to_current();
            }

            Action::ScrollHalfPageDown => {
                // Use a default page size of 20 lines
                const PAGE_SIZE: usize = 20;
                if self.focus == 0 {
                    self.library.scroll_half_page_down(PAGE_SIZE);
                } else {
                    self.queue.scroll_half_page_down(PAGE_SIZE);
                }
            }

            Action::ScrollHalfPageUp => {
                const PAGE_SIZE: usize = 20;
                if self.focus == 0 {
                    self.library.scroll_half_page_up(PAGE_SIZE);
                } else {
                    self.queue.scroll_half_page_up(PAGE_SIZE);
                }
            }

            // Overlays
            Action::ShowHelp => {
                self.show_help = true;
            }

            Action::HideHelp => {
                self.show_help = false;
            }

            Action::ShowTrackInfo => {
                self.show_track_info = true;
            }

            Action::HideTrackInfo => {
                self.show_track_info = false;
            }

            // Album art loading
            Action::LoadAlbumArt(id) => {
                self.load_album_art(&id).await?;
            }

            Action::AlbumArtLoaded(id, data) => {
                // Only apply if it matches the current song's cover art
                if self.now_playing.album_art_id.as_deref() == Some(&id) {
                    self.now_playing.set_album_art(&data);
                }
            }

            // Player events
            Action::PlayerProgress(progress) => {
                self.now_playing.position = (progress * self.now_playing.duration as f64) as u32;
            }

            Action::PlayerStateChanged(state) => {
                self.now_playing.state = state;
            }

            Action::TrackEnded => {
                self.handle_track_ended()?;
            }

            // Errors
            Action::Error(msg) => {
                self.error_message = Some(msg);
            }

            Action::ClearError => {
                self.error_message = None;
            }

            Action::None => {}
        }

        Ok(())
    }

    /// Handle player events.
    async fn handle_player_event(&mut self, event: PlayerEvent) -> Result<()> {
        match event {
            PlayerEvent::StateChanged(state) => {
                self.now_playing.state = state;
            }
            PlayerEvent::Progress { position, duration } => {
                self.now_playing.position = position.as_secs() as u32;
                self.now_playing.duration = duration.as_secs() as u32;
            }
            PlayerEvent::TrackEnded => {
                self.handle_track_ended()?;
            }
            PlayerEvent::Error(msg) => {
                self.error_message = Some(msg);
            }
        }
        Ok(())
    }

    /// Handle track ended - play next or stop.
    fn handle_track_ended(&mut self) -> Result<()> {
        match self.now_playing.repeat {
            RepeatMode::One => {
                // Replay the same song
                if let Some(song) = self.queue.current_song().cloned() {
                    self.play_song(song)?;
                }
            }
            RepeatMode::All => {
                // Play next, loop back to beginning
                if self.queue.advance().is_none() {
                    // Reached end, go back to beginning
                    if let Some(song) = self.queue.play_index(0).cloned() {
                        self.play_song(song)?;
                    }
                } else if let Some(song) = self.queue.current_song().cloned() {
                    self.play_song(song)?;
                }
            }
            RepeatMode::Off => {
                // Play next or stop
                if let Some(song) = self.queue.advance().cloned() {
                    self.play_song(song)?;
                } else {
                    self.now_playing.state = PlayerState::Stopped;
                }
            }
        }
        Ok(())
    }

    /// Play a song.
    fn play_song(&mut self, song: Song) -> Result<()> {
        if let (Some(player), Some(client)) = (&self.player, &self.client) {
            let url = client.stream_url(&song.id);

            // Trigger album art loading if we have cover art
            if let Some(cover_art_id) = &song.cover_art {
                self.action_tx
                    .send(Action::LoadAlbumArt(cover_art_id.clone()))?;
            }

            // Load lyrics for the new song if lyrics panel is visible
            if self.lyrics.visible {
                self.lyrics.loading = true;
                self.action_tx.send(Action::LoadLyrics(song.id.clone()))?;
            } else {
                // Clear lyrics so they reload when panel becomes visible
                self.lyrics.clear();
            }

            self.now_playing.set_song(song.clone());
            player.play(url, song)?;
        }
        Ok(())
    }

    /// Toggle play/pause.
    fn toggle_play_pause(&mut self) -> Result<()> {
        if let Some(player) = &self.player {
            match self.now_playing.state {
                PlayerState::Playing => {
                    player.pause()?;
                    self.now_playing.state = PlayerState::Paused;
                }
                PlayerState::Paused => {
                    player.resume()?;
                    self.now_playing.state = PlayerState::Playing;
                }
                PlayerState::Stopped => {
                    // Start playing from queue if there's something
                    if let Some(song) = self.queue.current_song().cloned() {
                        self.play_song(song)?;
                    } else if let Some(song) = self.queue.advance().cloned() {
                        self.play_song(song)?;
                    }
                }
                PlayerState::Buffering => {}
            }
        }
        Ok(())
    }

    /// Play next track.
    fn play_next(&mut self) -> Result<()> {
        if let Some(song) = self.queue.advance().cloned() {
            self.play_song(song)?;
        }
        Ok(())
    }

    /// Play previous track.
    fn play_previous(&mut self) -> Result<()> {
        if let Some(song) = self.queue.go_back().cloned() {
            self.play_song(song)?;
        }
        Ok(())
    }

    /// Play from a specific queue index.
    fn play_from_queue(&mut self, index: usize) -> Result<()> {
        if let Some(song) = self.queue.play_index(index).cloned() {
            self.play_song(song)?;
        }
        Ok(())
    }

    /// Handle selection in the library view.
    async fn handle_library_select(&mut self) -> Result<()> {
        match self.library.tab {
            Tab::Artists => {
                if self.library.view_depth == 0 {
                    // Select artist -> load albums
                    if let Some(artist) = self.library.selected_artist_item().cloned() {
                        self.library.loading = true;
                        self.action_tx.send(Action::LoadArtist(artist.id))?;
                    }
                } else if self.library.view_depth == 1 {
                    // Select album -> load songs
                    if let Some(album) = self.library.selected_album_item().cloned() {
                        self.library.loading = true;
                        self.action_tx.send(Action::LoadAlbum(album.id))?;
                    }
                } else {
                    // Depth 2: Select song -> play
                    if let Some(_song) = self.library.selected_song_item().cloned() {
                        // Add all songs from album to queue and play selected
                        self.queue.clear();
                        self.queue.add_all(self.library.album_songs.clone());
                        if let Some(idx) = self.library.album_songs_state.selected() {
                            self.play_from_queue(idx)?;
                        }
                    }
                }
            }
            Tab::Albums => {
                if self.library.view_depth == 0 {
                    // Select album -> load songs
                    if let Some(album) = self.library.selected_album_item().cloned() {
                        self.library.loading = true;
                        self.action_tx.send(Action::LoadAlbum(album.id))?;
                    }
                } else {
                    // Select song -> play
                    if let Some(_song) = self.library.selected_song_item().cloned() {
                        // Add all songs from album to queue and play selected
                        self.queue.clear();
                        self.queue.add_all(self.library.album_songs.clone());
                        if let Some(idx) = self.library.album_songs_state.selected() {
                            self.play_from_queue(idx)?;
                        }
                    }
                }
            }
            Tab::Songs => {
                // Select song -> play
                if let Some(song) = self.library.selected_song_item().cloned() {
                    self.queue.add(song.clone());
                    let idx = self.queue.len() - 1;
                    self.play_from_queue(idx)?;
                }
            }
            Tab::Playlists => {
                if self.library.view_depth == 0 {
                    // Select playlist -> load songs
                    if let Some(playlist) = self.library.selected_playlist_item().cloned() {
                        self.library.loading = true;
                        self.action_tx.send(Action::LoadPlaylist(playlist.id))?;
                    }
                } else {
                    // Select song -> play
                    if let Some(_song) = self.library.selected_song_item().cloned() {
                        // Add all songs from playlist to queue and play selected
                        self.queue.clear();
                        self.queue.add_all(self.library.album_songs.clone());
                        if let Some(idx) = self.library.album_songs_state.selected() {
                            self.play_from_queue(idx)?;
                        }
                    }
                }
            }
            Tab::Genres => {
                if self.library.view_depth == 0 {
                    // Select genre -> load albums
                    if let Some(genre) = self.library.selected_genre_item().cloned() {
                        self.library.loading = true;
                        self.action_tx.send(Action::LoadGenreAlbums(genre.value))?;
                    }
                } else if self.library.view_depth == 1 {
                    // Select album -> load songs
                    if let Some(album) = self.library.selected_genre_album_item().cloned() {
                        self.library.loading = true;
                        self.action_tx.send(Action::LoadAlbum(album.id))?;
                    }
                } else {
                    // Depth 2: Select song -> play
                    if let Some(_song) = self.library.selected_song_item().cloned() {
                        // Add all songs from album to queue and play selected
                        self.queue.clear();
                        self.queue.add_all(self.library.album_songs.clone());
                        if let Some(idx) = self.library.album_songs_state.selected() {
                            self.play_from_queue(idx)?;
                        }
                    }
                }
            }
            Tab::Favorites => {
                if self.library.view_depth == 0 {
                    // Top-level favorites - depends on section
                    match self.library.favorites_section {
                        0 => {
                            // Select artist -> load albums
                            if let Some(artist) = self.library.selected_favorite_artist().cloned() {
                                self.library.loading = true;
                                self.action_tx.send(Action::LoadArtist(artist.id))?;
                            }
                        }
                        1 => {
                            // Select album -> load songs
                            if let Some(album) = self.library.selected_favorite_album().cloned() {
                                self.library.loading = true;
                                self.action_tx.send(Action::LoadAlbum(album.id))?;
                            }
                        }
                        _ => {
                            // Select song -> play
                            if let Some(song) = self.library.selected_favorite_song().cloned() {
                                self.queue.add(song.clone());
                                let idx = self.queue.len() - 1;
                                self.play_from_queue(idx)?;
                            }
                        }
                    }
                } else if self.library.view_depth == 1 {
                    // Select album -> load songs
                    if let Some(album) = self.library.selected_album_item().cloned() {
                        self.library.loading = true;
                        self.action_tx.send(Action::LoadAlbum(album.id))?;
                    }
                } else {
                    // Depth 2: Select song -> play
                    if let Some(_song) = self.library.selected_song_item().cloned() {
                        self.queue.clear();
                        self.queue.add_all(self.library.album_songs.clone());
                        if let Some(idx) = self.library.album_songs_state.selected() {
                            self.play_from_queue(idx)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle selection in the queue view.
    fn handle_queue_select(&mut self) -> Result<()> {
        if let Some(idx) = self.queue.selected() {
            self.play_from_queue(idx)?;
        }
        Ok(())
    }

    /// Append selected item to queue without playing.
    async fn append_selected_to_queue(&mut self) -> Result<()> {
        if self.focus == 0 {
            // Library focused
            match self.library.tab {
                Tab::Artists => {
                    if self.library.view_depth == 2 {
                        // Add single song from album
                        if let Some(song) = self.library.selected_song_item().cloned() {
                            self.queue.add(song);
                        }
                    } else if self.library.view_depth == 1 {
                        // Add all songs from selected album
                        if let Some(album) = self.library.selected_album_item().cloned() {
                            // Need to load album songs first
                            if let Some(client) = &self.client {
                                if let Ok((_album, songs)) = client.get_album(&album.id).await {
                                    self.queue.add_all(songs);
                                }
                            }
                        }
                    }
                }
                Tab::Albums => {
                    if self.library.view_depth == 0 {
                        // Add all songs from selected album
                        if let Some(album) = self.library.selected_album_item().cloned() {
                            if let Some(client) = &self.client {
                                if let Ok((_album, songs)) = client.get_album(&album.id).await {
                                    self.queue.add_all(songs);
                                }
                            }
                        }
                    } else {
                        // Add single song
                        if let Some(song) = self.library.selected_song_item().cloned() {
                            self.queue.add(song);
                        }
                    }
                }
                Tab::Songs => {
                    if let Some(song) = self.library.selected_song_item().cloned() {
                        self.queue.add(song);
                    }
                }
                Tab::Playlists => {
                    if self.library.view_depth == 0 {
                        // Add all songs from playlist
                        if let Some(playlist) = self.library.selected_playlist_item().cloned() {
                            if let Some(client) = &self.client {
                                if let Ok((_playlist, songs)) =
                                    client.get_playlist(&playlist.id).await
                                {
                                    self.queue.add_all(songs);
                                }
                            }
                        }
                    } else {
                        // Add single song
                        if let Some(song) = self.library.selected_song_item().cloned() {
                            self.queue.add(song);
                        }
                    }
                }
                Tab::Genres => {
                    if self.library.view_depth == 2 {
                        // Add single song from album
                        if let Some(song) = self.library.selected_song_item().cloned() {
                            self.queue.add(song);
                        }
                    } else if self.library.view_depth == 1 {
                        // Add all songs from selected album
                        if let Some(album) = self.library.selected_genre_album_item().cloned() {
                            if let Some(client) = &self.client {
                                if let Ok((_album, songs)) = client.get_album(&album.id).await {
                                    self.queue.add_all(songs);
                                }
                            }
                        }
                    }
                }
                Tab::Favorites => {
                    if self.library.view_depth == 0 {
                        // Top-level favorites - depends on section
                        match self.library.favorites_section {
                            0 => {
                                // Add all songs from selected artist
                                if let Some(artist) =
                                    self.library.selected_favorite_artist().cloned()
                                {
                                    if let Some(client) = &self.client {
                                        if let Ok((_artist, albums)) =
                                            client.get_artist(&artist.id).await
                                        {
                                            for album in albums {
                                                if let Ok((_album, songs)) =
                                                    client.get_album(&album.id).await
                                                {
                                                    self.queue.add_all(songs);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            1 => {
                                // Add all songs from selected album
                                if let Some(album) = self.library.selected_favorite_album().cloned()
                                {
                                    if let Some(client) = &self.client {
                                        if let Ok((_album, songs)) =
                                            client.get_album(&album.id).await
                                        {
                                            self.queue.add_all(songs);
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Add single song
                                if let Some(song) = self.library.selected_favorite_song().cloned() {
                                    self.queue.add(song);
                                }
                            }
                        }
                    } else if self.library.view_depth == 1 {
                        // Add all songs from selected album
                        if let Some(album) = self.library.selected_album_item().cloned() {
                            if let Some(client) = &self.client {
                                if let Ok((_album, songs)) = client.get_album(&album.id).await {
                                    self.queue.add_all(songs);
                                }
                            }
                        }
                    } else {
                        // Add single song from album
                        if let Some(song) = self.library.selected_song_item().cloned() {
                            self.queue.add(song);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Move a queue item up or down.
    fn move_queue_item(&mut self, index: usize, direction: isize) {
        let new_index = if direction < 0 {
            index.saturating_sub((-direction) as usize)
        } else {
            (index + direction as usize).min(self.queue.songs.len().saturating_sub(1))
        };

        if new_index != index && index < self.queue.songs.len() {
            let song = self.queue.songs.remove(index);
            self.queue.songs.insert(new_index, song);

            // Update current_index if needed
            if let Some(current) = self.queue.current_index {
                if current == index {
                    self.queue.current_index = Some(new_index);
                } else if direction < 0 && current >= new_index && current < index {
                    self.queue.current_index = Some(current + 1);
                } else if direction > 0 && current > index && current <= new_index {
                    self.queue.current_index = Some(current - 1);
                }
            }

            // Update selection to follow the moved item
            self.queue.list_state.select(Some(new_index));
        }
    }

    /// Handle selection in the search view.
    async fn handle_search_select(&mut self) -> Result<()> {
        if let Some(artist) = self.search.selected_artist().cloned() {
            self.search.close();
            self.library.tab = Tab::Artists;
            self.library.loading = true;
            self.action_tx.send(Action::LoadArtist(artist.id))?;
        } else if let Some(album) = self.search.selected_album().cloned() {
            self.search.close();
            self.library.tab = Tab::Albums;
            self.library.loading = true;
            self.action_tx.send(Action::LoadAlbum(album.id))?;
        } else if let Some(song) = self.search.selected_song().cloned() {
            self.search.close();
            self.queue.add(song.clone());
            let idx = self.queue.len() - 1;
            self.play_from_queue(idx)?;
        }
        Ok(())
    }

    /// Perform a search.
    async fn perform_search(&mut self) -> Result<()> {
        if self.search.query.is_empty() {
            self.search.clear_results();
            return Ok(());
        }

        let query = self.search.query.clone();
        self.search.mark_search_started();

        if let Some(client) = &self.client {
            match client.search(&query, Some(20), Some(20), Some(20)).await {
                Ok((artists, albums, songs)) => {
                    self.action_tx.send(Action::SearchResults {
                        artists,
                        albums,
                        songs,
                    })?;
                }
                Err(e) => {
                    self.search.searching = false;
                    self.error_message = Some(format!("Search failed: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Load artists from the server.
    async fn load_artists(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            self.library.loading = true;
            match client.get_artists().await {
                Ok(artists) => {
                    self.action_tx.send(Action::ArtistsLoaded(artists))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    tracing::error!("Failed to load artists: {}", e);
                    self.error_message = Some(format!("Failed to load artists: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load albums from the server.
    async fn load_albums(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            self.library.loading = true;
            match client.get_album_list("newest", Some(100), None).await {
                Ok(albums) => {
                    self.action_tx.send(Action::AlbumsLoaded(albums))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    tracing::error!("Failed to load albums: {}", e);
                    self.error_message = Some(format!("Failed to load albums: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load a specific album.
    async fn load_album(&mut self, id: &str) -> Result<()> {
        if let Some(client) = &self.client {
            match client.get_album(id).await {
                Ok((album, songs)) => {
                    self.action_tx.send(Action::AlbumLoaded(album, songs))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    self.error_message = Some(format!("Failed to load album: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load a specific artist.
    async fn load_artist(&mut self, id: &str) -> Result<()> {
        if let Some(client) = &self.client {
            match client.get_artist(id).await {
                Ok((artist, albums)) => {
                    self.action_tx.send(Action::ArtistLoaded(artist, albums))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    self.error_message = Some(format!("Failed to load artist: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load playlists from the server.
    async fn load_playlists(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            match client.get_playlists().await {
                Ok(playlists) => {
                    self.action_tx.send(Action::PlaylistsLoaded(playlists))?;
                }
                Err(e) => {
                    tracing::error!("Failed to load playlists: {}", e);
                    self.error_message = Some(format!("Failed to load playlists: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load a specific playlist.
    async fn load_playlist(&mut self, id: &str) -> Result<()> {
        if let Some(client) = &self.client {
            match client.get_playlist(id).await {
                Ok((playlist, songs)) => {
                    self.action_tx
                        .send(Action::PlaylistLoaded(playlist, songs))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    self.error_message = Some(format!("Failed to load playlist: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load random songs for the Songs tab.
    async fn load_songs(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            self.library.loading = true;
            match client.get_random_songs(Some(100)).await {
                Ok(songs) => {
                    self.action_tx.send(Action::SongsLoaded(songs))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    tracing::error!("Failed to load songs: {}", e);
                    self.error_message = Some(format!("Failed to load songs: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load genres from the server.
    async fn load_genres(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            self.library.loading = true;
            match client.get_genres().await {
                Ok(genres) => {
                    self.action_tx.send(Action::GenresLoaded(genres))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    tracing::error!("Failed to load genres: {}", e);
                    self.error_message = Some(format!("Failed to load genres: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load albums for a specific genre.
    async fn load_genre_albums(&mut self, genre: &str) -> Result<()> {
        if let Some(client) = &self.client {
            match client.get_albums_by_genre(genre, Some(100), None).await {
                Ok(albums) => {
                    self.action_tx
                        .send(Action::GenreAlbumsLoaded(genre.to_string(), albums))?;
                }
                Err(e) => {
                    self.library.loading = false;
                    self.error_message = Some(format!("Failed to load genre albums: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Load starred (favorite) items from the server.
    async fn load_favorites(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            self.library.loading = true;
            match client.get_starred().await {
                Ok((artists, albums, songs)) => {
                    self.action_tx.send(Action::FavoritesLoaded {
                        artists,
                        albums,
                        songs,
                    })?;
                }
                Err(e) => {
                    self.library.loading = false;
                    tracing::error!("Failed to load favorites: {}", e);
                    self.error_message = Some(format!("Failed to load favorites: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Toggle star on the current song (from now playing, library, queue, or search).
    async fn toggle_star(&mut self) -> Result<()> {
        // Determine which song to star based on context
        let song_info: Option<(String, bool)> = if self.search.active {
            // Search view - get selected song
            self.search
                .selected_song()
                .map(|s| (s.id.clone(), s.starred.is_some()))
        } else if self.focus == 1 {
            // Queue view - get selected song
            self.queue
                .selected_song()
                .map(|s| (s.id.clone(), s.starred.is_some()))
        } else if self.focus == 0 {
            // Library view - check if we're viewing songs
            match self.library.tab {
                Tab::Songs => self
                    .library
                    .selected_song_item()
                    .map(|s| (s.id.clone(), s.starred.is_some())),
                Tab::Favorites if self.library.favorites_section == 2 => self
                    .library
                    .selected_favorite_song()
                    .map(|s| (s.id.clone(), s.starred.is_some())),
                _ if self.library.view_depth > 0 => {
                    // Album/playlist song view
                    self.library
                        .album_songs_state
                        .selected()
                        .and_then(|i| self.library.album_songs.get(i))
                        .map(|s| (s.id.clone(), s.starred.is_some()))
                }
                _ => None,
            }
        } else {
            None
        };

        // Fall back to now playing if no song selected in current context
        let song_info = song_info.or_else(|| {
            self.now_playing
                .current_song
                .as_ref()
                .map(|s| (s.id.clone(), s.starred.is_some()))
        });

        if let Some((song_id, is_starred)) = song_info {
            if let Some(client) = &self.client {
                let result = if is_starred {
                    client.unstar(Some(&song_id), None, None).await
                } else {
                    client.star(Some(&song_id), None, None).await
                };

                match result {
                    Ok(()) => {
                        let new_starred = if is_starred {
                            None
                        } else {
                            Some(chrono::Utc::now().to_rfc3339())
                        };

                        // Update local state in all places where this song might appear
                        if let Some(song) = self.now_playing.current_song.as_mut() {
                            if song.id == song_id {
                                song.starred = new_starred.clone();
                            }
                        }

                        // Update in library songs
                        for song in &mut self.library.songs {
                            if song.id == song_id {
                                song.starred = new_starred.clone();
                            }
                        }

                        // Update in album_songs
                        for song in &mut self.library.album_songs {
                            if song.id == song_id {
                                song.starred = new_starred.clone();
                            }
                        }

                        // Update in queue
                        for song in &mut self.queue.songs {
                            if song.id == song_id {
                                song.starred = new_starred.clone();
                            }
                        }

                        // Update in search results
                        for song in &mut self.search.songs {
                            if song.id == song_id {
                                song.starred = new_starred.clone();
                            }
                        }

                        // Refresh favorites list to reflect the change
                        self.action_tx.send(Action::LoadFavorites)?;
                    }
                    Err(e) => {
                        let action = if is_starred { "unstar" } else { "star" };
                        self.error_message = Some(format!("Failed to {} song: {}", action, e));
                    }
                }
            }
        }
        Ok(())
    }

    /// Scrobble the current song.
    async fn scrobble(&mut self) -> Result<()> {
        if let Some(song) = self.now_playing.current_song.as_ref() {
            if let Some(client) = &self.client {
                tracing::info!("Scrobbling: {}", song.title);
                if let Err(e) = client.scrobble(&song.id, true).await {
                    tracing::error!("Failed to scrobble: {}", e);
                    // Don't show error to user for scrobble failures - it's not critical
                }
            }
        }
        Ok(())
    }

    /// Seek relative to current position (in seconds, can be negative).
    fn seek_relative(&mut self, delta_secs: i32) -> Result<()> {
        let new_pos = if delta_secs < 0 {
            self.now_playing
                .position
                .saturating_sub((-delta_secs) as u32)
        } else {
            (self.now_playing.position + delta_secs as u32).min(self.now_playing.duration)
        };

        self.now_playing.position = new_pos;

        if let Some(player) = &self.player {
            player.seek(Duration::from_secs(new_pos as u64))?;
        }

        Ok(())
    }

    /// Load album art for a cover art ID.
    async fn load_album_art(&mut self, id: &str) -> Result<()> {
        if let Some(client) = &self.client {
            let url = client.cover_art_url(id, Some(300));
            let id_owned = id.to_string();

            // Fetch in background
            match reqwest::get(&url).await {
                Ok(response) => {
                    if let Ok(bytes) = response.bytes().await {
                        self.action_tx
                            .send(Action::AlbumArtLoaded(id_owned, bytes.to_vec()))?;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load album art: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Load lyrics for a song.
    async fn load_lyrics(&mut self, song_id: &str) -> Result<()> {
        if let Some(client) = &self.client {
            let song_id_owned = song_id.to_string();
            match client.get_lyrics_by_song_id(song_id).await {
                Ok(lyrics) => {
                    self.action_tx
                        .send(Action::LyricsLoaded(song_id_owned, lyrics))?;
                }
                Err(e) => {
                    tracing::warn!("Failed to load lyrics: {}", e);
                    self.action_tx
                        .send(Action::LyricsLoaded(song_id_owned, vec![]))?;
                }
            }
        }
        Ok(())
    }
}
