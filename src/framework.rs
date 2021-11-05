//! Platform-neutral framework for processing events and handling app configuration.

use crate::config::{Config, Error as ConfigError, UserTheme};
use crate::gpu::Gpu;
use crate::gui::{ErrorButton, Gui, ShowError};
use directories::ProjectDirs;
use egui::{ClippedMesh, CtxRef};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use font_loader::system_fonts::{self, FontPropertyBuilder};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::path::PathBuf;
use thiserror::Error;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoopProxy;
use winit::window::{Theme, Window};

/// Manages all state required for rendering egui.
pub(crate) struct Framework {
    // State for egui.
    egui_ctx: CtxRef,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    rpass: RenderPass,
    paint_jobs: Vec<ClippedMesh>,
    theme: Option<Theme>,
    gui: Gui,
}

/// Framework errors.
#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Reading config file failed: {0}")]
    ReadConfig(#[from] ConfigError),
}

/// User event handling is performed with this type.
#[derive(Debug)]
pub(crate) enum UserEvent {
    /// Configuration error handling events
    ConfigHandler(ConfigHandler),

    /// User wants to exit without saving.
    Exit,

    /// Change the path for setup export files.
    SetupPath(Option<PathBuf>),

    /// File system event for the setup export path.
    FsChange(hotwatch::Event),

    /// Change the theme preference.
    Theme(UserTheme),
}

/// How the user wants to handle errors with reading the config file.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ConfigHandler {
    /// There were no errors,
    /// or there were errors and the user wants to replace the config with a new one.
    Replace,

    /// There were errors and the user wants to keep the existing config.
    Keep,
}

impl Framework {
    /// Create a framework for egui.
    pub(crate) fn new(
        size: PhysicalSize<u32>,
        scale_factor: f32,
        theme: Theme,
        gui: Gui,
        gpu: &Gpu,
    ) -> Self {
        let width = size.width;
        let height = size.height;
        let font_definitions = create_fonts();
        let style = create_style(theme);

        let egui_state = egui_winit::State::from_pixels_per_point(scale_factor);
        let egui_ctx = CtxRef::default();
        let screen_descriptor = ScreenDescriptor {
            physical_width: width,
            physical_height: height,
            scale_factor,
        };
        let rpass = RenderPass::new(&gpu.device, wgpu::TextureFormat::Bgra8UnormSrgb, 1);

        egui_ctx.set_fonts(font_definitions);
        egui_ctx.set_style(style);

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            rpass,
            paint_jobs: Vec::new(),
            theme: None,
            gui,
        }
    }

    /// Handle input events from the window manager.
    pub(crate) fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        self.egui_state.on_event(&self.egui_ctx, event);
    }

    /// Handle file system change events.
    pub(crate) fn handle_fs_change(&mut self, event: hotwatch::Event) {
        self.gui.handle_fs_change(event);
    }

    /// Resize egui.
    pub(crate) fn resize(&mut self, size: PhysicalSize<u32>) {
        self.screen_descriptor.physical_width = size.width;
        self.screen_descriptor.physical_height = size.height;
    }

    /// Update scaling factor.
    pub(crate) fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.scale_factor = scale_factor as f32;
    }

    /// Prepare egui.
    pub(crate) fn prepare(&mut self, window: &Window) {
        // Begin the egui frame.
        let raw_input = self.egui_state.take_egui_input(window);
        self.egui_ctx.begin_frame(raw_input);

        // Draw the application GUI.
        update_theme(&mut self.theme, &self.egui_ctx);
        self.gui.ui(&self.egui_ctx, window);

        // End the egui frame and create all paint jobs to prepare for rendering.
        // TODO: Handle output.needs_repaint to avoid game-mode continuous redraws.
        let (output, paint_commands) = self.egui_ctx.end_frame();
        self.egui_state
            .handle_output(window, &self.egui_ctx, output);
        self.paint_jobs = self.egui_ctx.tessellate(paint_commands);
    }

    /// Render egui.
    pub(crate) fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        gpu: &Gpu,
    ) -> Result<(), BackendError> {
        // Upload all resources to the GPU.
        self.rpass
            .update_texture(&gpu.device, &gpu.queue, &self.egui_ctx.texture());
        self.rpass.update_user_textures(&gpu.device, &gpu.queue);
        self.rpass.update_buffers(
            &gpu.device,
            &gpu.queue,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        // Record all render passes.
        self.rpass.execute(
            encoder,
            render_target,
            &self.paint_jobs,
            &self.screen_descriptor,
            Some(wgpu::Color::BLACK),
        )
    }

    /// Call this when the system theme changes.
    ///
    /// `force` will ignore the user's configuration preference.
    pub(crate) fn change_theme(&mut self, theme: Theme, force: bool) {
        if force || self.gui.config.theme() == &UserTheme::Auto {
            self.theme = Some(theme);
        }
    }

    /// Get the minimum size allowed for the window.
    pub(crate) fn min_size() -> PhysicalSize<u32> {
        PhysicalSize::new(400, 300)
    }

    /// Try to load the configuration.
    ///
    /// This is an associated function because there will be no window or GUI available when loading
    /// the config.
    pub(crate) fn load_config() -> Result<Option<Config>, Error> {
        let min_size = Self::min_size();
        let config = Config::from_toml(config_path(), min_size)?;

        Ok(config)
    }

    /// Unwrap the result from [`Self::load_config`].
    ///
    /// This is an associated function because there will be no window or GUI available when loading
    /// the config.
    ///
    /// Always returns a valid config, and may optionally add a [`crate::gui::ShowError`] for the
    /// GUI to display an error message to the user.
    pub(crate) fn unwrap_config(
        show_errors: &mut VecDeque<ShowError>,
        event_loop_proxy: EventLoopProxy<UserEvent>,
        config: Result<Option<Config>, Error>,
    ) -> Config {
        match config {
            Ok(Some(config)) => config,
            Ok(None) => Config::new(config_path(), Self::min_size()),
            Err(err) => {
                // Default to keep when there is an error
                event_loop_proxy
                    .send_event(UserEvent::ConfigHandler(ConfigHandler::Keep))
                    .expect("Event loop must exist");

                let err = ShowError::new(
                    err,
                    "Unable to read the config file.\n\
                    It may be corrupt, do you want to keep or replace the config file?",
                    (
                        ErrorButton::new("Keep", || ()),
                        ErrorButton::new("Replace", move || {
                            event_loop_proxy
                                .send_event(UserEvent::ConfigHandler(ConfigHandler::Replace))
                                .expect("Event loop must exist");
                        }),
                    ),
                );
                show_errors.push_back(err);

                Config::new(config_path(), Self::min_size())
            }
        }
    }

    /// Try to save the configuration with the current window geometry.
    ///
    /// Returns true on success. When saving fails, the error is shown to the user and `false` is
    /// returned.
    pub(crate) fn save_config(&mut self, window: &winit::window::Window) -> bool {
        self.gui.config.update_window(window);
        match self.gui.config.write_toml() {
            Ok(()) => true,
            Err(err) => {
                let event_loop_proxy = self.gui.event_loop_proxy();

                // Error handling when saving the config fails
                let err = ShowError::new(
                    err,
                    "Unable to write the config file. Exit anyway?",
                    (
                        ErrorButton::new("Stay", || ()),
                        ErrorButton::new("Exit Anyway", move || {
                            event_loop_proxy
                                .send_event(UserEvent::Exit)
                                .expect("Event loop must exist");
                        }),
                    ),
                );

                self.add_error(err);

                false
            }
        }
    }

    /// Update the setups path on the config.
    pub(crate) fn update_setups_path(&mut self, setups_path: PathBuf) {
        self.gui.update_setups_path(setups_path);
    }

    /// Add an error message window to the GUI.
    ///
    /// The [`ShowError`] type allows asynchronous user feedback for error handling.
    pub(crate) fn add_error(&mut self, err: ShowError) {
        self.gui.add_error(err);
    }
}

