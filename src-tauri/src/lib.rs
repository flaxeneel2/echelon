use tauri::Manager;
use tokio::sync::RwLock;
use crate::user::register;

mod user;
mod client_handler;
use client_handler::ClientHandler;

pub struct ClientState(pub RwLock<Option<ClientHandler>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let client_state = ClientState(RwLock::new(None));
            app.manage(client_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![register])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}