/// Manages all state required for rendering the GUI.
pub(crate) struct Gui {
    about: bool,
}

impl Gui {
    /// Create a GUI.
    pub(crate) fn new() -> Self {
        Self { about: false }
    }

    /// Create the UI using egui.
    pub(crate) fn ui(&mut self, ctx: &egui::CtxRef) {
        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
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
            ui.label("Hello, world!");
        });

        self.about(ctx);
    }

    /// Show "About" window.
    fn about(&mut self, ctx: &egui::CtxRef) {
        egui::Window::new("About CarTunes")
            .open(&mut self.about)
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
}
