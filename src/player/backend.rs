//! Audio playback backend using rodio.

use std::io::{BufReader, Cursor};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use color_eyre::Result;
use rodio::{Decoder, OutputStream, Sink, Source};
use tokio::sync::mpsc;

use crate::action::PlayerState;
use crate::client::models::Song;

/// Messages sent to the player thread.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum PlayerCommand {
    Play(String, Song),
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    Seek(Duration),
}

/// Messages sent from the player thread.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    StateChanged(PlayerState),
    Progress {
        position: Duration,
        duration: Duration,
    },
    TrackEnded,
    Error(String),
}

/// Audio player that runs in a separate thread.
pub struct Player {
    command_tx: mpsc::UnboundedSender<PlayerCommand>,
    event_rx: mpsc::UnboundedReceiver<PlayerEvent>,
    state: Arc<PlayerStateShared>,
}

/// Shared player state accessible from multiple threads.
struct PlayerStateShared {
    is_playing: AtomicBool,
    position_ms: AtomicU64,
    duration_ms: AtomicU64,
    volume: AtomicU64,
}

impl Player {
    /// Create a new audio player.
    pub fn new() -> Result<Self> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let state = Arc::new(PlayerStateShared {
            is_playing: AtomicBool::new(false),
            position_ms: AtomicU64::new(0),
            duration_ms: AtomicU64::new(0),
            volume: AtomicU64::new(80),
        });

        let state_clone = Arc::clone(&state);

        // Spawn the player thread
        std::thread::spawn(move || {
            if let Err(e) = run_player_thread(command_rx, event_tx, state_clone) {
                tracing::error!("Player thread error: {}", e);
            }
        });

        Ok(Self {
            command_tx,
            event_rx,
            state,
        })
    }

    /// Play a song from a URL.
    pub fn play(&self, url: String, song: Song) -> Result<()> {
        self.command_tx.send(PlayerCommand::Play(url, song))?;
        Ok(())
    }

    /// Pause playback.
    pub fn pause(&self) -> Result<()> {
        self.command_tx.send(PlayerCommand::Pause)?;
        Ok(())
    }

    /// Resume playback.
    pub fn resume(&self) -> Result<()> {
        self.command_tx.send(PlayerCommand::Resume)?;
        Ok(())
    }

    /// Stop playback.
    pub fn stop(&self) -> Result<()> {
        self.command_tx.send(PlayerCommand::Stop)?;
        Ok(())
    }

    /// Set volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.command_tx.send(PlayerCommand::SetVolume(volume))?;
        self.state
            .volume
            .store((volume * 100.0) as u64, Ordering::SeqCst);
        Ok(())
    }

    /// Seek to a position.
    pub fn seek(&self, position: Duration) -> Result<()> {
        self.command_tx.send(PlayerCommand::Seek(position))?;
        Ok(())
    }

    /// Try to receive a player event (non-blocking).
    pub fn try_recv_event(&mut self) -> Option<PlayerEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get the current volume (0-100).
    #[allow(dead_code)]
    pub fn volume(&self) -> u8 {
        self.state.volume.load(Ordering::SeqCst) as u8
    }

    /// Check if currently playing.
    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        self.state.is_playing.load(Ordering::SeqCst)
    }

    /// Get current position in milliseconds.
    #[allow(dead_code)]
    pub fn position_ms(&self) -> u64 {
        self.state.position_ms.load(Ordering::SeqCst)
    }

    /// Get track duration in milliseconds.
    #[allow(dead_code)]
    pub fn duration_ms(&self) -> u64 {
        self.state.duration_ms.load(Ordering::SeqCst)
    }
}

