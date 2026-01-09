//! Audio playback backend using rodio.

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use color_eyre::Result;
use rodio::{OutputStream, Sink, Source};
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;
use symphonia::default::get_codecs;
use symphonia::default::get_probe;
use tokio::sync::mpsc;

use crate::action::PlayerState;
use crate::client::models::Song;

/// A wrapper around a byte buffer that implements `MediaSource` with proper byte length.
/// This is needed because rodio's `ReadSeekSource` returns `None` for `byte_len()`,
/// which causes symphonia to treat some formats as unseekable.
struct SeekableSource {
    cursor: Cursor<Vec<u8>>,
    len: u64,
}

impl SeekableSource {
    fn new(data: Vec<u8>) -> Self {
        let len = data.len() as u64;
        Self {
            cursor: Cursor::new(data),
            len,
        }
    }
}

impl Read for SeekableSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Seek for SeekableSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(pos)
    }
}

impl MediaSource for SeekableSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

/// A symphonia-based audio source that supports proper seeking.
struct SymphoniaSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    buffer: SampleBuffer<i16>,
    current_frame_offset: usize,
    spec: SignalSpec,
    total_duration: Option<Time>,
}

impl SymphoniaSource {
    fn new(data: Vec<u8>) -> Result<Self> {
        let source = SeekableSource::new(data);
        let mss = MediaSourceStream::new(Box::new(source), Default::default());

        let hint = Hint::new();
        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts = MetadataOptions::default();

        let probed = get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to probe format: {}", e))?;

        let track = probed
            .format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| color_eyre::eyre::eyre!("No supported audio track found"))?;

        let track_id = track.id;
        let total_duration = track
            .codec_params
            .time_base
            .zip(track.codec_params.n_frames)
            .map(|(base, frames)| base.calc_time(frames));

        let decoder = get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| color_eyre::eyre::eyre!("Failed to create decoder: {}", e))?;

        let mut source = Self {
            format: probed.format,
            decoder,
            track_id,
            buffer: SampleBuffer::new(
                0,
                SignalSpec::new(44100, symphonia::core::audio::Channels::FRONT_LEFT),
            ),
            current_frame_offset: 0,
            spec: SignalSpec::new(44100, symphonia::core::audio::Channels::FRONT_LEFT),
            total_duration,
        };

        // Decode first frame to get proper spec
        source.decode_next_frame();

        Ok(source)
    }

    fn seek(&mut self, position: Duration) -> Result<()> {
        let time = Time::from(position.as_secs_f64());

        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: None,
                },
            )
            .map_err(|e| color_eyre::eyre::eyre!("Seek failed: {}", e))?;

        // Decode a frame after seeking to update buffers
        self.decode_next_frame();
        Ok(())
    }

    fn decode_next_frame(&mut self) -> bool {
        const MAX_RETRIES: usize = 3;
        let mut retries = 0;

        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => return false,
            };

            // Skip packets not for our track
            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    // Copy data from decoded buffer immediately to avoid borrow issues
                    let spec = *decoded.spec();
                    let duration =
                        symphonia::core::units::Duration::from(decoded.capacity() as u64);
                    let mut buffer = SampleBuffer::new(duration, spec);
                    buffer.copy_interleaved_ref(decoded);

                    self.spec = spec;
                    self.buffer = buffer;
                    self.current_frame_offset = 0;
                    return true;
                }
                Err(_) => {
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return false;
                    }
                }
            }
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if self.current_frame_offset >= self.buffer.len() && !self.decode_next_frame() {
            return None;
        }

        let sample = *self.buffer.samples().get(self.current_frame_offset)?;
        self.current_frame_offset += 1;
        Some(sample)
    }
}

