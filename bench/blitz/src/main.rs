// blitzbench — Dioxus Native (Blitz) leg of the nest viewport spike.
// Same honeycomb scene + pan/zoom animation as bench.html. Mode is the first arg:
//   scene:   custom <object> widget, paint() records N filled paths into an
//            anyrender Scene each frame (Vello hybrid renders it)
//   texture: custom widget owning a wgpu texture; static vertex/index buffers,
//            pan/zoom via uniform, texture composited by Blitz as an image resource
//   dom:     N absolutely-positioned <div>s inside a CSS-transformed container
//            (squares, no clip-path); a 1px probe widget measures frame timing
// Prints one RESULT line per n and exits.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyrender::{Paint, PaintScene, RenderContext, ResourceId, Scene};
use dioxus_native::prelude::*;
use dioxus_native::{CustomWidgetAttr, Widget};
use kurbo::{Affine, BezPath, Rect};
use peniko::{Color, Fill, ImageBrush};

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

fn stats(v: &[f64]) -> (f64, f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f64| s[((s.len() as f64 * p) as usize).min(s.len() - 1)];
    (s.iter().sum::<f64>() / s.len() as f64, pct(0.5), pct(0.95))
}

// --- phase/timing state shared by all modes ---
struct Clock {
    mode: String,
    idx: usize,
    phase_start: Instant,
    last_frame: Option<Instant>,
    frame_ms: Vec<f64>,
    draw_ms: Vec<f64>,
}

impl Clock {
    fn new(mode: &str) -> Self {
        Self {
            mode: mode.into(),
            idx: 0,
            phase_start: Instant::now(),
            last_frame: None,
            frame_ms: Vec::new(),
            draw_ms: Vec::new(),
        }
    }

    fn n(&self) -> usize {
        SIZES[self.idx.min(SIZES.len() - 1)]
    }

    fn t(&self) -> f32 {
        (Instant::now() - self.phase_start).as_secs_f32()
    }

    // Called once per painted frame; returns true when the size changed.
    fn frame(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now - self.phase_start;
        if elapsed > WARMUP {
            if let Some(prev) = self.last_frame {
                self.frame_ms.push((now - prev).as_secs_f64() * 1000.0);
            }
        } else {
            self.draw_ms.clear();
        }
        self.last_frame = Some(now);
        if elapsed < WARMUP + RUN {
            return false;
        }
        let f = stats(&self.frame_ms);
        let d = stats(&self.draw_ms);
        println!(
            "RESULT blitz-{} n={} frames={} frameMean={:.2} p50={:.2} p95={:.2} drawMean={:.3} drawP95={:.3}",
            self.mode, self.n(), self.frame_ms.len(), f.0, f.1, f.2, d.0, d.2
        );
        self.idx += 1;
        if self.idx >= SIZES.len() {
            std::process::exit(0);
        }
        self.phase_start = Instant::now();
        self.last_frame = None;
        self.frame_ms.clear();
        self.draw_ms.clear();
        true
    }
}

// --- scene mode: vector paths recorded per frame ---
struct SceneWidget {
    clock: Clock,
    paths: Vec<(BezPath, Color)>,
}

fn build_paths(n: usize) -> Vec<(BezPath, Color)> {
    gen_shapes(n)
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut b = BezPath::new();
            b.move_to((p[0][0] as f64, p[0][1] as f64));
            for v in &p[1..] {
                b.line_to((v[0] as f64, v[1] as f64));
            }
            b.close_path();
            let c = rgb(i);
            (b, Color::new([c[0], c[1], c[2], 1.0]))
        })
        .collect()
}

impl Widget for SceneWidget {
    fn requires_redraw(&self) -> bool {
        true
    }

    fn paint(&mut self, _: &mut dyn RenderContext, _: &blitz_dom::node::ComputedStyles, _: u32, _: u32, scale: f64) -> Scene {
        if self.clock.frame() {
            self.paths = build_paths(self.clock.n());
        }
        let t0 = Instant::now();
        let (zoom, tx, ty) = view_at(self.clock.t());
        let xf = Affine::scale(scale * zoom as f64) * Affine::translate((-tx as f64, -ty as f64));
        let mut scene = Scene::new();
        for (path, color) in &self.paths {
            scene.fill(Fill::NonZero, xf, Paint::Solid(*color), None, path);
        }
        self.clock.draw_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        scene
    }
}

// --- texture mode: own wgpu pipeline into a texture ---
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

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct TextureWidget {
    clock: Clock,
    gpu: Option<Gpu>,
    target: Option<(u32, u32, wgpu::Texture, ResourceId)>,
    mesh: Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl TextureWidget {
    fn upload_mesh(&mut self) {
        use wgpu::util::DeviceExt;
        let Some(gpu) = &self.gpu else { return };
        let shapes = gen_shapes(self.clock.n());
        let mut verts: Vec<f32> = Vec::with_capacity(shapes.len() * 30);
        let mut idx: Vec<u32> = Vec::with_capacity(shapes.len() * 12);
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
        let vb = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hex-v"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hex-i"),
            contents: bytemuck::cast_slice(&idx),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.mesh = Some((vb, ib, idx.len() as u32));
    }
}

impl Widget for TextureWidget {
    fn requires_redraw(&self) -> bool {
        true
    }

