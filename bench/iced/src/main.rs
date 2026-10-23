// icedbench — Iced 0.14 leg of the nest viewport spike.
// Same honeycomb scene + pan/zoom animation as bench.html, two render paths:
//   canvas: canvas::Program, re-tessellates every polygon each frame (lyon)
//   shader: shader widget, static wgpu vertex/index buffers, pan/zoom via uniform
// Prints one RESULT line per (n, variant) and exits.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::mouse;
use iced::wgpu;
use iced::widget::canvas::{self, Path};
use iced::widget::shader::{self, Viewport};
use iced::widget::{canvas as canvas_widget, shader as shader_widget};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Subscription, Task, Theme, Vector};

const W: f32 = 1600.0;
const H: f32 = 900.0;
const SIZES: [usize; 4] = [1000, 5000, 10000, 25000];
const WARMUP: Duration = Duration::from_millis(300);
const RUN: Duration = Duration::from_secs(2);
const PAL: [u32; 20] = [
    0xe6194b, 0x3cb44b, 0xffe119, 0x4363d8, 0xf58231, 0x911eb4, 0x46f0f0, 0xf032e6, 0xbcf60c, 0xfabebe,
    0x008080, 0xe6beff, 0x9a6324, 0xfffac8, 0x800000, 0xaaffc3, 0x808000, 0xffd8b1, 0x000075, 0x808080,
];

fn rgb(i: usize) -> [f32; 3] {
    let c = PAL[i % PAL.len()];
    [((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0]
}

// --- scene generation (same honeycomb as bench.html) ---
fn gen_shapes(n: usize) -> Vec<[[f32; 2]; 6]> {
    let (r, gap) = (9.0f32, 0.6f32);
    let rad = r + gap;
    let wcell = rad * 3.0f32.sqrt();
    let cols = (W / wcell).floor().max(1.0) as usize;
    let mut out = Vec::with_capacity(n);
    let mut rw = 0;
    while out.len() < n {
        let offx = if rw % 2 == 1 { wcell / 2.0 } else { 0.0 };
        for c in 0..cols {
            if out.len() >= n {
                break;
            }
            let cx = offx + wcell * c as f32 + wcell / 2.0;
            let cy = H - rw as f32 * (3.0f32.sqrt() / 2.0 * rad);
            let mut pts = [[0.0; 2]; 6];
            for (k, p) in pts.iter_mut().enumerate() {
                let a = std::f32::consts::PI / 3.0 * k as f32;
                *p = [cx + r * a.cos(), cy + r * a.sin()];
            }
            out.push(pts);
        }
        rw += 1;
    }
    out
}

// Same animation as bench.html: zoom 1±0.5, drifting pan.
fn view_at(t: f32) -> (f32, f32, f32) {
    let ph = t * 2.0 * std::f32::consts::PI;
    (1.0 + 0.5 * ph.sin(), (ph * 40.0) % 2000.0, (ph * 25.0) % 1000.0)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Variant {
    Canvas,
    Shader,
}

struct Phase {
    n: usize,
    variant: Variant,
}

struct App {
    phases: Vec<Phase>,
    idx: usize,
    shapes: Arc<Vec<[[f32; 2]; 6]>>,
    mesh: Arc<Mesh>,
    phase_start: Instant,
    last_frame: Option<Instant>,
    t: f32,
    frame_ms: Vec<f64>,
    // canvas draw() runs during view/draw, which only has &self
    draw_ms: RefCell<Vec<f64>>,
}

#[derive(Debug, Clone)]
enum Message {
    Frame(Instant),
}

#[derive(Debug, Default)]
struct Mesh {
    n: usize,
    verts: Vec<f32>, // x, y, r, g, b
    idx: Vec<u32>,
}

fn build_mesh(shapes: &[[[f32; 2]; 6]]) -> Mesh {
    let mut verts = Vec::with_capacity(shapes.len() * 30);
    let mut idx = Vec::with_capacity(shapes.len() * 12);
    for (i, p) in shapes.iter().enumerate() {
        let base = (i * 6) as u32;
        let c = rgb(i);
        for v in p {
            verts.extend_from_slice(&[v[0], v[1], c[0], c[1], c[2]]);
        }
        for k in 1..=4u32 {
            idx.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }
    Mesh { n: shapes.len(), verts, idx }
}

impl App {
    fn new() -> Self {
        let mut phases = Vec::new();
        for n in SIZES {
            phases.push(Phase { n, variant: Variant::Canvas });
            phases.push(Phase { n, variant: Variant::Shader });
        }
        let shapes = Arc::new(gen_shapes(SIZES[0]));
        let mesh = Arc::new(build_mesh(&shapes));
        Self {
            phases,
            idx: 0,
            shapes,
            mesh,
            phase_start: Instant::now(),
            last_frame: None,
            t: 0.0,
            frame_ms: Vec::new(),
            draw_ms: RefCell::new(Vec::new()),
        }
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        let Message::Frame(now) = msg;
        let elapsed = now.saturating_duration_since(self.phase_start);
        self.t = elapsed.as_secs_f32();
        if elapsed > WARMUP {
            if let Some(prev) = self.last_frame {
                self.frame_ms.push((now - prev).as_secs_f64() * 1000.0);
            }
        } else {
            self.draw_ms.borrow_mut().clear();
        }
        self.last_frame = Some(now);
        if elapsed >= WARMUP + RUN {
            self.report();
            self.idx += 1;
            if self.idx >= self.phases.len() {
                return iced::exit();
            }
            let n = self.phases[self.idx].n;
            if n != self.shapes.len() {
                self.shapes = Arc::new(gen_shapes(n));
                self.mesh = Arc::new(build_mesh(&self.shapes));
            }
            self.phase_start = now;
            self.last_frame = None;
            self.frame_ms.clear();
            self.draw_ms.borrow_mut().clear();
        }
        Task::none()
    }

    fn report(&self) {
        let p = &self.phases[self.idx];
        let f = stats(&self.frame_ms);
        let d = stats(&self.draw_ms.borrow());
        println!(
            "RESULT iced-{:?} n={} frames={} frameMean={:.2} p50={:.2} p95={:.2} drawMean={:.3} drawP95={:.3}",
            p.variant, p.n, self.frame_ms.len(), f.0, f.1, f.2, d.0, d.2
        );
    }

    fn view(&self) -> Element<'_, Message> {
        match self.phases[self.idx.min(self.phases.len() - 1)].variant {
            Variant::Canvas => canvas_widget(CanvasScene { app: self }).width(Length::Fill).height(Length::Fill).into(),
            Variant::Shader => shader_widget(ShaderScene { mesh: self.mesh.clone(), t: self.t })
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::window::frames().map(Message::Frame)
    }
}

fn stats(v: &[f64]) -> (f64, f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f64| s[((s.len() as f64 * p) as usize).min(s.len() - 1)];
    (s.iter().sum::<f64>() / s.len() as f64, pct(0.5), pct(0.95))
}

// --- canvas variant ---
struct CanvasScene<'a> {
    app: &'a App,
}

impl canvas::Program<Message> for CanvasScene<'_> {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<canvas::Geometry> {
        let t0 = Instant::now();
        let (zoom, tx, ty) = view_at(self.app.t);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.scale(zoom);
        frame.translate(Vector::new(-tx, -ty));
        for (i, p) in self.app.shapes.iter().enumerate() {
            let path = Path::new(|b| {
                b.move_to(Point::new(p[0][0], p[0][1]));
                for v in &p[1..] {
                    b.line_to(Point::new(v[0], v[1]));
                }
                b.close();
            });
            let c = rgb(i);
            frame.fill(&path, Color::from_rgb(c[0], c[1], c[2]));
        }
        let geo = frame.into_geometry();
        self.app.draw_ms.borrow_mut().push(t0.elapsed().as_secs_f64() * 1000.0);
        vec![geo]
    }
}

// --- shader variant: static buffers, uniform transform ---
struct ShaderScene {
    mesh: Arc<Mesh>,
    t: f32,
}

impl shader::Program<Message> for ShaderScene {
    type State = ();
    type Primitive = HexPrimitive;

    fn draw(&self, _: &(), _: mouse::Cursor, _: Rectangle) -> HexPrimitive {
        let (zoom, tx, ty) = view_at(self.t);
        HexPrimitive { mesh: self.mesh.clone(), uniform: [zoom, tx, ty, 0.0] }
    }
}

#[derive(Debug)]
struct HexPrimitive {
    mesh: Arc<Mesh>,
    uniform: [f32; 4],
}

struct HexPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    buffers: Option<(usize, wgpu::Buffer, wgpu::Buffer, u32)>,
}

