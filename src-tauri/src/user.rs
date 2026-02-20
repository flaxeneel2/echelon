use futures_util::StreamExt;
use matrix_sdk::room::ParentSpace;
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
) -> Result<String, String> {
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
                child_rooms: None,
            }
        }).collect::<Vec<SpaceInfo>>()
    };
    Ok(serde_json::to_string(&result).unwrap())
}

#[tauri::command]
pub async fn get_rooms(
    state: State<'_, ClientState>
) -> Result<String, String> {
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

            // Collect parent spaces from the stream
            let mut parent_spaces_stream = room.parent_spaces().await.unwrap();
            let mut parent_spaces = Vec::new();
            while let Some(result) = parent_spaces_stream.next().await {
                if let Ok(parent_space) = result {
                    match parent_space {
                        ParentSpace::Reciprocal(room) => {
                            parent_spaces.push(room.name().unwrap_or("Unnamed Space".to_string()));
                        }
                        // i dont think i need to worry about these? watch these words come bite me later
                        ParentSpace::WithPowerlevel(_) => {}
                        ParentSpace::Illegitimate(_) => {}
                        ParentSpace::Unverifiable(_) => {}
                    }
                }
            }

            room_infos.push(
                RoomInfo {
                    id: room_id,
                    name,
                    topic,
                    avatar_url,
                    parent_spaces,
                }
            )
        }
        room_infos
    };
    Ok(serde_json::to_string(&result).unwrap())
}

#[tauri::command]
pub async fn get_space_tree(
    space_id: String,
    state: State<'_, ClientState>
) -> Result<String, String> {
    Ok("not implemented".into())
}

