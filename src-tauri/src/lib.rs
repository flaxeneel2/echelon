use tauri::Manager;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use crate::user::{login, logout, register};

mod user;
mod client_handler;
mod events;
mod sync_manager;
use client_handler::ClientHandler;

pub struct ClientState(pub RwLock<Option<ClientHandler>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let rt = Runtime::new().expect("failed to create runtime");
            let app_handle = app.handle().clone();
            let client = rt.block_on(ClientHandler::new(app_handle));
            let client_state = ClientState(RwLock::new(Some(client)));

            app.manage(client_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![register, login, logout])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}