//! Parsers and internal representations for iRacing setup exports.

use crate::config::Config;
use crate::gui::ShowWarning;
use crate::str_ext::Capitalize;
use kuchiki::traits::TendrilSink;
use ordered_multimap::ListOrderedMultimap;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

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

/// Internal representation of a setup export.
///
/// The structure is a tree that can be described with this ASCII pictograph.
///
/// ```text
/// [Setups]
/// ├── "Concord Speedway"
/// │   ├── "VW Beetle"
/// │   │   └── ([FileName], [Setup])
/// │   └── "Skip Barber Formula 2000"
/// │       ├── ([FileName], [Setup])
/// │       ├── ([FileName], [Setup])
/// │       └── ([FileName], [Setup])
/// └── "Okayama International Raceway"
///     └── "VW Beetle"
///         ├── ([FileName], [Setup])
///         └── ([FileName], [Setup])
/// ```
///
/// The first layer of depth contains track names (human-readable), meaning that setups are sorted
/// first by the track they were exported from.
///
/// At the second layer of depth are the car names (human readable). Setups are also sorted by the
/// cars there were exported for.
///
/// Finally at the third level, each car has a list of `Setup` trees along with the file name that
/// it was loaded from (without the extension). Each car can have as many setups as needed.
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

type Tracks = HashMap<String, Cars>;
type Cars = HashMap<String, Vec<(String, Setup)>>;
pub(crate) type Setup = ListOrderedMultimap<String, Props>;
type Props = ListOrderedMultimap<String, String>;

impl Setups {
    /// Recursively load all HTML files from the config setup exports path into a `Setups` tree.
    pub(crate) fn new(warnings: &mut VecDeque<ShowWarning>, config: &Config) -> Self {
        // Check if a directory entry is an HTML file.
        fn is_html(entry: &DirEntry) -> bool {
            entry
                .file_name()
                .to_str()
                .map(|s| s.ends_with(".htm") || s.ends_with(".html"))
                .unwrap_or(false)
        }

        let mut setups = Self::default();
        let path = config.get_setups_path();
        let walker = WalkDir::new(path)
            .into_iter()
            .filter_entry(|entry| entry.file_type().is_dir() || is_html(entry));

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

        setups
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
        setups.push((file_name, setup));

        Ok(())
    }
}

