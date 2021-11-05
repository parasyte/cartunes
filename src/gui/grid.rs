use crate::setup::Setup;
use crate::str_ext::HumanCompare;
use epaint::Galley;
use std::cmp::Ordering;
use std::sync::Arc;

/// Provides structure for representing a grid of string values.
pub(crate) struct SetupGrid<'setup> {
    /// Column widths are provided here.
    columns: Vec<f32>,

    /// The grid contains zero or more groups.
    groups: Vec<Group<'setup>>,
}

/// A group containing a matrix of strings.
struct Group<'setup> {
    /// Group name is shown in a collapsible header.
    name: &'setup str,

    /// The matrix is row-major.
    ///
    /// I.e. the inner vector is a list of columns with the same length as `Grid::columns`.
    matrix: Vec<Vec<Label>>,
}

/// A label that can be displayed in a column.
struct Label {
    /// Diffs get a background color.
    background: Option<egui::Color32>,

    /// Container for the label text, style, and color.
    galley: Arc<Galley>,
}

impl<'setup> SetupGrid<'setup> {
    /// Create a new `SetupGrid` from a slice of `Setup`s.
    pub(crate) fn new(
        ui: &egui::Ui,
        setups: &'setup [&'setup Setup],
        colors: &[egui::Color32],
        diff_colors: (egui::Color32, egui::Color32),
    ) -> Self {
        // Gather groups
        let groups = setups
            .iter()
            .map(|inner| inner.keys().map(|s| s.as_str()).collect::<Vec<_>>());
        let groups = intersect_keys(groups);

        let column_count = setups.len() + 1;
        let mut output = Self {
            columns: Vec::with_capacity(column_count),
            groups: Vec::with_capacity(groups.len()),
        };
        output.columns.resize(column_count, 0.0);

        for prop_group in groups {
            // Gather property names
            let prop_names = setups.iter().map(|setup| {
                setup
                    .get(prop_group)
                    .unwrap()
                    .keys()
                    .map(|k| k.as_str())
                    .collect::<Vec<_>>()
            });
            let prop_names = intersect_keys(prop_names);

            let mut group = Group {
                name: prop_group,
                matrix: Vec::with_capacity(prop_names.len()),
            };

            for prop_name in prop_names {
                let mut i = 0;
                let mut columns = Vec::with_capacity(column_count);

                // Calculate width of `prop_name`
                let galley = ui.fonts().layout_no_wrap(
                    prop_name.to_string(),
                    egui::TextStyle::Body,
                    ui.visuals().text_color(),
                );
                let width = galley.rect.width() + ui.spacing().item_spacing.x * 5.0;
                output.columns[i] = output.columns[i].max(width);
                i += 1;

                columns.push(Label {
                    background: None,
                    galley,
                });

                let mut colors = colors.iter().cloned().cycle();
                let mut first_value: Option<String> = None;

                for setup in setups {
                    let values = setup.get(prop_group).unwrap().get_all(prop_name);
                    let separator = if values
                        .clone()
                        .all(|v| v.starts_with(|ch: char| ch.is_ascii_digit()))
                    {
                        ", "
                    } else {
                        " "
                    };
                    let value: String = values
                        .enumerate()
                        .map(|(i, v)| {
                            if i > 0 {
                                format!("{}{}", separator, v)
                            } else {
                                v.to_string()
                            }
                        })
                        .collect();

                    // Compute diff between `value` and first column
                    let color = colors.next().unwrap_or_else(|| ui.visuals().text_color());
                    let (color, background) = if let Some(first_value) = first_value.as_ref() {
                        match value.human_compare(first_value) {
                            Ordering::Less => (ui.visuals().text_color(), Some(diff_colors.0)),
                            Ordering::Greater => (ui.visuals().text_color(), Some(diff_colors.1)),
                            Ordering::Equal => (color, None),
                        }
                    } else {
                        first_value = Some(value.clone());
                        (color, None)
                    };

                    let galley = ui
                        .fonts()
                        .layout_no_wrap(value, egui::TextStyle::Body, color);
                    let width = galley.rect.width() + ui.spacing().item_spacing.x * 2.0;
                    output.columns[i] = output.columns[i].max(width);
                    i += 1;

                    columns.push(Label { background, galley });
                }

                group.matrix.push(columns);
            }

            output.groups.push(group);
        }

        output
    }

    /// Draw the grid to the provided `Ui`.
    pub(crate) fn show(self, ui: &mut egui::Ui, car_name: &str) {
        let column_widths = &self.columns;

        // Draw headers
        for prop_group in self.groups.into_iter() {
            egui::CollapsingHeader::new(prop_group.name)
                .id_source(format!("{}-{}", car_name, prop_group.name))
                .default_open(true)
                .show(ui, |ui| {
                    // Draw each row
                    for row in prop_group.matrix.into_iter() {
                        ui.horizontal(|ui| {
                            // Draw each column
                            for (i, label) in row.into_iter().enumerate() {
                                let size =
                                    egui::Vec2::new(column_widths[i], label.galley.rect.height());
                                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

                                // Draw optional background color
                                if let Some(background) = label.background {
                                    let fill = egui::Rgba::from(ui.visuals().code_bg_color);
                                    let background = egui::Rgba::from(background);
                                    let color = egui::Color32::from(background * fill);
                                    let rect = egui::Rect::from_min_size(
                                        rect.min,
                                        label.galley.rect.size(),
                                    );

                                    ui.painter().rect_filled(rect.expand(3.0), 4.0, color);
                                }

                                // Draw text
                                ui.painter().galley(rect.min, label.galley);
                            }
                        });
                    }
                });
        }
    }
}

