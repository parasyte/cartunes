//! User interface structure, rendering, and state management.

use self::grid::SetupGrid;
use crate::config::{Config, UserTheme};
use crate::framework::UserEvent;
use crate::setup::{Setup, Setups};
use crate::str_ext::{Ellipsis, HumanCompare};
use crate::updates::{UpdateFrequency, UpdateNotification};
use copypasta::{ClipboardContext, ClipboardProvider};
use egui::widgets::color_picker::{color_edit_button_srgba, Alpha};
use egui::{CtxRef, Widget};
use hotwatch::Hotwatch;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::{Duration, Instant};
use thiserror::Error;
use winit::event_loop::EventLoopProxy;

mod grid;

/// Manages all state required for rendering the GUI.
pub(crate) struct Gui {
    /// Application configuration.
    pub(crate) config: Config,

    /// A tree of `Setups` containing all known setup exports.
    setups: Setups,

    /// Filesystem watcher for changes to any setup exports.
    hotwatch: Hotwatch,

    /// Selected track name.
    selected_track_name: Option<String>,

    /// Selected car name.
    selected_car_name: Option<String>,

    /// Selected setup indices.
    selected_setups: Vec<usize>,

    /// An event loop proxy for sending user events.
    event_loop_proxy: EventLoopProxy<UserEvent>,

    /// Show the "About..." window.
    about: bool,

    /// Show the "Preferences..." window.
    preferences: bool,

    /// Show the "Warning" window.
    warning: bool,

    /// Show the "Update Notification" window.
    update_notification: bool,

    /// Show an error message.
    show_errors: VecDeque<ShowError>,

    /// Show a warning message.
    show_warnings: VecDeque<ShowWarning>,

    /// Show an update notification message.
    show_update_notification: Option<UpdateNotification>,

    /// Show a tooltip.
    show_tooltips: HashMap<egui::Id, (String, Instant)>,
}

/// Holds state for an error message to show to the user, and provides a feedback mechanism for the
/// user to make a decision on how to handle the error.
pub(crate) struct ShowError {
    /// The actual error message.
    error: Box<dyn std::error::Error>,

    /// Provide some extra context to the user.
    context: String,

    /// Actions that the user may take to handle the error.
    buttons: (ErrorButton, ErrorButton),
}

/// Descriptor for a button used by the error window.
pub(crate) struct ErrorButton {
    /// Text to show on the button.
    label: String,

    /// An action to perform when the button is pressed.
    action: Box<dyn FnOnce()>,
}

/// Holds state for a warning message to show to the user.
pub(crate) struct ShowWarning {
    /// The actual warning message.
    warning: Box<dyn std::error::Error>,

    /// Provide some extra context to the user.
    context: String,
}

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("File system watch error: {0}")]
    Notify(#[from] hotwatch::Error),
}

impl Gui {
    /// Create a GUI.
    pub(crate) fn new(
        config: Config,
        setups: Setups,
        event_loop_proxy: EventLoopProxy<UserEvent>,
        show_errors: VecDeque<ShowError>,
        show_warnings: VecDeque<ShowWarning>,
    ) -> Result<Self, Error> {
        let mut hotwatch = Hotwatch::new()?;
        let watcher = Self::watch_setups_path(event_loop_proxy.clone());

        hotwatch.watch(config.get_setups_path(), watcher)?;

        Ok(Self {
            config,
            setups,
            hotwatch,
            selected_track_name: None,
            selected_car_name: None,
            selected_setups: Vec::new(),
            event_loop_proxy,
            about: false,
            preferences: false,
            warning: false,
            update_notification: false,
            show_errors,
            show_warnings,
            show_update_notification: None,
            show_tooltips: HashMap::new(),
        })
    }

