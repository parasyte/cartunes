#![deny(clippy::all)]

use crate::gpu::{Error, Gpu};
use crate::gui::Gui;
use log::error;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Theme, WindowBuilder};
use winit_input_helper::WinitInputHelper;

#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;

mod gpu;
mod gui;

fn main() -> Result<(), Error> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = WindowBuilder::new()
        .with_title("CarTunes")
        .build(&event_loop)
        .unwrap();

    let (mut gpu, mut gui) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor();
        let theme = if cfg!(target_os = "windows") {
            window.theme()
        } else {
            Theme::Dark
        };

        let gpu = Gpu::new(&window, window_size)?;
        let gui = Gui::new(window_size, scale_factor, theme, &gpu);

        (gpu, gui)
    };

    event_loop.run(move |event, _, control_flow| {
        // Update egui inputs
        gui.handle_event(&event);

        if let Event::WindowEvent {
            event: WindowEvent::ThemeChanged(theme),
            ..
        } = event
        {
            gui.change_theme(theme);
            window.request_redraw();
        }

        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            // Prepare egui
            gui.prepare();

            let render_result = gpu
                .prepare()
                .map_err(|e| error!("gpu.begin_render() failed: {}", e));

            // Basic error handling
            if render_result.is_err() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            let (mut encoder, frame) = render_result.unwrap();

            // Render egui
            gui.render(&mut encoder, &frame.output.view, &gpu);

            // Complete frame
            gpu.queue.submit(Some(encoder.finish()));
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                gui.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                if size.width > 0 && size.height > 0 {
                    gpu.resize(size);
                    gui.resize(size.width, size.height);
                }
            }

            // Update internal state and request a redraw
            window.request_redraw();
        }
    });
}
