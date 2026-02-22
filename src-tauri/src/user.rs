use std::collections::HashMap;
use futures_util::future::join_all;
use ruma::{OwnedRoomId};
use ruma::api::client::space::get_hierarchy;
use ruma::events::direct::{OwnedDirectUserIdentifier};
use ruma::events::{AnyGlobalAccountDataEvent, GlobalAccountDataEventType};
use crate::ClientState;
use tauri::State;
use tracing::{debug, trace};
use crate::account::account_reset_types::AccountResetType;
use crate::rooms::room_types::{DmRoom, RawRoom, SpaceRoom};
use crate::spaces::raw_space::{RawSpace};

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
) -> Result<Vec<SpaceRoom>, String> {
    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.get_client().joined_space_rooms().into_iter().map(|room| {
            let room_id = room.room_id().to_string();
            let name = room.name();
            let topic = room.topic();
            let avatar_url = room.avatar_url().map(|m| m.to_string());
            SpaceRoom {
                base: RawRoom {
                    id: room_id,
                    name,
                    topic,
                    avatar_url,
                },
                parent_spaces: Vec::new(), // Root spaces have no parents
            }
        }).collect::<Vec<SpaceRoom>>()
    };
    Ok(result)
}

#[tauri::command]
#[deprecated(note = "I don't see why this needs to exist anymore, get_all_spaces_with_trees should cover all the same use cases and more. This function will be removed soon after i discuss w/ others")]
pub async fn get_rooms(
    state: State<'_, ClientState>
) -> Result<Vec<RawRoom>, String> {
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
                RawRoom {
                    id: room_id,
                    name,
                    topic,
                    avatar_url,
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
) -> Result<HashMap<String, RawSpace>, String> {
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    let client = client_handler.get_client();


    let tasks = client.joined_space_rooms().into_iter().map(|space| {
        let space_id = space.room_id().to_string();
        let state_clone = state.clone();
        async move {
            match get_space_tree(space_id.clone(), state_clone).await {
                Ok(tree) => {
                    Some(RawSpace {
                        raw_room: RawRoom {
                            id: space_id,
                            name: space.name(),
                            topic: space.topic(),
                            avatar_url: space.avatar_url().map(|m| m.to_string()),
                        },
                        rooms: tree,
                    })
                },
                Err(e) => {
                    debug!("Failed to get tree for space {}: {}", space_id, e);
                    None
                },
            }
        }
    });
    let results = join_all(tasks).await;

    let mut root_map: HashMap<String, RawSpace> = HashMap::new();

    for res in results {
        match res {
            Some(raw_space) => {
                root_map.insert(raw_space.raw_room.id.clone(), raw_space);
            },
            None => continue,
        }
    }

    Ok(root_map)
}

#[tauri::command]
pub async fn get_space_tree(
    space_id: String,
    state: State<'_, ClientState>
) -> Result<Vec<SpaceRoom>, String> {
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

    let mut raw_rooms: Vec<RawRoom> = Vec::new();
    let mut child_to_parent: HashMap<String, String> = HashMap::new();
    let mut id_to_name: HashMap<String, String> = HashMap::new();

    for room_summary in &response.rooms {
        let room_id = room_summary.summary.room_id.to_string();
        let name = room_summary.summary.name.clone();
        let topic = room_summary.summary.topic.clone();
        let avatar_url = room_summary.summary.avatar_url.as_ref().map(|u| u.to_string());

        id_to_name.insert(room_id.clone(), name.clone().unwrap_or_else(|| "Unnamed".to_string()));

        for child_state in &room_summary.children_state {
            if let Ok(deserialized) = child_state.deserialize() {
                let child_id = deserialized.state_key.to_string();
                // This room is the parent of child_id
                child_to_parent.insert(child_id.clone(), room_id.clone());
            }
        }

        debug!("  Child: {:?} ({})", name, room_id);

        raw_rooms.push(RawRoom { id: room_id, name, topic, avatar_url });
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

    let mut rooms: Vec<SpaceRoom> = Vec::new();
    for raw in raw_rooms {
        let parent_spaces = build_parent_path(&raw.id);

        rooms.push(SpaceRoom {
            base: RawRoom {
                id: raw.id,
                name: raw.name,
                topic: raw.topic,
                avatar_url: raw.avatar_url,
            },
            parent_spaces,
        });
    }

    Ok(rooms)
}

#[tauri::command]
pub async fn get_dm_rooms(
    state: State<'_, ClientState>
) -> Result<Vec<DmRoom>, String> {
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    let client = client_handler.get_client();
    let mut dm_rooms: Vec<DmRoom> = Vec::new();

    let direct_rooms = client
        .state_store()
        .get_account_data_event(GlobalAccountDataEventType::Direct)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(direct_rooms) = direct_rooms {
        if let Ok(deserialized) = direct_rooms.deserialize() {
           match deserialized {
                AnyGlobalAccountDataEvent::Direct(direct_data) => {
                    let mut dm_room_user_map: HashMap<OwnedRoomId, Vec<OwnedDirectUserIdentifier>> = HashMap::new();
                    for (user_id, room_ids) in direct_data.content {
                        for room_id in room_ids {
                            dm_room_user_map.entry(room_id).or_insert_with(Vec::new).push(user_id.clone());
                        }
                    }
                    for (room_id, user_ids) in dm_room_user_map {
                        if let Some(room) = client.get_room(&room_id) {
                            let name = room.name();
                            let topic = room.topic();
                            let avatar_url = room.avatar_url().map(|u| u.to_string());
                            dm_rooms.push(DmRoom {
                                base: RawRoom {
                                    id: room_id.to_string(),
                                    name,
                                    topic,
                                    avatar_url,
                                },
                                members: user_ids.into_iter().map(|u| u.to_string()).collect(),
                            });
                        }
                    }
                    Ok(dm_rooms)
                }
                _ => Err("Unexpected account data event type, how".to_string()),
            }
        } else {
            Err("Failed to deserialize direct rooms data".to_string())
        }
    } else {
        Err("No direct message rooms found".to_string())
    }
}
