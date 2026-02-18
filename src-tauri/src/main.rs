// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tokio::sync::RwLock;
use crate::client_handler::ClientHandler;

mod client_handler;


pub struct ClientState(pub RwLock<Option<ClientHandler>>);

fn main() {
    tracing_subscriber::fmt::init();

    echelon_lib::run()
}