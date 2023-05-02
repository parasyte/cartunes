//! Parsers and internal representations for iRacing setup exports.

use crate::config::Config;
use crate::gui::ShowWarning;
use crate::str_ext::{Capitalize, HumanCompare};
use kuchiki::traits::TendrilSink;
use ordered_multimap::ListOrderedMultimap;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[cfg(test)]
mod tests;

// Parsing setup exports can fail.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// I/O error while reading export.
    #[error("I/O Error while reading {0:?}: {1}")]
    Io(PathBuf, #[source] std::io::Error),

    /// Export is missing a page header.
    #[error("Missing page header")]
    MissingHeader,

    /// Export is missing a car identifier.
    #[error("Missing car identifier")]
    MissingCar,

    /// Export is missing a track identifier.
    #[error("Missing track identifier")]
    MissingTrack,

    /// Export has duplicate property group.
    #[error("Duplicate property group: {0}")]
    DuplicatePropGroup(String),
}

impl Error {
    /// Shortcut to create an I/O Error.
    fn io<P: AsRef<Path>>(path: P, err: std::io::Error) -> Self {
        Self::Io(path.as_ref().to_path_buf(), err)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum UpdateKind {
    /// A setup has been added; the track name, car name, and index are provided.
    AddedSetup(String, String, usize),

    /// A setup has been removed; the track name, car name, and index are provided.
    RemovedSetup(String, String, usize),

    /// A car has been removed; the track name and car name are provided.
    RemovedCar(String, String),

    /// A track has been removed; the track name is provided.
    RemovedTrack(String),
}

/// Internal representation of a setup export.
///
/// The structure is a tree that can be described with this ASCII pictograph.
///
/// ```text
/// [Setups]
/// ├── "Concord Speedway"
/// │   ├── "VW Beetle"
/// │   │   └── SetupInfo
/// │   └── "Skip Barber Formula 2000"
/// │       ├── SetupInfo
/// │       ├── SetupInfo
/// │       └── SetupInfo
/// └── "Okayama International Raceway"
///     └── "VW Beetle"
///         ├── SetupInfo
///         └── SetupInfo
/// ```
///
/// The first layer of depth contains track names (human-readable), meaning that setups are sorted
/// first by the track they were exported from.
///
/// At the second layer of depth are the car names (human readable). Setups are also sorted by the
/// cars there were exported for.
///
/// Finally at the third level, each car has a list of [`SetupInfo`] headers, which contains a
/// `Setup` tree along with the file name that it was loaded from (without the extension) and the
/// full file path. Each car can have as many setups as needed.
///
/// The `Setup` type is similarly an alias for a deeply nested `HashMap` representing a single
/// instance of a car setup.
///
/// ```text
/// [Setup]
/// ├── "Front"
/// │   └── "Brake bias"
/// │       └── "54%"
/// ├── "Left Front"
/// │   ├── "Cold pressure"
/// │   │   └── "25.0 psi"
/// │   ├── "Last temps O M I"
/// │   │   ├── "119F"
/// │   │   ├── "119F"
/// │   │   └── "119F"
/// │   └── "Tread remaining"
/// │       ├── "100%"
/// │       ├── "100%"
/// │       └── "100%"
/// └── "Rear"
///     └── "Fuel level"
///         └── "4.2 gal"
/// ```
///
/// The first layer of depth in each `Setup` contains the property groups. These groups typically
/// reference general zones of the vehicle, such as "Front" and "Right Rear". Each car has unique
/// property group names defined in the setup export.
///
/// Under each property group is a list of property names. Each car has unique property names
/// defined in the setup export.
///
/// At the lowest layer, the leaf nodes contain a list of property values. A property with a single
/// value is still technically a list with a single element. Multiple values are used in cases where
/// the setup export describes things like tire temperature, which measures the <u>O</u>uter,
/// <u>M</u>iddle, and <u>I</u>nner temperatures across the tread respectively.
///
/// See the [iRacing User Manuals](https://www.iracing.com/user-manuals/) for technical details of
/// individual setup properties for each car.
#[derive(Default)]
pub(crate) struct Setups {
    tracks: Tracks,
}

/// Information about a setup
pub(crate) struct SetupInfo {
    /// The setup data.
    setup: Setup,
    /// Name of the setup (the filename without extension).
    name: String,
    /// Full file path for setup.
    path: PathBuf,
}

type Tracks = HashMap<String, Cars>;
type Cars = HashMap<String, Vec<SetupInfo>>;
pub(crate) type Setup = ListOrderedMultimap<String, Props>;
type Props = ListOrderedMultimap<String, String>;

impl Setups {
    /// Recursively load all HTML files from the config setup exports path into a `Setups` tree.
    pub(crate) fn new(warnings: &mut VecDeque<ShowWarning>, config: &Config) -> Self {
        let mut setups = Self::default();
        let path = config.get_setups_path();
        let walker = WalkDir::new(path).into_iter().filter_entry(|entry| {
            entry.file_type().is_dir() || is_html(entry.file_name().to_str())
        });

        for entry in walker {
            match entry {
                Err(err) => {
                    warnings.push_front(ShowWarning::new(
                        err,
                        "Encountered an error while looking for all exports.",
                    ));
                }
                Ok(entry) => {
                    if entry.file_type().is_file() {
                        if let Err(err) = setups.load_file(entry.path(), config) {
                            warnings.push_front(ShowWarning::new(
                                err,
                                format!(
                                    "Error while loading HTML setup export `{}`.",
                                    entry.path().to_string_lossy(),
                                ),
                            ));
                        }
                    }
                }
            }
        }

        // Sort `SetupInfo`s by name.
        for track in setups.tracks.values_mut() {
            for setups in track.values_mut() {
                setups.sort_by(|a, b| a.name().human_compare(b.name()));
            }
        }

        setups
    }

    /// Update setups when the file system changes.
    pub(crate) fn update(&mut self, event: &hotwatch::Event, config: &Config) -> Vec<UpdateKind> {
        use hotwatch::Event::*;

        let mut result = Vec::new();

        match event {
            Create(path) | Write(path) => {
                if path.is_file() && is_html(path.as_path().to_str()) {
                    self.add(&mut result, path, None, config);
                }
            }
            Remove(path) => {
                if is_html(path.as_path().to_str()) {
                    self.remove(&mut result, path);
                }
            }
            Rename(from, to) => {
                let old_name_is_html = is_html(from.as_path().to_str());
                let new_name_is_html = to.is_file() && is_html(to.as_path().to_str());

                if old_name_is_html && !new_name_is_html {
                    self.remove(&mut result, from);
                } else if new_name_is_html {
                    self.add(&mut result, to, Some(from), config);
                }
            }
            _ => (),
        }

        result
    }

    /// Add a path to the setup tree or replace an existing entry.
    fn add(
        &mut self,
        result: &mut Vec<UpdateKind>,
        path: &Path,
        old_path: Option<&Path>,
        config: &Config,
    ) {
        if let Ok((track_name, car_name, setup)) = setup_from_html(path, config) {
            let file_name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| car_name.clone());
            let cars = self.tracks.entry(track_name.clone()).or_default();
            let setups = cars.entry(car_name.clone()).or_default();

            // Find an existing SetupInfo by path
            let index = setups.iter().enumerate().find_map(|(i, setup_info)| {
                Some(i).filter(|_| {
                    setup_info.path == path || Some(setup_info.path.as_path()) == old_path
                })
            });

            if let Some(index) = index {
                // Special handling for replacements
                setups[index] = SetupInfo::new(setup, file_name, path);
            } else {
                // Find the index where the setup should be inserted
                let index = setups.partition_point(|setup_info| setup_info.name < file_name);
                setups.insert(index, SetupInfo::new(setup, file_name, path));

                // Only emit `AddedSetups` when adding a new entry
                result.push(UpdateKind::AddedSetup(track_name, car_name, index));
            }
        }
    }

    /// Remove a path from the setup tree.
    fn remove(&mut self, result: &mut Vec<UpdateKind>, path: &Path) {
        let mut remove_track = None;

        for (track_name, track) in self.tracks.iter_mut() {
            let mut remove_car = None;

            for (car_name, setups) in track.iter_mut() {
                // Find the SetupInfo by path
                let index = setups
                    .iter()
                    .enumerate()
                    .find_map(|(i, setup_info)| Some(i).filter(|_| setup_info.path == path));

                if let Some(index) = index {
                    // Remove the `SetupInfo` for the removed path
                    setups.remove(index);

                    // Record the removed setup
                    result.push(UpdateKind::RemovedSetup(
                        track_name.to_string(),
                        car_name.to_string(),
                        index,
                    ));

                    if setups.is_empty() {
                        // Record the removed car
                        result.push(UpdateKind::RemovedCar(
                            track_name.to_string(),
                            car_name.to_string(),
                        ));

                        remove_car = Some(car_name.to_string());
                    }
                }
            }

            if let Some(car_name) = remove_car {
                track.remove(&car_name);
            }

            if track.is_empty() {
                // Record the removed track
                result.push(UpdateKind::RemovedTrack(track_name.to_string()));

                remove_track = Some(track_name.to_string());
            }
        }

        if let Some(track_name) = remove_track {
            self.tracks.remove(&track_name);
        }
    }

    /// Get a reference to the tracks tree.
    pub(crate) fn tracks(&self) -> &Tracks {
        &self.tracks
    }

    /// Load an HTML export file into the `Setups` tree.
    fn load_file<P: AsRef<Path>>(&mut self, path: P, config: &Config) -> Result<(), Error> {
        let (track_name, car_name, setup) = setup_from_html(&path, config)?;

        let file_name = path
            .as_ref()
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| car_name.clone());
        let cars = self.tracks.entry(track_name).or_default();
        let setups = cars.entry(car_name).or_default();
        setups.push(SetupInfo::new(setup, file_name, path));

        Ok(())
    }
}