    /// Draw the UI using egui.
    pub(crate) fn ui(&mut self, ctx: &egui::CtxRef, window: &winit::window::Window) {
        // Show an error message (if any) in a modal window by disabling the rest of the UI.
        let enabled = self.error_window(ctx);

        // Draw the menu bar
        egui::TopBottomPanel::top("menubar-container").show(ctx, |ui| {
            ui.set_enabled(enabled);
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.set_min_width(200.0);
                    if ui.button("Preferences").clicked() {
                        ui.close_menu();
                        self.preferences = true;
                    }
                });
                ui.menu_button("Help", |ui| {
                    ui.set_min_width(200.0);
                    if ui.button("About CarTunes...").clicked() {
                        ui.close_menu();
                        self.about = true;
                    }
                    if ui.button("Support CarTunes on Patreon").clicked() {
                        ui.close_menu();
                        if let Err(err) = webbrowser::open("https://www.patreon.com/blipjoy") {
                            let warning = ShowWarning::new(err, "Unable to open web browser.");
                            self.show_warnings.push_front(warning);
                        }
                    }
                });
            });
        });

        // Draw the footer
        if self.show_update_notification.is_some() || !self.show_warnings.is_empty() {
            egui::TopBottomPanel::bottom("footer-container").show(ctx, |ui| {
                if self.show_update_notification.is_some() {
                    let rect = ui
                        .horizontal(|ui| {
                            let rect = ui.available_rect_before_wrap();
                            let size = ui.spacing().interact_size.y;
                            let center = egui::Vec2::splat(size / 2.0);
                            let green = egui::Color32::from_rgb(40, 210, 40);

                            ui.spacing_mut().item_spacing.x /= 2.0;
                            ui.painter()
                                .circle_filled(rect.min + center, center.x - 3.0, green);
                            ui.add_space(size);
                            ui.label("Update available");
                            ui.add_space(0.0);
                        })
                        .response
                        .rect;
                    let response = ui.interact(
                        rect,
                        egui::Id::new("update-notification-button"),
                        egui::Sense::click(),
                    );

                    if response.hovered() {
                        let hovered = ui.visuals().widgets.hovered;
                        ui.painter()
                            .rect_stroke(rect, hovered.corner_radius, hovered.bg_stroke);
                    }
                    if response.clicked() {
                        self.update_notification = true;
                    }
                }
                if !self.show_warnings.is_empty() {
                    let rect = ui
                        .horizontal(|ui| {
                            let rect = ui.available_rect_before_wrap();
                            let size = ui.spacing().interact_size.y;
                            let center = egui::Vec2::splat(size / 2.0);
                            let yellow = egui::Color32::from_rgb(210, 210, 40);
                            let len = self.show_warnings.len();

                            ui.spacing_mut().item_spacing.x /= 2.0;
                            ui.painter()
                                .circle_filled(rect.min + center, center.x - 3.0, yellow);
                            ui.add_space(size);
                            ui.label(format!("{} Warning{}", len, if len > 1 { "s" } else { "" }));
                            ui.add_space(0.0);
                        })
                        .response
                        .rect;
                    let response =
                        ui.interact(rect, egui::Id::new("warnings-button"), egui::Sense::click());

                    if response.hovered() {
                        let hovered = ui.visuals().widgets.hovered;
                        ui.painter()
                            .rect_stroke(rect, hovered.corner_radius, hovered.bg_stroke);
                    }
                    if response.clicked() {
                        self.warning = true;
                    }
                }
            });
        }

        // Draw the main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(enabled);

            // Draw car filters
            egui::containers::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.track_selection(ui);
                    self.car_selection(ui);
                });
            });

            // Draw setup filters
            let colors = self.config.colors();
            let diff_colors = self.config.diff_colors();
            let (track_name, car_name, setups) = self.setup_selection(ui, &colors);
            if !setups.is_empty() {
                // Draw setup properties grid
                egui::containers::ScrollArea::both()
                    .id_source(format!("{}{}", track_name, car_name))
                    .show(ui, |ui| {
                        SetupGrid::new(ui, &setups, &colors, diff_colors).show(ui, car_name);
                    });
            }
        });

        // Draw the windows (if requested by the user)
        self.about_window(ctx, enabled);
        self.prefs_window(ctx, enabled, window);
        if self.warning {
            self.warning_window(ctx, enabled);
        }
        self.show_update_notification(ctx, enabled);
    }

    /// Create a file system watcher.
    fn watch_setups_path(event_loop_proxy: EventLoopProxy<UserEvent>) -> impl Fn(hotwatch::Event) {
        move |event| {
            event_loop_proxy
                .send_event(UserEvent::FsChange(event))
                .expect("Event loop must exist");
        }
    }

    /// Handle file system change events.
    ///
    /// Called by the closure from `Self::watch_setups_path`.
    pub(crate) fn handle_fs_change(&mut self, event: hotwatch::Event) {
        use crate::setup::UpdateKind::*;

        // Update the setups tree.
        let updates = self.setups.update(&event, &self.config);
        for update in updates {
            match update {
                AddedSetup(track_name, car_name, index) => {
                    if self.selected_track_name.as_ref() == Some(&track_name)
                        && self.selected_car_name.as_ref() == Some(&car_name)
                    {
                        // Update selected setups when a new one is added
                        for i in self.selected_setups.iter_mut() {
                            if *i >= index {
                                *i += 1;
                            }
                        }
                    }
                }
                RemovedSetup(track_name, car_name, index) => {
                    if self.selected_track_name.as_ref() == Some(&track_name)
                        && self.selected_car_name.as_ref() == Some(&car_name)
                    {
                        // Update selected setups when an old one is removed
                        self.selected_setups.retain(|i| *i != index);
                        for i in self.selected_setups.iter_mut() {
                            if *i >= index {
                                *i -= 1;
                            }
                        }
                    }
                }
                RemovedCar(track_name, car_name) => {
                    if self.selected_track_name.as_ref() == Some(&track_name)
                        && self.selected_car_name.as_ref() == Some(&car_name)
                    {
                        self.selected_car_name = None;
                        self.selected_setups.clear();
                    }
                }
                RemovedTrack(track_name) => {
                    if self.selected_track_name.as_ref() == Some(&track_name) {
                        self.selected_track_name = None;
                        self.selected_car_name = None;
                        self.selected_setups.clear();
                    }
                }
            }
        }

        // Show warning window if necessary.
        if let hotwatch::Event::Error(error, path) = event {
            let msg = path.map_or("Error while watching file system".to_string(), |path| {
                format!("Error while watching path: `{:?}`", path)
            });

            self.show_warnings.push_front(ShowWarning::new(error, msg));
        }
    }

    /// Update setups export path.
    pub(crate) fn update_setups_path<P: AsRef<Path>>(&mut self, setups_path: P) {
        if let Err(error) = self.hotwatch.unwatch(self.config.get_setups_path()) {
            self.show_warnings.push_front(ShowWarning::new(
                error,
                format!(
                    "Unable to stop watching setup exports path for changes: `{:?}`",
                    self.config.get_setups_path()
                ),
            ));
        }

        self.config.update_setups_path(setups_path);
        self.setups = Setups::new(&mut self.show_warnings, &self.config);
        self.clear_filters();

        let watcher = Self::watch_setups_path(self.event_loop_proxy.clone());
        if let Err(error) = self.hotwatch.watch(self.config.get_setups_path(), watcher) {
            self.show_warnings.push_front(ShowWarning::new(
                error,
                format!(
                    "Unable to watch setup exports path for changes: `{:?}`",
                    self.config.get_setups_path()
                ),
            ));
        }
    }

    /// Clear track, car, and setup filters.
    fn clear_filters(&mut self) {
        self.selected_track_name = None;
        self.selected_car_name = None;
        self.selected_setups.clear();
    }

    /// Show track selection drop-down box.
    fn track_selection(&mut self, ui: &mut egui::Ui) {
        ui.label("Track:");
        let track_selection = egui::ComboBox::from_id_source("track-selection")
            .width(get_combo_box_width(ui, self.setups.tracks().keys()));
        let track_selection = match self.selected_track_name.as_ref() {
            Some(track_name) => track_selection.selected_text(track_name),
            None => track_selection,
        };
        track_selection.show_ui(ui, |ui| {
            let mut track_names: Vec<_> = self.setups.tracks().keys().collect();
            track_names.sort_unstable_by(|a, b| a.human_compare(b));

            for track_name in track_names {
                let checked = self.selected_track_name.as_ref() == Some(track_name);
                if ui.selectable_label(checked, track_name).clicked() {
                    self.selected_track_name = Some(track_name.to_string());
                    self.selected_car_name = None;
                    self.selected_setups.clear();
                }
            }
        });
    }

    /// Show car selection drop-down.
    fn car_selection(&mut self, ui: &mut egui::Ui) {
        ui.label("Car:");

        // Create a child Ui that can be temporarily disabled
        ui.scope(|ui| {
            ui.set_enabled(self.selected_track_name.is_some());

            let car_selection = egui::ComboBox::from_id_source("car-selection");
            let car_selection = match self.selected_track_name.as_ref() {
                Some(track_name) => {
                    // TODO: DRY
                    let car_names = self
                        .setups
                        .tracks()
                        .get(track_name)
                        .expect("Invalid track name")
                        .keys();

                    car_selection.width(get_combo_box_width(ui, car_names))
                }
                None => car_selection,
            };
            let car_selection = match self.selected_car_name.as_ref() {
                Some(car_name) => car_selection.selected_text(car_name),
                None => car_selection,
            };
            car_selection.show_ui(ui, |ui| {
                if let Some(track_name) = self.selected_track_name.as_ref() {
                    let mut car_names: Vec<_> = self
                        .setups
                        .tracks()
                        .get(track_name)
                        .expect("Invalid track name")
                        .keys()
                        .collect();
                    car_names.sort_unstable_by(|a, b| a.human_compare(b));

                    for car_name in car_names {
                        let checked = self.selected_car_name.as_ref() == Some(car_name);
                        if ui.selectable_label(checked, car_name).clicked() {
                            self.selected_car_name = Some(car_name.to_string());
                            self.selected_setups.clear();
                        }
                    }
                }
            });
        });
    }

    /// Show setup selection check boxes.
    fn setup_selection(
        &mut self,
        ui: &mut egui::Ui,
        colors: &[egui::Color32],
    ) -> (&str, &str, Vec<&Setup>) {
        let mut output = Vec::new();
        let mut output_track_name = "";
        let mut output_car_name = "";

        let selected_track_name = self.selected_track_name.as_ref();
        let selected_car_name = self.selected_car_name.as_ref();
        let selected_setups = &mut self.selected_setups;
        let tracks = self.setups.tracks();

        ui.horizontal_wrapped(|ui| {
            if let Some(track_name) = selected_track_name {
                if let Some(car_name) = selected_car_name {
                    output_track_name = track_name.as_str();
                    output_car_name = car_name.as_str();

                    let setups = tracks
                        .get(track_name)
                        .expect("Invalid track name")
                        .get(car_name)
                        .expect("Invalid car name");

                    for (i, info) in setups.iter().enumerate() {
                        let position = selected_setups.iter().position(|&v| v == i);
                        let mut checked = position.is_some();
                        let color = position
                            .and_then(|i| colors.iter().cycle().nth(i))
                            .cloned()
                            .unwrap_or_else(|| ui.visuals().text_color());

                        let checkbox = egui::Checkbox::new(
                            &mut checked,
                            egui::RichText::new(info.name()).color(color),
                        )
                        .ui(ui);
                        if checkbox.clicked() {
                            if checked {
                                selected_setups.push(i);
                            } else if let Some(i) = position {
                                selected_setups.remove(i);
                            }
                        }
                    }

                    for i in selected_setups {
                        output.push(setups[*i].setup());
                    }
                }
            }
        });

        (output_track_name, output_car_name, output)
    }

    /// Show "About" window.
    fn about_window(&mut self, ctx: &egui::CtxRef, enabled: bool) {
        egui::Window::new("About CarTunes")
            .open(&mut self.about)
            .enabled(enabled)
            .collapsible(false)
            .default_pos((175.0, 175.0))
            .fixed_size((350.0, 100.0))
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.label(concat!("CarTunes version ", env!("CARGO_PKG_VERSION")));
                ui.add_space(10.0);
                ui.label(env!("CARGO_PKG_DESCRIPTION"));
                ui.label(concat!("By: ", env!("CARGO_PKG_AUTHORS")));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label("Website:");
                    ui.hyperlink(env!("CARGO_PKG_HOMEPAGE"));
                });
            });
    }

    /// Show "Preferences" window.
    fn prefs_window(&mut self, ctx: &CtxRef, enabled: bool, window: &winit::window::Window) {
        let mut preferences = self.preferences;

        egui::Window::new("CarTunes Preferences")
            .open(&mut preferences)
            .enabled(enabled)
            .collapsible(false)
            .default_pos((150.0, 150.0))
            .fixed_size((500.0, 200.0))
            .show(ctx, |ui| {
                // Theme selection
                ui.horizontal(|ui| {
                    let current_theme = *self.config.theme();

                    ui.label("Theme:");
                    egui::ComboBox::from_id_source("theme-preference")
                        .selected_text(current_theme.to_string())
                        .show_ui(ui, |ui| {
                            let choices = [UserTheme::Auto, UserTheme::Dark, UserTheme::Light];
                            for choice in &choices {
                                let checked = current_theme == *choice;
                                let response = ui.selectable_label(checked, choice.to_string());
                                if response.clicked() {
                                    self.config.update_theme(*choice);
                                    self.event_loop_proxy
                                        .send_event(UserEvent::Theme(*choice))
                                        .expect("Event loop must exist");
                                }
                            }
                        });
                });

                // Update check frequency
                ui.horizontal(|ui| {
                    let update_check = self.config.get_update_check();

                    ui.label("Update checks:");
                    egui::ComboBox::from_id_source("update-check-preference")
                        .selected_text(update_check.to_string())
                        .show_ui(ui, |ui| {
                            let choices = [
                                UpdateFrequency::Never,
                                UpdateFrequency::Daily,
                                UpdateFrequency::Weekly,
                            ];
                            for choice in &choices {
                                let checked = update_check == *choice;
                                let response = ui.selectable_label(checked, choice.to_string());
                                if response.clicked() {
                                    self.config.set_update_check(*choice);
                                    self.event_loop_proxy
                                        .send_event(UserEvent::UpdateCheck)
                                        .expect("Event loop must exist");
                                }
                            }
                        });
                });

                // Setup exports path selection
                ui.horizontal(|ui| {
                    let setups_path = self.config.get_setups_path();

                    // Strip Windows path prefixes for display in the GUI
                    let label = setups_path.to_string_lossy();
                    let label = label
                        .strip_prefix(r"\\?\UNC\")
                        .or_else(|| label.strip_prefix(r"\\?\"))
                        .or_else(|| label.strip_prefix(r"\??\"))
                        .unwrap_or(&label)
                        .ellipsis(50);

                    ui.label("Setup exports path:");
                    if egui::Label::new(egui::RichText::new(label).code())
                        .sense(egui::Sense::click())
                        .ui(ui)
                        .clicked()
                    {
                        let event_loop_proxy = self.event_loop_proxy.clone();
                        let f = rfd::AsyncFileDialog::new()
                            .set_parent(window)
                            .set_directory(setups_path)
                            .pick_folder();

                        std::thread::spawn(move || {
                            let choice =
                                pollster::block_on(f).map(|selected| selected.path().to_path_buf());

                            event_loop_proxy
                                .send_event(UserEvent::SetupPath(choice))
                                .expect("Event loop must exist");
                        });
                    }
                });

                // Color choices
                ui.separator();
                ui.label("Column colors:");
                ui.horizontal_wrapped(|ui| {
                    let colors = self.config.colors_mut();
                    let mut changed = false;
                    let mut to_delete = None;

                    for (i, color) in colors.iter_mut().enumerate() {
                        let old_color = *color;

                        if color_edit_button_srgba(ui, color, Alpha::Opaque)
                            .on_hover_text("Right-click to remove")
                            .secondary_clicked()
                        {
                            to_delete = Some(i);
                        }

                        changed |= *color != old_color;
                    }

                    let add_clicked = ui.button("Add").clicked();
                    if add_clicked {
                        colors.push(ui.visuals().text_color());
                    } else if let Some(i) = to_delete {
                        colors.remove(i);
                    }

                    // Update colors in the config TOML doc
                    if changed || add_clicked || to_delete.is_some() {
                        self.config.update_colors();
                    }
                });

                ui.label("Diff colors:");
                ui.horizontal_wrapped(|ui| {
                    let colors = self.config.diff_colors_mut();
                    let old_colors = *colors;

                    color_edit_button_srgba(ui, &mut colors.0, Alpha::Opaque);
                    color_edit_button_srgba(ui, &mut colors.1, Alpha::Opaque);

                    if *colors != old_colors {
                        self.config.update_colors();
                    }
                });
            });

        self.preferences = preferences;
    }

    /// Add an error to the GUI.
    ///
    /// The new error will be shown to the user if it is the only one, or else it will wait in a
    /// queue until older errors have been acknowledged.
    pub(crate) fn add_error(&mut self, err: ShowError) {
        self.show_errors.push_front(err);
    }

    /// Show error window.
    fn error_window(&mut self, ctx: &egui::CtxRef) -> bool {
        let err = self.show_errors.pop_back();
        if let Some(err) = err {
            let mut result = true;
            let width = 550.0;
            let height = 185.0;
            let red = egui::Color32::from_rgb(210, 40, 40);

            egui::Window::new("Error")
                .collapsible(false)
                .default_pos((100.0, 100.0))
                .fixed_size((width, height))
                .show(ctx, |ui| {
                    ui.label(&err.context);

                    egui::ScrollArea::vertical()
                        .max_height(height)
                        .show(ui, |ui| {
                            let mut text = err.error.to_string();
                            let text_edit = egui::TextEdit::multiline(&mut text)
                                .text_style(egui::TextStyle::Monospace)
                                .text_color(red)
                                .desired_width(width)
                                .desired_rows(10);

                            ui.add_enabled(false, text_edit);
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        let tooltip_id = egui::Id::new("error-copypasta");

                        if ui.button("Copy to Clipboard").clicked() {
                            let mut copied = false;
                            if let Ok(mut clipboard) = ClipboardContext::new() {
                                copied = clipboard.set_contents(err.error.to_string()).is_ok();
                            }

                            let label = if copied {
                                "Copied!"
                            } else {
                                "Sorry, but the clipboard isn't working..."
                            };

                            self.add_tooltip(tooltip_id, label);
                        }

                        // Show the copy button tooltip for 3 seconds
                        self.tooltip(ctx, ui, tooltip_id, Duration::from_secs(3));

                        ui.with_layout(egui::Layout::right_to_left(), |ui| {
                            if ui.button(&err.buttons.0.label).clicked() {
                                let action = err.buttons.0.action;
                                action();
                                result = false;
                            } else if egui::Button::new(
                                egui::RichText::new(&err.buttons.1.label)
                                    .color(egui::Color32::BLACK),
                            )
                            .fill(red)
                            .ui(ui)
                            .clicked()
                            {
                                let action = err.buttons.1.action;
                                action();
                                result = false;
                            } else {
                                self.show_errors.push_back(err);
                            }
                        });
                    });
                });

            result
        } else {
            true
        }
    }

    /// Add a warning to the GUI.
    ///
    /// The new warning will be shown to the user if it is the only one, or else it will wait in a
    /// queue until older warnings have been acknowledged.
    pub(crate) fn add_warning(&mut self, warn: ShowWarning) {
        self.show_warnings.push_front(warn);
    }

    /// Show warning window.
    fn warning_window(&mut self, ctx: &egui::CtxRef, enabled: bool) {
        let mut window_open = self.warning;
        let warning = self.show_warnings.pop_back();
        if let Some(warning) = warning {
            let width = 550.0;
            let height = 185.0;
            let yellow = egui::Color32::from_rgb(156, 156, 40);

            egui::Window::new("Warning")
                .open(&mut window_open)
                .collapsible(false)
                .default_pos((125.0, 125.0))
                .fixed_size((width, height))
                .show(ctx, |ui| {
                    ui.set_enabled(enabled);
                    ui.label(&warning.context);

                    egui::ScrollArea::vertical()
                        .max_height(height)
                        .show(ui, |ui| {
                            let mut text = warning.warning.to_string();
                            let text_edit = egui::TextEdit::multiline(&mut text)
                                .text_style(egui::TextStyle::Monospace)
                                .text_color(yellow)
                                .desired_width(width)
                                .desired_rows(10);

                            ui.add_enabled(false, text_edit);
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        let tooltip_id = egui::Id::new("warning-copypasta");

                        if ui.button("Copy to Clipboard").clicked() {
                            let mut copied = false;
                            if let Ok(mut clipboard) = ClipboardContext::new() {
                                copied =
                                    clipboard.set_contents(warning.warning.to_string()).is_ok();
                            }

                            let label = if copied {
                                "Copied!"
                            } else {
                                "Sorry, but the clipboard isn't working..."
                            };

                            self.add_tooltip(tooltip_id, label);
                        }

                        // Show the copy button tooltip for 3 seconds
                        self.tooltip(ctx, ui, tooltip_id, Duration::from_secs(3));
                    });
                });

            if window_open {
                // Put the warning back
                self.show_warnings.push_back(warning);
            }
        }

        self.warning = window_open;
    }

    /// Add a n update notification to the GUI.
    pub(crate) fn add_update_notification(&mut self, notification: UpdateNotification) {
        self.show_update_notification = Some(notification);
    }

    /// Show update notification window.
    fn show_update_notification(&mut self, ctx: &egui::CtxRef, enabled: bool) {
        let update_notification = self.show_update_notification.as_ref();
        if let Some(update_notification) = update_notification {
            egui::Window::new("New update available")
                .open(&mut self.update_notification)
                .collapsible(false)
                .default_pos((125.0, 125.0))
                .fixed_size((350.0, 120.0))
                .show(ctx, |ui| {
                    let size = ui.spacing().interact_size.y;

                    ui.set_enabled(enabled);

                    ui.label(concat!(
                        "You are using version: ",
                        env!("CARGO_PKG_VERSION"),
                    ));
                    ui.label(format!("New version: {}", update_notification.version));
                    ui.add_space(size);
                    ui.label("Release notes:");
                    ui.label(&update_notification.release_notes);

                    ui.separator();

                    ui.hyperlink_to("Download update", &update_notification.update_url);
                });
        }
    }

    /// Add a tooltip to the GUI.
    ///
    /// The tooltip must be displayed until it expires or this will "leak" tooltips.
    fn add_tooltip(&mut self, tooltip_id: egui::Id, label: &str) {
        self.show_tooltips
            .insert(tooltip_id, (label.to_owned(), Instant::now()));
    }

    /// Show a tooltip at the current cursor position for the given duration.
    ///
    /// The tooltip must have already been added for it to be displayed.
    fn tooltip(
        &mut self,
        ctx: &egui::CtxRef,
        ui: &egui::Ui,
        tooltip_id: egui::Id,
        duration: Duration,
    ) {
        if let Some((label, created)) = self.show_tooltips.remove(&tooltip_id) {
            if Instant::now().duration_since(created) < duration {
                let tooltip_position = ui.available_rect_before_wrap().min;
                egui::containers::popup::show_tooltip_at(
                    ctx,
                    tooltip_id,
                    Some(tooltip_position),
                    |ui| {
                        ui.label(&label);
                    },
                );

                // Put the tooltip back until it expires
                self.show_tooltips.insert(tooltip_id, (label, created));
            }
        }
    }

    /// Get an event-loop proxy that can be used to send events back to the `winit` event loop.
    pub(crate) fn event_loop_proxy(&self) -> EventLoopProxy<UserEvent> {
        self.event_loop_proxy.clone()
    }
}

impl ShowError {
    /// Create an error message to be shown to the user.
    ///
    /// The two buttons have a precise order:
    /// 1. First is the "Cancel" button which is considered the default action and should do
    ///    something sane. E.g. this button should not delete anything.
    /// 2. Second is the "Ok" button which confirms a potentially dangerous action. It is
    ///    highlighted with a red background for emphasis on its potentially dangerous nature.
    pub(crate) fn new<E>(err: E, context: &str, buttons: (ErrorButton, ErrorButton)) -> Self
    where
        E: Into<Box<dyn std::error::Error>>,
    {
        Self {
            error: err.into(),
            context: context.to_owned(),
            buttons,
        }
    }
}

impl ErrorButton {
    /// Create a button for the error window.
    ///
    /// The label is the text written on the button, and the action is a function that is executed
    /// when the button is pressed. Because the action is executed asynchronously, it may internally
    /// use a channel or `Arc<T>` to signal when the action has been performed.
    pub(crate) fn new<F: FnOnce() + 'static>(label: &str, action: F) -> Self {
        Self {
            label: label.to_owned(),
            action: Box::new(action),
        }
    }
}

impl ShowWarning {
    /// Create a warning message to be shown to the user.
    pub(crate) fn new<E, S>(warning: E, context: S) -> Self
    where
        E: Into<Box<dyn std::error::Error>>,
        S: Into<String>,
    {
        Self {
            warning: warning.into(),
            context: context.into(),
        }
    }
}

/// Get the width for a combo box by finding the widest string that it contains.
fn get_combo_box_width<'a>(ui: &egui::Ui, choices: impl Iterator<Item = &'a String>) -> f32 {
    let spacing = ui.spacing();
    let default = spacing.interact_size.x + spacing.item_spacing.x + spacing.icon_width;
    choices.fold(default, |width, choice| {
        let galley = ui.fonts().layout_no_wrap(
            choice.to_string(),
            egui::TextStyle::Button,
            egui::Color32::TEMPORARY_COLOR,
        );

        width.max(
            galley.rect.width()
                + spacing.item_spacing.x
                + spacing.icon_width
                + spacing.scroll_bar_width,
        )
    })
}