/// Run the player thread.
fn run_player_thread(
    mut command_rx: mpsc::UnboundedReceiver<PlayerCommand>,
    event_tx: mpsc::UnboundedSender<PlayerEvent>,
    state: Arc<PlayerStateShared>,
) -> Result<()> {
    // Initialize audio output
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle)?));

    let mut current_duration: Option<Duration> = None;
    let mut current_audio_data: Option<Vec<u8>> = None;
    let mut current_volume: f32 = 0.8;

    loop {
        // Check for commands (non-blocking)
        match command_rx.try_recv() {
            Ok(cmd) => match cmd {
                PlayerCommand::Play(url, song) => {
                    // Stop current playback
                    {
                        let s = sink.lock().unwrap();
                        s.stop();
                    }
                    // Create new sink after stop
                    *sink.lock().unwrap() = Sink::try_new(&stream_handle)?;

                    // Get duration from song metadata
                    current_duration = song.duration.map(|d| Duration::from_secs(d as u64));

                    if let Some(dur) = current_duration {
                        state
                            .duration_ms
                            .store(dur.as_millis() as u64, Ordering::SeqCst);
                    }

                    // Fetch and decode the audio stream
                    match fetch_audio_data(&url) {
                        Ok(audio_data) => {
                            current_audio_data = Some(audio_data.clone());
                            if let Err(e) =
                                play_audio_data(&audio_data, &sink, current_volume, Duration::ZERO)
                            {
                                let _ = event_tx.send(PlayerEvent::Error(e.to_string()));
                            } else {
                                state.is_playing.store(true, Ordering::SeqCst);
                                state.position_ms.store(0, Ordering::SeqCst);
                                let _ =
                                    event_tx.send(PlayerEvent::StateChanged(PlayerState::Playing));
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(PlayerEvent::Error(e.to_string()));
                        }
                    }
                }
                PlayerCommand::Pause => {
                    sink.lock().unwrap().pause();
                    state.is_playing.store(false, Ordering::SeqCst);
                    let _ = event_tx.send(PlayerEvent::StateChanged(PlayerState::Paused));
                }
                PlayerCommand::Resume => {
                    sink.lock().unwrap().play();
                    state.is_playing.store(true, Ordering::SeqCst);
                    let _ = event_tx.send(PlayerEvent::StateChanged(PlayerState::Playing));
                }
                PlayerCommand::Stop => {
                    {
                        let s = sink.lock().unwrap();
                        s.stop();
                    }
                    *sink.lock().unwrap() = Sink::try_new(&stream_handle)?;
                    current_audio_data = None;
                    state.is_playing.store(false, Ordering::SeqCst);
                    state.position_ms.store(0, Ordering::SeqCst);
                    let _ = event_tx.send(PlayerEvent::StateChanged(PlayerState::Stopped));
                }
                PlayerCommand::SetVolume(vol) => {
                    current_volume = vol;
                    sink.lock().unwrap().set_volume(vol);
                }
                PlayerCommand::Seek(position) => {
                    // Seek by recreating the source with skip_duration
                    if let Some(ref audio_data) = current_audio_data {
                        // Stop current playback
                        {
                            let s = sink.lock().unwrap();
                            s.stop();
                        }
                        // Create new sink
                        *sink.lock().unwrap() = Sink::try_new(&stream_handle)?;

                        // Play from the new position
                        if let Err(e) = play_audio_data(audio_data, &sink, current_volume, position)
                        {
                            let _ =
                                event_tx.send(PlayerEvent::Error(format!("Seek failed: {}", e)));
                        } else {
                            state
                                .position_ms
                                .store(position.as_millis() as u64, Ordering::SeqCst);
                            state.is_playing.store(true, Ordering::SeqCst);
                            let _ = event_tx.send(PlayerEvent::StateChanged(PlayerState::Playing));
                        }
                    }
                }
            },
            Err(mpsc::error::TryRecvError::Empty) => {}
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Channel closed, exit thread
                break;
            }
        }

        // Check if track ended
        if sink.lock().unwrap().empty() && state.is_playing.load(Ordering::SeqCst) {
            state.is_playing.store(false, Ordering::SeqCst);
            let _ = event_tx.send(PlayerEvent::TrackEnded);
        }

        // Update progress (approximate based on time elapsed)
        if state.is_playing.load(Ordering::SeqCst) {
            // Increment position by tick interval
            let current = state.position_ms.load(Ordering::SeqCst);
            state.position_ms.store(current + 100, Ordering::SeqCst);

            if let Some(dur) = current_duration {
                let _ = event_tx.send(PlayerEvent::Progress {
                    position: Duration::from_millis(current),
                    duration: dur,
                });
            }
        }

        // Sleep to avoid busy waiting
        std::thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Fetch audio data from URL.
fn fetch_audio_data(url: &str) -> Result<Vec<u8>> {
    let response = reqwest::blocking::get(url)?;
    let bytes = response.bytes()?;
    Ok(bytes.to_vec())
}

/// Play audio data with optional skip duration for seeking.
fn play_audio_data(
    audio_data: &[u8],
    sink: &Arc<Mutex<Sink>>,
    volume: f32,
    skip: Duration,
) -> Result<()> {
    let cursor = Cursor::new(audio_data.to_vec());
    let source = Decoder::new(BufReader::new(cursor))?;

    let s = sink.lock().unwrap();
    if skip > Duration::ZERO {
        // Skip to the seek position
        s.append(source.skip_duration(skip));
    } else {
        s.append(source);
    }
    s.set_volume(volume);
    s.play();

    Ok(())
}
