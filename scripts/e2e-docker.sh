#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Deepnest-Commercial
# Build the app and run e2e/smoke.py under Xvfb + tauri-driver (inside the CI image).
set -euo pipefail
cd "$(dirname "$0")/.."
(cd src-tauri && cargo build)
Xvfb :99 -screen 0 1600x1000x24 >/dev/null 2>&1 &
export DISPLAY=:99
tauri-driver >/tmp/tauri-driver.log 2>&1 &
for _ in $(seq 1 50); do curl -s http://127.0.0.1:4444/status >/dev/null && break; sleep 0.2; done
dbus-run-session -- python3 e2e/smoke.py "$PWD/src-tauri/target/debug/deepnest-next"
