//! OpenSubsonic API response models.

use serde::{Deserialize, Serialize};

/// Root response wrapper for all API responses.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponse<T> {
    #[serde(rename = "subsonic-response")]
    pub subsonic_response: ResponseBody<T>,
}

/// Response body containing status and data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ResponseBody<T> {
    pub status: String,
    pub version: String,
    #[serde(rename = "type")]
    pub server_type: Option<String>,
    pub server_version: Option<String>,
    pub open_subsonic: Option<bool>,
    pub error: Option<ApiError>,
    #[serde(flatten)]
    pub data: Option<T>,
}

/// API error response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: i32,
    pub message: Option<String>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "API Error {}: {}",
            self.code,
            self.message.as_deref().unwrap_or("Unknown error")
        )
    }
}

impl std::error::Error for ApiError {}

// ============================================================================
// Artists
// ============================================================================

/// Response for getArtists endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistsResponse {
    pub artists: ArtistsData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ArtistsData {
    #[serde(default)]
    pub index: Vec<ArtistIndex>,
    pub ignored_articles: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ArtistIndex {
    pub name: String,
    #[serde(default)]
    pub artist: Vec<Artist>,
}

/// Artist (ID3 tag-based).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub album_count: Option<i32>,
    pub starred: Option<String>,
    pub music_brainz_id: Option<String>,
    pub sort_name: Option<String>,
}

/// Response for getArtist endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistResponse {
    pub artist: ArtistWithAlbums,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistWithAlbums {
    #[serde(flatten)]
    pub artist: Artist,
    #[serde(default)]
    pub album: Vec<Album>,
}

// ============================================================================
// Albums
// ============================================================================

/// Response for getAlbumList2 endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumListResponse {
    pub album_list2: AlbumListData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumListData {
    #[serde(default)]
    pub album: Vec<Album>,
}

/// Album (ID3 tag-based).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub artist: Option<String>,
    pub artist_id: Option<String>,
    pub cover_art: Option<String>,
    pub song_count: Option<i32>,
    pub duration: Option<i32>,
    pub play_count: Option<i64>,
    pub created: Option<String>,
    pub starred: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub music_brainz_id: Option<String>,
    #[serde(default)]
    pub genres: Vec<ItemGenre>,
    pub release_date: Option<ItemDate>,
    pub is_compilation: Option<bool>,
    pub sort_name: Option<String>,
    pub display_artist: Option<String>,
}

/// Response for getAlbum endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumResponse {
    pub album: AlbumWithSongs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumWithSongs {
    #[serde(flatten)]
    pub album: Album,
    #[serde(default)]
    pub song: Vec<Song>,
}

// ============================================================================
// Songs
// ============================================================================

/// Song/track (Child in OpenSubsonic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub parent: Option<String>,
    pub is_dir: Option<bool>,
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub track: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub size: Option<i64>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    pub transcoded_content_type: Option<String>,
    pub transcoded_suffix: Option<String>,
    pub duration: Option<i32>,
    pub bit_rate: Option<i32>,
    pub path: Option<String>,
    pub is_video: Option<bool>,
    pub user_rating: Option<i32>,
    pub average_rating: Option<f64>,
    pub play_count: Option<i64>,
    pub disc_number: Option<i32>,
    pub created: Option<String>,
    pub starred: Option<String>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub media_file_id: Option<String>,
    pub bpm: Option<i32>,
    pub comment: Option<String>,
    pub sort_name: Option<String>,
    pub music_brainz_id: Option<String>,
    #[serde(default)]
    pub genres: Vec<ItemGenre>,
    pub replay_gain: Option<ReplayGain>,
    pub channel_count: Option<i32>,
    pub sampling_rate: Option<i32>,
    pub bit_depth: Option<i32>,
}

impl Song {
    /// Get a display-friendly duration string (e.g., "3:45").
    pub fn duration_string(&self) -> String {
        match self.duration {
            Some(secs) => {
                let mins = secs / 60;
                let secs = secs % 60;
                format!("{mins}:{secs:02}")
            }
            None => String::from("--:--"),
        }
    }

