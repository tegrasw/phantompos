//! User settings: port, network mode, paper and default code page.
//!
//! Persisted as pretty-printed JSON (`settings.json`, the same camelCase
//! shape as the `Settings` payload) in the app config directory, resolved by
//! the Tauri path API in `lib.rs`. This module only takes a directory, so it
//! is tested without Tauri.
//!
//! - Saving is atomic: the JSON goes to a temporary file in the same
//!   directory, which is flushed to disk and then renamed over the old file
//!   (`rename` replaces the destination on every platform), so a crash never
//!   leaves a half-written file.
//! - Loading is tolerant and never fails: a missing, unreadable or corrupt
//!   file gives the defaults; in a valid object every field that is missing
//!   or invalid falls back to its default on its own, and unknown fields are
//!   ignored (forward compatibility with newer versions).
//! - Updates from the UI ([`SettingsPatch`]) are strict: unknown fields and
//!   invalid values are rejected.

use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

use phantompos_core::{CodePage, PaperProfile};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Port used by network thermal printers ("raw" / JetDirect printing).
pub const DEFAULT_PORT: u16 = 9100;

/// File name inside the app config directory.
pub const FILE_NAME: &str = "settings.json";

/// Who may connect to the virtual printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkMode {
    /// Only this machine: binds `127.0.0.1`.
    #[default]
    Local,
    /// Any device on the network: binds `0.0.0.0` (every IPv4 interface).
    Network,
}

impl NetworkMode {
    /// Address the listener binds in this mode.
    pub fn bind_address(self) -> IpAddr {
        match self {
            Self::Local => IpAddr::V4(Ipv4Addr::LOCALHOST),
            Self::Network => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        }
    }
}

/// The settings (payload of `settings:changed`, `get_state().settings` and
/// the result of `update_settings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// 1-65535.
    pub port: u16,
    pub network_mode: NetworkMode,
    /// Paper for receipts started from now on.
    pub paper: PaperProfile,
    /// Code page at power on and after `ESC @`, for connections accepted
    /// from now on.
    pub default_code_page: CodePage,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            network_mode: NetworkMode::Local,
            paper: PaperProfile::Mm80,
            default_code_page: CodePage::Cp437,
        }
    }
}

/// A partial [`Settings`], the argument of `update_settings`: fields that are
/// present replace the current value. Unknown fields are rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub network_mode: Option<NetworkMode>,
    #[serde(default)]
    pub paper: Option<PaperProfile>,
    #[serde(default)]
    pub default_code_page: Option<CodePage>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SettingsError {
    #[error("Invalid port 0: the port must be between 1 and 65535.")]
    InvalidPort,
}

impl Settings {
    /// The settings with the fields of `patch` that are set, or an error if
    /// a value is invalid (serde already rejects wrong types, unknown names
    /// and ports above 65535).
    pub fn merged(self, patch: SettingsPatch) -> Result<Self, SettingsError> {
        if patch.port == Some(0) {
            return Err(SettingsError::InvalidPort);
        }
        Ok(Self {
            port: patch.port.unwrap_or(self.port),
            network_mode: patch.network_mode.unwrap_or(self.network_mode),
            paper: patch.paper.unwrap_or(self.paper),
            default_code_page: patch.default_code_page.unwrap_or(self.default_code_page),
        })
    }

    /// Reads the settings from JSON text, field by field (see the module
    /// docs). Never fails.
    pub fn from_json_lenient(text: &str) -> Self {
        let defaults = Self::default();
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str(text) else {
            return defaults;
        };
        fn field<T: DeserializeOwned>(
            map: &serde_json::Map<String, serde_json::Value>,
            key: &str,
        ) -> Option<T> {
            map.get(key).and_then(|value| T::deserialize(value).ok())
        }
        Self {
            port: field::<u16>(&map, "port")
                .filter(|&port| port != 0)
                .unwrap_or(defaults.port),
            network_mode: field(&map, "networkMode").unwrap_or(defaults.network_mode),
            paper: field(&map, "paper").unwrap_or(defaults.paper),
            default_code_page: field(&map, "defaultCodePage").unwrap_or(defaults.default_code_page),
        }
    }
}

/// Where the settings file lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsStore {
    dir: PathBuf,
}

