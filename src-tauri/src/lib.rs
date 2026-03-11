use crate::user::{
    get_all_spaces_with_trees, get_dm_rooms, get_rooms, get_space_tree, get_spaces, login, logout,
    oauth_login, oauth_register, register, reset_account, restore_session,
};
use tauri::Manager;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

mod account;
mod client_handler;
mod events;
mod rooms;
mod secret;
mod spaces;
mod sync_manager;
mod user;
mod store;

use client_handler::ClientHandler;
use secret::SecretService;

pub struct ClientState(pub RwLock<Option<ClientHandler>>);
pub struct SecretState(SecretService);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {

            #[cfg(target_os = "android")]
            {
                use tracing_subscriber::util::SubscriberInitExt;
                use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

                keyring::use_native_store(false)?;

                let app_id = app.config().identifier.clone();
                let android_layer = tracing_android::layer(&app_id)
                    .expect("Failed to create android tracing layer");

                let fmt_layer = tracing_subscriber::fmt::layer()
                    .with_ansi(true)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_writer(std::io::stdout);

                tracing_subscriber::registry()
                    .with(android_layer)
                    .with(fmt_layer)
                    .with(tracing_subscriber::filter::LevelFilter::DEBUG)
                    .init();
            }

            let rt = Runtime::new().expect("failed to create runtime");
            let app_handle = app.handle().clone();
            let client = rt.block_on(ClientHandler::new(app_handle));
            let client_state = ClientState(RwLock::new(Some(client)));
            let app_data_dir = app.path().app_data_dir()?;
            let app_id = app.config().identifier.clone();

            let mut stronghold_dir = app_data_dir.clone();
            stronghold_dir.push("stronghold");
            let secret_service = SecretService::new(
                app_id,
                "stronghold-key".to_string(),
                stronghold_dir,
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
