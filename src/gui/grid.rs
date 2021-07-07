use crate::setup::Setup;
use std::collections::HashMap;

/// Draw all property groups and properties.
pub(crate) fn props_grid(ui: &mut egui::Ui, car_name: &str, setups: &[&Setup]) {
    // TODO: Colors

    // Gather headers
    let mut headers = intersect_keys(setups);
    headers.sort_unstable();

    // Draw headers
    for prop_group in headers {
        egui::CollapsingHeader::new(prop_group)
            .id_source(format!("{}-{}", car_name, prop_group))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new(format!("{}-grid", prop_group))
                    // .min_col_width(150.0)
                    .spacing(egui::Vec2::new(64.0, ui.spacing().item_spacing.y))
                    .show(ui, |ui| {
                        // Gather property names
                        let prop_names: Vec<_> = setups
                            .iter()
                            .map(|setup| setup.get(prop_group).unwrap())
                            .collect();
                        let mut prop_names = intersect_keys(&prop_names);
                        prop_names.sort_unstable();

                        for prop_name in prop_names {
                            ui.label(prop_name);

                            for setup in setups {
                                // ui.horizontal(|ui| {
                                //     let values =
                                //         setup.get(prop_group).unwrap().get(prop_name).unwrap();
                                //     for value in values {
                                //         ui.label(value);
                                //     }
                                // });
                                let values = setup
                                    .get(prop_group)
                                    .unwrap()
                                    .get(prop_name)
                                    .unwrap()
                                    .join(" ");
                                ui.label(values);
                            }
                            ui.end_row();
                        }
                    });
            });
    }
}

/// Get the intersection of keys that exists in each `HashMap`.
fn intersect_keys<'a, T>(hashmap: &'a [&'a HashMap<String, T>]) -> Vec<&'a str> {
    let mut all_keys = hashmap
        .iter()
        .map(|inner| inner.keys().map(|s| s.as_str()).collect());
    let mut output = if let Some(output) = all_keys.next() {
        output
    } else {
        Vec::new()
    };
    for keys in all_keys {
        output.retain(|key| keys.contains(key));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test `intersect_keys()` with two `HashMap`s.
    #[test]
    fn test_intersect_keys_two() {
        let mut map = HashMap::new();
        map.insert("foo".to_string(), ());
        map.insert("bar".to_string(), ());

        let maps = &[&map, &map];
        let keys = intersect_keys(maps);
        assert!(keys.contains(&"foo"));
        assert!(keys.contains(&"bar"));
        assert_eq!(keys.len(), 2);
    }

    /// Test `intersect_keys()` with three `HashMap`s.
    #[test]
    fn test_intersect_keys_three() {
        let mut map = HashMap::new();
        map.insert("foo".to_string(), ());
        map.insert("bar".to_string(), ());

        let maps = &[&map, &map, &map];
        let keys = intersect_keys(maps);
        assert!(keys.contains(&"foo"));
        assert!(keys.contains(&"bar"));
        assert_eq!(keys.len(), 2);
    }

    /// Test `intersect_keys()` with four `HashMap`s.
    #[test]
    fn test_intersect_keys_four() {
        let mut map = HashMap::new();
        map.insert("foo".to_string(), ());
        map.insert("bar".to_string(), ());

        let maps = &[&map, &map, &map, &map];
        let keys = intersect_keys(maps);
        assert!(keys.contains(&"foo"));
        assert!(keys.contains(&"bar"));
        assert_eq!(keys.len(), 2);
    }

    /// Test `intersect_keys()` with a "super" `HashMap` and a "sub" `HashMap`.
    ///
    /// The two maps are the same except "super" contains an additional key.
    #[test]
    fn test_intersect_keys_super_sub() {
        let mut sub_map = HashMap::new();
        sub_map.insert("foo".to_string(), ());
        sub_map.insert("bar".to_string(), ());
        let mut super_map = sub_map.clone();
        super_map.insert("baz".to_string(), ());

        let maps = &[&super_map, &sub_map];
        let keys = intersect_keys(maps);
        assert!(keys.contains(&"foo"));
        assert!(keys.contains(&"bar"));
        assert_eq!(keys.len(), 2);
    }

    /// Test `intersect_keys()` with a "sub" `HashMap` and a "super" `HashMap`.
    ///
    /// The two maps are the same except "super" contains an additional key.
    #[test]
    fn test_intersect_keys_sub_super() {
        let mut sub_map = HashMap::new();
        sub_map.insert("foo".to_string(), ());
        sub_map.insert("bar".to_string(), ());
        let mut super_map = sub_map.clone();
        super_map.insert("baz".to_string(), ());

        let maps = &[&sub_map, &super_map];
        let keys = intersect_keys(maps);
        assert!(keys.contains(&"foo"));
        assert!(keys.contains(&"bar"));
        assert_eq!(keys.len(), 2);
    }

    /// Test `intersect_keys()` with `HashMap`s that share only a few keys.
    #[test]
    fn test_intersect_keys_with_intersection() {
        let mut map_a = HashMap::new();
        map_a.insert("foo".to_string(), ());
        map_a.insert("bar".to_string(), ());
        let mut map_b = map_a.clone();
        map_a.insert("baz".to_string(), ());
        map_b.insert("quux".to_string(), ());

        let maps = &[&map_a, &map_b];
        let keys = intersect_keys(maps);
        assert!(keys.contains(&"foo"));
        assert!(keys.contains(&"bar"));
        assert_eq!(keys.len(), 2);
    }

    /// Test `intersect_keys()` with `HashMap`s that share no keys.
    #[test]
    fn test_intersect_keys_without_intersection() {
        let mut map_a = HashMap::new();
        map_a.insert("foo".to_string(), ());
        map_a.insert("bar".to_string(), ());
        let mut map_b = HashMap::new();
        map_b.insert("baz".to_string(), ());
        map_b.insert("quux".to_string(), ());

        let maps = &[&map_a, &map_b];
        let keys = intersect_keys(maps);
        assert_eq!(keys.len(), 0);
    }
}
