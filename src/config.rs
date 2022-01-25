//! Application configuration parsing and validation.

use crate::updates::UpdateFrequency;
use directories::UserDirs;
use patricia_tree::PatriciaSet;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::{Document, Item, TomlError};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::Theme;

#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;

/// Parsing and writing configurations can fail.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration parse error.
    #[error("Configuration parse error: {0}")]
    Parse(#[from] TomlError),

    /// Type error.
    #[error("Expected {path:?} to be type {expected}")]
    Type { path: String, expected: String },

    /// Color format.
    #[error("Expected {0:?} to be a hex color in `#rrggbb` format")]
    Color(String),
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

    /// User's theme choice.
    theme: UserTheme,

    /// User's color-coding choices.
    colors: Vec<egui::Color32>,

    /// User's diff color choices.
    diff_colors: (egui::Color32, egui::Color32),

    /// User's update check frequency choice.
    update_check: UpdateFrequency,

    /// Map raw track IDs to unique track IDs.
    track_ids: PatriciaSet,

    /// Map track IDs to track names.
    tracks: HashMap<String, String>,

    /// Map car IDs to car names.
    cars: HashMap<String, String>,
}

/// Window settings.
pub(crate) struct Window {
    /// Window outer position.
    pub(crate) position: PhysicalPosition<i32>,

    /// Window inner size.
    pub(crate) size: PhysicalSize<u32>,
}

