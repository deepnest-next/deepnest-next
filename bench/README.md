# Spike: nest viewport rendering options

Question: which render path should the nest viewport use inside the Tauri webview
(WebKitGTK on Linux), and is a native Rust renderer (Vello) worth it?

Scene: flat-top hexagon honeycomb, 1600x900, N = 1k/5k/10k/25k polygons, animated
pan+zoom for 2 s per renderer. Above ~10k shapes part of the sheet is off-screen
(realistic for a zoomed viewport).

- `bench.html` — SVG DOM (viewBox pan) vs Canvas2D (full redraw) vs WebGL2 (static VBO, uniform transform)
- `cdp.mjs` — headless Chromium driver (`node cdp.mjs file://.../bench.html [--use-angle=vulkan|swiftshader]`)
- `wkhub.mjs` — static server + result collector for WebKitGTK (`epiphany http://127.0.0.1:8999/?submit`)
- `gpui/` — GPUI 0.2.2 window, per-frame `paint_path` (two variants)
- `iced/` — Iced 0.14 window, `canvas` vs `shader` widget (custom wgpu pipeline)
- `blitz/` — Dioxus Native 0.8.0-alpha.3 (Blitz) window: custom `<object>` widget (vector scene / own wgpu texture) and plain DOM
- `vello/` — native Vello CPU (`vello_cpu` 0.3) and GPU (`vello_gpu` 0.3, wgpu/Vulkan) on the same scene
- `results/` — raw outputs

Host: AMD RX 5700 (RADV), 28 cores, Fedora 44.

## Results @ 25k shapes (renderer cost per frame, ms)

| Path | SVG build (once) | SVG pan p95 | Canvas2D draw mean / p95 | WebGL2 draw mean |
|---|---|---|---|---|
| Chromium, ANGLE Vulkan | 700–800 | 17.8–34.5 | 12.8–14.7 / 17–20 | 0.01 |
| Chromium, SwiftShader | 869 | 34.9 | 16.3 / 22.8 | 0.7 |
| WebKitGTK (Epiphany ≈ Tauri Linux) | 195 | 17 | 13.3 / 23 | 0.03 |

Canvas2D scales linearly: ~1.3 ms @1k, ~5–6 @5k, ~9 @10k — blows the 16.6 ms budget at ~20k.
WebKitGTK SVG had a 1.1 s first-frame hitch at 1k (cold start), otherwise smooth.

Native Vello, full re-encode each frame (encode = path → strips on CPU):

| N | CPU 1-thread total | GPU (Vulkan) encode / render / total |
|---|---|---|
| 1k | 2.9 | 1.4 / 0.6 / 2.1 |
| 5k | 11.4 | 7.9 / 2.3 / 10.2 |
| 10k | 19.7 | 15.2 / 4.3 / 19.5 |
| 25k | 23.7 | 17.3 / 4.2 / 21.6 |

## Native UI frameworks (real Wayland window, vsync 60 Hz)

Frame interval mean / p95 and CPU draw time per frame, ms. 16.7 = full 60 fps.

| N | GPUI builder (lyon) | GPUI fan (`Path::line_to`) | Iced canvas | Iced shader widget |
|---|---|---|---|---|
| 1k | 20.9 / 33.8, draw 8.6 | 16.7 / 18.6, draw 4.2 | 17.1 / 18.1, draw 2.1 | 16.8 / 18.5 |
| 5k | 41.3 / 53.7, draw 30.3 | 30.9 / 43.7, draw 19.5 | 19.0 / 33.6, draw 9.5 | 16.7 / 19.0 |
| 10k | 69.4 / 106, draw 56.3 | 46.9 / 83.2, draw 31.9 | 33.4 / 37.8, draw 17.7 | 16.7 / 18.3 |
| 25k | 107 / 139, draw 90.4 | 55.8 / 75.4, draw 40.8 | 47.3 / 65.6, draw 36.6 | 16.8 / 19.1 |

- **Iced `shader` widget** (static vertex/index buffers, pan/zoom uniform) holds 60 fps at
  25k — same class as WebGL2 in the webview. Iced exposes wgpu directly, so the viewport
  can be a proper GPU renderer inside an otherwise normal Iced UI.
- **Iced `canvas`** re-tessellates every frame → same scaling as Canvas2D. A `canvas::Cache`
  only helps while the view is static.
- **GPUI** has no public hook for custom GPU pipelines/buffers (0.2.2): every path is
  rebuilt and re-submitted each frame, and per-path cost is high (~1.6 µs build+paint with
  the cheap fan, ~3.6 µs with lyon). Misses 60 fps from ~5k parts. Workarounds: fork GPUI,
  or render the viewport offscreen (wgpu/Vello) and paint it as an image each frame (not benched).
  Zed `main` may be faster than the crates.io 0.2.2 snapshot.

