# deepnest-next

This repo will be used for version 2.0 of deepnest.

If you like to support this project, keep an eye on our website https://www.deepnest.net where we will soon give more details.

## Desktop app (work in progress)

Tauri 2 + Solid + Tailwind. Import DXF/SVG parts, set sheet and spacing, nest, export a
DXF/SVG cut file. The nesting engine (`nest-adapters` + `nesting-babushka`) runs in-process
behind Tauri commands (`src-tauri/src/engine.rs`); the nest viewport is WebGL2
(`src/lib/renderer.ts`) — see `bench/README.md` for why.

```shell
bun install
bun run dev:tauri      # dev build with hot reload
cd src-tauri && cargo test --lib   # engine tests
```

The engine crates are pulled from private git repositories, so building needs GitHub
access to `deepnest-next/rust-api-server`, `rust-nesting-lib` and `node-dxf2svg-processor`.

## Licensing

This software is provided under a dual license. You may choose one of the following licenses:

### AGPL v3

By default, this software is licensed under the [AGPL v3](LICENSE-AGPL.md). This requires you to make your source code available under the same license if you distribute your software or run it as a network service.

### Commercial License

If the AGPL v3 does not meet your requirements—for instance, you wish to integrate this software into proprietary applications, or do not want to distribute your own source code—then you must acquire a commercial license.

For inquiries regarding commercial licenses, please [contact us](mailto:stuff@deepnest.net).