/// Configure the theme based on system settings.
fn update_theme(theme: &mut Option<Theme>, ctx: &egui::CtxRef) {
    if let Some(theme) = theme.take() {
        // Set the style
        ctx.set_style(create_style(theme));
    }
}

/// Get the application configuration path.
fn config_path() -> PathBuf {
    // If a project directory cannot be found, use the current working directory.
    let mut config_path = ProjectDirs::from("org", "KodeWerx", "CarTunes")
        .map_or_else(|| PathBuf::from("."), |dir| dir.config_dir().to_path_buf());
    config_path.push("config.toml");

    config_path
}

/// Create fonts for egui from the embedded TTFs.
fn create_fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    // Add font data
    let props = FontPropertyBuilder::new().monospace().build();
    let font = system_fonts::get(&props)
        .expect("Unable to find a monospace font")
        .0;
    fonts
        .font_data
        .insert("MonoSpace".to_owned(), Cow::from(font));

    let props = FontPropertyBuilder::new().family("sans-serif").build();
    let font = system_fonts::get(&props)
        .or_else(|| {
            let props = FontPropertyBuilder::new().family("Helvetica Neue").build();
            system_fonts::get(&props)
        })
        .expect("Unable to find a sans-serif font")
        .0;
    fonts
        .font_data
        .insert("SansSerif".to_owned(), Cow::from(font));

    // Set font families
    fonts
        .fonts_for_family
        .insert(egui::FontFamily::Monospace, vec!["MonoSpace".to_owned()]);
    fonts
        .fonts_for_family
        .insert(egui::FontFamily::Proportional, vec!["SansSerif".to_owned()]);

    if let Some(mut monospace) = fonts.family_and_size.get_mut(&egui::TextStyle::Monospace) {
        // The default monospace size is too small.
        monospace.1 = 14.0;
    }

    if let Some(mut heading) = fonts.family_and_size.get_mut(&egui::TextStyle::Heading) {
        // The default heading size is WAY too big.
        heading.1 = 16.0;
    }

    fonts
}

/// Create the default style for egui based on system settings.
fn create_style(theme: Theme) -> egui::Style {
    let mut visuals = match theme {
        Theme::Dark => egui::Visuals::dark(),
        Theme::Light => {
            let mut visuals = egui::Visuals::light();

            // The default light theme has grey fonts. We want solid black.
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::BLACK;
            visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_gray(50);

            visuals
        }
    };

    // Show a background behind collapsing headers.
    visuals.collapsing_header_frame = true;

    egui::Style {
        visuals,
        ..egui::Style::default()
    }
}
