//! MPRIS D-Bus integration for system media controls.
//!
//! This module provides MPRIS support allowing the player to be controlled
//! by system media keys, desktop widgets, and tools like playerctl.
//!
//! The MPRIS server runs on a dedicated thread with a single-threaded runtime
//! because mpris_server::Player is !Send + !Sync.

use std::thread;
use std::time::Duration;

use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time};
use tokio::sync::mpsc;

/// MPRIS event sent from the MPRIS server to the app.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants reserved for future use
pub enum MprisEvent {
    PlayPause,
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek(i64),        // Offset in microseconds
    SetPosition(u64), // Absolute position in microseconds
    SetVolume(f64),   // 0.0 to 1.0
    SetLoopStatus(LoopStatus),
    SetShuffle(bool),
    Raise,
    Quit,
}

/// Commands sent from the app to the MPRIS server.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants reserved for future use
pub enum MprisCommand {
    SetPlaybackStatus(PlaybackStatus),
    SetMetadata {
        track_id: String,
        title: String,
        artist: Option<String>,
        album: Option<String>,
        duration: Option<u32>,
        cover_art_url: Option<String>,
    },
    SetPosition(Duration),
    SetVolume(f64),
    SetLoopStatus(LoopStatus),
    SetShuffle(bool),
    Seeked(Duration),
    SetCanGoNext(bool),
    SetCanGoPrevious(bool),
    Shutdown,
}

/// MPRIS server handle for communication with the MPRIS thread.
#[allow(dead_code)] // Some methods reserved for future use
pub struct MprisHandle {
    event_rx: mpsc::UnboundedReceiver<MprisEvent>,
    command_tx: mpsc::UnboundedSender<MprisCommand>,
    _thread_handle: thread::JoinHandle<()>,
}

#[allow(dead_code)] // Some methods reserved for future use
impl MprisHandle {
    /// Create a new MPRIS server running on a dedicated thread.
    pub fn new() -> Result<Self, String> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let thread_handle = thread::Builder::new()
            .name("mpris-server".to_string())
            .spawn(move || {
                run_mpris_thread(event_tx, command_rx);
            })
            .map_err(|e| format!("Failed to spawn MPRIS thread: {}", e))?;

        Ok(Self {
            event_rx,
            command_tx,
            _thread_handle: thread_handle,
        })
    }

    /// Try to receive an MPRIS event (non-blocking).
    pub fn try_recv(&mut self) -> Option<MprisEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Send a command to the MPRIS server.
    pub fn send(&self, command: MprisCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send MPRIS command: {}", e))
    }

    /// Update playback status.
    pub fn set_playback_status(&self, status: PlaybackStatus) -> Result<(), String> {
        self.send(MprisCommand::SetPlaybackStatus(status))
    }

    /// Update metadata for current track.
    pub fn set_metadata(
        &self,
        track_id: &str,
        title: &str,
        artist: Option<&str>,
        album: Option<&str>,
        duration: Option<u32>,
        cover_art_url: Option<&str>,
    ) -> Result<(), String> {
        self.send(MprisCommand::SetMetadata {
            track_id: track_id.to_string(),
            title: title.to_string(),
            artist: artist.map(String::from),
            album: album.map(String::from),
            duration,
            cover_art_url: cover_art_url.map(String::from),
        })
    }

    /// Update current position.
    pub fn set_position(&self, position: Duration) -> Result<(), String> {
        self.send(MprisCommand::SetPosition(position))
    }

    /// Update volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        self.send(MprisCommand::SetVolume(volume))
    }

    /// Update loop status.
    pub fn set_loop_status(&self, status: LoopStatus) -> Result<(), String> {
        self.send(MprisCommand::SetLoopStatus(status))
    }

    /// Update shuffle status.
    pub fn set_shuffle(&self, shuffle: bool) -> Result<(), String> {
        self.send(MprisCommand::SetShuffle(shuffle))
    }

    /// Emit seeked signal.
    pub fn seeked(&self, position: Duration) -> Result<(), String> {
        self.send(MprisCommand::Seeked(position))
    }

    /// Set whether next track is available.
    pub fn set_can_go_next(&self, can: bool) -> Result<(), String> {
        self.send(MprisCommand::SetCanGoNext(can))
    }

    /// Set whether previous track is available.
    pub fn set_can_go_previous(&self, can: bool) -> Result<(), String> {
        self.send(MprisCommand::SetCanGoPrevious(can))
    }

    /// Shutdown the MPRIS server.
    pub fn shutdown(&self) -> Result<(), String> {
        self.send(MprisCommand::Shutdown)
    }
}

