// vellobench — native GPU/CPU vector rendering reference for the nest viewport.
// Renders the same hexagon-honeycomb scene (1600x900) as the browser benchmark,
// using Vello CPU (pure Rust, SIMD) and Vello GPU (wgpu/Vulkan), and reports
// per-frame ms: encode (draw-command recording) vs full render vs total.

use std::time::Instant;

// --- scene generation (same honeycomb as bench.html) ---
const W: u16 = 1600;
const H: u16 = 900;

fn hex_points(cx: f64, cy: f64, r: f64) -> Vec<(f64, f64)> {
    (0..6)
        .map(|k| {
            let a = std::f64::consts::PI / 3.0 * k as f64;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

fn gen_shapes(n: usize) -> Vec<Vec<(f64, f64)>> {
    let r = 9.0;
    let gap = 0.6;
    let rad = r + gap;
    let wcell = rad * 3.0f64.sqrt();
    let cols = ((W as f64) / wcell).floor().max(1.0) as usize;
    let rows = (n + cols - 1) / cols;
    let mut shapes = Vec::with_capacity(n);
    let mut y = H as f64;
    'outer: for rw in 0..rows {
        let offx = if rw % 2 == 1 { wcell / 2.0 } else { 0.0 };
        for c in 0..cols {
            let cx = offx + wcell * c as f64 + wcell / 2.0;
            let cy = y - rw as f64 * (3.0f64.sqrt() / 2.0 * rad);
            shapes.push(hex_points(cx, cy, r));
            if shapes.len() >= n {
                break 'outer;
            }
        }
    }
    shapes
}

fn cpu_paths(shapes: &[Vec<(f64, f64)>]) -> Vec<vello_cpu::kurbo::BezPath> {
    use vello_cpu::kurbo::{BezPath, PathEl};
    shapes
        .iter()
        .map(|pts| {
            let mut p = BezPath::new();
            p.move_to((pts[0].0, pts[0].1));
            for pt in &pts[1..] {
                p.line_to((pt.0, pt.1));
            }
            p.close_path();
            p
    })
        .collect()
}

fn bench_cpu(shapes: &[Vec<Vec<(f64, f64)>>]) -> Vec<(usize, f64, f64, f64, u64)> {
    use vello_cpu::color::palette::css::{BLUE, CYAN, GREEN, MAGENTA, ORANGE, PINK, PURPLE, RED, TEAL, YELLOW};
    use vello_cpu::{Level, Pixmap, RasterizerSettings, RenderContext, RenderMode, RenderSettings, Resources};

    let mut results = Vec::new();
    for (n_idx, scene) in shapes.iter().enumerate() {
        let n = scene.len();
        let paths = cpu_paths(scene);

        let settings = RenderSettings {
            level: Level::new(),
            num_threads: 0,
        };
        let rs = RasterizerSettings {
            render_mode: RenderMode::OptimizeSpeed,
            ..Default::default()
        };
        let mut ctx = RenderContext::new_with(W, H, settings);
        let mut resources = Resources::new();
        let mut pixmap = Pixmap::new(W, H);
        let pal = [RED, GREEN, BLUE, YELLOW, MAGENTA, CYAN, ORANGE, PURPLE, PINK, TEAL];

        // warmup
        for _ in 0..3 {
            ctx.reset();
            for (i, p) in paths.iter().enumerate() {
                ctx.set_paint(pal[i % pal.len()]);
                ctx.fill_path(p);
            }
            ctx.flush();
            ctx.render_with(&mut pixmap, &mut resources, rs);
        }

        const FRAMES: usize = 30;
        let mut encode_sum = 0.0f64;
        let mut total_sum = 0.0;
        for _ in 0..FRAMES {
            let t0 = Instant::now();
            ctx.reset();
            for (i, p) in paths.iter().enumerate() {
                ctx.set_paint(pal[i % pal.len()]);
                ctx.fill_path(p);
            }
            let t1 = Instant::now();
            ctx.flush();
            ctx.render_with(&mut pixmap, &mut resources, rs);
            let t2 = Instant::now();
            encode_sum += (t1 - t0).as_secs_f64() * 1000.0;
            total_sum += (t2 - t0).as_secs_f64() * 1000.0;
        }
        let encode = encode_sum / FRAMES as f64;
        let total = total_sum / FRAMES as f64;
        results.push((n, encode, total - encode, total, 0));
        println!(
            "CPU n={:>6} encodeMs={:7.2} renderMs={:8.2} totalMs={:8.2}",
            n, encode, total - encode, total
        );
    }
    results
}

fn bench_gpu(shapes: &[Vec<Vec<(f64, f64)>>]) -> Vec<(usize, f64, f64, f64, u64)> {
    use vello_common::kurbo::{BezPath, PathEl};
    use vello_common::peniko::color::palette::css::{BLUE, CYAN, GREEN, MAGENTA, ORANGE, PINK, PURPLE, RED, TEAL, YELLOW};
    use vello_gpu::{
        ClearSettings, DimensionConstraints, RenderSize, RenderTargetConfig, Renderer, Scene,
        TargetInit, TextureBindings,
    };

    let mut results = Vec::new();

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .expect("no adapter");
    let dev = adapter.get_info();
    println!("# adapter: {} ({:?})", dev.name, dev.backend);
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("bench"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        },
    ))
    .expect("device");

    for (n_idx, scene) in shapes.iter().enumerate() {
        // GPU Scene uses its own width/height in pixels
        let (gw, gh) = (u32::from(W), u32::from(H));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("t"),
            size: wgpu::Extent3d { width: gw, height: gh, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let render_size = RenderSize { width: W, height: H };
        let (mut renderer, mut resources) = Renderer::new(
            &device,
            &RenderTargetConfig { format: texture.format(), width: W, height: H },
        );
        let depth = Renderer::create_depth_texture_view(&device, &render_size);
        let clear = TargetInit::Clear(ClearSettings::default());
        let bindings = TextureBindings::new();

        // build paths once
        let paths: Vec<BezPath> = scene
            .iter()
            .map(|pts| {
                let mut p = BezPath::new();
                p.move_to((pts[0].0, pts[0].1));
                for pt in &pts[1..] {
                    p.line_to((pt.0, pt.1));
                }
                p.close_path();
                p
            })
            .collect();
        let n = scene.len();
        let pal = [RED, GREEN, BLUE, YELLOW, MAGENTA, CYAN, ORANGE, PURPLE, PINK, TEAL];

        // warmup
        for _ in 0..3 {
            let mut scene = Scene::new(W, H);
            for (i, p) in paths.iter().enumerate() {
                scene.set_paint(pal[i % pal.len()]);
                scene.fill_path(p);
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            renderer
                .render(&scene, &mut resources, &device, &queue, &mut enc, &render_size, &view, Some(&depth), &bindings, clear)
                .unwrap();
            queue.submit([enc.finish()]);
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }

        const FRAMES: usize = 30;
        let mut encode_sum = 0.0f64;
        let mut total_sum = 0.0f64;
        for _ in 0..FRAMES {
            let t0 = Instant::now();
            let mut scene = Scene::new(W, H);
            for (i, p) in paths.iter().enumerate() {
                scene.set_paint(pal[i % pal.len()]);
                scene.fill_path(p);
            }
            let t1 = Instant::now();
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            renderer
                .render(&scene, &mut resources, &device, &queue, &mut enc, &render_size, &view, Some(&depth), &bindings, clear)
                .unwrap();
            queue.submit([enc.finish()]);
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let t2 = Instant::now();
            encode_sum += (t1 - t0).as_secs_f64() * 1000.0;
            total_sum += (t2 - t0).as_secs_f64() * 1000.0;
        }
        let encode = encode_sum / FRAMES as f64;
        let total = total_sum / FRAMES as f64;
        results.push((n, encode, total - encode, total, 0));
        println!(
            "GPU n={:>6} encodeMs={:7.2} renderMs={:8.2} totalMs={:8.2}",
            n, encode, total - encode, total
        );
    }
    results
}

fn main() {
    let sizes = [1000usize, 5000, 10000, 25000];
    let shapes: Vec<Vec<Vec<(f64, f64)>>> = sizes
        .iter()
        .map(|n| gen_shapes(*n))
        .collect();
    println!("# vellobench — Vello CPU (single-thread) & Vello GPU (wgpu/Vulkan), 1600x900");
    let cpu = bench_cpu(&shapes);
    let gpu = bench_gpu(&shapes);
    let _ = (cpu, gpu);
    println!("# done");
}