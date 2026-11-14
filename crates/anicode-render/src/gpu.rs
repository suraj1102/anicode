use anicode_core::{Color, DrawItem};
use vello::kurbo::Stroke;
use vello::peniko::{self, Fill};
use vello::wgpu;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use vello_cpu::peniko::ImageAlphaType;
use vello_cpu::{PixelMetadata, Pixmap};

/// Renders with Vello on the GPU (Metal on macOS, Vulkan on Linux) and copies
/// each frame back to memory.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    width: u32,
    height: u32,
    padded_row_bytes: u32,
}

impl GpuRenderer {
    pub fn new(width: u32, height: u32) -> Result<GpuRenderer, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|error| format!("no GPU adapter: {error}"))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|error| format!("cannot open GPU device: {error}"))?;
        let renderer = Renderer::new(
            &device,
            RendererOptions {
                antialiasing_support: AaSupport::area_only(),
                ..Default::default()
            },
        )
        .map_err(|error| format!("cannot start Vello: {error}"))?;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("anicode frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Buffer rows must be a multiple of 256 bytes for texture copies.
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row_bytes = (width * 4).div_ceil(alignment) * alignment;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("anicode readback"),
            size: u64::from(padded_row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(GpuRenderer {
            device,
            queue,
            renderer,
            texture,
            view,
            readback,
            width,
            height,
            padded_row_bytes,
        })
    }

    pub fn render(&mut self, items: &[DrawItem], background: Color) -> Result<Pixmap, String> {
        let mut scene = Scene::new();
        for item in items {
            if let Some(fill) = item.fill {
                scene.fill(
                    Fill::NonZero,
                    item.transform,
                    peniko::Color::new(fill),
                    None,
                    &item.path,
                );
            }
            if let Some((stroke, width)) = item.stroke {
                scene.stroke(
                    &Stroke::new(width),
                    item.transform,
                    peniko::Color::new(stroke),
                    None,
                    &item.path,
                );
            }
        }

        let params = RenderParams {
            base_color: peniko::Color::new(background),
            width: self.width,
            height: self.height,
            antialiasing_method: AaConfig::Area,
        };
        self.renderer
            .render_to_texture(&self.device, &self.queue, &scene, &self.view, &params)
            .map_err(|error| format!("GPU render failed: {error}"))?;

        Ok(self.read_back())
    }

    fn read_back(&self) -> Pixmap {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row_bytes),
                    rows_per_image: None,
                },
            },
            self.texture.size(),
        );
        self.queue.submit([encoder.finish()]);

        self.readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .expect("GPU stopped responding");

        let row_bytes = (self.width * 4) as usize;
        let mut pixels = Vec::with_capacity(row_bytes * self.height as usize);
        {
            let mapped = self
                .readback
                .get_mapped_range(..)
                .expect("readback buffer is mapped");
            for row in mapped.chunks(self.padded_row_bytes as usize) {
                pixels.extend_from_slice(&row[..row_bytes]);
            }
        }
        self.readback.unmap();

        // Vello writes straight alpha; the CPU path and MLT use premultiplied.
        Pixmap::from_parts(
            pixels,
            self.width as u16,
            self.height as u16,
            PixelMetadata {
                may_have_transparency: true,
                alpha_type: ImageAlphaType::Alpha,
            },
        )
    }
}