impl Drop for MprisHandle {
    fn drop(&mut self) {
        let _ = self.command_tx.send(MprisCommand::Shutdown);
    }
}

/// Run the MPRIS server on a dedicated single-threaded runtime.
fn run_mpris_thread(
    event_tx: mpsc::UnboundedSender<MprisEvent>,
    mut command_rx: mpsc::UnboundedReceiver<MprisCommand>,
) {
    // Create a single-threaded runtime for this thread
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("Failed to create MPRIS runtime: {}", e);
            return;
        }
    };

    let local = tokio::task::LocalSet::new();

    local.block_on(&rt, async move {
        // Build the MPRIS player
        let player = match Player::builder("subsonic_tui")
            .identity("Subsonic TUI")
            .desktop_entry("subsonic-tui")
            .can_play(true)
            .can_pause(true)
            .can_go_next(true)
            .can_go_previous(true)
            .can_seek(true)
            .can_control(true)
            .can_quit(true)
            .can_raise(false)
            .build()
            .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to build MPRIS player: {}", e);
                return;
            }
        };

        // Set up event handlers
        let tx = event_tx.clone();
        player.connect_play_pause(move |_| {
            let _ = tx.send(MprisEvent::PlayPause);
        });

        let tx = event_tx.clone();
        player.connect_play(move |_| {
            let _ = tx.send(MprisEvent::Play);
        });

        let tx = event_tx.clone();
        player.connect_pause(move |_| {
            let _ = tx.send(MprisEvent::Pause);
        });

        let tx = event_tx.clone();
        player.connect_stop(move |_| {
            let _ = tx.send(MprisEvent::Stop);
        });

        let tx = event_tx.clone();
        player.connect_next(move |_| {
            let _ = tx.send(MprisEvent::Next);
        });

        let tx = event_tx.clone();
        player.connect_previous(move |_| {
            let _ = tx.send(MprisEvent::Previous);
        });

        let tx = event_tx.clone();
        player.connect_seek(move |_, offset| {
            let _ = tx.send(MprisEvent::Seek(offset.as_micros()));
        });

        let tx = event_tx.clone();
        player.connect_set_position(move |_, _track_id, position| {
            let _ = tx.send(MprisEvent::SetPosition(position.as_micros() as u64));
        });

        let tx = event_tx.clone();
        player.connect_set_volume(move |_, volume| {
            let _ = tx.send(MprisEvent::SetVolume(volume));
        });

        let tx = event_tx.clone();
        player.connect_set_loop_status(move |_, status| {
            let _ = tx.send(MprisEvent::SetLoopStatus(status));
        });

        let tx = event_tx.clone();
        player.connect_set_shuffle(move |_, shuffle| {
            let _ = tx.send(MprisEvent::SetShuffle(shuffle));
        });

        player.connect_raise(move |_| {
            // We don't support raise
        });

        let tx = event_tx.clone();
        player.connect_quit(move |_| {
            let _ = tx.send(MprisEvent::Quit);
        });

        // Spawn the player run task locally
        let player_run = player.run();
        tokio::task::spawn_local(async move {
            player_run.await;
        });

        tracing::info!("MPRIS server started");

        // Process commands
        loop {
            tokio::select! {
                cmd = command_rx.recv() => {
                    match cmd {
                        Some(MprisCommand::SetPlaybackStatus(status)) => {
                            if let Err(e) = player.set_playback_status(status).await {
                                tracing::warn!("Failed to set playback status: {}", e);
                            }
                        }
                        Some(MprisCommand::SetMetadata { track_id, title, artist, album, duration, cover_art_url }) => {
                            let metadata = build_metadata(
                                &track_id,
                                &title,
                                artist.as_deref(),
                                album.as_deref(),
                                duration,
                                cover_art_url.as_deref(),
                            );
                            if let Err(e) = player.set_metadata(metadata).await {
                                tracing::warn!("Failed to set metadata: {}", e);
                            }
                        }
                        Some(MprisCommand::SetPosition(pos)) => {
                            player.set_position(Time::from_micros(pos.as_micros() as i64));
                        }
                        Some(MprisCommand::SetVolume(vol)) => {
                            if let Err(e) = player.set_volume(vol).await {
                                tracing::warn!("Failed to set volume: {}", e);
                            }
                        }
                        Some(MprisCommand::SetLoopStatus(status)) => {
                            if let Err(e) = player.set_loop_status(status).await {
                                tracing::warn!("Failed to set loop status: {}", e);
                            }
                        }
                        Some(MprisCommand::SetShuffle(shuffle)) => {
                            if let Err(e) = player.set_shuffle(shuffle).await {
                                tracing::warn!("Failed to set shuffle: {}", e);
                            }
                        }
                        Some(MprisCommand::Seeked(pos)) => {
                            if let Err(e) = player.seeked(Time::from_micros(pos.as_micros() as i64)).await {
                                tracing::warn!("Failed to emit seeked signal: {}", e);
                            }
                        }
                        Some(MprisCommand::SetCanGoNext(can)) => {
                            if let Err(e) = player.set_can_go_next(can).await {
                                tracing::warn!("Failed to set can_go_next: {}", e);
                            }
                        }
                        Some(MprisCommand::SetCanGoPrevious(can)) => {
                            if let Err(e) = player.set_can_go_previous(can).await {
                                tracing::warn!("Failed to set can_go_previous: {}", e);
                            }
                        }
                        Some(MprisCommand::Shutdown) | None => {
                            tracing::info!("MPRIS server shutting down");
                            break;
                        }
                    }
                }
                // Yield to allow the player to process D-Bus messages
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
    });
}

