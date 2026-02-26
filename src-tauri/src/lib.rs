use tauri::Manager;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use crate::user::{login, logout, register, reset_account, restore_session};
use keyring::{Entry, credential};
use getrandom;
use hex;

mod user;
mod client_handler;
mod events;
mod sync_manager;
mod account;

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

            // create new entry object for the stronghold key
            let entry = Entry::new("echelon", "stronghold-key").unwrap();

            // if a credential does not exist on the OS, then create one
            match entry.get_password() {
                Err(keyring::Error::NoEntry) => {
                    // generate a random password in keyring
                    let mut buf = [0u8; 32];
                    getrandom::fill(&mut buf).expect("Failed to generate a random password for keyring");
                    let _ = entry.set_password(&hex::encode(buf));
                },
                _ => println!("Keyring password already set"),
            }

            app.manage(client_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            register,
            login,
            logout,
            restore_session,
            reset_account,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}