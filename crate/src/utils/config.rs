//! App-wide configuration, read from a user-editable file on disk.
//!
//! The config file sits at `<bundle>/config/settings.json` — inside the
//! bundle's `config/` directory, next to `bin/` (the binary is at
//! `<bundle>/bin/kindle-hello`) — so it can be edited by the user without
//! rebuilding. It is read exactly once at startup, in `App::new` (the app's
//! composition root).
//!
//! The file carries the *default* configuration. User choices made at
//! runtime live in the shared `AppConfig` store held by `App` (an
//! `Arc<RwLock<AppConfig>>` cloned into the screens that need it) for the
//! rest of the app's lifetime.

use std::path::{Path, PathBuf};

use log::{info, warn};
use serde::{Deserialize, Deserializer, Serialize};

use crate::models::puzzle::PuzzleParams;

/// Name of the config file inside the bundle's `config/` directory.
/// e08s01: renamed from `default_cfg.json` to `settings.json`.
const CONFIG_FILE: &str = "settings.json";

/// Board orientation preference (e08s01).
///
/// Controls which color sits at the bottom of the board on game/puzzle
/// screens. `Auto` follows the player's color (white at bottom when you
/// play white, black at bottom when you play black) — the pre-e08 behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BoardOrientation {
    /// Follow the player's color (default, pre-e08 behavior).
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// Always show white at the bottom.
    #[serde(rename = "white")]
    White,
    /// Always show black at the bottom.
    #[serde(rename = "black")]
    Black,
}

impl BoardOrientation {
    /// All variants in cycle order (for tap-to-cycle UI in e08s02).
    pub const ALL: [Self; 3] = [Self::Auto, Self::White, Self::Black];

    /// Next variant in cycle order.
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

/// General (app-wide) settings shared across all screens (e08s01).
///
/// Persisted inside the `general` section of `settings.json`. Screen-specific
/// settings (e.g. puzzle params) stay as top-level fields on `AppConfig`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// UI language (Fluent locale id), e.g. "en" / "de".
    pub language: String,
    /// Which color to show at the bottom of the board.
    pub board_orientation: BoardOrientation,
    /// Whether to check for app updates automatically on startup.
    pub auto_update_check: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            language: crate::i18n::DEFAULT_LOCALE.to_string(),
            board_orientation: BoardOrientation::Auto,
            auto_update_check: true,
        }
    }
}

/// Root of the app configuration.
///
/// New sections (app-wide settings, …) are added as new fields.
/// Deserialization is tolerant in both directions: missing fields fall back
/// to `Default`, unknown fields are ignored, and the legacy pre-e08
/// top-level `language` field is migrated into `general.language`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppConfig {
    /// General app-wide settings (language, board orientation, update check).
    pub general: GeneralSettings,
    /// Defaults for the puzzle screens. Confirmed user choices replace these
    /// in the in-memory store for the app's lifetime.
    pub puzzle: PuzzleParams,
    /// Start in chrome mode (Kindle status bar PC:T overlaid on the app)
    /// instead of fullscreen. Default false = current fullscreen behavior;
    /// the top.png sidebar button toggles it at runtime either way.
    pub chrome_on_start: bool,
    /// e07s12: show file/rank labels on the board edges. Default true.
    /// UI toggle deferred to e08-settings-tabs.
    pub board_labels: bool,
}

/// Custom deserialization with backward compatibility for the pre-e08
/// top-level `language` field. Old config files like
/// `{"language": "de", "puzzle": {...}}` are migrated so that `language`
/// lands in `general.language`.
impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(default)]
        struct Raw {
            general: GeneralSettings,
            puzzle: PuzzleParams,
            chrome_on_start: bool,
            board_labels: bool,
            /// Legacy pre-e08 top-level language field.
            language: Option<String>,
        }
        impl Default for Raw {
            fn default() -> Self {
                let cfg = AppConfig::default();
                Self {
                    general: cfg.general,
                    puzzle: cfg.puzzle,
                    chrome_on_start: cfg.chrome_on_start,
                    board_labels: cfg.board_labels,
                    language: None,
                }
            }
        }
        let raw = Raw::deserialize(deserializer)?;
        let mut general = raw.general;
        // Migrate legacy top-level language into general if present.
        if let Some(lang) = raw.language {
            general.language = lang;
        }
        Ok(Self {
            general,
            puzzle: raw.puzzle,
            chrome_on_start: raw.chrome_on_start,
            board_labels: raw.board_labels,
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            puzzle: PuzzleParams::default(),
            chrome_on_start: false,
            board_labels: true,
        }
    }
}