    fn can_create_surfaces(&mut self, ctx: &mut dyn RenderContext) {
        let Some(any) = ctx.renderer_specific_context() else { return };
        let Ok(handle) = any.downcast::<dioxus_native::DeviceHandle>() else {
            eprintln!("renderer context is not a wgpu DeviceHandle");
            return;
        };
        let device = handle.device.clone();
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
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hex"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 20,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        self.gpu = Some(Gpu { device, queue: handle.queue.clone(), pipeline, uniform, bind_group });
        self.target = None;
        self.upload_mesh();
    }

    fn destroy_surfaces(&mut self) {
        self.gpu = None;
        self.target = None;
        self.mesh = None;
    }

    fn paint(&mut self, ctx: &mut dyn RenderContext, _: &blitz_dom::node::ComputedStyles, width: u32, height: u32, _: f64) -> Scene {
        if self.clock.frame() {
            self.upload_mesh();
        }
        let mut scene = Scene::new();
        let Some(gpu) = &self.gpu else { return scene };
        let t0 = Instant::now();

        if self.target.as_ref().is_none_or(|t| t.0 != width || t.1 != height) {
            if let Some((.., old)) = self.target.take() {
                ctx.unregister_resource(old);
            }
            let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("viewport"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            match ctx.try_register_custom_resource(Box::new(tex.clone())) {
                Ok(id) => self.target = Some((width, height, tex, id)),
                Err(e) => {
                    eprintln!("register texture failed: {e:?}");
                    return scene;
                }
            }
        }
        let (_, _, tex, id) = self.target.as_ref().unwrap();

        let (zoom, tx, ty) = view_at(self.clock.t());
        gpu.queue.write_buffer(&gpu.uniform, 0, bytemuck::cast_slice(&[zoom, tx, ty, 0.0f32]));
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hex"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some((vb, ib, count)) = &self.mesh {
                pass.set_pipeline(&gpu.pipeline);
                pass.set_bind_group(0, &gpu.bind_group, &[]);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*count, 0, 0..1);
            }
        }
        gpu.queue.submit([enc.finish()]);

        let brush = ImageBrush { image: *id, sampler: Default::default() };
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Paint::Resource(brush),
            None,
            &Rect::new(0.0, 0.0, width as f64, height as f64),
        );
        self.clock.draw_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        scene
    }
}

// --- dom mode: probe widget only times frames, DOM is driven by a signal ---
struct ProbeWidget {
    clock: Rc<RefCell<Clock>>,
}

impl Widget for ProbeWidget {
    fn requires_redraw(&self) -> bool {
        true
    }

    fn paint(&mut self, _: &mut dyn RenderContext, _: &blitz_dom::node::ComputedStyles, _: u32, _: u32, _: f64) -> Scene {
        self.clock.borrow_mut().frame();
        Scene::new()
    }
}

fn mode() -> String {
    std::env::args().nth(1).unwrap_or_else(|| "scene".into())
}

fn app() -> Element {
    match mode().as_str() {
        "texture" => rsx! {
            object {
                style: "display:block;width:100vw;height:100vh",
                data: CustomWidgetAttr::new(TextureWidget {
                    clock: Clock::new("texture"),
                    gpu: None,
                    target: None,
                    mesh: None,
                }),
            }
        },
        "dom" => rsx! { DomBench {} },
        _ => rsx! {
            object {
                style: "display:block;width:100vw;height:100vh",
                data: CustomWidgetAttr::new(SceneWidget { clock: Clock::new("scene"), paths: build_paths(SIZES[0]) }),
            }
        },
    }
}

#[component]
fn DomBench() -> Element {
    let clock = use_hook(|| Rc::new(RefCell::new(Clock::new("dom"))));
    let mut view = use_signal(|| (SIZES[0], 0.0f32));
    let probe_clock = clock.clone();
    let loop_clock = clock.clone();
    use_future(move || {
        let clock = loop_clock.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(4)).await;
                let c = clock.borrow();
                view.set((c.n(), c.t()));
            }
        }
    });
    let (n, t) = view();
    let (zoom, tx, ty) = view_at(t);
    if std::env::var_os("BENCH_TRACE").is_some() {
        eprintln!("render n={n} t={t:.3}");
    }
    rsx! {
        div { style: "position:relative;width:100vw;height:100vh;overflow:hidden;background:white",
            div { style: "position:absolute;left:0;top:0;transform-origin:0 0;transform:scale({zoom}) translate({-tx}px,{-ty}px)",
                Cells { n }
            }
            object {
                style: "position:absolute;left:0;top:0;width:1px;height:1px",
                data: CustomWidgetAttr::new(ProbeWidget { clock: probe_clock.clone() }),
            }
        }
    }
}

// Separate component so a pan/zoom tick only diffs the container style, not N children.
#[component]
fn Cells(n: usize) -> Element {
    let shapes = use_memo(move || gen_shapes(n));
    rsx! {
        for (i, p) in shapes.read().iter().enumerate() {
            div {
                key: "{i}",
                style: "position:absolute;left:{p[3][0]}px;top:{p[1][1]}px;width:16px;height:16px;background:#{PAL[i % PAL.len()]:06x}",
            }
        }
    }
}

fn main() {
    let attrs = dioxus_native::WindowAttributes::default()
        .with_title("blitzbench")
        .with_surface_size(dioxus_native::LogicalSize::new(W as f64, H as f64));
    dioxus_native::launch_cfg(app, vec![], vec![Box::new(dioxus_native::Config::new().with_window_attributes(attrs))]);
}
