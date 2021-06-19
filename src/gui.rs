use crate::gpu::Gpu;
use egui::ClippedMesh;
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use std::borrow::Cow;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::window::Theme;

/// Manages all state required for rendering egui over `Pixels`.
pub(crate) struct Gui {
    // State for egui.
    start_time: Instant,
    platform: Platform,
    screen_descriptor: ScreenDescriptor,
    rpass: RenderPass,
    paint_jobs: Vec<ClippedMesh>,
    theme: Option<Theme>,

    // State for the demo app.
    about: bool,
}

impl Gui {
    /// Create egui.
    pub(crate) fn new(size: PhysicalSize<u32>, scale_factor: f64, theme: Theme, gpu: &Gpu) -> Self {
        let width = size.width;
        let height = size.height;
        let platform = Platform::new(PlatformDescriptor {
            physical_width: width,
            physical_height: height,
            scale_factor,
            ..PlatformDescriptor::default()
        });
        let screen_descriptor = ScreenDescriptor {
            physical_width: width,
            physical_height: height,
            scale_factor: scale_factor as f32,
        };
        let rpass = RenderPass::new(&gpu.device, wgpu::TextureFormat::Bgra8UnormSrgb, 1);

        install_fonts(&platform.context());

        Self {
            start_time: Instant::now(),
            platform,
            screen_descriptor,
            rpass,
            paint_jobs: Vec::new(),
            theme: Some(theme),
            about: false,
        }
    }

    /// Handle input events from the window manager.
    pub(crate) fn handle_event(&mut self, event: &winit::event::Event<'_, ()>) {
        self.platform.handle_event(event);
    }

    /// Resize egui.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.screen_descriptor.physical_width = width;
        self.screen_descriptor.physical_height = height;
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
        self.ui(&self.platform.context());

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

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &egui::CtxRef) {
        self.update_theme(ctx);

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

    /// Configure the theme based on system settings.
    fn update_theme(&mut self, ctx: &egui::CtxRef) {
        if let Some(theme) = self.theme.take() {
            // The default light theme has grey fonts. We want solid black.
            let style = egui::Style {
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
            };
            ctx.set_style(style);

            let mut fonts = ctx.fonts().definitions().clone();
            if let Some(font) = fonts
                .fonts_for_family
                .get_mut(&egui::FontFamily::Proportional)
            {
                // Set the appropriate font weight for the theme.
                // The best choice was found experimentally.
                font[0] = match theme {
                    Theme::Dark => "Ubuntu-Light".to_owned(),
                    Theme::Light => "Ubuntu-Regular".to_owned(),
                };
            }
            ctx.set_fonts(fonts);
        }
    }

    /// Call this when the system theme changes.
    pub(crate) fn change_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
    }
}

/// Install embedded fonts.
fn install_fonts(ctx: &egui::CtxRef) {
    let mut fonts = egui::FontDefinitions::default();
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

    ctx.set_fonts(fonts);
}
