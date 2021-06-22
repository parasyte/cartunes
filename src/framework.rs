use crate::config::{Config, Error as ConfigError};
use crate::gpu::Gpu;
use crate::gui::{ErrorButton, Gui, ShowError};
use directories::ProjectDirs;
use egui::ClippedMesh;
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use num_enum::{FromPrimitive, IntoPrimitive};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use std::time::Instant;
use thiserror::Error;
use winit::dpi::PhysicalSize;
use winit::window::Theme;

/// Manages all state required for rendering egui.
pub(crate) struct Framework {
    // State for egui.
    start_time: Instant,
    platform: Platform,
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

/// Type representing a choice that the user needs to make, e.g. in response to an error.
///
/// The generic type needs to be convertible to and from `u8`.
pub(crate) struct UserChoice<T>(Arc<AtomicU8>, PhantomData<T>);

/// How the user wants to handle errors with reading the config file.
#[derive(Debug, Eq, PartialEq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub(crate) enum ConfigHandler {
    /// There were no errors.
    #[num_enum(default)]
    None,

    /// There were errors and the user wants to keep the existing config.
    Keep,

    /// There were errors and the user wants to replace the config with a new one.
    Replace,
}

/// Whether the user wants to exit the app.
#[derive(Debug, Eq, PartialEq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub(crate) enum Exiting {
    /// Continue running the app.
    #[num_enum(default)]
    No,

    /// The user has requested to exit without saving.
    Yes,
}

impl ConfigHandler {
    /// Create a new config handler with a `UserChoice` wrapper.
    pub(crate) fn new() -> UserChoice<Self> {
        UserChoice(Arc::new(AtomicU8::new(Self::None.into())), PhantomData)
    }
}

impl Exiting {
    /// Create a new exiting request with a `UserChoice` wrapper.
    pub(crate) fn new() -> UserChoice<Self> {
        UserChoice(Arc::new(AtomicU8::new(Self::No.into())), PhantomData)
    }
}

impl UserChoice<ConfigHandler> {
    /// Get the current config handler value.
    pub(crate) fn get(&self) -> ConfigHandler {
        ConfigHandler::from(self.0.load(Ordering::Relaxed))
    }

    /// Set the config handler value.
    pub(crate) fn set(&self, value: ConfigHandler) {
        self.0.store(value.into(), Ordering::Relaxed);
    }
}

impl UserChoice<Exiting> {
    /// Get the current exiting request value.
    pub(crate) fn get(&self) -> Exiting {
        Exiting::from(self.0.load(Ordering::Relaxed))
    }

    /// Set the exiting request value.
    pub(crate) fn set(&self, value: Exiting) {
        self.0.store(value.into(), Ordering::Relaxed);
    }
}

impl<T> Clone for UserChoice<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), PhantomData)
    }
}

impl Framework {
    /// Create a framework for egui.
    pub(crate) fn new(
        size: PhysicalSize<u32>,
        scale_factor: f64,
        theme: Theme,
        gui: Gui,
        gpu: &Gpu,
    ) -> Self {
        let width = size.width;
        let height = size.height;
        let font_definitions = create_fonts();
        let style = create_style(theme);
        let platform = Platform::new(PlatformDescriptor {
            physical_width: width,
            physical_height: height,
            scale_factor,
            font_definitions,
            style,
        });
        let screen_descriptor = ScreenDescriptor {
            physical_width: width,
            physical_height: height,
            scale_factor: scale_factor as f32,
        };
        let rpass = RenderPass::new(&gpu.device, wgpu::TextureFormat::Bgra8UnormSrgb, 1);

        Self {
            start_time: Instant::now(),
            platform,
            screen_descriptor,
            rpass,
            paint_jobs: Vec::new(),
            theme: None,
            gui,
        }
    }

