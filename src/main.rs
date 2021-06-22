#![deny(clippy::all)]

use crate::framework::{ConfigHandler, Exiting, Framework, UserChoice};
use crate::gpu::{Error as GpuError, Gpu};
use crate::gui::Gui;
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

/// Application error handling.
#[derive(Debug, Error)]
enum Error {
    #[error("Window creation error: {0}")]
    Winit(#[from] winit::error::OsError),

    #[error("GPU Error: {0}")]
    Gpu(#[from] GpuError),
}

/// Load configuration and create a window.
fn create_window(
    keep_config: UserChoice<ConfigHandler>,
) -> Result<(EventLoop<()>, winit::window::Window, Gpu, Framework), Error> {
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

    let event_loop = EventLoop::new();
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

        let (config, error) = Framework::unwrap_config(keep_config, config);

        let gui = Gui::new(config, error);
        let gpu = Gpu::new(&window, window_size)?;
        let framework = Framework::new(window_size, scale_factor, theme, gui, &gpu);

        (gpu, framework)
    };

    Ok((event_loop, window, gpu, framework))
}

// TODO: Better error handling
fn main() -> Result<(), Error> {
    env_logger::init();

    let keep_config = ConfigHandler::new();
    let exiting = Exiting::new();

    let (event_loop, window, mut gpu, mut framework) = create_window(keep_config.clone())?;
    let mut input = WinitInputHelper::new();

    event_loop.run(move |event, _, control_flow| {
        // Check for exit events
        if exiting.get() == Exiting::Yes {
            *control_flow = ControlFlow::Exit;
            return;
        }

        // Update egui inputs
        framework.handle_event(&event);

        if let Event::WindowEvent { ref event, .. } = event {
            match event {
                WindowEvent::ThemeChanged(theme) => {
                    framework.change_theme(*theme);
                    window.request_redraw();
                }
                WindowEvent::CloseRequested => {
                    // Exit immediately if we've been asked to keep the config file, or if saving was successful
                    if keep_config.get() == ConfigHandler::Keep
                        || framework.save_config(exiting.clone(), &window)
                    {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
                _ => (),
            }
        }

        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            // Prepare egui
            framework.prepare();

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
    });
}
