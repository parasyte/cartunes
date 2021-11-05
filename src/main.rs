//! # CarTunes
//!
//! Simple comparison app for iRacing car setups.
//!
//! CarTunes is written in Rust and aims to be platform neutral. It runs on Windows, macOS, and
//! Linux. The application provides a basic "spreadsheet"-like layout to help make comparisons easy
//! between car setup exports from [iRacing](https://www.iracing.com/).
//!
//! The GUI is unique because it is 100% rendered on the GPU using Vulkan (on Windows and Linux) and
//! the Metal graphics API on macOS. Dark mode and light mode OS themes are both included, although
//! automatic theme switching
//! [may not work on all platforms](https://github.com/rust-windowing/winit/issues/1549).
#![cfg_attr(not(any(test, debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::all)]

use crate::framework::{ConfigHandler, Framework, UserEvent};
use crate::gpu::{Error as GpuError, Gpu};
use crate::gui::{Error as GuiError, Gui};
use crate::setup::Setups;
use log::error;
use std::collections::VecDeque;
use thiserror::Error;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

#[cfg(windows)]
use winit::platform::windows::IconExtWindows;
#[cfg(windows)]
use winit::window::Icon;

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

    #[error("GUI Error: {0}")]
    Gui(#[from] GuiError),

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

    let window_builder = {
        #[cfg(target_os = "windows")]
        {
            // Magic number from cartunes.rc
            const ICON_RESOURCE_ID: u16 = 2;

            window_builder.with_window_icon(Some(
                Icon::from_resource(ICON_RESOURCE_ID, None).expect("Unable to load icon"),
            ))
        }

        #[cfg(not(target_os = "windows"))]
        window_builder
    };

    let event_loop = EventLoop::with_user_event();
    let window = window_builder
        .with_title("CarTunes")
        .with_min_inner_size(Framework::min_size())
        .build(&event_loop)?;

    let (gpu, framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;

        let mut errors = VecDeque::new();
        let mut warnings = VecDeque::new();
        let config = Framework::unwrap_config(&mut errors, event_loop.create_proxy(), config);
        let setups = Setups::new(&mut warnings, &config);
        let theme = config.theme().as_winit_theme(&window);
        let gui = Gui::new(config, setups, event_loop.create_proxy(), errors, warnings)?;
        let gpu = Gpu::new(&window, window_size)?;
        let framework = Framework::new(window_size, scale_factor, theme, gui, &gpu);

        (gpu, framework)
    };

    Ok((event_loop, window, gpu, framework))
}

// TODO: Better error handling
fn main() -> Result<(), Error> {
    #[cfg(any(debug_assertions, not(windows)))]
    env_logger::init();

    let (event_loop, window, mut gpu, mut framework) = create_window()?;
    let mut input = WinitInputHelper::new();
    let mut keep_config = ConfigHandler::Replace;

    event_loop.run(move |event, _, control_flow| {
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
                UserEvent::FsChange(event) => {
                    framework.handle_fs_change(event);
                }
                UserEvent::Theme(theme) => {
                    let theme = theme.as_winit_theme(&window);
                    framework.change_theme(theme, true);
                    window.request_redraw();
                }
                _ => (),
            },
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                framework.handle_event(&event);

                match event {
                    WindowEvent::ThemeChanged(theme) => {
                        framework.change_theme(theme, false);
                        window.request_redraw();
                    }
                    WindowEvent::CloseRequested => {
                        // Exit immediately if we've been asked to keep the config file,
                        // or if saving was successful
                        if keep_config == ConfigHandler::Keep || framework.save_config(&window) {
                            *control_flow = ControlFlow::Exit;
                        }
                    }
                    _ => (),
                }
            }
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

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                // Render egui
                let render_result = framework.render(&mut encoder, &view, &gpu);
                if let Err(err) = render_result {
                    error!("framework.render() failed: {}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                // Complete frame
                gpu.queue.submit(Some(encoder.finish()));
                frame.present();
            }
            _ => (),
        }
    });
}
