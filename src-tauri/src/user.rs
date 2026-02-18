use std::sync::Mutex;
use tauri::State;
use crate::client_handler::ClientHandler;
use crate::ClientState;

#[tauri::command]
pub async fn register(
    username: String,
    password: String,
    homeserver: String,
    registration_token: Option<String>,
    state: State<'_, ClientState>
) -> Result<String, String> {
    println!("Registering user: {} with password", username);

    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("username and password are required".into());
    }

    println!("Registering user: {} with token: {:?}", username, registration_token);

    let handler = ClientHandler::register(username, password, homeserver, registration_token)
        .await
        .map_err(|e| format!("Registration failed: {}", e))?;

    // store the new client handler into the managed state
    let mut write_guard = state.0.write().await;
    *write_guard = Some(handler);

    Ok("registered".into())
}