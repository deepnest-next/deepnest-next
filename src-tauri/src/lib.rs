mod engine;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use engine::{CardId, CardInfo, Format, Library, NestJob, NestOutcome};
use serde::Serialize;
use tauri::{Emitter, Manager, State};

type Lib = Mutex<Library>;

#[tauri::command]
fn import_part(lib: State<'_, Lib>, name: String, content: String) -> Result<CardInfo, String> {
    lib.lock().unwrap().import(&name, &content)
}

#[tauri::command]
fn remove_part(lib: State<'_, Lib>, id: CardId) {
    lib.lock().unwrap().remove(id);
}

#[derive(Clone, Serialize)]
struct Progress {
    elapsed_ms: u64,
    budget_ms: u64,
}

/// Runs the engine on a blocking thread. The engine has no progress callback, so the UI gets
/// `nest-progress` events with elapsed time against the budget.
#[tauri::command]
async fn nest(app: tauri::AppHandle, job: NestJob) -> Result<NestOutcome, String> {
    let budget_ms = job.time_ms;
    let done = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (app, done) = (app.clone(), done.clone());
        std::thread::spawn(move || {
            let start = Instant::now();
            while !done.load(Ordering::Relaxed) {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let _ = app.emit(
                    "nest-progress",
                    Progress {
                        elapsed_ms,
                        budget_ms,
                    },
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let lib = worker.state::<Lib>();
        let mut lib = lib.lock().unwrap();
        lib.nest(&job)
    })
    .await
    .map_err(|e| format!("nesting engine crashed: {e}"));
    done.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    result?
}

/// Write the last nest to `path` (chosen in the save dialog).
#[tauri::command]
fn export_layout(
    lib: State<'_, Lib>,
    path: String,
    format: Format,
    kerf: f64,
    lead_in: f64,
) -> Result<(), String> {
    let bytes = lib.lock().unwrap().export(format, kerf, lead_in)?;
    std::fs::write(&path, bytes).map_err(|e| format!("writing {path}: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)] // only enable instrumentation in development builds
    let devtools = tauri_plugin_devtools::init();

    let mut builder = tauri::Builder::default();

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(Lib::default())
        .invoke_handler(tauri::generate_handler![
            import_part,
            remove_part,
            nest,
            export_layout
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
