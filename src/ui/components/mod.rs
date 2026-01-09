//! UI components module.

pub mod library;
pub mod lyrics;
pub mod now_playing;
pub mod queue;
pub mod search;

pub use library::{render_library, LibraryState};
pub use lyrics::{render_lyrics, LyricsState};
pub use now_playing::{render_now_playing, NowPlayingState};
pub use queue::{render_queue, QueueState};
pub use search::{render_search, SearchState};
