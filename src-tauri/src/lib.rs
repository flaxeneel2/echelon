use tauri::Manager;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use crate::user::{get_rooms, get_space_tree, get_spaces, login, logout, oauth_login, oauth_register, register, reset_account, restore_session, get_all_spaces_with_trees, get_dm_rooms};

mod user;
mod client_handler;
mod events;
mod sync_manager;
mod account;
mod spaces;
mod rooms;
mod secret;

use client_handler::ClientHandler;
use secret::SecretService;

pub struct ClientState(pub RwLock<Option<ClientHandler>>);
pub struct SecretState(SecretService);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let rt = Runtime::new().expect("failed to create runtime");
            let app_handle = app.handle().clone();
            let client = rt.block_on(ClientHandler::new(app_handle));
            let client_state = ClientState(RwLock::new(Some(client)));

            let app_data_dir = app.path().app_data_dir()?;
            let mut stronghold_dir = app_data_dir;
            stronghold_dir.push("stronghold");
            let secret_service = SecretService::new(
                "echelon".to_string(),
                "stronghold-key".to_string(),
                stronghold_dir
            );
            let secret_state = SecretState(secret_service);

            app.manage(client_state);
            app.manage(secret_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            register,
            login,
            logout,
            restore_session,
            reset_account,
            oauth_login,
            oauth_register,
            get_spaces,
            get_rooms,
            get_all_spaces_with_trees,
            get_space_tree,
            get_dm_rooms,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}