/// User's theme choice.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum UserTheme {
    /// Auto-select based on OS preferences (with fallback to dark mode).
    Auto,

    /// Dark mode.
    Dark,

    /// Light mode.
    Light,
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
            doc_path: doc_path.as_ref().to_path_buf(),
            doc: include_str!("default.toml").parse().unwrap(),
            setups_path: PathBuf::new(),
            min_size,
            theme: UserTheme::Auto,
            colors: Vec::new(),
            diff_colors: (egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT),
            update_check: UpdateFrequency::default(),
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
            dirs.document_dir()
                .unwrap_or_else(|| dirs.home_dir())
                .to_path_buf()
        });
        setups_path.push("iRacing");

        config.update_setups_path(setups_path);
        config.load_tracks_and_cars().unwrap();
        config.load_colors().unwrap();

        config
    }

    /// Parse TOML into a Config.
    ///
    /// The path is allowed to be nonexistent. It isn't an error, but there will be no config.
    pub(crate) fn from_toml<P: AsRef<Path>>(
        doc_path: P,
        min_size: PhysicalSize<u32>,
    ) -> Result<Option<Self>, Error> {
        let doc_path = doc_path.as_ref().to_path_buf();
        if !doc_path.exists() {
            return Ok(None);
        }

        let doc: Document = fs::read_to_string(&doc_path)?.parse()?;

        let setups_path = PathBuf::from(
            doc.get("config")
                .and_then(|t| t.get("setups_path"))
                .and_then(|t| t.as_str())
                .ok_or_else(|| Error::type_error("config.setups_path", "string"))?,
        );

        let theme = doc
            .get("config")
            .and_then(|t| t.get("theme"))
            .and_then(|t| t.as_str())
            .unwrap_or("auto");
        let theme = UserTheme::from_str(theme);

        let update_check = doc
            .get("config")
            .and_then(|t| t.get("update_check"))
            .and_then(|t| t.as_str())
            .map(UpdateFrequency::from)
            .unwrap_or_default();

        let mut config = Self::new(doc_path, min_size);
        config.doc = doc;
        config.update_setups_path(setups_path);
        config.update_theme(theme);
        config.set_update_check(update_check);
        config.load_tracks_and_cars()?;
        config.load_colors()?;

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
        let window = &self.doc.get("window")?;

        let x = window.get("x").and_then(|t| t.as_integer())?;
        let y = window.get("y").and_then(|t| t.as_integer())?;
        let position = PhysicalPosition::new(x as i32, y as i32);

        let width = window.get("width").and_then(|t| t.as_integer())?;
        let height = window.get("height").and_then(|t| t.as_integer())?;
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

    /// Get a reference to the setup exports path.
    pub(crate) fn get_setups_path(&self) -> &Path {
        &self.setups_path
    }

    /// Update the setup exports path.
    pub(crate) fn update_setups_path<P: AsRef<Path>>(&mut self, setups_path: P) {
        self.setups_path = setups_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| setups_path.as_ref().to_path_buf());

        // Note that to_string_lossy() is destructive when the path contains invalid UTF-8 sequences.
        // If this is a problem in practice, we _could_ write unencodable paths as an array of
        // integers. It would allow reconstructing the path from TOML (which must be valid UTF-8)
        // even when the path cannot be encoded as valid UTF-8.
        let setups_path = self.setups_path.as_path().to_string_lossy();

        self.doc["config"]["setups_path"] = toml_edit::value(setups_path.as_ref());
    }

    /// Get a reference to the theme preference.
    pub(crate) fn theme(&self) -> &UserTheme {
        &self.theme
    }

    /// Update the theme preference.
    pub(crate) fn update_theme(&mut self, theme: UserTheme) {
        self.theme = theme;
        self.doc["config"]["theme"] = toml_edit::value(theme.as_str());
    }

    /// Get a reference for mapping raw track IDs to unique track IDs.
    pub(crate) fn track_ids(&self) -> &PatriciaSet {
        &self.track_ids
    }

    /// Get a reference for mapping track IDs to track names.
    pub(crate) fn tracks(&self) -> &HashMap<String, String> {
        &self.tracks
    }

    /// Get a reference for mapping car IDs to car names.
    pub(crate) fn cars(&self) -> &HashMap<String, String> {
        &self.cars
    }

    /// Get user's color-coding choices.
    pub(crate) fn colors(&self) -> Vec<egui::Color32> {
        self.colors.clone()
    }

    /// Modify user's color-coding choices.
    pub(crate) fn colors_mut(&mut self) -> &mut Vec<egui::Color32> {
        &mut self.colors
    }

    /// Update colors in TOML document.
    pub(crate) fn update_colors(&mut self) {
        let mut colors = toml_edit::Array::default();

        for color in &self.colors {
            let color = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());
            colors.push(color);
        }

        self.doc["config"]["colors"] = toml_edit::value(colors);

        self.doc["config"]["background_decrease"] = toml_edit::value(format!(
            "#{:02x}{:02x}{:02x}",
            self.diff_colors.0.r(),
            self.diff_colors.0.g(),
            self.diff_colors.0.b()
        ));
        self.doc["config"]["background_increase"] = toml_edit::value(format!(
            "#{:02x}{:02x}{:02x}",
            self.diff_colors.1.r(),
            self.diff_colors.1.g(),
            self.diff_colors.1.b()
        ));
    }

    /// Get user's diff color choices.
    pub(crate) fn diff_colors(&self) -> (egui::Color32, egui::Color32) {
        self.diff_colors
    }

    /// Modify user's diff color choices.
    pub(crate) fn diff_colors_mut(&mut self) -> &mut (egui::Color32, egui::Color32) {
        &mut self.diff_colors
    }

    /// Update the frequency for update checks.
    pub(crate) fn get_update_check(&self) -> UpdateFrequency {
        self.update_check
    }

    /// Update the frequency for update checks.
    pub(crate) fn set_update_check(&mut self, update_check: UpdateFrequency) {
        self.update_check = update_check;
        self.doc["config"]["update_check"] = toml_edit::value(self.update_check.as_str());
    }

    /// Load track and car info from config.
    fn load_tracks_and_cars(&mut self) -> Result<(), Error> {
        let table = &self.doc.get("tracks").and_then(|t| t.as_table());
        if let Some(tracks) = table {
            for (id, name) in tracks.iter() {
                let name = name
                    .as_str()
                    .ok_or_else(|| Error::type_error(&format!("tracks.{}", id), "string"))?;

                self.track_ids.insert(id);
                self.tracks.insert(id.to_string(), name.to_string());
            }
        } else if !table.is_none() {
            return Err(Error::type_error("tracks", "table"));
        }

        let cars = &self.doc.get("cars").and_then(|t| t.as_table());
        if let Some(cars) = cars {
            for (id, name) in cars.iter() {
                let name = name
                    .as_str()
                    .ok_or_else(|| Error::type_error(&format!("cars.{}", id), "string"))?;

                self.cars.insert(id.to_string(), name.to_string());
            }
        } else if !cars.is_none() {
            return Err(Error::type_error("cars", "table"));
        }

        Ok(())
    }

    /// Load column colors and background colors from config.
    fn load_colors(&mut self) -> Result<(), Error> {
        let mut parsed = Vec::new();
        let colors = &self
            .doc
            .get("config")
            .and_then(|t| t.get("colors"))
            .and_then(|t| t.as_array());

        if let Some(colors) = colors {
            for (i, color) in colors.iter().enumerate() {
                let color = color
                    .as_str()
                    .ok_or_else(|| Error::type_error(&format!("config.colors[{}]", i), "string"))?;
                let color = color_from_str(color)
                    .map_err(|_| Error::Color(format!("config.colors[{}]", i)))?;

                parsed.push(color);
            }
        } else if !colors.is_none() {
            return Err(Error::type_error("config.colors", "array"));
        }

        // Parse background colors
        let mut background = Vec::new();
        for name in &["background_decrease", "background_increase"] {
            let color = self
                .doc
                .get("config")
                .and_then(|t| t.get(name))
                .and_then(|t| t.as_str());

            if let Some(color) = color {
                let color =
                    color_from_str(color).map_err(|_| Error::Color(format!("config.{}", name)))?;

                background.push(color);
            }
        }

        // If all colors are parsed successfully, replace the entire config
        if !parsed.is_empty() {
            self.colors = parsed;
        }
        if background.len() == 2 {
            self.diff_colors = (background[0], background[1]);
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
        let mut output = toml_edit::table();

        output["x"] = toml_edit::value(self.position.x as i64);
        output["y"] = toml_edit::value(self.position.y as i64);
        output["width"] = toml_edit::value(self.size.width as i64);
        output["height"] = toml_edit::value(self.size.height as i64);

        output
    }
}

