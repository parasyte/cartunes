use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::{table, value, Document, Item, TomlError};
use winit::dpi::{PhysicalPosition, PhysicalSize};

/// Parsing and writing configurations can fail.
#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration parse error: {0}")]
    Parse(#[from] TomlError),

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

    /// Tuning directory.
    tuning_path: PathBuf,

    /// Window minimum inner size.
    min_size: PhysicalSize<u32>,
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
    pub(crate) fn new<P: AsRef<Path>>(path: P, min_size: PhysicalSize<u32>) -> Self {
        Self {
            doc_path: PathBuf::from(path.as_ref()),
            doc: include_str!("default.toml").parse().unwrap(),
            tuning_path: PathBuf::new(),
            min_size,
        }
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

        let tuning_path = PathBuf::from(
            doc["config"]["tuning_path"]
                .as_str()
                .ok_or_else(|| Error::type_error("config.tuning_path", "string"))?,
        );

        Ok(Some(Self {
            doc_path,
            doc,
            tuning_path,
            min_size,
        }))
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

    /// Return a reference to the tuning path.
    pub(crate) fn get_tuning_path(&self) -> &Path {
        &self.tuning_path
    }

    /// Update the tuning path.
    pub(crate) fn update_tuning_path<P: AsRef<Path>>(&mut self, tuning_path: P) {
        self.tuning_path = PathBuf::from(tuning_path.as_ref());

        // Note that to_string_lossy() is destructive when the path contains invalid UTF-8 sequences.
        // If this is a problem in practice, we _could_ write unencodable paths as an array of
        // integers. It would allow reconstructing the path from TOML (which must be valid UTF-8)
        // even when the path cannot be encoded as valid UTF-8.
        let tuning_path = self.tuning_path.as_path().to_string_lossy();

        self.doc["config"]["tuning_path"] = value(tuning_path.as_ref());
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
