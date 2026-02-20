use crate::ClientState;
use tauri::State;
use tracing::{debug, trace};
use crate::account::account_reset_types::AccountResetType;

#[tauri::command]
pub async fn register(
    username: String,
    password: String,
    homeserver: String,
    registration_token: Option<String>,
    state: State<'_, ClientState>,
) -> Result<String, String> {
    trace!("Registering user: {} with password", username);

    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("username and password are required".into());
    }

    // Call register in a separate scope to drop the read lock
    let handler = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.register(username, password, homeserver, registration_token).await
    }; // Read lock is dropped here

    let handler = handler.map_err(|e| format!("Registration failed: {}", e))?;

    // Get the client before storing the handler
    let client = handler.get_client().clone();

    // Start the sync task
    handler.sync_manager.start_sync(client).await;

    // Now acquire write lock - read lock has been dropped
    let mut write_guard = state.0.write().await;
    *write_guard = Some(handler);

    Ok("registered".into())
}

#[tauri::command]
pub async fn login(
    username: String,
    password: String,
    homeserver: Option<String>,
    state: State<'_, ClientState>,
) -> Result<String, String> {
    trace!("Logging user: {} with password", username);
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("username and password are required".to_string())
    }

    // Call login in a separate scope to drop the read lock
    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.login(username, password, homeserver.unwrap_or("".to_string())).await
    }; // Read lock is dropped here

    match result {
        Ok(Some(handler)) => {
            // Get the client before storing the handler
            let client = handler.get_client().clone();

            // Start the sync task
            handler.sync_manager.start_sync(client).await;

            // Now acquire write lock - read lock has been dropped
            let mut write_guard = state.0.write().await;
            *write_guard = Some(handler);

            Ok("logged in".into())
        },
        Ok(None) => Err("Login failed: No client handler returned".into()),
        Err(e) => Err(format!("Login failed: {}", e))
    }
}

#[tauri::command]
pub async fn logout(
    state: State<'_, ClientState>,
) -> Result<String, String> {
    debug!("Logging out user...");

    // Stop the sync task first
    {
        let state_r = state.0.read().await;
        if let Some(handler) = state_r.as_ref() {
            handler.sync_manager.stop_sync().await;
        }
    }

    // Clear the client state
    let mut write_guard = state.0.write().await;
    *write_guard = None;

    Ok("logged out".into())
}

#[tauri::command]
pub async fn restore_session(
    username: String,
    homeserver: String,
    state: State<'_, ClientState>,
) -> Result<String, String> {
    debug!("Restoring session for user: {}", username);
    if username.trim().is_empty() {
        return Err("username is required".to_string())
    }

    // Call restore_session in a separate scope to drop the read lock
    let handler = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.restore_session(username, homeserver).await
    }; // Read lock is dropped here

    match handler {
        Ok(Some(handler)) => {
            // Get the client before storing the handler
            let client = handler.get_client().clone();

            // Start the sync task
            handler.sync_manager.start_sync(client).await;

            // Now acquire write lock - read lock has been dropped
            let mut write_guard = state.0.write().await;
            *write_guard = Some(handler);

            Ok("session restored".into())
        },
        Ok(None) => Err("Session restoration failed: No client handler returned".into()),
        Err(e) => Err(format!("Session restoration failed: {}", e))
    }
}

#[tauri::command]
pub async fn reset_account(
    account_reset_type: AccountResetType,
    password: Option<String>,
    key_backup: Option<String>,
    state: State<'_, ClientState>
) -> Result<String, String> {
    // Call reset_account in a separate scope to drop the read lock
    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.reset_account(account_reset_type, password, key_backup).await
    }; // Read lock is dropped here

    match result {
        Ok(_) => Ok("account reset successful".into()),
        Err(e) => Err(format!("Account reset failed: {}", e))
    }
}