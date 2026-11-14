//! Turns a frame's draw items into pixels. Both backends take the same
//! `DrawItem` list and return a premultiplied RGBA `Pixmap`.

mod cpu;
mod gpu;

pub use cpu::CpuRenderer;
pub use gpu::GpuRenderer;
pub use vello_cpu::Pixmap;
