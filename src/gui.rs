use std::collections::VecDeque;

use crate::config::Config;
use egui::Widget;
use log::error;

/// Manages all state required for rendering the GUI.
pub(crate) struct Gui {
    /// Application configuration.
    pub(crate) config: Config,

    /// Show the "About..." window.
    about: bool,

    /// Show an error message.
    show_errors: VecDeque<ShowError>,
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
    /// Text to show on the button/
    label: String,

    /// An action to perform when the button is pressed.
    action: Box<dyn FnOnce()>,
}

impl Gui {
    /// Create a GUI.
    pub(crate) fn new(config: Config, show_error: Option<ShowError>) -> Self {
        let mut show_errors = VecDeque::new();
        if let Some(err) = show_error {
            show_errors.push_front(err);
        }

        Self {
            about: false,
            config,
            show_errors,
        }
    }

    /// Create the UI using egui.
    pub(crate) fn ui(&mut self, ctx: &egui::CtxRef) {
        let enabled = self.error(ctx);

        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
            ui.set_enabled(enabled);
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| if ui.button("Open...").clicked() {});
                egui::menu::menu(ui, "Help", |ui| {
                    if ui.button("About CarTunes...").clicked() {
                        self.about = true;
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(enabled);
            ui.label("Hello, world!");
        });

        self.about(ctx, enabled);
    }

    /// Show "About" window.
    fn about(&mut self, ctx: &egui::CtxRef, enabled: bool) {
        egui::Window::new("About CarTunes")
            .open(&mut self.about)
            .enabled(enabled)
            .collapsible(false)
            .default_pos((200.0, 150.0))
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

    /// Add an error to the GUI.
    ///
    /// The new error will be shown to the user if it is the only one, or else it will wait in a
    /// queue until older errors have been acknowledged.
    pub(crate) fn add_error(&mut self, err: ShowError) {
        self.show_errors.push_front(err);
    }

    /// Show error window.
    fn error(&mut self, ctx: &egui::CtxRef) -> bool {
        let err = self.show_errors.pop_back();
        if let Some(err) = err {
            // TODO: Need to add error context and button labels
            let mut result = true;
            let width = 500.0;
            let red = egui::Color32::from_rgb(210, 40, 40);

            egui::Window::new("Error")
                .collapsible(false)
                .default_pos((100.0, 100.0))
                .fixed_size((width, 175.0))
                .show(ctx, |ui| {
                    ui.label(&err.context);

                    egui::ScrollArea::from_max_height(300.0).show(ui, |ui| {
                        egui::TextEdit::multiline(&mut err.error.to_string())
                            .enabled(false)
                            .text_style(egui::TextStyle::Monospace)
                            .text_color(red)
                            .desired_width(width)
                            .desired_rows(10)
                            .ui(ui);
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Copy to Clipboard").clicked() {
                            error!("TODO: Copy to clipboard");
                        }

                        ui.with_layout(egui::Layout::right_to_left(), |ui| {
                            if ui.button(&err.buttons.0.label).clicked() {
                                let action = err.buttons.0.action;
                                action();
                                result = false;
                            } else if egui::Button::new(&err.buttons.1.label)
                                .text_color(egui::Color32::BLACK)
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
