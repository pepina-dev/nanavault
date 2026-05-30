mod backup;
mod blossom;
mod commands;
mod crypto;
mod error;
mod manifest;
mod metadata;
mod recover;
mod relay;
mod secret;
#[cfg(test)]
mod testutil;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::backup,
            commands::generate_backup_code,
            commands::export_manifest,
            commands::recover_with_password,
            commands::recover_with_shares,
            commands::save_recovered,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
