#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Deepnest-Commercial
"""End-to-end smoke test of the desktop app through tauri-driver (W3C WebDriver).

Drives the real binary: the workspace UI (Tauri target) loads, `inspect_drawing` answers,
pressing Nest runs nest_start → nest_status → nest_result in-process and the sheet renders.

    tauri-driver &                       # (with WebKitWebDriver + a display, e.g. Xvfb)
    python3 e2e/smoke.py src-tauri/target/debug/deepnest-next
"""
import json
import sys
import time
import urllib.request

DRIVER = "http://127.0.0.1:4444"
APP = sys.argv[1]


def call(method, path, body=None):
    req = urllib.request.Request(
        DRIVER + path,
        method=method,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.load(r)["value"]


def wait(fn, what, timeout=90):
    t0 = time.time()
    last = None
    while time.time() - t0 < timeout:
        try:
            last = fn()
            if last:
                return last
        except Exception as e:  # element not there yet
            last = e
        time.sleep(0.5)
    raise SystemExit(f"timed out waiting for {what} (last: {last})")


session = call("POST", "/session", {"capabilities": {"alwaysMatch": {"tauri:options": {"application": APP}}}})["sessionId"]
S = f"/session/{session}"
try:
    def find(css):
        return call("POST", f"{S}/element", {"using": "css selector", "value": css})

    def text(css):
        el = find(css)
        return call("GET", f"{S}/element/{list(el.values())[0]}/text")

    def script(src, args=()):
        return call("POST", f"{S}/execute/async", {"script": src, "args": list(args)})

    wait(lambda: find('[data-testid="nest"]'), "the workspace UI")
    print("✓ workspace UI loaded")

    svg = '<svg xmlns="http://www.w3.org/2000/svg" width="30mm" height="20mm" viewBox="0 0 30 20"><rect width="30" height="20" fill="none" stroke="#000"/></svg>'
    insp = script(
        "const done = arguments[arguments.length - 1];"
        "window.__TAURI_INTERNALS__.invoke('inspect_drawing', {drawing: {kind: 'svg', data: arguments[0]}})"
        ".then(r => done({units: r.report.source_units, svg: r.preview_svg.length}), e => done({error: e}));",
        [svg],
    )
    assert "error" not in insp and insp["units"] == "px@96dpi", insp
    print(f"✓ inspect_drawing → {insp}")

    nest = find('[data-testid="nest"]')
    call("POST", f"{S}/element/{list(nest.values())[0]}/click", {})
    status = wait(lambda: (t := text('[data-testid="nest-status"]')) and "placed" in t and t, "the nest result")
    print(f"✓ nest: {status}")
    phase = text('[data-testid="phase"]')
    assert phase == "live nest · done", phase
    pieces = call("POST", f"{S}/execute/sync", {"script": "return document.querySelectorAll('[data-testid=sheet] polygon.outer').length", "args": []})
    assert pieces >= 1, pieces
    print(f"✓ sheet rendered with {pieces} placed part(s)")
finally:
    call("DELETE", S)
print("OK")