impl SetupInfo {
    /// Create a new `SetupInfo` descriptor.
    pub(crate) fn new<P: AsRef<Path>>(setup: Setup, name: String, path: P) -> Self {
        let path = path.as_ref().to_path_buf();

        Self { setup, name, path }
    }

    /// Get a reference to the inner [`Setup`].
    pub(crate) fn setup(&self) -> &Setup {
        &self.setup
    }

    /// Get a reference to the name.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

// Check if a directory entry is an HTML file.
fn is_html(file_name: Option<&str>) -> bool {
    file_name
        .map(|s| s.ends_with(".htm") || s.ends_with(".html"))
        .unwrap_or(false)
}

/// Parse an HTML file into a `Setup`.
fn setup_from_html<P: AsRef<Path>>(
    path: P,
    config: &Config,
) -> Result<(String, String, Setup), Error> {
    let bytes = fs::read(&path).map_err(|err| Error::io(path, err))?;
    let html = encoding_rs::mem::decode_latin1(&bytes);
    let document = kuchiki::parse_html().one(html.as_ref());

    // Find the document header and gather its text contents
    let text = document
        .select(r#"h2[align="center"]"#)
        .unwrap()
        .next()
        .ok_or(Error::MissingHeader)?
        .text_contents();

    let mut lines = text.lines().skip(1);

    // Get the car unique identifier
    let car_id = lines
        .next()
        .ok_or(Error::MissingCar)?
        .trim()
        .split(" setup: ")
        .next()
        .ok_or(Error::MissingCar)?
        .replace(' ', "_");

    // Map car ID to a human-readable name
    let car_name = config
        .cars()
        .get(&car_id)
        .map_or(car_id, |name| name.to_string());

    // Get the track ambiguous identifier
    let track_id = lines
        .next()
        .ok_or(Error::MissingTrack)?
        .split_once(' ')
        .ok_or(Error::MissingTrack)?
        .1
        .trim()
        .replace(' ', "_");

    // Get the track unique identifier
    let track_id = config
        .track_ids()
        .get_longest_common_prefix(&track_id)
        .unwrap_or(track_id.as_bytes());
    let track_id = String::from_utf8_lossy(track_id).to_string();

    // Map track ID to a human-readable name
    let track_name = config
        .tracks()
        .get(&track_id)
        .map_or(track_id, |name| name.to_string());

    // Get all property groups
    let groups = document
        .select(r#"h2:not([align="center"])"#)
        .unwrap()
        .take_while(|node| {
            let text = node.text_contents().to_lowercase();
            !text.starts_with("notes") && !text.starts_with("driver aids")
        });

    // Populate the Setup
    let mut group_name = String::new();
    let mut setup = Setup::default();
    for group in groups {
        let props = get_properties(group.as_node().next_sibling());

        // Get the name of the first group following one with properties
        if group_name.is_empty() {
            group_name = group.text_contents().capitalize_words().to_string();
            group_name.retain(|ch| ch != ':');
        }

        // Skip remaining groups until properties are found
        if props.is_empty() {
            continue;
        }

        // Heuristic that determines whether the group corresponds to a tire
        if props.keys().any(|k| k.starts_with("Tread"))
            && !props.keys().any(|k| {
                k == "Camber"
                    || k == "Caster"
                    || k == "Ride height"
                    || k == "Corner weight"
                    || k.starts_with("Spring")
            })
            && !group_name.ends_with("Tire")
        {
            group_name += " Tire";
        }

        if setup.insert(group_name.clone(), props).is_some() {
            return Err(Error::DuplicatePropGroup(group_name));
        }

        // Clear the last known group name so it can be recreated when needed
        group_name.clear();
    }

    Ok((track_name, car_name, setup))
}

fn get_properties(mut node_ref: Option<kuchiki::NodeRef>) -> Props {
    let mut last_was_br = false;
    let mut map = Props::default();
    let mut name = String::new();
    let mut values = Vec::new();

    while let Some(ref node) = node_ref {
        if let Some(element) = node.as_element() {
            // The node is an element
            if &element.name.local == "br" {
                // Early return when it's a <br> following a <br>
                if last_was_br {
                    break;
                }
            } else {
                // Early return when the name is empty
                if name.is_empty() {
                    break;
                }

                // This is a property value
                values.push(node.text_contents().trim().to_string());
            }
            last_was_br = &element.name.local == "br";
        } else {
            // The node is text
            let text = node.text_contents();
            let text = text.trim();
            if let Some(text) = text.strip_suffix(':') {
                // Move any existing values to the map
                if !name.is_empty() {
                    for value in values.drain(..) {
                        map.append(name.clone(), value);
                    }
                }

                // This is the property name
                name = text.to_string();
            } else {
                // This is a continuation of a property value
                values.push(text.to_string());
            }
        }
        node_ref = node.next_sibling();
    }

    if !name.is_empty() {
        for value in values.into_iter() {
            map.append(name.clone(), value);
        }
    }

    map
}
