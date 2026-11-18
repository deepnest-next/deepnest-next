# deepnest-next

This repo will be used for version 2.0 of deepnest.

The desktop app (Tauri 2). Its UI is the shared **workspace UI**
([deepnest-next/workspace-ui](https://github.com/deepnest-next/workspace-ui), Tauri target).
The nesting backend runs in-process: the commands in `src-tauri/src/commands.rs` call
`nest-service`, the same pipelines that run behind rust-api-server's HTTP API.

## Run

```sh
bun install                 # Tauri CLI only
bun run ui                  # download + verify the workspace UI release pinned in workspace-ui.lock → ui/dist
bun run build               # tauri build (fetches the UI first)
cd src-tauri && cargo test  # command tests (inspect, BOM, nest job round trip)
```

To work on the UI and the shell together, run `bun run dev:tauri:host` in workspace-ui.
That starts the UI dev server on port 1420. Then run `bun run dev` here; `tauri dev` loads
`http://localhost:1420`. To use a local UI build instead of the release, run `bun run ui:local`.

**End-to-end test.** `e2e/smoke.py` drives the real app through `tauri-driver`. Run it in
the CI image:

```sh
docker build -t deepnest-tauri-ci -f ci/tauri.Dockerfile ci
docker run --rm -v "$SSH_AUTH_SOCK:/ssh-agent" -e SSH_AUTH_SOCK=/ssh-agent -v "$PWD:/src" -w /src \
  deepnest-tauri-ci bash scripts/e2e-docker.sh
```

**Updating the UI.** Set `WORKSPACE_UI_VERSION` and the `tauri` checksum in
`workspace-ui.lock`. Take both from the release's `SHA256SUMS`.

If you like to support this project, keep an eye on our website https://www.deepnest.net where we will soon give more details.

## Licensing

This software is provided under a dual license. You may choose one of the following licenses:

### AGPL v3

By default, this software is licensed under the [AGPL v3](LICENSE-AGPL.md). This requires you to make your source code available under the same license if you distribute your software or run it as a network service.

### Commercial License

If the AGPL v3 does not meet your requirements—for instance, you wish to integrate this software into proprietary applications, or do not want to distribute your own source code—then you must acquire a commercial license.

For inquiries regarding commercial licenses, please [contact us](mailto:stuff@deepnest.net).