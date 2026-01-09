//! Application configuration management.

use std::path::PathBuf;

use color_eyre::Result;
use serde::{Deserialize, Serialize};

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,

    /// Player configuration
    #[serde(default)]
    pub player: PlayerConfig,

    /// UI configuration
    #[serde(default)]
    pub ui: UiConfig,
}

/// Server connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server URL (e.g., "https://music.example.com")
    pub url: String,

    /// Username for authentication
    pub username: String,

    /// Password (optional, will be prompted if not provided)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// API key for OpenSubsonic servers (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Player configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerConfig {
    /// Volume level (0-100)
    #[serde(default = "default_volume")]
    pub volume: u8,

    /// Enable gapless playback
    #[serde(default = "default_true")]
    pub gapless: bool,

    /// Preferred audio format for streaming
    #[serde(default)]
    pub format: Option<String>,

    /// Maximum bitrate for streaming (0 = no limit)
    #[serde(default)]
    pub max_bitrate: u32,
}

/// UI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Show the queue panel
    #[serde(default = "default_true")]
    pub show_queue: bool,

    /// Show album art (requires sixel/kitty support)
    #[serde(default = "default_true")]
    pub show_album_art: bool,

    /// Color theme
    #[serde(default)]
    pub theme: String,
}

fn default_volume() -> u8 {
    80
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                url: String::new(),
                username: String::new(),
                password: None,
                api_key: None,
            },
            player: PlayerConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            volume: default_volume(),
            gapless: true,
            format: None,
            max_bitrate: 0,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_queue: true,
            show_album_art: true,
            theme: String::from("default"),
        }
    }
}

impl Config {
    /// Get the configuration file path.
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Could not determine config directory"))?;

        Ok(config_dir.join("subsonic-tui").join("config.toml"))
    }

    /// Load configuration from file.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&path)?;
        let mut config: Config = toml::from_str(&contents)?;

        // Clamp volume to valid range (0-100)
        config.player.volume = config.player.volume.min(100);

        Ok(config)
    }

    /// Save configuration to file.
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;

        Ok(())
    }

    /// Check if the configuration is valid for connecting.
    pub fn is_valid(&self) -> bool {
        // URL must be non-empty and start with http:// or https://
        let valid_url = !self.server.url.is_empty()
            && (self.server.url.starts_with("http://") || self.server.url.starts_with("https://"));

        // Must have either a valid API key or username+password
        let valid_auth = self.server.api_key.as_ref().is_some_and(|k| !k.is_empty())
            || (!self.server.username.is_empty()
                && self.server.password.as_ref().is_some_and(|p| !p.is_empty()));

        valid_url && valid_auth
    }
}
