// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Deepnest-Commercial
//! deepnest-next desktop shell: hosts the workspace UI (`@deepnest-next/workspace-ui`, Tauri
//! target) and implements its backend in-process with `nest-service` (see `commands`).

pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::Jobs::default())
        .invoke_handler(tauri::generate_handler![
            commands::inspect_drawing,
            commands::parse_bom,
            commands::nest_start,
            commands::nest_status,
            commands::nest_result,
        ]);

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_devtools::init());
    }

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