/// Gets the children of a space, including nested children, but only for joined rooms/spaces. Subspaces must be explicitly joined to appear in results.
#[tauri::command]
#[deprecated(note = "this is garbage code that needs to be redone.")]
pub async fn get_space_children(
    space_id: String,
    state: State<'_, ClientState>
) -> Result<Vec<SpaceInfo>, String> {
    use std::pin::Pin;
    use std::future::Future;
    use std::collections::HashMap;
    use ruma::RoomId;

    debug!("Getting children for space: {}", space_id);
    debug!("NOTE: This function only returns joined rooms/spaces. Subspaces must be explicitly joined to appear in results.");

    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        let client = client_handler.get_client();

        // Parse space_id to RoomId
        let space_room_id = <&RoomId>::try_from(space_id.as_str()).map_err(|e| format!("Invalid room ID: {}", e))?;

        // Find the space room
        let space = client.joined_rooms()
            .into_iter()
            .find(|room| room.room_id() == space_room_id)
            .ok_or("Space not found")?;

        let space_name = space.name().unwrap_or_else(|| "Unnamed Space".to_string());
        debug!("Found space: {} ({})", space_name, space_id);

        // Build a map of room_id/space_id -> (Room, direct_child_ids, is_space)
        // We need to check ALL rooms (including spaces) to find children
        let all_rooms = client.joined_rooms();
        debug!("Total joined rooms from client: {}", all_rooms.len());
        let mut room_map: HashMap<String, (matrix_sdk::Room, Vec<String>, bool)> = HashMap::new();

        for room in all_rooms {
            let room_id = room.room_id().to_string();
            let room_name = room.name().unwrap_or_else(|| "Unnamed".to_string());
            // Check if this room is a space using the is_space() method
            // This correctly identifies both directly-joined spaces AND subspaces
            // (unlike joined_space_rooms() which only returns directly-joined spaces)
            let is_space = room.is_space();
            debug!("  Room found: '{}' ({}) - is_space: {}", room_name, room_id, is_space);
            room_map.insert(room_id.clone(), (room, Vec::new(), is_space));
        }

        debug!("Total rooms/spaces in map: {} ({} spaces)",
            room_map.len(),
            room_map.values().filter(|(_, _, is_space)| *is_space).count()
        );

        // Now check parent_spaces for each room/space to build parent->child relationships
        // First collect all relationships to avoid borrow checker issues
        let mut parent_child_pairs: Vec<(String, String)> = Vec::new();

        for (child_id, (child_room, _, _)) in room_map.iter() {
            let parent_spaces_result = child_room.parent_spaces().await;

            if let Ok(mut parent_spaces_stream) = parent_spaces_result {
                while let Some(result) = parent_spaces_stream.next().await {
                    if let Ok(parent_space) = result {
                        if let ParentSpace::Reciprocal(parent_room) = parent_space {
                            let parent_id = parent_room.room_id().to_string();
                            debug!("Found parent-child relationship: {} -> {}", parent_id, child_id);
                            parent_child_pairs.push((parent_id, child_id.clone()));
                        }
                    }
                }
            }
        }

        debug!("Total parent-child relationships found: {}", parent_child_pairs.len());

        // Now update the room_map with the collected relationships
        for (parent_id, child_id) in parent_child_pairs {
            if let Some((_, children, _)) = room_map.get_mut(&parent_id) {
                children.push(child_id);
            }
        }

        // Log how many direct children the requested space has
        if let Some((_, children, _)) = room_map.get(&space_id) {
            debug!("Space {} has {} direct children", space_id, children.len());
        } else {
            debug!("Space {} not found in room_map", space_id);
        }

        // Helper function to recursively collect children with their parent paths
        fn collect_children_recursive<'a>(
            space_id: &'a str,
            parent_path: Vec<String>,
            room_map: &'a HashMap<String, (matrix_sdk::Room, Vec<String>, bool)>,
        ) -> Pin<Box<dyn Future<Output = Vec<SpaceInfo>> + Send + 'a>> {
            Box::pin(async move {
                let mut all_children = Vec::new();

                if let Some((_, child_ids, _)) = room_map.get(space_id) {
                    debug!("Processing {} children for space {}", child_ids.len(), space_id);
                    for child_id in child_ids {
                        if let Some((child_room, _, is_space)) = room_map.get(child_id) {
                            let child_name = child_room.name();
                            let child_topic = child_room.topic();
                            let child_avatar_url = child_room.avatar_url().map(|m| m.to_string());

                            debug!("  Child: {} ({}) - is_space: {}",
                                child_name.as_ref().unwrap_or(&"Unnamed".to_string()),
                                child_id,
                                is_space
                            );

                            let space_info = SpaceInfo {
                                id: child_id.clone(),
                                name: child_name.clone(),
                                topic: child_topic,
                                avatar_url: child_avatar_url,
                                parent_spaces: parent_path.clone(),
                                child_rooms: None, // We will populate this for direct children only
                            };

                            all_children.push(space_info);

                            // Only recurse if this child is also a space
                            if *is_space {
                                // Add current child to the path for its descendants
                                let mut child_parent_path = parent_path.clone();
                                child_parent_path.push(child_name.unwrap_or_else(|| "Unnamed Space".to_string()));

                                debug!("  Recursing into subspace: {}", child_id);
                                // Recursively get children of this child space
                                let subchildren = collect_children_recursive(child_id, child_parent_path, room_map).await;
                                debug!("  Subspace {} returned {} children", child_id, subchildren.len());
                                all_children.extend(subchildren);
                            }
                        }
                    }
                }

                all_children
            })
        }

        // Start with the root space name in the path
        let initial_path = vec![space_name];

        collect_children_recursive(&space_id, initial_path, &room_map).await
    };

    debug!("Returning {} total children (including nested)", result.len());
    Ok(result)
}