/// Get the intersection of keys that exists in each `HashMap`.
fn intersect_keys<'a>(mut all_keys: impl Iterator<Item = Vec<&'a str>>) -> Vec<&'a str> {
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

    /// Test `intersect_keys()` with two sets.
    #[test]
    fn test_intersect_keys_two() {
        let expected = vec!["foo", "bar"];
        let list = vec![expected.clone(), expected.clone()];
        let keys = intersect_keys(list.into_iter());
        assert_eq!(keys, expected);
    }

    /// Test `intersect_keys()` with three sets.
    #[test]
    fn test_intersect_keys_three() {
        let expected = vec!["foo", "bar"];
        let list = vec![expected.clone(), expected.clone(), expected.clone()];
        let keys = intersect_keys(list.into_iter());
        assert_eq!(keys, expected);
    }

    /// Test `intersect_keys()` with four sets.
    #[test]
    fn test_intersect_keys_four() {
        let expected = vec!["foo", "bar"];
        let list = vec![
            expected.clone(),
            expected.clone(),
            expected.clone(),
            expected.clone(),
        ];
        let keys = intersect_keys(list.into_iter());
        assert_eq!(keys, expected);
    }

    /// Test `intersect_keys()` with a superset and a subset.
    ///
    /// The two maps are the same except "super" contains an additional key.
    #[test]
    fn test_intersect_keys_super_sub() {
        let expected = vec!["foo", "bar"];
        let subkeys = expected.clone();
        let mut superkeys = subkeys.clone();
        superkeys.push("baz");

        let list = vec![superkeys, subkeys];
        let keys = intersect_keys(list.into_iter());
        assert_eq!(keys, expected);
    }

    /// Test `intersect_keys()` with a subset and a superset.
    ///
    /// The two maps are the same except "super" contains an additional key.
    #[test]
    fn test_intersect_keys_sub_super() {
        let expected = vec!["foo", "bar"];
        let subkeys = expected.clone();
        let mut superkeys = subkeys.clone();
        superkeys.push("baz");

        let list = vec![subkeys, superkeys];
        let keys = intersect_keys(list.into_iter());
        assert_eq!(keys, expected);
    }

    /// Test `intersect_keys()` with sets that share only a few keys.
    #[test]
    fn test_intersect_keys_with_intersection() {
        let expected = vec!["foo", "bar"];
        let mut keys_a = expected.clone();
        let mut keys_b = expected.clone();
        keys_a.push("baz");
        keys_b.push("qux");

        let list = vec![keys_a, keys_b];
        let keys = intersect_keys(list.into_iter());
        assert_eq!(keys, expected);
    }

    /// Test `intersect_keys()` with sets that share no keys.
    #[test]
    fn test_intersect_keys_without_intersection() {
        let keys_a = vec!["foo", "bar"];
        let keys_b = vec!["baz", "qux"];

        let list = vec![keys_a, keys_b];
        let keys = intersect_keys(list.into_iter());
        assert!(keys.is_empty());
    }
}
