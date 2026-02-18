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
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();

    let handler = client_handler.register(username, password, homeserver, registration_token)
        .await
        .map_err(|e| format!("Registration failed: {}", e))?;

    // store the new client handler into the managed state
    let mut write_guard = state.0.write().await;
    *write_guard = Some(handler);

    Ok("registered".into())
}

#[tauri::command]
pub async fn login(
    username: String,
    password: String,
    homeserver: Option<String>,
    state: State<'_, ClientState>
) -> Result<String, String> {
    println!("Logging user: {} with password", username);
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("username and password are required".to_string())
    }
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    match client_handler.login(username, password, homeserver.unwrap_or("".to_string())).await {
        Ok(Some(handler)) => {
            // store the new client handler into the managed state
            let mut write_guard = state.0.write().await;
            *write_guard = Some(handler);
            Ok("logged in".into())
        },
        Ok(None) => Err("Login failed: No client handler returned".into()),
        Err(e) => Err(format!("Login failed: {}", e))
    }
}