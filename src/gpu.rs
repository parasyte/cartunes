//! Platform-neutral GPU state management and rendering.

use raw_window_handle::HasRawWindowHandle;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    /// No suitable [`wgpu::Adapter`] found
    #[error("No suitable `wgpu::Adapter` found.")]
    AdapterNotFound,
    /// Equivalent to [`wgpu::RequestDeviceError`]
    #[error("No wgpu::Device found.")]
    DeviceNotFound(wgpu::RequestDeviceError),
    /// Equivalent to [`wgpu::SurfaceError`]
    #[error("The GPU failed to acquire a surface frame.")]
    Surface(wgpu::SurfaceError),
}

pub(crate) struct Gpu {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    surface: wgpu::Surface,
    window_size: winit::dpi::PhysicalSize<u32>,
}

impl Gpu {
    pub(crate) fn new<W: HasRawWindowHandle>(
        window: &W,
        window_size: winit::dpi::PhysicalSize<u32>,
    ) -> Result<Self, Error> {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
        });
        let adapter = pollster::block_on(adapter).ok_or(Error::AdapterNotFound)?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(Error::DeviceNotFound)?;

        let gpu = Self {
            device,
            queue,
            surface,
            window_size,
        };
        gpu.reconfigure_surface();

        Ok(gpu)
    }

    fn reconfigure_surface(&self) {
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                width: self.window_size.width,
                height: self.window_size.height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        )
    }

    pub(crate) fn resize(&mut self, window_size: winit::dpi::PhysicalSize<u32>) {
        self.window_size = window_size;
        self.reconfigure_surface();
    }

    pub(crate) fn prepare(&mut self) -> Result<(wgpu::CommandEncoder, wgpu::SurfaceFrame), Error> {
        let frame = self
            .surface
            .get_current_frame()
            .or_else(|err| match err {
                wgpu::SurfaceError::Outdated => {
                    // Recreate the swap chain to mitigate race condition on drawing surface resize.
                    self.reconfigure_surface();
                    self.surface.get_current_frame()
                }
                err => Err(err),
            })
            .map_err(Error::Surface)?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_command_encoder"),
            });

        Ok((encoder, frame))
    }
}