#[tauri::command]
#[deprecated(note = "this is garbage code that needs to be redone.")]
pub async fn get_space_hierarchy(
    space_id: String,
    state: State<'_, ClientState>
) -> Result<Vec<SpaceInfo>, String> {
    use ruma::RoomId;
    use ruma::api::client::space::get_hierarchy;
    use std::collections::HashMap;

    debug!("Getting space hierarchy (including unjoined) for space: {}", space_id);

    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        let client = client_handler.get_client();

        // Parse space_id to RoomId
        let space_room_id = <&RoomId>::try_from(space_id.as_str())
            .map_err(|e| format!("Invalid room ID: {}", e))?;

        // Get the space room to get its name for the parent path
        let space = client.joined_rooms()
            .into_iter()
            .find(|room| room.room_id() == space_room_id)
            .ok_or("Space not found")?;

        let space_name = space.name().unwrap_or_else(|| "Unnamed Space".to_string());
        debug!("Found space: {} ({})", space_name, space_id);

        // Use the space hierarchy API to get all children (including unjoined)
        let mut request = get_hierarchy::v1::Request::new(space_room_id.to_owned());
        request.max_depth = Some(100u32.into()); // Set a reasonable depth limit

        let hierarchy_response = client.send(request).await
            .map_err(|e| format!("Failed to get space hierarchy: {}", e))?;

        debug!("Space hierarchy returned {} rooms", hierarchy_response.rooms.len());

        // First pass: Build a map of room_id -> (room_data, parent_ids, room_name)
        let mut room_data_map: HashMap<String, (ruma::api::client::space::SpaceHierarchyRoomsChunk, Vec<String>, String)> = HashMap::new();

        for room_summary in hierarchy_response.rooms {
            let room_id = room_summary.summary.room_id.to_string();
            let room_name = room_summary.summary.name.clone().unwrap_or_else(|| "Unnamed".to_string());

            // Initialize with empty parent list
            room_data_map.insert(room_id, (room_summary, Vec::new(), room_name));
        }

        // Second pass: Build parent-child relationships by examining children_state
        // The children_state events tell us what children each room has
        let mut parent_child_relationships: Vec<(String, String)> = Vec::new();

        for (parent_id, (parent_summary, _, _)) in room_data_map.iter() {
            for child_event in &parent_summary.children_state {
                // Deserialize the Raw event to get the state_key (which is the child room ID)
                if let Ok(deserialized) = child_event.deserialize() {
                    let child_id = deserialized.state_key.to_string();
                    debug!("  Found parent->child relationship: {} -> {}", parent_id, child_id);
                    parent_child_relationships.push((parent_id.clone(), child_id));
                }
            }
        }

        // Now update the map with the relationships
        for (parent_id, child_id) in parent_child_relationships {
            if let Some((_, child_parents, _)) = room_data_map.get_mut(&child_id) {
                child_parents.push(parent_id);
            }
        }

        // Third pass: Build parent paths recursively
        fn build_parent_path(
            room_id: &str,
            root_space_id: &str,
            root_space_name: &str,
            room_data_map: &HashMap<String, (ruma::api::client::space::SpaceHierarchyRoomsChunk, Vec<String>, String)>,
            visited: &mut Vec<String>, // To prevent cycles
        ) -> Vec<String> {
            // If this is the root space, return just the root name
            if room_id == root_space_id {
                return vec![root_space_name.to_string()];
            }

            // If we've already visited this room (cycle detection), return empty
            if visited.contains(&room_id.to_string()) {
                return vec![root_space_name.to_string()];
            }
            visited.push(room_id.to_string());

            // Get the parent IDs for this room
            if let Some((_, parent_ids, _)) = room_data_map.get(room_id) {
                if parent_ids.is_empty() {
                    // No parents means it's a direct child of root
                    return vec![root_space_name.to_string()];
                }

                // Get the first parent (in Matrix, a room can have multiple parents, but we'll use the first)
                let parent_id = &parent_ids[0];

                // Recursively build the path from the parent
                let mut parent_path = build_parent_path(parent_id, root_space_id, root_space_name, room_data_map, visited);

                // Add the parent's name to the path, but only if it's not the root space
                // (the root space name is already in the path from the recursive call)
                if *parent_id != root_space_id {
                    if let Some((_, _, parent_name)) = room_data_map.get(parent_id) {
                        parent_path.push(parent_name.clone());
                    }
                }

                parent_path
            } else {
                vec![root_space_name.to_string()]
            }
        }

        // Fourth pass: Create SpaceInfo objects with correct parent paths
        let mut children = Vec::new();

        for (child_id, (room_summary, _, _)) in room_data_map.iter() {
            // Skip the root space itself
            if *child_id == space_id {
                continue;
            }

            let child_name = room_summary.summary.name.clone();
            let child_topic = room_summary.summary.topic.clone();
            let child_avatar_url = room_summary.summary.avatar_url.as_ref().map(|u| u.to_string());

            // Determine if this is a space
            let is_space = room_summary.summary.room_type.as_ref()
                .map(|t| t.to_string() == "m.space")
                .unwrap_or(false);

            // Build the parent path for this room
            let mut visited = Vec::new();
            let parent_path = build_parent_path(child_id, &space_id, &space_name, &room_data_map, &mut visited);

            debug!("  Child: {} ({}) - is_space: {}, num_joined_members: {}, parent_path: {:?}",
                child_name.as_ref().unwrap_or(&"Unnamed".to_string()),
                child_id,
                is_space,
                room_summary.summary.num_joined_members,
                parent_path
            );

            let space_info = SpaceInfo {
                id: child_id.clone(),
                name: child_name,
                topic: child_topic,
                avatar_url: child_avatar_url,
                parent_spaces: parent_path,
                child_rooms: None, // The hierarchy API doesn't provide child room details in the response, so we'll leave this as None
            };

            children.push(space_info);
        }

        children
    };

    debug!("Returning {} total children from hierarchy API", result.len());
    Ok(result)
}
