use tauri::Manager;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use keyring::Entry;
use getrandom;
use hex;
use iota_stronghold;
use iota_stronghold::Stronghold;
use crate::user::{get_rooms, get_space_tree, get_spaces, login, logout, oauth_login, oauth_register, register, reset_account, restore_session, get_all_spaces_with_trees, get_dm_rooms};

mod user;
mod client_handler;
mod events;
mod sync_manager;
mod account;
mod spaces;
mod rooms;

use client_handler::ClientHandler;

pub struct ClientState(pub RwLock<Option<ClientHandler>>);
pub struct StrongholdS(pub RwLock<Option<Stronghold>>);

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

            // load the key from the entry and create a new keyprovider to encrypt the stronghold
            let kp = iota_stronghold::KeyProvider::with_passphrase_hashed_blake2b(entry.get_password().unwrap()).unwrap();
            let stronghold_dir = format!("{}/stronghold", app.handle().path().app_data_dir()?.display());
            println!("{:?}", stronghold_dir);
            let stronghold = Stronghold::default();
            // need to check for if snapshot file is missing
            let snapshot = stronghold.load_snapshot(&kp, &iota_stronghold::SnapshotPath::from_path(&stronghold_dir));
            println!("{:?}", snapshot);
            // attempt to load a temporary client we called "USERNAME"
            let mut stronghold_client = stronghold.load_client("USERNAME");
            match &stronghold_client {
                Err(iota_stronghold::ClientError::ClientDataNotPresent) => {
                    println!("client no exist :(");
                    stronghold_client = stronghold.create_client("USERNAME");
                },
                _ => {
                    println!("client USERNAME exists yippee");
                },
            }
            // get the store within the client d try to insert values, save and then print the values inside
            let store = stronghold_client.unwrap().store();
            //store.insert(b"test1".to_vec(), b"test123".to_vec(), None);
            //stronghold.commit_with_keyprovider(&iota_stronghold::SnapshotPath::from_path(&stronghold_dir), &kp);

            println!("{:?}", store.get(b"test"));
            println!("{:?}", store.keys());

            app.manage(client_state);
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
            get_dm_rooms
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}