/// Helper to build metadata from song information.
fn build_metadata(
    track_id: &str,
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
    duration: Option<u32>,
    cover_art_url: Option<&str>,
) -> Metadata {
    let mut builder = Metadata::builder()
        .trackid(
            mpris_server::TrackId::try_from(format!("/org/subsonic_tui/track/{}", track_id))
                .unwrap_or(mpris_server::TrackId::NO_TRACK),
        )
        .title(title);

    if let Some(artist) = artist {
        builder = builder.artist([artist]);
    }

    if let Some(album) = album {
        builder = builder.album(album);
    }

    if let Some(duration) = duration {
        builder = builder.length(Time::from_secs(duration as i64));
    }

    if let Some(url) = cover_art_url {
        builder = builder.art_url(url);
    }

    builder.build()
}

/// Convert MPRIS LoopStatus to our RepeatMode.
pub fn loop_status_to_repeat(status: LoopStatus) -> crate::action::RepeatMode {
    match status {
        LoopStatus::None => crate::action::RepeatMode::Off,
        LoopStatus::Track => crate::action::RepeatMode::One,
        LoopStatus::Playlist => crate::action::RepeatMode::All,
    }
}

/// Convert our RepeatMode to MPRIS LoopStatus.
pub fn repeat_to_loop_status(mode: crate::action::RepeatMode) -> LoopStatus {
    match mode {
        crate::action::RepeatMode::Off => LoopStatus::None,
        crate::action::RepeatMode::One => LoopStatus::Track,
        crate::action::RepeatMode::All => LoopStatus::Playlist,
    }
}
