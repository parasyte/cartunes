//! Application configuration parsing and validation.

use directories::UserDirs;
use patricia_tree::PatriciaSet;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::{table, value, Document, Item, TomlError};
use winit::dpi::{PhysicalPosition, PhysicalSize};

/// Parsing and writing configurations can fail.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration parse error.
    #[error("Configuration parse error: {0}")]
    Parse(#[from] TomlError),

    /// Type error
    #[error("Expected {path:?} to be type {expected}")]
    Type { path: String, expected: String },
}

/// Application configuration backed by TOML.
///
/// This struct retains the original parsed TOML and allows runtime changes while preserving
/// comments and original document structure. It is also strongly typed, so error handling only
/// needs to be done when reading and writing TOML.
pub(crate) struct Config {
    /// Original path to TOML file.
    doc_path: PathBuf,

    /// Original parsed TOML.
    doc: Document,

    /// Setup exports path.
    setups_path: PathBuf,

    /// Window minimum inner size.
    min_size: PhysicalSize<u32>,

    /// Map raw track IDs to unique track IDs (READ-ONLY).
    pub(crate) track_ids: PatriciaSet,

    /// Map track IDs to track names (READ-ONLY).
    pub(crate) tracks: HashMap<String, String>,

    /// Map car IDs to car names (READ-ONLY).
    pub(crate) cars: HashMap<String, String>,
}

/// Window settings.
pub(crate) struct Window {
    /// Window outer position.
    pub(crate) position: PhysicalPosition<i32>,

    /// Window inner size.
    pub(crate) size: PhysicalSize<u32>,
}

impl Error {
    /// Shortcut for creating a `TypeError`.
    fn type_error(path: &str, expected: &str) -> Self {
        let path = path.to_owned();
        let expected = expected.to_owned();

        Self::Type { path, expected }
    }
}

impl Config {
    /// Create a new Config.
    ///
    /// The path is allowed to be nonexistent. It will not be created until the TOML is written.
    pub(crate) fn new<P: AsRef<Path>>(doc_path: P, min_size: PhysicalSize<u32>) -> Self {
        let mut config = Self {
            doc_path: PathBuf::from(doc_path.as_ref()),
            doc: include_str!("default.toml").parse().unwrap(),
            setups_path: PathBuf::new(),
            min_size,
            track_ids: PatriciaSet::new(),
            tracks: HashMap::new(),
            cars: HashMap::new(),
        };

        // Default setup exports path is selected with the following precedence:
        // 1. `$HOME/Documents/iRacing`
        // 2. `$HOME/iRacing`
        // 3. `iRacing`
        // This path may not exist and is _not_ created by this application.
        let mut setups_path = UserDirs::new().map_or_else(PathBuf::default, |dirs| {
            PathBuf::from(dirs.document_dir().unwrap_or_else(|| dirs.home_dir()))
        });
        setups_path.push("iRacing");

        config.update_setups_path(setups_path);
        config.load_tracks_and_cars().unwrap();

        config
    }

    /// Parse TOML into a Config.
    ///
    /// The path is allowed to be nonexistent. It isn't an error, but there will be no config.
    pub(crate) fn from_toml<P: AsRef<Path>>(
        doc_path: P,
        min_size: PhysicalSize<u32>,
    ) -> Result<Option<Self>, Error> {
        let doc_path = PathBuf::from(doc_path.as_ref());
        if !doc_path.exists() {
            println!("Doesn't exist!");
            return Ok(None);
        }

        let doc: Document = fs::read_to_string(&doc_path)?.parse()?;

        let setups_path = PathBuf::from(
            doc["config"]["setups_path"]
                .as_str()
                .ok_or_else(|| Error::type_error("config.setups_path", "string"))?,
        );

        let mut config = Self::new(doc_path, min_size);
        config.doc = doc;
        config.update_setups_path(setups_path);
        config.load_tracks_and_cars()?;

        Ok(Some(config))
    }

    /// Create TOML file from this Config.
    ///
    /// The Config remembers the original TOML path, and this method rewrites that file. The config
    /// file is created if it does not exist, along with all intermediate directories in the path.
    pub(crate) fn write_toml(&self) -> Result<(), Error> {
        let toml = self.doc.to_string();
        if let Some(parent) = self.doc_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let result = fs::write(&self.doc_path, toml)?;

        Ok(result)
    }