### Dioxus Native / Blitz (0.8.0-alpha.3, Vello hybrid renderer)

| N | widget → anyrender scene | widget → own wgpu texture | DOM: N `<div>`s, CSS transform on parent |
|---|---|---|---|
| 1k | 16.7 / 17.5 | 16.6 / 22.2 | 17.1 / 19.3 |
| 5k | 17.1 / 18.0 | 16.7 / 21.7 | 16.8 / 19.5 |
| 10k | 20.0 / 29.3 | 16.7 / 19.9 | 16.7 / 20.4 |
| 25k | 32.3 / 38.1 | 16.7 / 19.5 | 16.8 / 19.9 |

(frame interval mean / p95, ms)

- **Texture widget** (custom `Widget`, gets the wgpu `DeviceHandle`, renders static
  buffers + uniform into its own texture, registered as an image resource) holds 60 fps
  at 25k — same class as WebGL2 / Iced shader. CPU cost ~0.2 ms/frame.
- **Scene widget** (paths recorded into an anyrender `Scene`, rendered by Vello hybrid)
  is the best "plain vector" result of all frameworks: 60 fps to ~5k, 32 ms at 25k.
- **DOM**: a CSS transform on a container of 25k static `<div>`s stays at 60 fps (Blitz
  does no relayout for transform-only changes). The first attempt re-rendered the
  children in the same component each tick and fell to ~700 ms/frame at 5k — keep the
  big static subtree in its own memoized component. Divs are squares; real parts would
  be inline SVG paths (not benched). At 1k the bench only re-rendered 15× per phase
  (startup), so that row understates the work.
- API is alpha: `CustomWidgetAttr` on `<object data=…>`, `blitz_dom::Widget`
  (`can_create_surfaces` / `paint` / `requires_redraw`) — undocumented, read from source.

Build notes: GPUI 0.2.2 from crates.io needs a patched `xattr` (see `gpui/Cargo.toml`) and
linking against libxcb/libxkbcommon (`RUSTFLAGS="-L $PWD/linkshim"` with symlinks to the
`.so.N` files, or install `libxcb-devel libxkbcommon-devel libxkbcommon-x11-devel`).
Blitz needs fontconfig at build time; without `fontconfig-devel`, `blitz/linkshim/` holds a
hand-written `fontconfig.pc` + `.so` symlink and `blitz/.cargo/config.toml` points
`PKG_CONFIG_PATH` at it (both machine-specific, git-ignored).
Run with `WAYLAND_DISPLAY=wayland-0` from a desktop session; `blitzbench scene|texture|dom`.

## Conclusions

1. **WebGL2 in the existing webview is the clear winner** for the sheet/parts viewport:
   upload geometry once, pan/zoom via a uniform → effectively free, works in WebKitGTK.
   No need to leave Tauri/Solid for rendering performance.
2. **Canvas2D** is an acceptable fallback/simple path up to ~5–10k parts; needs
   caching (Path2D / offscreen bitmaps) beyond that.
3. **SVG DOM** stays fine for import preview, part editing and export (<~5k nodes);
   not for large nests (expensive build, janky pan).
4. **Native Vello (sparse-strips) does not beat webview WebGL** for this workload — CPU
   encode dominates and is paid per frame. Only worth revisiting if the UI moves off
   the webview entirely (e.g. egui/Slint/Iced shell) or for high-quality AA export
   rasterisation.
5. **If leaving the webview: Iced or Blitz fit the viewport; GPUI doesn't (yet).**
   Iced's shader widget and Blitz's texture widget both match WebGL2. GPUI makes
   polished UI easy but the nest view would need a fork or an offscreen-image workaround.
6. **Blitz performs well but is alpha.** It has the best vector path (Vello hybrid), a
   working GPU escape hatch and HTML/CSS styling (closest port from the current
   Solid + Tailwind UI). Risk is API churn and missing CSS/input features, not speed.

## Not done / follow-ups

- egui/eframe leg (`egui_wgpu` paint callback should behave like the Iced shader widget; painter meshes like Iced canvas).
- GPUI from Zed `main`, and an offscreen-texture viewport inside GPUI.

- Classic `vello` 0.11 (GPU compute pipeline) — crate fetched, not benched.
- `vello_cpu` multithreaded (`num_threads > 0`, `multithreading` feature).
- Run inside an actual Tauri window (WebKitGTK + Tauri IPC) rather than Epiphany; also WebView2 (Windows) / WKWebView (macOS).
- Real nest geometry (concave parts with holes, ~50–200 verts) instead of hexagons — WebGL then needs earcut triangulation + stroke outlines.
- Hit-testing / selection cost per path.
