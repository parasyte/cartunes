//! # Cartunes
//!
//! Simple comparison app for iRacing car setups.
//!
//! Cartunes is written in Rust and aims to be platform neutral. It runs on Windows, macOS, and
//! Linux. The application provides a basic "spreadsheet"-like layout to help make comparisons easy
//! between car setup exports from [iRacing](https://www.iracing.com/). CSV export is available for
//! more advanced data processing needs.
//!
//! The GUI is unique because it is 100% rendered on the GPU using Vulkan (on Windows and Linux) and
//! the Metal graphics API on macOS. Dark mode and light mode OS themes are both included, although
//! automatic theme switching
//! [may not work on all platforms](https://github.com/rust-windowing/winit/issues/1549).
#![deny(clippy::all)]

use crate::framework::{ConfigHandler, Framework, UserEvent};
use crate::gpu::{Error as GpuError, Gpu};
use crate::gui::Gui;
use crate::setup::Setups;
use log::error;
use thiserror::Error;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
#[cfg(not(target_os = "windows"))]
use winit::window::Theme;

mod config;
mod framework;
mod gpu;
mod gui;
mod setup;
mod str_ext;

/// Application error handling.
#[derive(Debug, Error)]
enum Error {
    #[error("Window creation error: {0}")]
    Winit(#[from] winit::error::OsError),

    #[error("GPU Error: {0}")]
    Gpu(#[from] GpuError),
}

/// Load configuration and create a window.
fn create_window() -> Result<(EventLoop<UserEvent>, winit::window::Window, Gpu, Framework), Error> {
    let config = Framework::load_config();

    let window_builder = if let Ok(Some(config)) = config.as_ref() {
        if let Some(window) = config.get_window() {
            WindowBuilder::new()
                .with_position(window.position)
                .with_inner_size(window.size)
        } else {
            WindowBuilder::new()
        }
    } else {
        WindowBuilder::new()
    };

    let event_loop = EventLoop::with_user_event();
    let window = window_builder
        .with_title("CarTunes")
        .with_min_inner_size(Framework::min_size())
        .build(&event_loop)?;

    let (gpu, framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor();

        #[cfg(target_os = "windows")]
        let theme = window.theme();
        #[cfg(not(target_os = "windows"))]
        let theme = Theme::Dark;

        let (config, error) = Framework::unwrap_config(event_loop.create_proxy(), config);
        // TODO: Load all setup exports.
        let gui = Gui::new(config, event_loop.create_proxy(), Setups::default(), error);
        let gpu = Gpu::new(&window, window_size)?;
        let framework = Framework::new(window_size, scale_factor, theme, gui, &gpu);

        (gpu, framework)
    };

    Ok((event_loop, window, gpu, framework))
}

// TODO: Better error handling
fn main() -> Result<(), Error> {
    env_logger::init();

    let (event_loop, window, mut gpu, mut framework) = create_window()?;
    let event_loop_proxy = event_loop.create_proxy();
    let mut input = WinitInputHelper::new();
    let mut keep_config = ConfigHandler::Replace;

    event_loop.run(move |event, _, control_flow| {
        // Update egui inputs
        framework.handle_event(&event);

        // Handle input events
        if input.update(&event) {
            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                if size.width > 0 && size.height > 0 {
                    gpu.resize(size);
                    framework.resize(size);
                }
            }

            // Update internal state and request a redraw
            window.request_redraw();
        }

        match event {
            Event::UserEvent(event) => match event {
                UserEvent::ConfigHandler(config_handler) => {
                    keep_config = config_handler;
                }
                UserEvent::Exit => {
                    *control_flow = ControlFlow::Exit;
                }
                UserEvent::SetupPath(Some(setups_path)) => {
                    framework.update_setups_path(setups_path);
                }
                _ => (),
            },
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::ThemeChanged(theme) => {
                    framework.change_theme(theme);
                    window.request_redraw();
                }
                WindowEvent::CloseRequested => {
                    // Exit immediately if we've been asked to keep the config file,
                    // or if saving was successful
                    if keep_config == ConfigHandler::Keep
                        || framework.save_config(event_loop_proxy.clone(), &window)
                    {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                // Prepare egui
                framework.prepare(&window);

                let (mut encoder, frame) = match gpu.prepare() {
                    Ok((encoder, frame)) => (encoder, frame),
                    Err(err) => {
                        error!("gpu.prepare() failed: {}", err);
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                };

                // Render egui
                framework.render(&mut encoder, &frame.output.view, &gpu);

                // Complete frame
                gpu.queue.submit(Some(encoder.finish()));
            }
            _ => (),
        }
    });
}
