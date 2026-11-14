use anicode_core::{Color, DrawItem};
use vello_cpu::kurbo::{Affine, Rect, Stroke};
use vello_cpu::peniko;
use vello_cpu::{Level, Pixmap, RenderContext, RenderSettings, Resources};

pub struct CpuRenderer {
    context: RenderContext,
    resources: Resources,
    width: u16,
    height: u16,
}

impl CpuRenderer {
    pub fn new(width: u16, height: u16) -> CpuRenderer {
        let settings = RenderSettings {
            // Plain scalar code instead of SIMD, so every platform produces the
            // same pixels and golden frames match on macOS and Debian.
            level: Level::fallback(),
            num_threads: 0,
        };
        CpuRenderer {
            context: RenderContext::new_with(width, height, settings),
            resources: Resources::new(),
            width,
            height,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn render(&mut self, items: &[DrawItem], background: Color) -> Pixmap {
        let context = &mut self.context;
        context.reset();
        if background[3] > 0.0 {
            context.set_transform(Affine::IDENTITY);
            context.set_paint(peniko::Color::new(background));
            context.fill_rect(&Rect::new(
                0.0,
                0.0,
                f64::from(self.width),
                f64::from(self.height),
            ));
        }
        for item in items {
            context.set_transform(item.transform);
            if let Some(fill) = item.fill {
                context.set_paint(peniko::Color::new(fill));
                context.fill_path(&item.path);
            }
            if let Some((stroke, width)) = item.stroke {
                context.set_paint(peniko::Color::new(stroke));
                context.set_stroke(Stroke::new(width));
                context.stroke_path(&item.path);
            }
        }
        context.flush();

        let mut pixmap = Pixmap::new(self.width, self.height);
        context.render(&mut pixmap, &mut self.resources);
        pixmap
    }
}