/// Parse an HTML file into a `Setup`.
fn setup_from_html<P: AsRef<Path>>(
    path: P,
    config: &Config,
) -> Result<(String, String, Setup), Error> {
    let html = fs::read_to_string(&path).map_err(|err| Error::io(path, err))?;
    let document = kuchiki::parse_html().one(html.as_str());

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
        .replace(" ", "_");

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
        .replace(" ", "_");

    // Get the track unique identifier
    let track_id = config
        .track_ids()
        .get_longest_common_prefix(&track_id)
        .unwrap_or_else(|| track_id.as_bytes());
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
    let mut setup = Setup::default();
    for group in groups {
        let mut group_name = group.text_contents().capitalize_words().to_string();
        group_name.retain(|ch| ch != ':');

        let props = get_properties(group.as_node().next_sibling());

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

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalSize;

    fn create_ordered_multimap(list: &[(&str, &str)]) -> ListOrderedMultimap<String, String> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_load_dir() {
        let mut config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
        config.update_setups_path("./fixtures");
        let mut warnings = VecDeque::new();
        let setups = Setups::new(&mut warnings, &config);

        let cars = setups
            .tracks()
            .get("Centripetal Circuit")
            .unwrap()
            .get("Skip Barber Formula 2000")
            .unwrap();
        assert_eq!(cars.len(), 1);
        let (file_name, skip_barber) = &cars[0];
        assert_eq!(file_name, "skip_barber_centripetal");
        assert_eq!(skip_barber.keys().len(), 6);

        let cars = setups
            .tracks()
            .get("Charlotte Motor Speedway")
            .unwrap()
            .get("Global Mazda MX-5 Cup")
            .unwrap();
        assert_eq!(cars.len(), 1);
        let (file_name, mx5) = &cars[0];
        assert_eq!(file_name, "mx5_charlotte_legends_oval");
        assert_eq!(mx5.keys().len(), 6);

        let cars = setups
            .tracks()
            .get("Circuit des 24 Heures du Mans")
            .unwrap()
            .get("Dallara P217")
            .unwrap();
        assert_eq!(cars.len(), 1);
        let (file_name, dallara) = &cars[0];
        assert_eq!(file_name, "2021S2_ARA_LMP2_LeMans_V1");
        assert_eq!(dallara.keys().len(), 18);

        let cars = setups
            .tracks()
            .get("Nürburgring Combined")
            .unwrap()
            .get("Porsche 911 GT3 R")
            .unwrap();
        assert_eq!(cars.len(), 1);
        let (file_name, porche911) = &cars[0];
        assert_eq!(file_name, "baseline");
        assert_eq!(porche911.keys().len(), 12);

        assert_eq!(setups.tracks().len(), 4);
    }

    #[test]
    fn test_setup_skip_barber() {
        let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
        let (track_name, car_name, setup) =
            setup_from_html("./fixtures/skip_barber_centripetal.htm", &config).unwrap();

        assert_eq!(track_name, "Centripetal Circuit".to_string());
        assert_eq!(car_name, "Skip Barber Formula 2000".to_string());
        assert_eq!(setup.keys().len(), 6);

        // Front
        let expected = create_ordered_multimap(&[("Brake bias", "54%")]);
        let front = setup.get("Front").unwrap();
        assert_eq!(front, &expected);

        // Left Front
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "25.0 psi"),
            ("Last hot pressure", "25.0 psi"),
            ("Last temps O M I", "119F"),
            ("Last temps O M I", "119F"),
            ("Last temps O M I", "119F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "301 lbs"),
            ("Ride height", "1.95 in"),
            ("Spring perch offset", "5 x 1/16 in."),
            ("Camber", "-1.6 deg"),
            ("Caster", "+12.2 deg"),
        ]);
        let left_front = setup.get("Left Front").unwrap();
        assert_eq!(left_front, &expected);

        // Left Rear
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "25.0 psi"),
            ("Last hot pressure", "25.0 psi"),
            ("Last temps O M I", "119F"),
            ("Last temps O M I", "119F"),
            ("Last temps O M I", "119F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "438 lbs"),
            ("Ride height", "3.20 in"),
            ("Camber", "-2.1 deg"),
        ]);
        let left_rear = setup.get("Left Rear").unwrap();
        assert_eq!(left_rear, &expected);

        // Right Front
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "25.0 psi"),
            ("Last hot pressure", "25.0 psi"),
            ("Last temps I M O", "119F"),
            ("Last temps I M O", "119F"),
            ("Last temps I M O", "119F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "301 lbs"),
            ("Ride height", "1.95 in"),
            ("Spring perch offset", "5 x 1/16 in."),
            ("Camber", "-1.6 deg"),
            ("Caster", "+12.2 deg"),
        ]);
        let right_front = setup.get("Right Front").unwrap();
        assert_eq!(right_front, &expected);

        // Right Rear
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "25.0 psi"),
            ("Last hot pressure", "25.0 psi"),
            ("Last temps I M O", "119F"),
            ("Last temps I M O", "119F"),
            ("Last temps I M O", "119F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "438 lbs"),
            ("Ride height", "3.20 in"),
            ("Camber", "-2.1 deg"),
        ]);
        let right_rear = setup.get("Right Rear").unwrap();
        assert_eq!(right_rear, &expected);

        // Rear
        let expected =
            create_ordered_multimap(&[("Fuel level", "4.2 gal"), ("Anti-roll bar", "6")]);
        let rear = setup.get("Rear").unwrap();
        assert_eq!(rear, &expected);
    }

    #[test]
    fn test_setup_mx5() {
        let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
        let (track_name, car_name, setup) =
            setup_from_html("./fixtures/mx5_charlotte_legends_oval.htm", &config).unwrap();

        assert_eq!(track_name, "Charlotte Motor Speedway".to_string());
        assert_eq!(car_name, "Global Mazda MX-5 Cup".to_string());
        assert_eq!(setup.keys().len(), 6);

        // Front
        let expected = create_ordered_multimap(&[
            ("Toe-in", r#"-0/16""#),
            ("Cross weight", "50.0%"),
            ("Anti-roll bar", "Firm"),
        ]);
        let front = setup.get("Front").unwrap();
        assert_eq!(front, &expected);

        // Left Front
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "30.0 psi"),
            ("Last hot pressure", "30.0 psi"),
            ("Last temps O M I", "103F"),
            ("Last temps O M I", "103F"),
            ("Last temps O M I", "103F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "605 lbs"),
            ("Ride height", "4.83 in"),
            ("Spring perch offset", r#"2.563""#),
            ("Bump stiffness", "+10 clicks"),
            ("Rebound stiffness", "+8 clicks"),
            ("Camber", "-2.7 deg"),
        ]);
        let left_front = setup.get("Left Front").unwrap();
        assert_eq!(left_front, &expected);

        // Left Rear
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "30.0 psi"),
            ("Last hot pressure", "30.0 psi"),
            ("Last temps O M I", "103F"),
            ("Last temps O M I", "103F"),
            ("Last temps O M I", "103F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "540 lbs"),
            ("Ride height", "4.86 in"),
            ("Spring perch offset", r#"1.625""#),
            ("Bump stiffness", "+8 clicks"),
            ("Rebound stiffness", "+10 clicks"),
            ("Camber", "-2.7 deg"),
        ]);
        let left_rear = setup.get("Left Rear").unwrap();
        assert_eq!(left_rear, &expected);

        // Right Front
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "30.0 psi"),
            ("Last hot pressure", "30.0 psi"),
            ("Last temps I M O", "103F"),
            ("Last temps I M O", "103F"),
            ("Last temps I M O", "103F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "552 lbs"),
            ("Ride height", "4.84 in"),
            ("Spring perch offset", r#"2.781""#),
            ("Bump stiffness", "+10 clicks"),
            ("Rebound stiffness", "+8 clicks"),
            ("Camber", "-2.7 deg"),
        ]);
        let right_front = setup.get("Right Front").unwrap();
        assert_eq!(right_front, &expected);

        // Right Rear
        let expected = create_ordered_multimap(&[
            ("Cold pressure", "30.0 psi"),
            ("Last hot pressure", "30.0 psi"),
            ("Last temps I M O", "103F"),
            ("Last temps I M O", "103F"),
            ("Last temps I M O", "103F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Corner weight", "488 lbs"),
            ("Ride height", "4.87 in"),
            ("Spring perch offset", r#"1.844""#),
            ("Bump stiffness", "+8 clicks"),
            ("Rebound stiffness", "+10 clicks"),
            ("Camber", "-2.7 deg"),
        ]);
        let right_rear = setup.get("Right Rear").unwrap();
        assert_eq!(right_rear, &expected);

        // Rear
        let expected = create_ordered_multimap(&[
            ("Fuel level", "5.3 gal"),
            ("Toe-in", r#"+2/16""#),
            ("Anti-roll bar", "Unhooked"),
        ]);
        let rear = setup.get("Rear").unwrap();
        assert_eq!(rear, &expected);
    }

    #[test]
    fn test_setup_dallara_p217() {
        let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
        let (track_name, car_name, setup) =
            setup_from_html("./fixtures/2021S2_ARA_LMP2_LeMans_V1.htm", &config).unwrap();

        assert_eq!(track_name, "Circuit des 24 Heures du Mans".to_string());
        assert_eq!(car_name, "Dallara P217".to_string());
        assert_eq!(setup.keys().len(), 18);

        // Left Front Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.0 psi"),
            ("Last hot pressure", "22.0 psi"),
            ("Last temps O M I", "178F"),
            ("Last temps O M I", "182F"),
            ("Last temps O M I", "187F"),
            ("Tread remaining", "99%"),
            ("Tread remaining", "98%"),
            ("Tread remaining", "98%"),
        ]);
        let left_front_tire = setup.get("Left Front Tire").unwrap();
        assert_eq!(left_front_tire, &expected);

        // Left Rear Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.0 psi"),
            ("Last hot pressure", "22.3 psi"),
            ("Last temps O M I", "186F"),
            ("Last temps O M I", "196F"),
            ("Last temps O M I", "200F"),
            ("Tread remaining", "98%"),
            ("Tread remaining", "97%"),
            ("Tread remaining", "97%"),
        ]);
        let left_rear_tire = setup.get("Left Rear Tire").unwrap();
        assert_eq!(left_rear_tire, &expected);

        // Right Front Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.0 psi"),
            ("Last hot pressure", "21.8 psi"),
            ("Last temps I M O", "183F"),
            ("Last temps I M O", "179F"),
            ("Last temps I M O", "173F"),
            ("Tread remaining", "98%"),
            ("Tread remaining", "98%"),
            ("Tread remaining", "99%"),
        ]);
        let right_front_tire = setup.get("Right Front Tire").unwrap();
        assert_eq!(right_front_tire, &expected);

        // Right Rear Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.0 psi"),
            ("Last hot pressure", "22.1 psi"),
            ("Last temps I M O", "199F"),
            ("Last temps I M O", "195F"),
            ("Last temps I M O", "182F"),
            ("Tread remaining", "97%"),
            ("Tread remaining", "97%"),
            ("Tread remaining", "98%"),
        ]);
        let right_rear_tire = setup.get("Right Rear Tire").unwrap();
        assert_eq!(right_rear_tire, &expected);

        // Aero Settings
        let expected = create_ordered_multimap(&[
            ("Downforce trim", "Low"),
            ("Rear wing angle", "12 deg"),
            ("# of dive planes", "1"),
            ("Wing gurney setting", "Off"),
            ("Deck gurney setting", "Off"),
        ]);
        let aero_settings = setup.get("Aero Settings").unwrap();
        assert_eq!(aero_settings, &expected);

        // Aero Calculator
        let expected = create_ordered_multimap(&[
            ("Front RH at speed", r#"1.417""#),
            ("Rear RH at speed", r#"0.945""#),
            ("Downforce balance", "41.67%"),
            ("L/D", "4.991"),
        ]);
        let aero_calculator = setup.get("Aero Calculator").unwrap();
        assert_eq!(aero_calculator, &expected);

        // Front
        let expected = create_ordered_multimap(&[
            ("Third spring", "1143 lbs/in"),
            ("Third perch offset", r#"0.886""#),
            ("Third spring defl", "0.183 in"),
            ("Third spring defl", "of"),
            ("Third spring defl", "2.539 in"),
            ("Third slider defl", "0.975 in"),
            ("Third slider defl", "of"),
            ("Third slider defl", "3.937 in"),
            ("ARB size", "Medium"),
            ("ARB blades", "P4"),
            ("Toe-in", r#"-2/32""#),
            ("Third pin length", r#"7.480""#),
            ("Front pushrod length", r#"7.323""#),
            ("Power steering assist", "3"),
            ("Steering ratio", "11.0"),
            ("Display page", "Race1"),
        ]);
        let front = setup.get("Front").unwrap();
        assert_eq!(front, &expected);

        // Left Front
        let expected = create_ordered_multimap(&[
            ("Corner weight", "528 lbs"),
            ("Ride height", "1.771 in"),
            ("Shock defl", "0.612 in"),
            ("Shock defl", "of"),
            ("Shock defl", "1.969 in"),
            ("Torsion bar defl", "0.362 in"),
            ("Torsion bar turns", "2.750 Turns"),
            ("Torsion bar O.D.", "13.90 mm"),
            ("LS comp damping", "4 clicks"),
            ("HS comp damping", "3 clicks"),
            ("HS comp damp slope", "9 clicks"),
            ("LS rbd damping", "5 clicks"),
            ("HS rbd damping", "9 clicks"),
            ("Camber", "-2.5 deg"),
        ]);
        let left_front = setup.get("Left Front").unwrap();
        assert_eq!(left_front, &expected);

        // Left Rear
        let expected = create_ordered_multimap(&[
            ("Corner weight", "652 lbs"),
            ("Ride height", "1.748 in"),
            ("Shock defl", "1.478 in"),
            ("Shock defl", "of"),
            ("Shock defl", "2.953 in"),
            ("Spring defl", "0.604 in"),
            ("Spring defl", "of"),
            ("Spring defl", "3.525 in"),
            ("Spring perch offset", r#"1.969""#),
            ("Spring rate", "600 lbs/in"),
            ("LS comp damping", "5 clicks"),
            ("HS comp damping", "3 clicks"),
            ("HS comp damp slope", "9 clicks"),
            ("LS rbd damping", "8 clicks"),
            ("HS rbd damping", "9 clicks"),
            ("Camber", "-1.5 deg"),
            ("Toe-in", r#"+0/32""#),
        ]);
        let left_rear = setup.get("Left Rear").unwrap();
        assert_eq!(left_rear, &expected);

        // Right Front
        let expected = create_ordered_multimap(&[
            ("Corner weight", "528 lbs"),
            ("Ride height", "1.771 in"),
            ("Shock defl", "0.612 in"),
            ("Shock defl", "of"),
            ("Shock defl", "1.969 in"),
            ("Torsion bar defl", "0.362 in"),
            ("Torsion bar turns", "2.750 Turns"),
            ("Torsion bar O.D.", "13.90 mm"),
            ("LS comp damping", "4 clicks"),
            ("HS comp damping", "3 clicks"),
            ("HS comp damp slope", "9 clicks"),
            ("LS rbd damping", "5 clicks"),
            ("HS rbd damping", "9 clicks"),
            ("Camber", "-2.5 deg"),
        ]);
        let right_front = setup.get("Right Front").unwrap();
        assert_eq!(right_front, &expected);

        // Right Rear
        let expected = create_ordered_multimap(&[
            ("Corner weight", "652 lbs"),
            ("Ride height", "1.748 in"),
            ("Shock defl", "1.478 in"),
            ("Shock defl", "of"),
            ("Shock defl", "2.953 in"),
            ("Spring defl", "0.604 in"),
            ("Spring defl", "of"),
            ("Spring defl", "3.525 in"),
            ("Spring perch offset", r#"1.969""#),
            ("Spring rate", "600 lbs/in"),
            ("LS comp damping", "5 clicks"),
            ("HS comp damping", "3 clicks"),
            ("HS comp damp slope", "9 clicks"),
            ("LS rbd damping", "8 clicks"),
            ("HS rbd damping", "9 clicks"),
            ("Camber", "-1.5 deg"),
            ("Toe-in", r#"+0/32""#),
        ]);
        let right_rear = setup.get("Right Rear").unwrap();
        assert_eq!(right_rear, &expected);

        // Rear
        let expected = create_ordered_multimap(&[
            ("Third spring", "800 lbs/in"),
            ("Third perch offset", r#"1.358""#),
            ("Third spring defl", "0.247 in"),
            ("Third spring defl", "of"),
            ("Third spring defl", "3.266 in"),
            ("Third slider defl", "2.480 in"),
            ("Third slider defl", "of"),
            ("Third slider defl", "5.906 in"),
            ("ARB size", "Medium"),
            ("ARB blades", "P5"),
            ("Rear pushrod length", r#"6.516""#),
            ("Third pin length", r#"6.890""#),
            ("Cross weight", "50.0%"),
        ]);
        let rear = setup.get("Rear").unwrap();
        assert_eq!(rear, &expected);

        // Lighting
        let expected = create_ordered_multimap(&[("Roof ID light color", "Blue")]);
        let lighting = setup.get("Lighting").unwrap();
        assert_eq!(lighting, &expected);

        // Brake Spec
        let expected =
            create_ordered_multimap(&[("Pad compound", "High"), ("Brake pressure bias", "48.8%")]);
        let brake_spec = setup.get("Brake Spec").unwrap();
        assert_eq!(brake_spec, &expected);

        // Fuel
        let expected = create_ordered_multimap(&[("Fuel level", "19.8 gal")]);
        let fuel = setup.get("Fuel").unwrap();
        assert_eq!(fuel, &expected);

        // Traction Control
        let expected = create_ordered_multimap(&[
            ("Traction control gain", "3 (TC)"),
            ("Traction control slip", "2 (TC)"),
            ("Throttle shape", "1"),
        ]);
        let fuel = setup.get("Traction Control").unwrap();
        assert_eq!(fuel, &expected);

        // Gear Ratios
        let expected = create_ordered_multimap(&[
            ("Gear stack", "Tall"),
            ("Speed in first", "86.7 mph"),
            ("Speed in second", "112.1 mph"),
            ("Speed in third", "131.6 mph"),
            ("Speed in forth", "156.3 mph"),
            ("Speed in fifth", "182.7 mph"),
            ("Speed in sixth", "210.2 mph"),
        ]);
        let gear_ratios = setup.get("Gear Ratios").unwrap();
        assert_eq!(gear_ratios, &expected);

        // Rear Diff Spec
        let expected = create_ordered_multimap(&[
            ("Drive/coast ramp angles", "45/55"),
            ("Clutch friction faces", "10"),
            ("Preload", "81 ft-lbs"),
        ]);
        let rear_diff_spec = setup.get("Rear Diff Spec").unwrap();
        assert_eq!(rear_diff_spec, &expected);
    }

    #[test]
    fn test_setup_porche_911_gt3_r() {
        let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
        let (track_name, car_name, setup) =
            setup_from_html("./fixtures/baseline.htm", &config).unwrap();

        assert_eq!(track_name, "Nürburgring Combined".to_string());
        assert_eq!(car_name, "Porsche 911 GT3 R".to_string());
        assert_eq!(setup.keys().len(), 12);

        // Left Front Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.5 psi"),
            ("Last hot pressure", "20.5 psi"),
            ("Last temps O M I", "112F"),
            ("Last temps O M I", "112F"),
            ("Last temps O M I", "112F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
        ]);
        let left_front_tire = setup.get("Left Front Tire").unwrap();
        assert_eq!(left_front_tire, &expected);

        // Left Rear Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.5 psi"),
            ("Last hot pressure", "20.5 psi"),
            ("Last temps O M I", "112F"),
            ("Last temps O M I", "112F"),
            ("Last temps O M I", "112F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
        ]);
        let left_rear_tire = setup.get("Left Rear Tire").unwrap();
        assert_eq!(left_rear_tire, &expected);

        // Right Front Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.5 psi"),
            ("Last hot pressure", "20.5 psi"),
            ("Last temps I M O", "112F"),
            ("Last temps I M O", "112F"),
            ("Last temps I M O", "112F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
        ]);
        let right_front_tire = setup.get("Right Front Tire").unwrap();
        assert_eq!(right_front_tire, &expected);

        // Right Rear Tire
        let expected = create_ordered_multimap(&[
            ("Starting pressure", "20.5 psi"),
            ("Last hot pressure", "20.5 psi"),
            ("Last temps I M O", "112F"),
            ("Last temps I M O", "112F"),
            ("Last temps I M O", "112F"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
            ("Tread remaining", "100%"),
        ]);
        let right_rear_tire = setup.get("Right Rear Tire").unwrap();
        assert_eq!(right_rear_tire, &expected);

        // Aero Balance Calc
        let expected = create_ordered_multimap(&[
            ("Front RH at speed", r#"1.929""#),
            ("Rear RH at speed", r#"2.835""#),
            ("Wing setting", "7 degrees"),
            ("Front downforce", "39.83%"),
        ]);
        let aero_balance_calc = setup.get("Aero Balance Calc").unwrap();
        assert_eq!(aero_balance_calc, &expected);

        // Front
        let expected = create_ordered_multimap(&[
            ("ARB diameter", "45 mm"),
            ("ARB setting", "Soft"),
            ("Toe-in", r#"-2/32""#),
            ("Front master cyl.", "0.811 in"),
            ("Rear master cyl.", "0.811 in"),
            ("Brake pads", "Medium friction"),
            ("Fuel level", "15.9 gal"),
            ("Cross weight", "50.0%"),
        ]);
        let front = setup.get("Front").unwrap();
        assert_eq!(front, &expected);

        // Left Front
        let expected = create_ordered_multimap(&[
            ("Corner weight", "605 lbs"),
            ("Ride height", "2.034 in"),
            ("Spring perch offset", r#"2.441""#),
            ("Spring rate", "1371 lbs/in"),
            ("LS Comp damping", "-6 clicks"),
            ("HS Comp damping", "-10 clicks"),
            ("LS Rbd damping", "-8 clicks"),
            ("HS Rbd damping", "-10 clicks"),
            ("Camber", "-4.0 deg"),
            ("Caster", "+7.6 deg"),
        ]);
        let left_front = setup.get("Left Front").unwrap();
        assert_eq!(left_front, &expected);

        // Left Rear
        let expected = create_ordered_multimap(&[
            ("Corner weight", "945 lbs"),
            ("Ride height", "3.026 in"),
            ("Spring perch offset", r#"2.717""#),
            ("Spring rate", "1600 lbs/in"),
            ("LS Comp damping", "-6 clicks"),
            ("HS Comp damping", "-10 clicks"),
            ("LS Rbd damping", "-8 clicks"),
            ("HS Rbd damping", "-10 clicks"),
            ("Camber", "-3.4 deg"),
            ("Toe-in", r#"+1/64""#),
        ]);
        let left_rear = setup.get("Left Rear").unwrap();
        assert_eq!(left_rear, &expected);

        // In-Car Dials
        let expected = create_ordered_multimap(&[
            ("Display page", "Race 1"),
            ("Brake pressure bias", "54.0%"),
            ("Trac Ctrl (TCC) setting", "5 (TCC)"),
            ("Trac Ctrl (TCR) setting", "5 (TCR)"),
            ("Throttle Map setting", "4"),
            ("ABS setting", "11 (ABS)"),
            ("Engine map setting", "4 (MAP)"),
            ("Night LED strips", "Blue"),
        ]);
        let in_car_dials = setup.get("In-Car Dials").unwrap();
        assert_eq!(in_car_dials, &expected);

        // Right Front
        let expected = create_ordered_multimap(&[
            ("Corner weight", "605 lbs"),
            ("Ride height", "2.034 in"),
            ("Spring perch offset", r#"2.441""#),
            ("Spring rate", "1371 lbs/in"),
            ("LS Comp damping", "-6 clicks"),
            ("HS Comp damping", "-10 clicks"),
            ("LS Rbd damping", "-8 clicks"),
            ("HS Rbd damping", "-10 clicks"),
            ("Camber", "-4.0 deg"),
            ("Caster", "+7.6 deg"),
        ]);
        let right_front = setup.get("Right Front").unwrap();
        assert_eq!(right_front, &expected);

        // Right Rear
        let expected = create_ordered_multimap(&[
            ("Corner weight", "945 lbs"),
            ("Ride height", "3.026 in"),
            ("Spring perch offset", r#"2.717""#),
            ("Spring rate", "1600 lbs/in"),
            ("LS Comp damping", "-6 clicks"),
            ("HS Comp damping", "-10 clicks"),
            ("LS Rbd damping", "-8 clicks"),
            ("HS Rbd damping", "-10 clicks"),
            ("Camber", "-3.4 deg"),
            ("Toe-in", r#"+1/64""#),
        ]);
        let right_rear = setup.get("Right Rear").unwrap();
        assert_eq!(right_rear, &expected);

        // Rear
        let expected = create_ordered_multimap(&[
            ("ARB diameter", "35 mm"),
            ("ARB setting", "Med"),
            ("Diff preload", "74 ft-lbs"),
            ("Wing setting", "7 degrees"),
        ]);
        let rear = setup.get("Rear").unwrap();
        assert_eq!(rear, &expected);
    }
}