impl Source for SymphoniaSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(
            self.buffer
                .samples()
                .len()
                .saturating_sub(self.current_frame_offset),
        )
    }

    fn channels(&self) -> u16 {
        self.spec.channels.count() as u16
    }

    fn sample_rate(&self) -> u32 {
        self.spec.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
            .map(|t| Duration::from_secs_f64(t.seconds as f64 + t.frac))
    }
}

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
    // Flag to prevent false TrackEnded events during seek operations
    let mut is_seeking: bool = false;
    // Track the last known play time for accurate position tracking
    let mut last_tick_time: Option<std::time::Instant> = None;

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
                                last_tick_time = Some(std::time::Instant::now());
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
                    last_tick_time = None; // Stop tracking time while paused
                    let _ = event_tx.send(PlayerEvent::StateChanged(PlayerState::Paused));
                }
                PlayerCommand::Resume => {
                    sink.lock().unwrap().play();
                    state.is_playing.store(true, Ordering::SeqCst);
                    last_tick_time = Some(std::time::Instant::now()); // Resume tracking
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
                    last_tick_time = None;
                    let _ = event_tx.send(PlayerEvent::StateChanged(PlayerState::Stopped));
                }
                PlayerCommand::SetVolume(vol) => {
                    current_volume = vol;
                    sink.lock().unwrap().set_volume(linear_to_log_volume(vol));
                }
                PlayerCommand::Seek(position) => {
                    // Since our SymphoniaSource supports seeking, we recreate it with
                    // the new position. This is fast because symphonia seeks directly
                    // to the position in the compressed stream.
                    if let Some(ref audio_data) = current_audio_data {
                        // Remember if we were playing before seek
                        let was_playing = state.is_playing.load(Ordering::SeqCst);

                        // Set seeking flag to prevent false TrackEnded events
                        is_seeking = true;

                        {
                            let s = sink.lock().unwrap();
                            s.stop();
                        }
                        *sink.lock().unwrap() = Sink::try_new(&stream_handle)?;

                        if let Err(e) = play_audio_data(audio_data, &sink, current_volume, position)
                        {
                            let _ =
                                event_tx.send(PlayerEvent::Error(format!("Seek failed: {}", e)));
                        } else {
                            state
                                .position_ms
                                .store(position.as_millis() as u64, Ordering::SeqCst);

                            // Restore previous play/pause state
                            if was_playing {
                                state.is_playing.store(true, Ordering::SeqCst);
                                last_tick_time = Some(std::time::Instant::now());
                                // Sink is already playing from play_audio_data
                            } else {
                                // Was paused, so pause after seek
                                sink.lock().unwrap().pause();
                                state.is_playing.store(false, Ordering::SeqCst);
                                last_tick_time = None;
                                let _ =
                                    event_tx.send(PlayerEvent::StateChanged(PlayerState::Paused));
                            }
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

        // Check if track ended (but not during seek operations)
        if !is_seeking && sink.lock().unwrap().empty() && state.is_playing.load(Ordering::SeqCst) {
            state.is_playing.store(false, Ordering::SeqCst);
            let _ = event_tx.send(PlayerEvent::TrackEnded);
        }

        // Reset seeking flag after track-end check
        is_seeking = false;

        // Update progress based on actual elapsed time
        if state.is_playing.load(Ordering::SeqCst) {
            if let Some(last_time) = last_tick_time {
                let elapsed_ms = last_time.elapsed().as_millis() as u64;
                let current = state.position_ms.load(Ordering::SeqCst);
                state
                    .position_ms
                    .store(current + elapsed_ms, Ordering::SeqCst);
                last_tick_time = Some(std::time::Instant::now());

                if let Some(dur) = current_duration {
                    let _ = event_tx.send(PlayerEvent::Progress {
                        position: Duration::from_millis(current + elapsed_ms),
                        duration: dur,
                    });
                }
            } else {
                // Initialize last_tick_time if not set
                last_tick_time = Some(std::time::Instant::now());
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

/// Convert linear volume (0.0-1.0) to logarithmic/perceptual volume.
/// Human hearing perceives loudness logarithmically, so we need to convert
/// the linear slider position to an exponential amplitude scale.
/// Uses a curve that feels natural: amplitude = volume^2.5
fn linear_to_log_volume(linear: f32) -> f32 {
    if linear <= 0.0 {
        0.0
    } else if linear >= 1.0 {
        1.0
    } else {
        // Using power of 2.5 gives a good perceptual curve
        // At 50% slider, amplitude is ~17.7% which sounds roughly half as loud
        linear.powf(2.5)
    }
}

/// Play audio data with optional seek position.
/// Uses SymphoniaSource directly to ensure proper seeking support.
fn play_audio_data(
    audio_data: &[u8],
    sink: &Arc<Mutex<Sink>>,
    volume: f32,
    seek_to: Duration,
) -> Result<()> {
    // Create our custom symphonia source with proper byte_len() support
    let mut source = SymphoniaSource::new(audio_data.to_vec())?;

    // If we need to seek, do it before appending to sink
    if seek_to > Duration::ZERO {
        source.seek(seek_to)?;
    }

    let s = sink.lock().unwrap();
    s.append(source);
    s.set_volume(linear_to_log_volume(volume));
    s.play();

    Ok(())
}
