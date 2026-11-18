// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Deepnest-Commercial
//! Backend commands of the workspace UI (`@deepnest-next/workspace-ui`, Tauri target).
//!
//! Names, argument names and payloads follow the UI's host contract
//! (`docs/host-contract.md`, `TAURI_COMMANDS`): the request/response types are the
//! rust-api-server DTOs from `nest-service`, so the desktop app runs exactly the pipelines
//! behind the HTTP API — in-process, without a server.

use nest_service::convert::{self, DrawingInput, InspectResponse};
use nest_service::dto::nest_bom_dto::{BomNestRequestDto, BomParseRequestDto, BomRowDto};
use nest_service::dto::nest_job_dto::{JobStatusResponseDto, JobStore};
use nest_service::jobs::{self, Task};
use serde::Serialize;

/// Error shape the UI decodes into `HostError { code, message }`.
#[derive(Debug, Serialize, PartialEq)]
pub struct HostError {
    pub code: String,
    pub message: String,
}

impl HostError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        HostError {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Nest result bytes (serialised as a number array) + MIME type.
#[derive(Debug, Serialize)]
pub struct NestResult {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// Shared job store (managed Tauri state).
#[derive(Default)]
pub struct Jobs(pub JobStore);

/// Run the CPU-bound nest on Tauri's blocking pool.
fn tauri_spawn(task: Task) {
    tauri::async_runtime::spawn_blocking(task);
}

// ---- command bodies (plain functions, unit-tested without a webview) ----

pub fn inspect_impl(drawing: &DrawingInput) -> Result<InspectResponse, HostError> {
    let bytes = drawing
        .bytes()
        .map_err(|m| HostError::new("invalid_option", m))?;
    if bytes.is_empty() {
        return Err(HostError::new("empty_drawing", "the drawing is empty"));
    }
    convert::inspect(&bytes, &Default::default())
        .map_err(|e| HostError::new(e.code(), e.to_string()))
}

pub fn parse_bom_impl(input: &BomParseRequestDto) -> Result<Vec<BomRowDto>, HostError> {
    nest_service::pipeline::parse_bom_rows(input)
        .map(|r| r.rows)
        .map_err(|e| HostError::new("invalid_bom", format!("{}: {}", e.error, e.reason)))
}

pub fn status_impl(store: &JobStore, id: &str) -> Result<JobStatusResponseDto, HostError> {
    jobs::status(store, id)
        .ok_or_else(|| HostError::new("unknown_job", format!("no nesting job with id {id}")))
}

pub fn result_impl(store: &JobStore, id: &str) -> Result<NestResult, HostError> {
    jobs::result(store, id)
        .map(|d| NestResult {
            bytes: d.body,
            content_type: d.content_type,
        })
        .map_err(|e| {
            let code = match e.status.as_u16() {
                404 => "unknown_job",
                409 => "not_finished",
                _ => "nest_failed",
            };
            HostError::new(code, e.reason)
        })
}

// ---- Tauri commands ----

#[tauri::command]
pub async fn inspect_drawing(drawing: DrawingInput) -> Result<InspectResponse, HostError> {
    tauri::async_runtime::spawn_blocking(move || inspect_impl(&drawing))
        .await
        .map_err(|e| HostError::new("internal", e.to_string()))?
}

#[tauri::command]
pub fn parse_bom(input: BomParseRequestDto) -> Result<Vec<BomRowDto>, HostError> {
    parse_bom_impl(&input)
}

#[tauri::command]
pub fn nest_start(
    jobs_state: tauri::State<'_, Jobs>,
    request: BomNestRequestDto,
) -> Result<String, HostError> {
    Ok(jobs::submit_bom(&jobs_state.0, &tauri_spawn, request))
}

#[tauri::command]
pub fn nest_status(
    jobs_state: tauri::State<'_, Jobs>,
    id: String,
) -> Result<JobStatusResponseDto, HostError> {
    status_impl(&jobs_state.0, &id)
}

#[tauri::command]
pub fn nest_result(
    jobs_state: tauri::State<'_, Jobs>,
    id: String,
) -> Result<NestResult, HostError> {
    result_impl(&jobs_state.0, &id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="20mm" viewBox="0 0 20 20"><rect width="20" height="20" fill="none" stroke="#000"/></svg>"##;

    #[test]
    fn inspect_returns_preview_and_units() {
        let r = inspect_impl(&DrawingInput {
            kind: "svg".into(),
            data: SVG.into(),
        })
        .unwrap();
        // mm canvas of ~20 mm (usvg works in f32, so allow rounding).
        let w: f64 = r
            .preview_svg
            .split("width=\"")
            .nth(1)
            .and_then(|v| v.split("mm").next())
            .unwrap()
            .parse()
            .unwrap();
        assert!((w - 20.0).abs() < 1e-3, "{w}");
        assert!(!r.report.units_assumed);
        let e = inspect_impl(&DrawingInput {
            kind: "dxf".into(),
            data: "%PDF".into(),
        })
        .unwrap_err();
        assert_eq!(e.code, "unknown_format");
    }

    #[test]
    fn bom_rows() {
        let input: BomParseRequestDto =
            serde_json::from_value(serde_json::json!({ "csv": "part,qty\nlid,4\n" })).unwrap();
        let rows = parse_bom_impl(&input).unwrap();
        assert_eq!(rows[0].part, "lid");
        assert_eq!(rows[0].quantity, Some(4));
    }

    #[test]
    fn nest_job_round_trip_matches_the_ui_contract() {
        let store = JobStore::default();
        let request: BomNestRequestDto = serde_json::from_value(serde_json::json!({
            "csv": "part,qty,rot,grain,mirror,optional\np1,2,1,none,no,no\n",
            "library": [{ "name": "p1", "svg": SVG }],
            "auto_units": true, "sheet_width": 100.0, "sheet_height": 60.0,
            "time_budget_ms": 500, "seed": 1, "output": "svg"
        }))
        .unwrap();
        let id = jobs::submit_bom(&store, &jobs::thread_spawn, request);
        let t0 = Instant::now();
        let st = loop {
            let st = status_impl(&store, &id).unwrap();
            let v = serde_json::to_value(&st).unwrap();
            if v["status"] == "done" || v["status"] == "failed" {
                break v;
            }
            assert!(t0.elapsed() < Duration::from_secs(60));
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(st["status"], "done", "{st}");
        assert_eq!(st["dxf"]["placed"], 2);
        let res = result_impl(&store, &id).unwrap();
        assert_eq!(res.content_type, "image/svg+xml");
        // The UI's InvokeBackend expects { bytes: number[], content_type }.
        let wire = serde_json::to_value(&res).unwrap();
        assert!(wire["bytes"].is_array());
        assert_eq!(status_impl(&store, "x").err().unwrap().code, "unknown_job");
    }
}
