# Building deepnest-next

Frontend: Vite + TypeScript, managed with Bun. Desktop shell: Tauri 2 (`src-tauri/`).

## External dependencies

| Needed for | What |
|---|---|
| Frontend | [Bun](https://bun.sh/) (`bun.lockb` is the lockfile) |
| `src-tauri` | A Rust toolchain plus the platform libraries below |

| Platform | Install |
|---|---|
| Debian/Ubuntu | `sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev patchelf` |
| Fedora | `sudo dnf install gcc pkgconf-pkg-config webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel dbus-devel patchelf` |
| macOS | Xcode Command Line Tools (`xcode-select --install`) |
| Windows | MSVC Build Tools (C++ workload) and the WebView2 runtime (preinstalled on Windows 11) |

`libdbus-1-dev` is for `libdbus-sys`, which Tauri 2.12 pulls in through `tao` 0.37; `patchelf` is only
needed by `bun run build:tauri` when it bundles an AppImage.

Without the Linux packages the Rust build stops in `glib-sys` / `gobject-sys` with
`failed to run custom build command` (pkg-config cannot find `glib-2.0`).

## Build

```bash
bun install
bun run dev:tauri     # dev window (runs `bun run dev` first)
bun run build:tauri   # release bundle (runs `bun run build` first)
```

`tauri::generate_context!` reads `frontendDist` (`../dist`) at compile time, so a bare
`cargo check` / `cargo build` in `src-tauri/` panics with `proc macro panicked` until the
frontend has been built once (`bun run build`) or an empty `dist/` exists.

No system packages? Check the Rust side in a throwaway container:

```bash
docker run --rm -v "$PWD":/src:ro rust:1-bookworm bash -c \
  'apt-get update && apt-get install -y pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev && mkdir /w && cd /src && tar --exclude=node_modules --exclude=target -cf - . | tar -xf - -C /w && mkdir -p /w/dist && cd /w/src-tauri && cargo check --all-targets'
```