    /// Get window configuration if it's valid.
    pub(crate) fn get_window(&self) -> Option<Window> {
        let window = &self.doc["window"];

        let x = window["x"].as_integer()?;
        let y = window["y"].as_integer()?;
        let position = PhysicalPosition::new(x as i32, y as i32);

        let width = window["width"].as_integer()?;
        let height = window["height"].as_integer()?;
        let size = PhysicalSize::new(
            (width as u32).max(self.min_size.width),
            (height as u32).max(self.min_size.height),
        );

        Some(Window::new(position, size))
    }

    /// Update config with external state.
    pub(crate) fn update_window(&mut self, window: &winit::window::Window) {
        self.doc["window"] = Window::from_winit(window).to_table();
    }

    /// Return a reference to the setup exports path.
    pub(crate) fn get_setups_path(&self) -> &Path {
        &self.setups_path
    }

    /// Update the setup exports path.
    pub(crate) fn update_setups_path<P: AsRef<Path>>(&mut self, setups_path: P) {
        self.setups_path = PathBuf::from(setups_path.as_ref());

        // Note that to_string_lossy() is destructive when the path contains invalid UTF-8 sequences.
        // If this is a problem in practice, we _could_ write unencodable paths as an array of
        // integers. It would allow reconstructing the path from TOML (which must be valid UTF-8)
        // even when the path cannot be encoded as valid UTF-8.
        let setups_path = self.setups_path.as_path().to_string_lossy();

        self.doc["config"]["setups_path"] = value(setups_path.as_ref());
    }

    /// Load track and car info from config.
    fn load_tracks_and_cars(&mut self) -> Result<(), Error> {
        if let Some(tracks) = self.doc["tracks"].as_table() {
            for (id, name) in tracks.iter() {
                let name = name
                    .as_str()
                    .ok_or_else(|| Error::type_error(&format!("tracks.{}", id), "string"))?;

                self.track_ids.insert(id.to_string());
                self.tracks.insert(id.to_string(), name.to_string());
            }
        }

        if let Some(cas) = self.doc["cars"].as_table() {
            for (id, name) in cas.iter() {
                let name = name
                    .as_str()
                    .ok_or_else(|| Error::type_error(&format!("cars.{}", id), "string"))?;

                self.cars.insert(id.to_string(), name.to_string());
            }
        }

        Ok(())
    }
}

impl Window {
    /// Create a Window configuration.
    fn new(position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> Self {
        Self { position, size }
    }

    /// Create a Window from a `winit` window.
    fn from_winit(window: &winit::window::Window) -> Self {
        #[cfg(target_os = "macos")]
        let position = window.inner_position();
        #[cfg(not(target_os = "macos"))]
        let position = window.outer_position();

        let position = position.unwrap_or_else(|_| PhysicalPosition::default());
        let size = window.inner_size();

        Self { position, size }
    }

    /// Create a TOML table from this Window.
    fn to_table(&self) -> Item {
        let mut output = table();

        output["x"] = value(self.position.x as i64);
        output["y"] = value(self.position.y as i64);
        output["width"] = value(self.size.width as i64);
        output["height"] = value(self.size.height as i64);

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test default config file.
    #[test]
    fn test_default_config() {
        let mut config = Config::new("/tmp/some/path.toml", PhysicalSize::new(100, 100));

        assert!(config.load_tracks_and_cars().is_ok());

        // Expect the PatriciaSet to have proper prefix matching.
        let track_ids = &config.track_ids;
        assert_eq!(
            track_ids.get_longest_common_prefix("charlotte_2018_2019_roval"),
            Some("charlotte_2018".as_bytes()),
        );
        assert_eq!(
            track_ids.get_longest_common_prefix("charlotte_fullroadcoarse"),
            Some("charlotte".as_bytes()),
        );
        assert_eq!(track_ids.get_longest_common_prefix("san_francisco"), None,);

        // Expectations for track name mapping.
        assert_eq!(
            config.tracks.get("charlotte_2018"),
            Some(&"Charlotte Motor Speedway".to_string())
        );
        assert_eq!(
            config.tracks.get("charlotte"),
            Some(&"[Legacy] Charlotte Motor Speedway - 2008".to_string())
        );

        // Expectations for car name mapping.
        assert_eq!(
            config.cars.get("rt2000"),
            Some(&"Skip Barber Formula 2000".to_string())
        )
    }
}