    /// Handle input events from the window manager.
    pub(crate) fn handle_event(&mut self, event: &winit::event::Event<'_, ()>) {
        self.platform.handle_event(event);
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
    pub(crate) fn prepare(&mut self) {
        self.platform
            .update_time(self.start_time.elapsed().as_secs_f64());

        // Begin the egui frame.
        self.platform.begin_frame();

        // Draw the application GUI.
        let ctx = self.platform.context();
        self.update_theme(&ctx);
        self.gui.ui(&ctx);

        // End the egui frame and create all paint jobs to prepare for rendering.
        // TODO: Handle output.needs_repaint to avoid game-mode continuous redraws.
        let (_output, paint_commands) = self.platform.end_frame();
        self.paint_jobs = self.platform.context().tessellate(paint_commands);
    }

    /// Render egui.
    pub(crate) fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        gpu: &Gpu,
    ) {
        // Upload all resources to the GPU.
        self.rpass
            .update_texture(&gpu.device, &gpu.queue, &self.platform.context().texture());
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
        );
    }

    /// Call this when the system theme changes.
    pub(crate) fn change_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
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
    /// Always returns a valid config, and may optionally return a [`gui::ShowError`] for the GUI to
    /// display an error message to the user.
    ///
    /// The `keep_config` is a user choice that will be set to [`ConfigHandler::Keep`] when an error
    /// is unwrapped, and may at some time in the future be changed to a [`ConfigHandler::Replace`]
    /// when the user makes a decision.
    pub(crate) fn unwrap_config(
        keep_config: UserChoice<ConfigHandler>,
        config: Result<Option<Config>, Error>,
    ) -> (Config, Option<ShowError>) {
        match config {
            Ok(Some(config)) => (config, None),
            Ok(None) => (Config::new(config_path(), Self::min_size()), None),
            Err(err) => {
                // Default to keep when there is an error
                keep_config.set(ConfigHandler::Keep);

                let err = ShowError::new(
                    err,
                    "Unable to read the config file.\n\
                    It may be corrupt, do you want to keep or replace the config file?",
                    (
                        ErrorButton::new("Keep", || ()),
                        ErrorButton::new("Replace", move || {
                            keep_config.set(ConfigHandler::Replace);
                        }),
                    ),
                );

                (Config::new(config_path(), Self::min_size()), Some(err))
            }
        }
    }

    /// Try to save the configuration with the current window geometry.
    ///
    /// Returns true on success. When saving fails, the error is shown to the user and `false` is
    /// returned. At some time in the future, `exiting` will be set to `Exiting::Yes` if the user
    /// requests to exit anyway.
    pub(crate) fn save_config(
        &mut self,
        exiting: UserChoice<Exiting>,
        window: &winit::window::Window,
    ) -> bool {
        self.gui.config.update_window(window);
        match self.gui.config.write_toml() {
            Ok(()) => true,
            Err(err) => {
                // Error handling when saving the config fails
                let err = ShowError::new(
                    err,
                    "Unable to write the config file. Exit anyway?",
                    (
                        ErrorButton::new("Stay", || ()),
                        ErrorButton::new("Exit Anyway", move || {
                            exiting.set(Exiting::Yes);
                        }),
                    ),
                );

                self.add_error(err);

                false
            }
        }
    }

    /// Add an error message window to the GUI.
    ///
    /// The [`ShowError`] type allows asynchronous user feedback for error handling.
    pub(crate) fn add_error(&mut self, err: ShowError) {
        self.gui.add_error(err);
    }

    /// Configure the theme based on system settings.
    fn update_theme(&mut self, ctx: &egui::CtxRef) {
        if let Some(theme) = self.theme.take() {
            // Set the style
            ctx.set_style(create_style(theme));

            // Set the appropriate font weight for the theme.
            // The best choice was found experimentally.
            let mut fonts = ctx.fonts().definitions().clone();
            if let Some(font) = fonts
                .fonts_for_family
                .get_mut(&egui::FontFamily::Proportional)
            {
                font[0] = match theme {
                    Theme::Dark => "Ubuntu-Light".to_owned(),
                    Theme::Light => "Ubuntu-Regular".to_owned(),
                };
            }
            ctx.set_fonts(fonts);
        }
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
    fonts.font_data.insert(
        "ProggyClean".to_owned(),
        Cow::Borrowed(include_bytes!("../fonts/ProggyClean.ttf")),
    );
    fonts.font_data.insert(
        "Ubuntu-Regular".to_owned(),
        Cow::Borrowed(include_bytes!("../fonts/Ubuntu-Regular.ttf")),
    );
    fonts.font_data.insert(
        "Ubuntu-Light".to_owned(),
        Cow::Borrowed(include_bytes!("../fonts/Ubuntu-Light.ttf")),
    );

    // Set font families
    if let Some(font) = fonts.fonts_for_family.get_mut(&egui::FontFamily::Monospace) {
        font.push("ProggyClean".to_owned());
        font.push("Ubuntu-Light".to_owned());
    }
    if let Some(font) = fonts
        .fonts_for_family
        .get_mut(&egui::FontFamily::Proportional)
    {
        font.push("Ubuntu-Regular".to_owned());
    }

    if let Some(mut heading) = fonts.family_and_size.get_mut(&egui::TextStyle::Heading) {
        // The default heading size is WAY too big.
        heading.1 = 16.0;
    }

    fonts
}

/// Create the default style for egui based on system settings.
fn create_style(theme: Theme) -> egui::Style {
    // The default light theme has grey fonts. We want solid black.
    egui::Style {
        visuals: match theme {
            Theme::Dark => egui::Visuals::dark(),
            Theme::Light => {
                let mut visuals = egui::Visuals::light();

                visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::BLACK;
                visuals.widgets.inactive.fg_stroke.color = egui::Color32::BLACK;

                visuals
            }
        },
        ..egui::Style::default()
    }
}
