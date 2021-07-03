use crate::config::Config;
use crate::str_ext::Capitalize;
use kuchiki::traits::TendrilSink;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

// Parsing setup exports can fail.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// I/O error while reading export.
    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),

    /// Export is missing a page header.
    #[error("Missing page header")]
    MissingHeader,

    /// Export is missing a car identifier.
    #[error("Missing car identifier")]
    MissingCar,

    /// Export is missing a track identifier.
    #[error("Missing track identifier")]
    MissingTrack,
}

/// Internal representation of a setup export.
#[derive(Default)]
pub(crate) struct Setups(Tracks);

type Tracks = HashMap<String, Cars>;
type Cars = HashMap<String, Setup>;
type Setup = HashMap<String, Props>;
type Props = HashMap<String, Vec<String>>;

/// Parse an HTML file into a `Setup`.
fn setup_from_html<P: AsRef<Path>>(
    path: P,
    config: &Config,
) -> Result<(String, String, Setup), Error> {
    let html = fs::read_to_string(path)?;
    let document = kuchiki::parse_html().one(html.as_str());

    let text = document
        .select(r#"h2[align="center"]"#)
        .unwrap()
        .next()
        .ok_or(Error::MissingHeader)?
        .text_contents();

    let mut lines = text.lines().skip(1);

    // Get car name
    let car_id = lines
        .next()
        .ok_or(Error::MissingCar)?
        .trim()
        .split(" setup: ")
        .next()
        .ok_or(Error::MissingCar)?
        .replace(" ", "_");
    let car_name = config
        .cars
        .get(&car_id)
        .map_or(car_id, |name| name.to_string());

    // Get track name
    let track_id = lines
        .next()
        .ok_or(Error::MissingTrack)?
        .split_once(' ')
        .ok_or(Error::MissingTrack)?
        .1
        .trim()
        .replace(" ", "_");
    let track_id = config
        .track_ids
        .get_longest_common_prefix(&track_id)
        .unwrap(); // XXX
    let track_id = String::from_utf8_lossy(track_id).to_string();
    let track_name = config
        .tracks
        .get(&track_id)
        .map_or(track_id, |name| name.to_string());

    let mut setup = HashMap::new();

    // Get all group names
    let groups = document
        .select(r#"h2:not([align="center"])"#)
        .unwrap()
        .take_while(|node| {
            let text = node.text_contents().to_lowercase();
            !text.starts_with("notes") && !text.starts_with("driver aids")
        });

    for group in groups {
        let mut group_name = group.text_contents().capitalize_words().to_string();
        group_name.retain(|ch| ch != ':');
        let props = setup.entry(group_name).or_default();

        consume_properties(props, group.as_node().next_sibling());
    }

    Ok((track_name, car_name, setup))
}

fn consume_properties(map: &mut Props, mut node_ref: Option<kuchiki::NodeRef>) {
    let mut last_was_br = false;
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
                values.push(node.text_contents());
            }
            last_was_br = &element.name.local == "br";
        } else {
            // The node is text

            // Move any existing values to the map
            if !name.is_empty() && !values.is_empty() {
                map.insert(name, values.drain(..).collect());
            }

            // This is the property name
            name = node.text_contents().trim().to_string();
            name.retain(|ch| ch != ':');
        }
        node_ref = node.next_sibling();
    }

    if !name.is_empty() && !values.is_empty() {
        map.insert(name, values);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalSize;

    #[test]
    fn test_setup_skip_barber() {
        let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
        let (track_name, car_name, setup) =
            setup_from_html("./fixtures/skip_barber_centripetal.htm", &config).unwrap();

        assert_eq!(track_name, "Centripetal Circuit".to_string());
        assert_eq!(car_name, "Skip Barber Formula 2000".to_string());

        // Front
        let front = setup.get("Front").unwrap();
        assert_eq!(front.get("Brake bias").unwrap(), &vec!["54%".to_string()]);

        // Left Front
        let expected = [
            ("Cold pressure", vec!["25.0 psi"]),
            ("Last hot pressure", vec!["25.0 psi"]),
            ("Last temps O M I", vec!["119F", "119F", "119F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["301 lbs"]),
            ("Ride height", vec!["1.95 in"]),
            ("Spring perch offset", vec!["5 x 1/16 in."]),
            ("Camber", vec!["-1.6 deg"]),
            ("Caster", vec!["+12.2 deg"]),
        ];
        let left_front = setup.get("Left Front").unwrap();
        for expected in &expected {
            let actual = left_front.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Left Rear
        let expected = [
            ("Cold pressure", vec!["25.0 psi"]),
            ("Last hot pressure", vec!["25.0 psi"]),
            ("Last temps O M I", vec!["119F", "119F", "119F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["438 lbs"]),
            ("Ride height", vec!["3.20 in"]),
            ("Camber", vec!["-2.1 deg"]),
        ];
        let left_rear = setup.get("Left Rear").unwrap();
        for expected in &expected {
            let actual = left_rear.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Right Front
        let expected = [
            ("Cold pressure", vec!["25.0 psi"]),
            ("Last hot pressure", vec!["25.0 psi"]),
            ("Last temps I M O", vec!["119F", "119F", "119F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["301 lbs"]),
            ("Ride height", vec!["1.95 in"]),
            ("Spring perch offset", vec!["5 x 1/16 in."]),
            ("Camber", vec!["-1.6 deg"]),
            ("Caster", vec!["+12.2 deg"]),
        ];
        let left_front = setup.get("Right Front").unwrap();
        for expected in &expected {
            let actual = left_front.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Right Rear
        let expected = [
            ("Cold pressure", vec!["25.0 psi"]),
            ("Last hot pressure", vec!["25.0 psi"]),
            ("Last temps I M O", vec!["119F", "119F", "119F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["438 lbs"]),
            ("Ride height", vec!["3.20 in"]),
            ("Camber", vec!["-2.1 deg"]),
        ];
        let left_rear = setup.get("Right Rear").unwrap();
        for expected in &expected {
            let actual = left_rear.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Rear
        let front = setup.get("Rear").unwrap();
        assert_eq!(
            front.get("Fuel level").unwrap(),
            &vec!["4.2 gal".to_string()]
        );
        assert_eq!(front.get("Anti-roll bar").unwrap(), &vec!["6".to_string()]);
    }

    #[test]
    fn test_setup_mx5() {
        let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
        let (track_name, car_name, setup) =
            setup_from_html("./fixtures/mx5_charlotte_legends_oval.htm", &config).unwrap();

        assert_eq!(track_name, "Charlotte Motor Speedway".to_string());
        assert_eq!(car_name, "Global Mazda MX-5 Cup".to_string());

        // Front
        let front = setup.get("Front").unwrap();
        assert_eq!(front.get("Toe-in").unwrap(), &vec![r#"-0/16""#.to_string()]);
        assert_eq!(
            front.get("Cross weight").unwrap(),
            &vec!["50.0%".to_string()]
        );
        assert_eq!(
            front.get("Anti-roll bar").unwrap(),
            &vec!["Firm".to_string()]
        );

        // Left Front
        let expected = [
            ("Cold pressure", vec!["30.0 psi"]),
            ("Last hot pressure", vec!["30.0 psi"]),
            ("Last temps O M I", vec!["103F", "103F", "103F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["605 lbs"]),
            ("Ride height", vec!["4.83 in"]),
            ("Spring perch offset", vec![r#"2.563""#]),
            ("Bump stiffness", vec!["+10 clicks"]),
            ("Rebound stiffness", vec!["+8 clicks"]),
            ("Camber", vec!["-2.7 deg"]),
        ];
        let left_front = setup.get("Left Front").unwrap();
        for expected in &expected {
            let actual = left_front.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Left Rear
        let expected = [
            ("Cold pressure", vec!["30.0 psi"]),
            ("Last hot pressure", vec!["30.0 psi"]),
            ("Last temps O M I", vec!["103F", "103F", "103F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["540 lbs"]),
            ("Ride height", vec!["4.86 in"]),
            ("Spring perch offset", vec![r#"1.625""#]),
            ("Bump stiffness", vec!["+8 clicks"]),
            ("Rebound stiffness", vec!["+10 clicks"]),
            ("Camber", vec!["-2.7 deg"]),
        ];
        let left_rear = setup.get("Left Rear").unwrap();
        for expected in &expected {
            let actual = left_rear.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Right Front
        let expected = [
            ("Cold pressure", vec!["30.0 psi"]),
            ("Last hot pressure", vec!["30.0 psi"]),
            ("Last temps I M O", vec!["103F", "103F", "103F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["552 lbs"]),
            ("Ride height", vec!["4.84 in"]),
            ("Spring perch offset", vec![r#"2.781""#]),
            ("Bump stiffness", vec!["+10 clicks"]),
            ("Rebound stiffness", vec!["+8 clicks"]),
            ("Camber", vec!["-2.7 deg"]),
        ];
        let left_front = setup.get("Right Front").unwrap();
        for expected in &expected {
            let actual = left_front.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Right Rear
        let expected = [
            ("Cold pressure", vec!["30.0 psi"]),
            ("Last hot pressure", vec!["30.0 psi"]),
            ("Last temps I M O", vec!["103F", "103F", "103F"]),
            ("Tread remaining", vec!["100%", "100%", "100%"]),
            ("Corner weight", vec!["488 lbs"]),
            ("Ride height", vec!["4.87 in"]),
            ("Spring perch offset", vec![r#"1.844""#]),
            ("Bump stiffness", vec!["+8 clicks"]),
            ("Rebound stiffness", vec!["+10 clicks"]),
            ("Camber", vec!["-2.7 deg"]),
        ];
        let left_rear = setup.get("Right Rear").unwrap();
        for expected in &expected {
            let actual = left_rear.get(expected.0).unwrap();
            let expected: Vec<_> = expected.1.iter().map(|s| s.to_string()).collect();

            assert_eq!(actual, &expected);
        }

        // Rear
        let front = setup.get("Rear").unwrap();
        assert_eq!(
            front.get("Fuel level").unwrap(),
            &vec!["5.3 gal".to_string()]
        );
        assert_eq!(front.get("Toe-in").unwrap(), &vec![r#"+2/16""#.to_string()]);
        assert_eq!(
            front.get("Anti-roll bar").unwrap(),
            &vec!["Unhooked".to_string()]
        );
    }
}