impl AppConfig {
    /// Candidate config file locations, in priority order:
    ///
    /// 1. `<bundle>/config/settings.json`, resolved relative to the
    ///    running binary at `<bundle>/bin/kindle-hello` — works on the
    ///    Kindle and whenever the bundle's own binary is run.
    /// 2. `<ROOT_DIR>/config/settings.json` — the bundle root baked in
    ///    at compile time (the same mechanism as `secrets/token.json`);
    ///    covers dev runs via `run-dev.sh`, where the binary lives in
    ///    `target/` rather than in the bundle.
    fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Ok(exe) = std::env::current_exe()
            && let Some(bundle) = exe.parent().and_then(|bin| bin.parent())
        {
            paths.push(bundle.join("config").join(CONFIG_FILE));
        }
        paths.push(Path::new(env!("ROOT_DIR")).join("config").join(CONFIG_FILE));
        paths
    }

    /// Parse a config from a JSON string. Exposed for tests.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Read and parse the config file at `path`.
    fn load_from(path: &Path) -> Result<Self, String> {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_json(&json).map_err(|e| e.to_string())
    }

    /// Persist the configuration to `path`, creating parent dirs as needed.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Persist to the primary config location (the bundle's own `config/`
    /// dir next to the running binary, falling back to the baked-in ROOT_DIR).
    /// Never fails hard — a read-only /mnt/us must not crash the app.
    pub fn save(&self) {
        let path = Self::candidate_paths()
            .into_iter()
            .next()
            .unwrap_or_else(|| Path::new(env!("ROOT_DIR")).join("config").join(CONFIG_FILE));
        match self.save_to(&path) {
            Ok(()) => info!("Config saved to {}", path.display()),
            Err(e) => warn!("Failed to save config to {}: {}", path.display(), e),
        }
    }

    /// Load the configuration at startup.
    ///
    /// Never fails: the first candidate that exists and parses wins, a
    /// malformed file is skipped with a warning, and when no usable file is
    /// found the built-in defaults apply. A missing or bad config must never
    /// keep the app from starting on the Kindle.
    pub fn load() -> Self {
        for path in Self::candidate_paths() {
            if !path.exists() {
                continue;
            }
            match Self::load_from(&path) {
                Ok(config) => {
                    info!("Config loaded from {}", path.display());
                    return config;
                }
                Err(e) => {
                    warn!("Config file {} is invalid ({e}); skipping", path.display());
                }
            }
        }
        info!("No usable config file; using built-in defaults");
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::puzzle::{Difficulty, PuzzleColor, PuzzlePhase};

    fn temp_config_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "kindle-chess-config-{tag}-{}.json",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn parses_full_puzzle_section() {
        let config = AppConfig::from_json(
            r#"{ "puzzle": { "difficulty": "hardest", "phase": "endgame", "color": "black" } }"#,
        )
        .unwrap();
        assert_eq!(config.puzzle.difficulty, Difficulty::Hardest);
        assert_eq!(config.puzzle.phase, PuzzlePhase::Endgame);
        assert_eq!(config.puzzle.color, PuzzleColor::Black);
    }

    #[test]
    fn empty_object_falls_back_to_defaults() {
        let config = AppConfig::from_json("{}").unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn partial_puzzle_section_fills_missing_fields() {
        let config = AppConfig::from_json(r#"{ "puzzle": { "difficulty": "easiest" } }"#).unwrap();
        assert_eq!(config.puzzle.difficulty, Difficulty::Easiest);
        assert_eq!(config.puzzle.phase, PuzzlePhase::Any);
        assert_eq!(config.puzzle.color, PuzzleColor::Random);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        // Forward compatibility: a config written by a newer version must
        // still load on an older binary.
        let config = AppConfig::from_json(r#"{ "future_setting": true, "puzzle": {} }"#).unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(AppConfig::from_json("").is_err());
        assert!(AppConfig::from_json("{ not json").is_err());
        assert!(AppConfig::from_json(r#"{ "puzzle": { "difficulty": "impossible" } }"#).is_err());
    }

    #[test]
    fn round_trips_through_json() {
        // Serialize must stay symmetric with Deserialize — the store is
        // expected to be writable back out once persistence is added.
        let config = AppConfig {
            puzzle: PuzzleParams {
                difficulty: Difficulty::Harder,
                phase: PuzzlePhase::RookEndgame,
                color: PuzzleColor::White,
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(AppConfig::from_json(&json).unwrap(), config);
    }

    #[test]
    fn built_in_defaults_round_trip_through_json() {
        // Locks the built-in fallback defaults to the serde vocabulary.
        let json = serde_json::to_string(&AppConfig::default()).unwrap();
        assert_eq!(AppConfig::from_json(&json).unwrap(), AppConfig::default());
    }

    #[test]
    fn load_from_missing_file_is_an_error() {
        let path = Path::new("/nonexistent/kindle-chess-test-config");
        assert!(AppConfig::load_from(path).is_err());
    }

    #[test]
    fn load_from_reads_and_parses_file() {
        let path = temp_config_path("valid");
        std::fs::write(&path, r#"{ "puzzle": { "difficulty": "hardest" } }"#).unwrap();
        let config = AppConfig::load_from(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(config.puzzle.difficulty, Difficulty::Hardest);
        assert_eq!(config.puzzle.phase, PuzzlePhase::Any);
        assert_eq!(config.puzzle.color, PuzzleColor::Random);
    }

    #[test]
    fn load_from_invalid_file_is_an_error() {
        let path = temp_config_path("invalid");
        std::fs::write(&path, "not json").unwrap();
        let result = AppConfig::load_from(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn load_never_fails() {
        // The result depends on the machine's bundle layout, but loading must
        // always succeed.
        let _ = AppConfig::load();
    }
}

#[cfg(test)]
mod general_settings_tests {
    use super::*;

    #[test]
    fn default_language_is_english() {
        assert_eq!(AppConfig::default().general.language, "en");
    }

    #[test]
    fn default_board_orientation_is_auto() {
        assert_eq!(
            AppConfig::default().general.board_orientation,
            BoardOrientation::Auto
        );
    }

    #[test]
    fn default_auto_update_check_is_true() {
        assert!(AppConfig::default().general.auto_update_check);
    }

    #[test]
    fn general_settings_serde_roundtrip() {
        let cfg = AppConfig {
            general: GeneralSettings {
                language: "de".into(),
                board_orientation: BoardOrientation::White,
                auto_update_check: false,
            },
            ..Default::default()
        };
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("\"general\""));
        assert!(s.contains("\"language\":\"de\""));
        assert!(s.contains("\"board_orientation\":\"white\""));
        assert!(s.contains("\"auto_update_check\":false"));
        let back: AppConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back.general.language, "de");
        assert_eq!(back.general.board_orientation, BoardOrientation::White);
        assert!(!back.general.auto_update_check);
    }

    #[test]
    fn legacy_top_level_language_migrates_to_general() {
        // Pre-e08 config file with top-level language, no general section.
        let old = r#"{"language":"de","puzzle":{"difficulty":"hardest"},"chrome_on_start":true}"#;
        let parsed: AppConfig = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.general.language, "de");
        assert_eq!(parsed.general.board_orientation, BoardOrientation::Auto);
        assert!(parsed.general.auto_update_check);
        assert!(parsed.chrome_on_start);
    }

    #[test]
    fn legacy_language_without_general_section_defaults_other_general_fields() {
        let old = r#"{"language":"de"}"#;
        let parsed: AppConfig = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.general.language, "de");
        assert_eq!(parsed.general.board_orientation, BoardOrientation::Auto);
    }

    #[test]
    fn new_format_general_section_parses() {
        let json = r#"{"general":{"language":"de","board_orientation":"black","auto_update_check":false}}"#;
        let parsed: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.general.language, "de");
        assert_eq!(parsed.general.board_orientation, BoardOrientation::Black);
        assert!(!parsed.general.auto_update_check);
    }

    #[test]
    fn board_orientation_cycles_through_all_variants() {
        let mut o = BoardOrientation::Auto;
        o = o.next();
        assert_eq!(o, BoardOrientation::White);
        o = o.next();
        assert_eq!(o, BoardOrientation::Black);
        o = o.next();
        assert_eq!(o, BoardOrientation::Auto);
    }

    #[test]
    fn general_settings_missing_fields_default() {
        // Old config files without a general section still parse.
        let old = r#"{"puzzle":{"difficulty":"easiest"},"chrome_on_start":true}"#;
        let parsed: AppConfig = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.general, GeneralSettings::default());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::models::puzzle::Difficulty;

    #[test]
    fn language_and_puzzle_survive_save_load_roundtrip() {
        let path = std::env::temp_dir().join(format!(
            "kindle-chess-persist-{}.json",
            uuid::Uuid::new_v4()
        ));
        let cfg = AppConfig {
            general: GeneralSettings {
                language: "de".into(),
                ..Default::default()
            },
            puzzle: crate::models::puzzle::PuzzleParams {
                difficulty: Difficulty::Hardest,
                ..Default::default()
            },
            ..Default::default()
        };
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded.general.language, "de");
        assert_eq!(loaded.puzzle.difficulty, Difficulty::Hardest);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn save_to_creates_missing_parent_dirs() {
        let dir =
            std::env::temp_dir().join(format!("kindle-chess-persist-{}", uuid::Uuid::new_v4()));
        let path = dir.join("config").join("settings.json");
        AppConfig::default().save_to(&path).unwrap();
        assert!(path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_file_is_named_settings_json() {
        assert_eq!(CONFIG_FILE, "settings.json");
    }
}