    /// Get display artist, falling back to "Unknown Artist".
    pub fn display_artist(&self) -> &str {
        self.artist.as_deref().unwrap_or("Unknown Artist")
    }

    /// Get display album, falling back to "Unknown Album".
    pub fn display_album(&self) -> &str {
        self.album.as_deref().unwrap_or("Unknown Album")
    }
}

// ============================================================================
// Playlists
// ============================================================================

/// Response for getPlaylists endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistsResponse {
    pub playlists: PlaylistsData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistsData {
    #[serde(default)]
    pub playlist: Vec<Playlist>,
}

/// Playlist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub comment: Option<String>,
    pub owner: Option<String>,
    pub public: Option<bool>,
    pub song_count: Option<i32>,
    pub duration: Option<i32>,
    pub created: Option<String>,
    pub changed: Option<String>,
    pub cover_art: Option<String>,
}

/// Response for getPlaylist endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistResponse {
    pub playlist: PlaylistWithSongs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistWithSongs {
    #[serde(flatten)]
    pub playlist: Playlist,
    #[serde(default)]
    pub entry: Vec<Song>,
}

// ============================================================================
// Search
// ============================================================================

/// Response for search3 endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub search_result3: SearchResult,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    #[serde(default)]
    pub artist: Vec<Artist>,
    #[serde(default)]
    pub album: Vec<Album>,
    #[serde(default)]
    pub song: Vec<Song>,
}

// ============================================================================
// System
// ============================================================================

/// Response for ping endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {}

/// Response for getOpenSubsonicExtensions endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsResponse {
    pub open_subsonic_extensions: Vec<OpenSubsonicExtension>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct OpenSubsonicExtension {
    pub name: String,
    pub versions: Vec<i32>,
}

// ============================================================================
// Common Types
// ============================================================================

/// Genre item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemGenre {
    pub name: String,
}

/// Date item (OpenSubsonic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDate {
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
}

/// Replay gain information (OpenSubsonic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayGain {
    pub track_gain: Option<f64>,
    pub album_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_peak: Option<f64>,
    pub base_gain: Option<f64>,
}

// ============================================================================
// Genres
// ============================================================================

/// Response for getGenres endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenresResponse {
    pub genres: GenresData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenresData {
    #[serde(default)]
    pub genre: Vec<Genre>,
}

/// Genre with counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub value: String,
    pub song_count: Option<i32>,
    pub album_count: Option<i32>,
}

// ============================================================================
// Random Songs
// ============================================================================

/// Response for getRandomSongs endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomSongsResponse {
    pub random_songs: RandomSongsData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomSongsData {
    #[serde(default)]
    pub song: Vec<Song>,
}

// ============================================================================
// Starred (Favorites)
// ============================================================================

/// Response for getStarred2 endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarredResponse {
    pub starred2: StarredData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarredData {
    #[serde(default)]
    pub artist: Vec<Artist>,
    #[serde(default)]
    pub album: Vec<Album>,
    #[serde(default)]
    pub song: Vec<Song>,
}

// ============================================================================
// Lyrics (OpenSubsonic)
// ============================================================================

/// Response for getLyricsBySongId endpoint (OpenSubsonic).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResponse {
    pub lyrics_list: LyricsList,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsList {
    #[serde(default)]
    pub structured_lyrics: Vec<StructuredLyrics>,
}

/// Structured lyrics with optional sync.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredLyrics {
    /// Display name for the lyrics source
    pub display_artist: Option<String>,
    pub display_title: Option<String>,
    /// Language code (e.g., "eng", "jpn")
    pub lang: String,
    /// Whether the lyrics are synced (have timestamps)
    pub synced: bool,
    /// Offset in milliseconds to apply to all timestamps
    #[serde(default)]
    pub offset: i64,
    /// The lyric lines
    #[serde(default)]
    pub line: Vec<LyricLine>,
}

/// A single line of lyrics.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    /// Start time in milliseconds (only for synced lyrics)
    pub start: Option<i64>,
    /// The lyric text
    pub value: String,
}
