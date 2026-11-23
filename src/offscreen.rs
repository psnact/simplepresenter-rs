//! Renders the live output into an offscreen GPU texture and reads it back, so the projection
//! stream works whether or not any window showing it is visible.

use eframe::{egui, egui_wgpu, wgpu};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

pub struct Offscreen {
    tx: mpsc::Sender<egui::ColorImage>,
    rx: mpsc::Receiver<egui::ColorImage>,
    busy: Arc<AtomicBool>,
}

impl Default for Offscreen {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            busy: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Offscreen {
    /// The most recent finished frame, if any.
    pub fn take(&self, rs: Option<&egui_wgpu::RenderState>) -> Option<egui::ColorImage> {
        if let Some(rs) = rs {
            let _ = rs.device.poll(wgpu::PollType::Poll);
        }
        self.rx.try_iter().last()
    }

    /// Draw `shapes` (laid out in points from the origin) into a `size` pixel image.
    pub fn render(
        &self,
        rs: &egui_wgpu::RenderState,
        ctx: &egui::Context,
        shapes: Vec<egui::epaint::ClippedShape>,
        size: [u32; 2],
        ppp: f32,
    ) {
        if self.busy.load(Ordering::Acquire) {
            return;
        }
        let order = match rs.target_format {
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => [0, 1, 2, 3],
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => [2, 1, 0, 3],
            _ => return,
        };
        let max = rs.device.limits().max_texture_dimension_2d;
        let [w, h] = [size[0].clamp(1, max), size[1].clamp(1, max)];
        let prims = ctx.tessellate(shapes, ppp);
        let (device, queue) = (&rs.device, &rs.queue);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sp_offscreen"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: rs.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [w, h],
            pixels_per_point: ppp,
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sp_offscreen"),
        });
        let extra = {
            let mut renderer = rs.renderer.write();
            let extra = renderer.update_buffers(device, queue, &mut encoder, &prims, &screen);
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("sp_offscreen"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            renderer.render(&mut pass, &prims, &screen);
            extra
        };
        let unpadded = w * 4;
        let padded = wgpu::util::align_to(unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sp_offscreen"),
            size: u64::from(padded) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }));
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
        queue.submit(extra.into_iter().chain(std::iter::once(encoder.finish())));
        self.busy.store(true, Ordering::Release);
        let (tx, busy, ctx, mapped) = (
            self.tx.clone(),
            self.busy.clone(),
            ctx.clone(),
            buffer.clone(),
        );
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                busy.store(false, Ordering::Release);
                if result.is_err() {
                    return;
                }
                let mut pixels = Vec::with_capacity(w as usize * h as usize);
                {
                    let slice = mapped.slice(..);
                    let Ok(data) = slice.get_mapped_range() else {
                        return;
                    };
                    for row in data.chunks(padded as usize) {
                        for px in row
                            .get(..unpadded as usize)
                            .unwrap_or_default()
                            .chunks_exact(4)
                        {
                            let c = |i: usize| px.get(order[i]).copied().unwrap_or(0);
                            pixels.push(egui::Color32::from_rgb(c(0), c(1), c(2)));
                        }
                    }
                }
                mapped.unmap();
                if pixels.len() == w as usize * h as usize {
                    let _ = tx.send(egui::ColorImage::new([w as usize, h as usize], pixels));
                    ctx.request_repaint();
                }
            });
    }
}
