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
    /// Equivalent to [`wgpu::SwapChainError`]
    #[error("The GPU failed to acquire a swapchain frame.")]
    Swapchain(wgpu::SwapChainError),
}

pub(crate) struct Gpu {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    surface: wgpu::Surface,
    window_size: winit::dpi::PhysicalSize<u32>,
    swap_chain: wgpu::SwapChain,
}

impl Gpu {
    pub(crate) fn new<W: HasRawWindowHandle>(
        window: &W,
        window_size: winit::dpi::PhysicalSize<u32>,
    ) -> Result<Self, Error> {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
        });
        let adapter = pollster::block_on(adapter).ok_or(Error::AdapterNotFound)?;
        let (mut device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(Error::DeviceNotFound)?;
        let swap_chain = create_swap_chain(
            &mut device,
            &surface,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::PresentMode::Fifo,
            &window_size,
        );

        Ok(Self {
            device,
            queue,
            surface,
            window_size,
            swap_chain,
        })
    }

    fn recreate_swap_chain(&mut self) {
        self.swap_chain = create_swap_chain(
            &mut self.device,
            &self.surface,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::PresentMode::Fifo,
            &self.window_size,
        );
    }

    pub(crate) fn resize(&mut self, window_size: winit::dpi::PhysicalSize<u32>) {
        self.window_size = window_size;
        self.recreate_swap_chain();
    }

    pub(crate) fn prepare(
        &mut self,
    ) -> Result<(wgpu::CommandEncoder, wgpu::SwapChainFrame), Error> {
        let frame = self
            .swap_chain
            .get_current_frame()
            .or_else(|err| match err {
                wgpu::SwapChainError::Outdated => {
                    // Recreate the swap chain to mitigate race condition on drawing surface resize.
                    self.recreate_swap_chain();
                    self.swap_chain.get_current_frame()
                }
                err => Err(err),
            })
            .map_err(Error::Swapchain)?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_command_encoder"),
            });

        Ok((encoder, frame))
    }
}

fn create_swap_chain(
    device: &mut wgpu::Device,
    surface: &wgpu::Surface,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    surface_size: &winit::dpi::PhysicalSize<u32>,
) -> wgpu::SwapChain {
    device.create_swap_chain(
        surface,
        &wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format,
            width: surface_size.width,
            height: surface_size.height,
            present_mode,
        },
    )
}
