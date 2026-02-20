use tauri::Manager;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use crate::user::{get_rooms, get_space_children, get_space_hierarchy, get_space_tree, get_spaces, login, logout, register, reset_account, restore_session};

mod user;
mod client_handler;
mod events;
mod sync_manager;
mod account;
mod spaces;
mod rooms;

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
        .invoke_handler(tauri::generate_handler![
            register,
            login,
            logout,
            restore_session,
            reset_account,
            get_spaces,
            get_rooms,
            get_space_children,
            get_space_hierarchy,
            get_space_tree
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}