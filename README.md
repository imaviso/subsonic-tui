# subsonic-tui

A terminal user interface (TUI) music player for OpenSubsonic-compatible servers like [Navidrome](https://www.navidrome.org/), [Subsonic](http://www.subsonic.org/), [gonic](https://github.com/sentriz/gonic), and others.

Built with Rust and [ratatui](https://github.com/ratatui-org/ratatui).

## Features

- Browse your music library by Artists, Albums, Songs, Playlists, Genres, and Favorites
- Queue management with shuffle and repeat modes
- Synced lyrics display (OpenSubsonic extension)
- Search across artists, albums, and songs
- Star/unstar tracks
- Scrobbling support
- Vim-style keyboard navigation
- Mouse support

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/subsonic-tui.git
cd subsonic-tui

# Build with Cargo
cargo build --release

# The binary will be at target/release/subsonic-tui
```

### With Nix

```bash
# Run directly
nix run github:yourusername/subsonic-tui

# Or install to profile
nix profile install github:yourusername/subsonic-tui

# Or build locally
nix build
./result/bin/subsonic-tui
```

### Development

```bash
# Enter development shell
nix develop

# Build and run
cargo run

# Run checks
nix flake check
```

## Configuration

Create a configuration file at `~/.config/subsonic-tui/config.toml`:

```toml
[server]
url = "https://your-server.com"
username = "your-username"
# Use either password or api_key (api_key is preferred if your server supports it)
password = "your-password"
# api_key = "your-api-key"

[player]
volume = 80
```

### Command Line Options

```
Usage: subsonic-tui [OPTIONS]

Options:
  -c, --config <CONFIG>      Path to configuration file
  -s, --server <SERVER>      Server URL (overrides config)
  -u, --username <USERNAME>  Username (overrides config)
  -p, --password <PASSWORD>  Password (overrides config)
  -h, --help                 Print help
  -V, --version              Print version
```

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` or `↑` / `↓` | Move up/down |
| `h` / `l` or `←` / `→` | Switch focus / navigate |
| `Enter` | Select item |
| `Esc` / `Backspace` | Go back |
| `g` / `G` | Jump to top/bottom |
| `Ctrl+d` / `Ctrl+u` | Scroll half page down/up |
| `1` - `6` | Switch tabs (Artists/Albums/Songs/Playlists/Genres/Favorites) |

### Playback

| Key | Action |
|-----|--------|
| `Space` | Play/Pause |
| `n` / `p` | Next/Previous track |
| `,` / `.` | Seek backward/forward (10s) |
| `+` / `-` | Volume up/down |
| `s` | Toggle shuffle |
| `r` | Cycle repeat mode (Off → All → One) |

### Queue & Library

| Key | Action |
|-----|--------|
| `a` | Add to queue (without playing) |
| `c` | Clear queue |
| `d` / `Delete` | Remove selected from queue |
| `o` | Jump to current track in queue |
| `J` / `K` | Move queue item down/up |
| `*` | Toggle star on current song |
| `R` | Refresh library |

### Other

| Key | Action |
|-----|--------|
| `/` | Open search |
| `L` | Toggle lyrics panel |
| `i` | Show track info |
| `?` | Show help |
| `x` | Clear error message |
| `q` | Quit |

## Tabs

### Artists (1)
Browse all artists in your library. Select an artist to view their albums, then select an album to view its songs.

### Albums (2)
Browse all albums sorted by newest first. Select an album to view its songs.

### Songs (3)
Browse random songs from your library.

### Playlists (4)
Browse your playlists. Select a playlist to view its songs.

### Genres (5)
Browse all genres in your library. Select a genre to view albums in that genre.

### Favorites (6)
Browse your starred/favorite items. The view is split into three columns:
- **Artists**: Starred artists
- **Albums**: Starred albums
- **Songs**: Starred songs

Use `h`/`l` or arrow keys to switch between columns.

## Requirements

- A Subsonic-compatible server (Navidrome, Subsonic, gonic, Airsonic, etc.)
- Audio output device
- Terminal with image support (Kitty, Ghostty, iTerm2, WezTerm, foot, etc.) for album art display
- [Nerd Font](https://www.nerdfonts.com/) for icons

### Linux Dependencies

On Linux, ALSA is required for audio playback. The Nix build handles this automatically. For other systems:

```bash
# Debian/Ubuntu
sudo apt install libasound2-dev

# Fedora
sudo dnf install alsa-lib-devel

# Arch
sudo pacman -S alsa-lib
```

### OpenSubsonic Extensions

For the best experience, use a server that supports [OpenSubsonic](https://opensubsonic.netlify.app/) extensions:
- **Synced Lyrics**: Requires the `getLyricsBySongId` endpoint

## Logging

Logs are written to `~/.cache/subsonic-tui/subsonic-tui.log`.

## Building

### With Cargo

```bash
cargo build --release
```

### With Nix

```bash
# Build wrapped binary (includes runtime dependencies)
nix build

# Build unwrapped binary
nix build .#unwrapped

# Run checks (build + clippy)
nix flake check
```

## License

MIT License

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [rodio](https://github.com/RustAudio/rodio) - Audio playback
- [symphonia](https://github.com/pdeljanov/symphonia) - Audio decoding and seeking
- [OpenSubsonic](https://opensubsonic.netlify.app/) - API specification
