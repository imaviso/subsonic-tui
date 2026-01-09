//! OpenSubsonic API client implementation.

use color_eyre::Result;
use reqwest::Client;
use thiserror::Error;

use super::auth::Auth;
use super::models::*;

/// API client errors.
#[derive(Debug, Error)]
pub enum ApiClientError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("API error: {0}")]
    Api(#[from] ApiError),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Server returned failed status: {message}")]
    ServerError { code: i32, message: String },
}

/// OpenSubsonic API client.
#[derive(Debug, Clone)]
pub struct SubsonicClient {
    /// HTTP client
    client: Client,

    /// Base server URL
    base_url: String,

    /// Authentication credentials
    auth: Auth,

    /// Client identifier
    client_name: String,

    /// API version to use
    api_version: String,

    /// Whether the server supports OpenSubsonic
    is_open_subsonic: bool,

    /// Server extensions (if OpenSubsonic)
    extensions: Vec<String>,
}

impl SubsonicClient {
    /// Create a new API client.
    pub fn new(base_url: impl Into<String>, auth: Auth) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            auth,
            client_name: String::from("subsonic-tui"),
            api_version: String::from("1.16.1"),
            is_open_subsonic: false,
            extensions: Vec::new(),
        }
    }

    /// Build the URL for an API endpoint with query parameters.
    fn build_url(&self, endpoint: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!("{}/rest/{}", self.base_url, endpoint);

        // Add common parameters
        let mut query_parts: Vec<String> = vec![
            format!("v={}", self.api_version),
            format!("c={}", self.client_name),
            String::from("f=json"),
        ];

        // Add auth parameters
        for (key, value) in self.auth.query_params() {
            query_parts.push(format!("{}={}", key, urlencoding::encode(&value)));
        }

        // Add endpoint-specific parameters
        for (key, value) in params {
            query_parts.push(format!("{}={}", key, urlencoding::encode(value)));
        }

        url.push('?');
        url.push_str(&query_parts.join("&"));
        url
    }

    /// Make a GET request to an API endpoint.
    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<T, ApiClientError> {
        let url = self.build_url(endpoint, params);

        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;

        // Parse the response
        let parsed: SubsonicResponse<T> = serde_json::from_str(&text).map_err(|e| {
            ApiClientError::InvalidResponse(format!(
                "Failed to parse response: {}. Body: {}",
                e,
                &text[..text.len().min(500)]
            ))
        })?;

        // Check for errors
        if parsed.subsonic_response.status != "ok" {
            if let Some(error) = parsed.subsonic_response.error {
                return Err(ApiClientError::ServerError {
                    code: error.code,
                    message: error
                        .message
                        .unwrap_or_else(|| String::from("Unknown error")),
                });
            }
            return Err(ApiClientError::InvalidResponse(String::from(
                "Server returned failed status without error details",
            )));
        }

        parsed
            .subsonic_response
            .data
            .ok_or_else(|| ApiClientError::InvalidResponse(String::from("Missing response data")))
    }

    /// Get the streaming URL for a song.
    pub fn stream_url(&self, id: &str) -> String {
        self.build_url("stream", &[("id", id)])
    }

    /// Get the cover art URL for an item.
    pub fn cover_art_url(&self, id: &str, size: Option<u32>) -> String {
        let size_str;
        let params: Vec<(&str, &str)> = if let Some(s) = size {
            size_str = s.to_string();
            vec![("id", id), ("size", &size_str)]
        } else {
            vec![("id", id)]
        };
        self.build_url("getCoverArt", &params)
    }

    // =========================================================================
    // System endpoints
    // =========================================================================

    /// Test connectivity with the server.
    pub async fn ping(&self) -> Result<(), ApiClientError> {
        let _: PingResponse = self.get("ping", &[]).await?;
        Ok(())
    }

    /// Check if the server supports OpenSubsonic and get extensions.
    pub async fn get_open_subsonic_extensions(
        &mut self,
    ) -> Result<Vec<OpenSubsonicExtension>, ApiClientError> {
        let response: ExtensionsResponse = self.get("getOpenSubsonicExtensions", &[]).await?;

        self.is_open_subsonic = true;
        self.extensions = response
            .open_subsonic_extensions
            .iter()
            .map(|e| e.name.clone())
            .collect();

        Ok(response.open_subsonic_extensions)
    }

    /// Check if the server supports a specific extension.
    #[allow(dead_code)]
    pub fn supports_extension(&self, name: &str) -> bool {
        self.extensions.iter().any(|e| e == name)
    }

    /// Check if connected to an OpenSubsonic server.
    #[allow(dead_code)]
    pub fn is_open_subsonic(&self) -> bool {
        self.is_open_subsonic
    }

    // =========================================================================
    // Browsing endpoints
    // =========================================================================

    /// Get all artists.
    pub async fn get_artists(&self) -> Result<Vec<Artist>, ApiClientError> {
        let response: ArtistsResponse = self.get("getArtists", &[]).await?;

        let artists: Vec<Artist> = response
            .artists
            .index
            .into_iter()
            .flat_map(|idx| idx.artist)
            .collect();

        Ok(artists)
    }

    /// Get an artist by ID.
    pub async fn get_artist(&self, id: &str) -> Result<(Artist, Vec<Album>), ApiClientError> {
        let response: ArtistResponse = self.get("getArtist", &[("id", id)]).await?;

        Ok((response.artist.artist, response.artist.album))
    }

    /// Get an album by ID.
    pub async fn get_album(&self, id: &str) -> Result<(Album, Vec<Song>), ApiClientError> {
        let response: AlbumResponse = self.get("getAlbum", &[("id", id)]).await?;

        Ok((response.album.album, response.album.song))
    }

    /// Get album list.
    pub async fn get_album_list(
        &self,
        list_type: &str,
        size: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Album>, ApiClientError> {
        let size_str = size.unwrap_or(50).to_string();
        let offset_str = offset.unwrap_or(0).to_string();

        let response: AlbumListResponse = self
            .get(
                "getAlbumList2",
                &[
                    ("type", list_type),
                    ("size", &size_str),
                    ("offset", &offset_str),
                ],
            )
            .await?;

        Ok(response.album_list2.album)
    }

    /// Get random songs.
    pub async fn get_random_songs(&self, size: Option<u32>) -> Result<Vec<Song>, ApiClientError> {
        let size_str = size.unwrap_or(50).to_string();

        let response: RandomSongsResponse =
            self.get("getRandomSongs", &[("size", &size_str)]).await?;

        Ok(response.random_songs.song)
    }

    /// Get all genres.
    pub async fn get_genres(&self) -> Result<Vec<Genre>, ApiClientError> {
        let response: GenresResponse = self.get("getGenres", &[]).await?;
        Ok(response.genres.genre)
    }

    /// Get albums by genre.
    pub async fn get_albums_by_genre(
        &self,
        genre: &str,
        size: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Album>, ApiClientError> {
        let size_str = size.unwrap_or(100).to_string();
        let offset_str = offset.unwrap_or(0).to_string();

        let response: AlbumListResponse = self
            .get(
                "getAlbumList2",
                &[
                    ("type", "byGenre"),
                    ("genre", genre),
                    ("size", &size_str),
                    ("offset", &offset_str),
                ],
            )
            .await?;

        Ok(response.album_list2.album)
    }

    /// Get starred (favorite) items.
    pub async fn get_starred(
        &self,
    ) -> Result<(Vec<Artist>, Vec<Album>, Vec<Song>), ApiClientError> {
        let response: StarredResponse = self.get("getStarred2", &[]).await?;

        Ok((
            response.starred2.artist,
            response.starred2.album,
            response.starred2.song,
        ))
    }

    // =========================================================================
    // Playlist endpoints
    // =========================================================================

    /// Get all playlists.
    pub async fn get_playlists(&self) -> Result<Vec<Playlist>, ApiClientError> {
        let response: PlaylistsResponse = self.get("getPlaylists", &[]).await?;
        Ok(response.playlists.playlist)
    }

    /// Get a playlist by ID.
    pub async fn get_playlist(&self, id: &str) -> Result<(Playlist, Vec<Song>), ApiClientError> {
        let response: PlaylistResponse = self.get("getPlaylist", &[("id", id)]).await?;
        Ok((response.playlist.playlist, response.playlist.entry))
    }

    // =========================================================================
    // Search endpoints
    // =========================================================================

    /// Search for artists, albums, and songs.
    pub async fn search(
        &self,
        query: &str,
        artist_count: Option<u32>,
        album_count: Option<u32>,
        song_count: Option<u32>,
    ) -> Result<(Vec<Artist>, Vec<Album>, Vec<Song>), ApiClientError> {
        let artist_count_str = artist_count.unwrap_or(20).to_string();
        let album_count_str = album_count.unwrap_or(20).to_string();
        let song_count_str = song_count.unwrap_or(20).to_string();

        let response: SearchResponse = self
            .get(
                "search3",
                &[
                    ("query", query),
                    ("artistCount", &artist_count_str),
                    ("albumCount", &album_count_str),
                    ("songCount", &song_count_str),
                ],
            )
            .await?;

        Ok((
            response.search_result3.artist,
            response.search_result3.album,
            response.search_result3.song,
        ))
    }

    // =========================================================================
    // Media annotation endpoints
    // =========================================================================

    /// Star an item.
    pub async fn star(
        &self,
        id: Option<&str>,
        album_id: Option<&str>,
        artist_id: Option<&str>,
    ) -> Result<(), ApiClientError> {
        let mut params = Vec::new();
        if let Some(id) = id {
            params.push(("id", id));
        }
        if let Some(album_id) = album_id {
            params.push(("albumId", album_id));
        }
        if let Some(artist_id) = artist_id {
            params.push(("artistId", artist_id));
        }

        let _: PingResponse = self.get("star", &params).await?;
        Ok(())
    }

    /// Unstar an item.
    pub async fn unstar(
        &self,
        id: Option<&str>,
        album_id: Option<&str>,
        artist_id: Option<&str>,
    ) -> Result<(), ApiClientError> {
        let mut params = Vec::new();
        if let Some(id) = id {
            params.push(("id", id));
        }
        if let Some(album_id) = album_id {
            params.push(("albumId", album_id));
        }
        if let Some(artist_id) = artist_id {
            params.push(("artistId", artist_id));
        }

        let _: PingResponse = self.get("unstar", &params).await?;
        Ok(())
    }

    /// Scrobble a song (report playback).
    pub async fn scrobble(&self, id: &str, submission: bool) -> Result<(), ApiClientError> {
        let submission_str = submission.to_string();
        let _: PingResponse = self
            .get("scrobble", &[("id", id), ("submission", &submission_str)])
            .await?;
        Ok(())
    }

    // =========================================================================
    // Lyrics endpoints (OpenSubsonic)
    // =========================================================================

    /// Get lyrics for a song by ID (OpenSubsonic extension).
    pub async fn get_lyrics_by_song_id(
        &self,
        id: &str,
    ) -> Result<Vec<StructuredLyrics>, ApiClientError> {
        let response: LyricsResponse = self.get("getLyricsBySongId", &[("id", id)]).await?;
        Ok(response.lyrics_list.structured_lyrics)
    }
}