const WGSL: &str = r#"
struct U { zoom: f32, tx: f32, ty: f32, _p: f32 };
@group(0) @binding(0) var<uniform> u: U;
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) col: vec3<f32> };
@vertex fn vs(@location(0) p: vec2<f32>, @location(1) c: vec3<f32>) -> VOut {
    let s = (p - vec2<f32>(u.tx, u.ty)) * u.zoom;
    var o: VOut;
    o.pos = vec4<f32>(s.x / 800.0 - 1.0, 1.0 - s.y / 450.0, 0.0, 1.0);
    o.col = c;
    return o;
}
@fragment fn fs(i: VOut) -> @location(0) vec4<f32> { return vec4<f32>(i.col, 1.0); }
"#;

impl shader::Pipeline for HexPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hex"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hex-u"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hex"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 20,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self { pipeline, uniform, bind_group, buffers: None }
    }
}

impl shader::Primitive for HexPrimitive {
    type Pipeline = HexPipeline;

    fn prepare(&self, p: &mut HexPipeline, device: &wgpu::Device, queue: &wgpu::Queue, _: &Rectangle, _: &Viewport) {
        use wgpu::util::DeviceExt;
        let stale = p.buffers.as_ref().is_none_or(|b| b.0 != self.mesh.n);
        if stale {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hex-v"),
                contents: bytemuck::cast_slice(&self.mesh.verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hex-i"),
                contents: bytemuck::cast_slice(&self.mesh.idx),
                usage: wgpu::BufferUsages::INDEX,
            });
            p.buffers = Some((self.mesh.n, vb, ib, self.mesh.idx.len() as u32));
        }
        queue.write_buffer(&p.uniform, 0, bytemuck::cast_slice(&self.uniform));
    }

    fn draw(&self, p: &HexPipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some((_, vb, ib, count)) = &p.buffers else { return true };
        pass.set_pipeline(&p.pipeline);
        pass.set_bind_group(0, &p.bind_group, &[]);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..*count, 0, 0..1);
        true
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("icedbench")
        .window_size((W, H))
        .run()
}
