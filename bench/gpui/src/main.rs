// gpuibench — GPUI 0.2 leg of the nest viewport spike.
// Same honeycomb scene + pan/zoom animation as bench.html. GPUI has no public
// hook for custom GPU buffers, so every frame re-submits all paths:
//   builder: PathBuilder (lyon tessellation) per polygon per frame
//   fan:     Path::new + line_to (triangle fan, no lyon) per polygon per frame
// Prints one RESULT line per (n, variant) and quits.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    canvas, div, point, prelude::*, px, rgb, size, App, Application, Bounds, Context, Path, PathBuilder, Pixels,
    Point, Render, Window, WindowBounds, WindowOptions,
};

const W: f32 = 1600.0;
const H: f32 = 900.0;
const SIZES: [usize; 4] = [1000, 5000, 10000, 25000];
const WARMUP: Duration = Duration::from_millis(300);
const RUN: Duration = Duration::from_secs(2);
const PAL: [u32; 20] = [
    0xe6194b, 0x3cb44b, 0xffe119, 0x4363d8, 0xf58231, 0x911eb4, 0x46f0f0, 0xf032e6, 0xbcf60c, 0xfabebe,
    0x008080, 0xe6beff, 0x9a6324, 0xfffac8, 0x800000, 0xaaffc3, 0x808000, 0xffd8b1, 0x000075, 0x808080,
];

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

#[derive(Clone, Copy, Debug)]
enum Variant {
    Builder,
    Fan,
}

struct Bench {
    phases: Vec<(usize, Variant)>,
    idx: usize,
    shapes: Rc<Vec<[[f32; 2]; 6]>>,
    phase_start: Instant,
    last_frame: Option<Instant>,
    frame_ms: Vec<f64>,
    // filled by the canvas paint closure
    draw_ms: Rc<RefCell<Vec<f64>>>,
    done: bool,
}

impl Bench {
    fn new() -> Self {
        let phases = SIZES.iter().flat_map(|&n| [(n, Variant::Builder), (n, Variant::Fan)]).collect();
        Self {
            phases,
            idx: 0,
            shapes: Rc::new(gen_shapes(SIZES[0])),
            phase_start: Instant::now(),
            last_frame: None,
            frame_ms: Vec::new(),
            draw_ms: Rc::new(RefCell::new(Vec::new())),
            done: false,
        }
    }

    fn report(&self) {
        let (n, v) = self.phases[self.idx];
        let f = stats(&self.frame_ms);
        let d = stats(&self.draw_ms.borrow());
        println!(
            "RESULT gpui-{:?} n={} frames={} frameMean={:.2} p50={:.2} p95={:.2} drawMean={:.3} drawP95={:.3}",
            v, n, self.frame_ms.len(), f.0, f.1, f.2, d.0, d.2
        );
    }

    // Advance timing state; returns false once all phases are done.
    fn tick(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now - self.phase_start;
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
                return false;
            }
            let n = self.phases[self.idx].0;
            if n != self.shapes.len() {
                self.shapes = Rc::new(gen_shapes(n));
            }
            self.phase_start = Instant::now();
            self.last_frame = None;
            self.frame_ms.clear();
            self.draw_ms.borrow_mut().clear();
        }
        true
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

impl Render for Bench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.done && !self.tick() {
            self.done = true;
            cx.defer(|cx| cx.quit());
        }
        window.request_animation_frame();

        let shapes = self.shapes.clone();
        let draw_ms = self.draw_ms.clone();
        let variant = self.phases[self.idx.min(self.phases.len() - 1)].1;
        let (zoom, tx, ty) = view_at((Instant::now() - self.phase_start).as_secs_f32());

        div().size_full().bg(rgb(0xffffff)).child(
            canvas(
                |_, _, _| {},
                move |bounds: Bounds<Pixels>, _, window: &mut Window, _: &mut App| {
                    let t0 = Instant::now();
                    let o = bounds.origin;
                    let xf = |p: &[f32; 2]| -> Point<Pixels> {
                        point(o.x + px((p[0] - tx) * zoom), o.y + px((p[1] - ty) * zoom))
                    };
                    for (i, p) in shapes.iter().enumerate() {
                        let color = rgb(PAL[i % PAL.len()]);
                        let path = match variant {
                            Variant::Builder => {
                                let mut b = PathBuilder::fill();
                                let pts: Vec<Point<Pixels>> = p.iter().map(xf).collect();
                                b.add_polygon(&pts, true);
                                match b.build() {
                                    Ok(path) => path,
                                    Err(_) => continue,
                                }
                            }
                            Variant::Fan => {
                                let mut path = Path::new(xf(&p[0]));
                                for v in &p[1..] {
                                    path.line_to(xf(v));
                                }
                                path
                            }
                        };
                        window.paint_path(path, color);
                    }
                    draw_ms.borrow_mut().push(t0.elapsed().as_secs_f64() * 1000.0);
                },
            )
            .size_full(),
        )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(W), px(H)), cx);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |_, cx| cx.new(|_| Bench::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}
