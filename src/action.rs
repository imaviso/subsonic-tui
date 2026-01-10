//! Application actions/events that drive state changes.

use crate::client::models::{Album, Artist, Genre, Playlist, Song, StructuredLyrics};

/// Actions that can be dispatched to update application state.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
#[allow(clippy::large_enum_variant)]
pub enum Action {
    // Application lifecycle
    Quit,
    Tick,
    Render,
    Resize(u16, u16),

    // Navigation
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    Select,
    Back,
    SwitchTab(Tab),
    NextTab,
    PrevTab,

    // Mouse
    MouseClick(u16, u16),
    MouseScroll(i16), // positive = down, negative = up

    // Search
    OpenSearch,
    CloseSearch,
    SearchInput(char),
    SearchBackspace,
    SearchSubmit,

    // Playback controls
    PlayPause,
    Stop,
    NextTrack,
    PreviousTrack,
    SeekForward,
    SeekBackward,
    SeekForwardLarge,
    SeekBackwardLarge,
    SeekTo(u32), // Seek to absolute position in seconds
    VolumeUp,
    VolumeDown,
    SetVolume(u8), // Set volume to specific value (0-100)
    ToggleShuffle,
    CycleRepeat,
    SetRepeat(RepeatMode), // Set specific repeat mode

    // Queue management
    AddToQueue(Song),
    AddAlbumToQueue(Vec<Song>),
    AppendToQueue, // Add selected item to queue without playing
    ClearQueue,
    RemoveFromQueue(usize),
    RemoveSelectedFromQueue, // Remove currently selected item from queue
    PlayFromQueue(usize),
    MoveQueueItem(usize, isize), // Move item up (-1) or down (+1)

    // Library actions
    LoadArtists,
    LoadAlbums,
    LoadAlbum(String),
    LoadArtist(String),
    LoadPlaylists,
    LoadPlaylist(String),
    LoadSongs,
    LoadGenres,
    LoadGenreAlbums(String),
    LoadFavorites,
    RefreshLibrary,

    // API responses
    ArtistsLoaded(Vec<Artist>),
    AlbumsLoaded(Vec<Album>),
    AlbumLoaded(Album, Vec<Song>),
    ArtistLoaded(Artist, Vec<Album>),
    PlaylistsLoaded(Vec<Playlist>),
    PlaylistLoaded(Playlist, Vec<Song>),
    SongsLoaded(Vec<Song>),
    GenresLoaded(Vec<Genre>),
    GenreAlbumsLoaded(String, Vec<Album>),
    FavoritesLoaded {
        artists: Vec<Artist>,
        albums: Vec<Album>,
        songs: Vec<Song>,
    },
    SearchResults {
        artists: Vec<Artist>,
        albums: Vec<Album>,
        songs: Vec<Song>,
    },

    // Media annotation
    ToggleStar,
    Scrobble,

    // Lyrics
    ToggleLyrics,
    LoadLyrics(String),
    LyricsLoaded(String, Vec<StructuredLyrics>),

    // Navigation enhancements
    JumpToTop,
    JumpToBottom,
    JumpToCurrentTrack,
    ScrollHalfPageDown,
    ScrollHalfPageUp,

    // Overlays
    ShowHelp,
    HideHelp,
    ShowTrackInfo,
    HideTrackInfo,

    // Album art
    LoadAlbumArt(String),
    AlbumArtLoaded(String, Vec<u8>),

    // Player state updates
    PlayerProgress(f64),
    PlayerStateChanged(PlayerState),
    TrackEnded,

    // Errors
    Error(String),
    ClearError,

    // No-op
    None,
}

/// Current playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum PlayerState {
    #[default]
    Stopped,
    Playing,
    Paused,
    Buffering,
}

/// Repeat mode for playback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Off => "  ",
            Self::All => "󰑖 ",
            Self::One => "󰑘 ",
        }
    }
}

/// Application tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Artists,
    Albums,
    Songs,
    Playlists,
    Genres,
    Favorites,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Artists,
            Tab::Albums,
            Tab::Songs,
            Tab::Playlists,
            Tab::Genres,
            Tab::Favorites,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Artists => "Artists",
            Self::Albums => "Albums",
            Self::Songs => "Songs",
            Self::Playlists => "Playlists",
            Self::Genres => "Genres",
            Self::Favorites => "Favorites",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Self::Artists => 0,
            Self::Albums => 1,
            Self::Songs => 2,
            Self::Playlists => 3,
            Self::Genres => 4,
            Self::Favorites => 5,
        }
    }

    /// Get the next tab (wraps around).
    pub fn next(&self) -> Tab {
        match self {
            Self::Artists => Self::Albums,
            Self::Albums => Self::Songs,
            Self::Songs => Self::Playlists,
            Self::Playlists => Self::Genres,
            Self::Genres => Self::Favorites,
            Self::Favorites => Self::Artists,
        }
    }

    /// Get the previous tab (wraps around).
    pub fn prev(&self) -> Tab {
        match self {
            Self::Artists => Self::Favorites,
            Self::Albums => Self::Artists,
            Self::Songs => Self::Albums,
            Self::Playlists => Self::Songs,
            Self::Genres => Self::Playlists,
            Self::Favorites => Self::Genres,
        }
    }
}