impl SettingsStore {
    /// A store in `dir` (the app config directory); created on first save.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(FILE_NAME)
    }

    /// The saved settings, or the defaults for whatever cannot be read.
    pub fn load(&self) -> Settings {
        match fs::read_to_string(self.path()) {
            Ok(text) => Settings::from_json_lenient(&text),
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    eprintln!(
                        "phantompos: could not read {}: {error}; using the defaults",
                        self.path().display()
                    );
                }
                Settings::default()
            }
        }
    }

    /// Writes `settings` atomically (temporary file + rename), creating the
    /// directory if needed.
    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let mut text = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
        text.push('\n');
        let temp = self.dir.join(format!("{FILE_NAME}.tmp"));
        let written =
            write_synced(&temp, text.as_bytes()).and_then(|()| fs::rename(&temp, self.path()));
        if written.is_err() {
            // Best effort: do not leave the temporary file behind.
            let _ = fs::remove_file(&temp);
        }
        written
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh directory under the system temp dir, removed on drop.
    pub(crate) struct TempDir(pub PathBuf);

    impl TempDir {
        pub(crate) fn new(name: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("phantompos-test-{name}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn json_shape_and_defaults() {
        assert_eq!(
            serde_json::to_string(&Settings::default()).unwrap(),
            r#"{"port":9100,"networkMode":"local","paper":"80mm","defaultCodePage":"cp437"}"#
        );
        let other = Settings {
            port: 9101,
            network_mode: NetworkMode::Network,
            paper: PaperProfile::Mm58,
            default_code_page: CodePage::Windows1252,
        };
        assert_eq!(
            serde_json::to_string(&other).unwrap(),
            r#"{"port":9101,"networkMode":"network","paper":"58mm","defaultCodePage":"windows1252"}"#
        );
        assert_eq!(NetworkMode::Local.bind_address().to_string(), "127.0.0.1");
        assert_eq!(NetworkMode::Network.bind_address().to_string(), "0.0.0.0");
    }

    #[test]
    fn round_trip_through_the_file() {
        let dir = TempDir::new("settings-round-trip");
        // The directory does not exist yet: created by save.
        let store = SettingsStore::new(dir.0.join("nested"));
        assert_eq!(store.load(), Settings::default());
        let settings = Settings {
            port: 9200,
            network_mode: NetworkMode::Network,
            paper: PaperProfile::Mm58,
            default_code_page: CodePage::Cp860,
        };
        store.save(&settings).unwrap();
        assert_eq!(store.load(), settings);
        assert!(!dir.0.join("nested").join("settings.json.tmp").exists());
        // Overwriting works (rename over an existing file).
        let next = Settings {
            port: 9300,
            ..settings
        };
        store.save(&next).unwrap();
        assert_eq!(store.load(), next);
    }

    #[test]
    fn load_is_tolerant() {
        let dir = TempDir::new("settings-tolerant");
        let store = SettingsStore::new(&dir.0);
        fs::create_dir_all(&dir.0).unwrap();
        for corrupt in ["", "{", "null", "[1,2]", "\"text\"", "\u{0}\u{1}"] {
            fs::write(store.path(), corrupt).unwrap();
            assert_eq!(store.load(), Settings::default(), "{corrupt:?}");
        }
        // Bad fields fall back one by one; good ones and unknown ones are kept
        // or ignored.
        fs::write(
            store.path(),
            r#"{"port":0,"networkMode":"network","paper":"110mm","defaultCodePage":"cp850","theme":"dark"}"#,
        )
        .unwrap();
        assert_eq!(
            store.load(),
            Settings {
                port: 9100,
                network_mode: NetworkMode::Network,
                paper: PaperProfile::Mm80,
                default_code_page: CodePage::Cp850,
            }
        );
        fs::write(store.path(), r#"{"port":70000,"networkMode":3}"#).unwrap();
        assert_eq!(store.load(), Settings::default());
        fs::write(store.path(), r#"{"port":"9101","paper":"58mm"}"#).unwrap();
        assert_eq!(store.load().port, 9100);
        assert_eq!(store.load().paper, PaperProfile::Mm58);
    }

    fn patch(json: &str) -> Result<SettingsPatch, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn partial_updates_are_validated() {
        let base = Settings::default();
        let merged = base.merged(patch(r#"{"paper":"58mm"}"#).unwrap()).unwrap();
        assert_eq!(
            merged,
            Settings {
                paper: PaperProfile::Mm58,
                ..base
            }
        );
        let merged = base
            .merged(
                patch(r#"{"port":65535,"networkMode":"network","defaultCodePage":"cp858"}"#)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(merged.port, 65535);
        assert_eq!(merged.network_mode, NetworkMode::Network);
        assert_eq!(merged.default_code_page, CodePage::Cp858);
        assert_eq!(base.merged(patch("{}").unwrap()).unwrap(), base);

        assert_eq!(
            base.merged(patch(r#"{"port":0}"#).unwrap()),
            Err(SettingsError::InvalidPort)
        );
        for invalid in [
            r#"{"port":65536}"#,
            r#"{"port":-1}"#,
            r#"{"port":9100.5}"#,
            r#"{"port":"9100"}"#,
            r#"{"networkMode":"lan"}"#,
            r#"{"paper":"110mm"}"#,
            r#"{"defaultCodePage":"cp1252"}"#,
            r#"{"theme":"dark"}"#,
        ] {
            assert!(patch(invalid).is_err(), "{invalid} was accepted");
        }
    }
}