impl UserTheme {
    /// Create a `UserTheme` from a string slice.
    fn from_str(value: &str) -> Self {
        match value {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => Self::Auto,
        }
    }

    /// Get a string slice that is TOML-compatible for this `UserTheme`.
    fn as_str(&self) -> &str {
        match self {
            Self::Auto => "auto",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// Create a [`winit::window::Theme`] from this `UserTheme`.
    ///
    /// When the `UserTheme` value is set to `Auto`, the `window` reference will be used to select
    /// the theme based on OS preferences.
    #[allow(unused_variables)]
    pub(crate) fn as_winit_theme(&self, window: &winit::window::Window) -> Theme {
        match self {
            Self::Auto => {
                #[cfg(target_os = "windows")]
                let theme = window.theme();
                #[cfg(not(target_os = "windows"))]
                let theme = Theme::Dark;

                theme
            }
            Self::Dark => winit::window::Theme::Dark,
            Self::Light => winit::window::Theme::Light,
        }
    }
}

impl std::fmt::Display for UserTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Auto => "Automatic",
            Self::Dark => "Dark Mode",
            Self::Light => "Light Mode",
        };
        write!(f, "{}", text)
    }
}

fn color_from_str(color: &str) -> Result<egui::Color32, ()> {
    // Validate color format. Require HTML hex `#rrggbb` for convenience
    let mut validator = color.chars();
    if color.len() != 7
        || validator.next().unwrap() != '#'
        || validator.any(|ch| !ch.is_ascii_hexdigit())
    {
        return Err(());
    }

    let r = u8::from_str_radix(&color[1..3], 16).unwrap();
    let g = u8::from_str_radix(&color[3..5], 16).unwrap();
    let b = u8::from_str_radix(&color[5..7], 16).unwrap();

    Ok(egui::Color32::from_rgb(r, g, b))
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
            track_ids.get_longest_common_prefix("charlotte_fullroadcourse"),
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
            Some(&"[Legacy] Charlotte Motor Speedway".to_string())
        );

        // Expectations for car name mapping.
        assert_eq!(
            config.cars.get("rt2000"),
            Some(&"Skip Barber Formula 2000".to_string())
        )
    }
}
