use std::collections::HashMap;
use futures_util::StreamExt;
use matrix_sdk::room::ParentSpace;
use ruma::{OwnedRoomId, RoomId};
use ruma::api::client::space::get_hierarchy;
use ruma::room::RoomType;
use crate::ClientState;
use tauri::State;
use tracing::{debug, trace};
use crate::account::account_reset_types::AccountResetType;
use crate::rooms::room_info::RoomInfo;
use crate::spaces::space_info::SpaceInfo;

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

#[tauri::command]
pub async fn get_spaces(
    state: State<'_, ClientState>
) -> Result<Vec<SpaceInfo>, String> {
    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.get_client().joined_space_rooms().into_iter().map(|room| {
            let room_id = room.room_id().to_string();
            let name = room.name();
            let topic = room.topic();
            let avatar_url = room.avatar_url().map(|m| m.to_string());
            SpaceInfo {
                id: room_id,
                name,
                topic,
                avatar_url,
                parent_spaces: Vec::new(), // Root spaces have no parents
            }
        }).collect::<Vec<SpaceInfo>>()
    };
    Ok(result)
}

#[tauri::command]
#[deprecated(note = "I don't see why this needs to exist anymore, get_spaces + get_space_tree should cover all the same use cases and more. This function will be removed soon after i discuss w/ others")]
pub async fn get_rooms(
    state: State<'_, ClientState>
) -> Result<Vec<RoomInfo>, String> {
    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        let rooms = client_handler.get_client().joined_rooms();
        let mut room_infos = Vec::new();
        for room in rooms {
            let room_id = room.room_id().to_string();
            let name = room.name();
            let topic = room.topic();
            let avatar_url = room.avatar_url().map(|m| m.to_string());

            room_infos.push(
                RoomInfo {
                    id: room_id,
                    name,
                    topic,
                    avatar_url,
                    parent_spaces: vec!["this function no longer returns parents, use get_space_tree instead".to_string()],
                }
            )
        }
        room_infos
    };
    Ok(result)
}


/// This is relatively expensive, as it fetches the entire hierarchy for each space, but it is useful
/// for the initial load of the app to get all spaces and their parent relationships in one call.
/// For more dynamic use cases, it's better to call get_space_tree for a specific space when needed.
#[tauri::command]
pub async fn get_all_spaces_with_trees(
    state: State<'_, ClientState>
) -> Result<Vec<SpaceInfo>, String> {
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    let client = client_handler.get_client();

    let mut all_spaces: Vec<SpaceInfo> = Vec::new();
    for space in client.joined_space_rooms() {
        let space_id = space.room_id().to_string();
        match get_space_tree(space_id.clone(), state.clone()).await {
            Ok(mut tree) => all_spaces.append(&mut tree),
            Err(e) => debug!("Failed to get tree for space {}: {}", space_id, e),
        }
    }
    Ok(all_spaces)
}

#[tauri::command]
pub async fn get_space_tree(
    space_id: String,
    state: State<'_, ClientState>
) -> Result<Vec<SpaceInfo>, String> {
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    let client = client_handler.get_client();
    let space_room_id = OwnedRoomId::try_from(space_id).map_err(|e| e.to_string())?;
    let room = client.get_room(&*space_room_id);

    let Some(room) = room else {
        return Err("Space not found".to_string());
    };
    if !room.is_space() {
        return Err("Given space ID does not correspond to a space room".to_string());
    }

    let request = get_hierarchy::v1::Request::new(space_room_id.clone());
    let response: get_hierarchy::v1::Response = client.send(request).await.map_err(|e| e.to_string())?;

    debug!("Space hierarchy returned {} rooms", response.rooms.len());

    struct RawRoom {
        id: String,
        name: Option<String>,
        topic: Option<String>,
        avatar_url: Option<String>,
        is_space: bool,
    }

    let mut raw_rooms: Vec<RawRoom> = Vec::new();
    let mut child_to_parent: HashMap<String, String> = HashMap::new();
    let mut id_to_name: HashMap<String, String> = HashMap::new();

    for room_summary in &response.rooms {
        let room_id = room_summary.summary.room_id.to_string();
        let name = room_summary.summary.name.clone();
        let topic = room_summary.summary.topic.clone();
        let avatar_url = room_summary.summary.avatar_url.as_ref().map(|u| u.to_string());
        let is_space = room_summary.summary.room_type.as_ref()
            .map(|t| *t == RoomType::Space)
            .unwrap_or(false);

        id_to_name.insert(room_id.clone(), name.clone().unwrap_or_else(|| "Unnamed".to_string()));

        for child_state in &room_summary.children_state {
            if let Ok(deserialized) = child_state.deserialize() {
                let child_id = deserialized.state_key.to_string();
                // This room is the parent of child_id
                child_to_parent.insert(child_id.clone(), room_id.clone());
            }
        }

        debug!("  Child: {:?} ({}) - is_space: {}", name, room_id, is_space);

        raw_rooms.push(RawRoom { id: room_id, name, topic, avatar_url, is_space });
    }

    let build_parent_path = |start_id: &str| -> Vec<String> {
        let mut path: Vec<String> = Vec::new();
        let mut current = start_id.to_string();
        let mut visited = std::collections::HashSet::new();
        while let Some(parent_id) = child_to_parent.get(&current) {
            if visited.contains(parent_id) {
                break; // cycle guard
            }
            visited.insert(parent_id.clone());
            path.push(id_to_name.get(parent_id).cloned().unwrap_or_else(|| parent_id.clone()));
            current = parent_id.clone();
        }
        path.reverse();
        path
    };

    let mut rooms: Vec<SpaceInfo> = Vec::new();
    for raw in raw_rooms {
        let parent_spaces = build_parent_path(&raw.id);

        rooms.push(SpaceInfo {
            id: raw.id,
            name: raw.name,
            topic: raw.topic,
            avatar_url: raw.avatar_url,
            parent_spaces,
        });
    }

    Ok(rooms)
}
