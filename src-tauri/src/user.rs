use std::collections::HashMap;
use futures_util::future::join_all;
use matrix_sdk::{Client, RoomMemberships};
use ruma::{OwnedRoomId};
use ruma::api::client::space::get_hierarchy;
use ruma::events::direct::{OwnedDirectUserIdentifier};
use ruma::events::{AnyGlobalAccountDataEvent, GlobalAccountDataEventType, StateEventType};
use crate::ClientState;
use tauri::State;
use tracing::{debug, error, trace};
use crate::account::account_reset_types::AccountResetType;
use crate::rooms::room_types::{DmRoom, RawRoom, SpaceRoom};
use crate::spaces::raw_space::{RawSpace};

/// Log in a user with OAuth2 authentication using their homeserver
///
/// # Arguments
/// * `homeserver` - The URL of the homeserver to log in to.
/// * `state` - The client state containing the Matrix client to perform the login on.
#[tauri::command]
pub async fn oauth_login(
    homeserver: String,
    state: State<'_, ClientState>,
) -> Result<String, String> {
    trace!("Starting OAuth login for homeserver: {}", homeserver);
    if homeserver.trim().is_empty() {
        return Err("homeserver is required".to_string());
    }

    // Call oauth_login in a separate scope to drop the read lock
    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.oauth_login(homeserver, true).await
    };

    match result {
        Ok(Some(handler)) => {
            // Get the client before storing the handler
            let client = handler.get_client().clone();

            // Start the sync task
            handler.sync_manager.start_sync(client).await;

            // Now acquire write lock - read lock has been dropped
            let mut write_guard = state.0.write().await;
            *write_guard = Some(handler);
            Ok("oauth login successful".into())
        },
        Ok(None) => Err("OAuth login failed: no handler returned".into()),
        Err(e) => Err(format!("OAuth login failed: {}", e)),
    }
}

/// Register a user with OAuth2 authentication using their homeserver
///
/// # Arguments
/// * `homeserver` - The URL of the homeserver to register with.
/// * `state` - The client state containing the Matrix client to perform the login on.
#[tauri::command]
pub async fn oauth_register(
    homeserver: String,
    state: State<'_, ClientState>,
) -> Result<String, String> {
    trace!("Starting OAuth register for homeserver: {}", homeserver);
    if homeserver.trim().is_empty() {
        return Err("homeserver is required".to_string());
    }

    // Call oauth_login in a separate scope to drop the read lock
    let result = {
        let state_r = state.0.read().await;
        let client_handler = state_r.as_ref().unwrap();
        client_handler.oauth_login(homeserver, false).await
    };

    match result {
        Ok(Some(handler)) => {
            // Get the client before storing the handler
            let client = handler.get_client().clone();

            // Start the sync task
            handler.sync_manager.start_sync(client).await;

            // Now acquire write lock - read lock has been dropped
            let mut write_guard = state.0.write().await;
            *write_guard = Some(handler);
            Ok("oauth registration successful".into())
        },
        Ok(None) => Err("OAuth registration failed: no handler returned".into()),
        Err(e) => Err(format!("OAuth registration failed: {}", e)),
    }
}

/// Register a new user with the given username, password, and homeserver. Optionally takes a
/// registration token if the homeserver requires it.
///
/// # Arguments
/// * `username` - The desired username for the new account.
/// * `password` - The desired password for the new account.
/// * `homeserver` - The URL of the homeserver to register the account on.
/// * `registration_token` - An optional registration token, required if the homeserver has registration restrictions that require it.
/// * `state` - The client state containing the Matrix client to perform the
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

/// Log in a user with the given username, password, and homeserver.
///
/// # Arguments
/// * `username` - The username of the account to log in to.
/// * `password` - The password of the account to log in to.
/// * `homeserver` - The URL of the homeserver to log in to.
/// * `state` - The client state containing the Matrix client to perform the login on.
#[tauri::command]
pub async fn login(
    username: String,
    password: String,
    homeserver: String,
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
        client_handler.login(username, password, homeserver).await
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

/// Restore a previous session for the given username and homeserver. This will attempt to load the session
/// from the client's store, and if successful, will start the sync loop for that session.
/// This is used for session persistence across app restarts.
///
/// # Arguments
/// * `username` - The username of the session to restore, used to identify the correct
/// * `homeserver` - The homeserver of the session to restore, used to identify the correct session in case of multiple sessions for different homeservers
/// * `state` - The client state containing the Matrix client to perform the session restoration on
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


/// Reset the account based on the specified reset type and provided credentials or backup key.
/// This function handles different types of account resets, such as password reset or complete
/// account wipe, depending on the `AccountResetType` provided.
///
/// # Arguments
/// * `account_reset_type` - The type of account reset to perform, defined by the
///   `AccountResetType` enum, which specifies the reset method (e.g., password reset, key backup, etc.).
/// * `password` - An optional password, required for identity reset
/// * `key_backup` - An optional key backup, required for key backup reset
/// * `state` - The client state containing the Matrix client to perform the reset operation on
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

/// Get the joined spaces. This is for when the frontend only requires spaces and not their full hierarchies
/// This could serve as a quick placeholder while the hierarchies are being loaded.
///
/// # Arguments
/// * `state` - The client state containing the Matrix client to fetch spaces from.
///
/// ### Returns
///
/// A list of `SpaceRoom` objects representing the spaces the user has joined, with their parent spaces listed in order from immediate parent to root space. The list includes only the spaces themselves, without any of the rooms under those spaces. For a more detailed hierarchy including all rooms under each space, use [`get_all_spaces_with_trees`] or [`get_space_tree`] instead.
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
                    is_space: room.is_space(),
                },
                parent_spaces: Vec::new(), // Root spaces have no parents
            }
        }).collect::<Vec<SpaceRoom>>()
    };
    Ok(result)
}


/// Get all joined rooms, including spaces and non-spaces. This was gonna be used to build hierarchy
/// trees in the frontend, but [`get_space_tree`] and [`get_all_spaces_with_trees`] do it all in the backend,
/// making this function redundant. It is still here for now but will definitely be released in future
/// iterations. Please just use [`get_all_spaces_with_trees`] instead
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
                    is_space: room.is_space(),
                }
            )
        }
        room_infos
    };
    Ok(result)
}


/// This is relatively expensive, as it fetches the entire hierarchy for each space, but it is useful
/// for the initial load of the app to get all spaces and their parent relationships in one call.
/// For more dynamic use cases, it's better to call [`get_space_tree`] for a specific space when needed.
///
/// # Arguments
/// * `state` - The client state containing the Matrix client to fetch spaces and their hierarchies
///
/// ### Returns
/// A map of space ID to RawSpace, where each RawSpace contains the basic room info for the space
/// as well as a list of all rooms in the hierarchy under that space. This allows the frontend to
/// build the entire space tree structure without needing to make additional calls to fetch the
/// hierarchy for each individual space.
#[tauri::command]
pub async fn get_all_spaces_with_trees(
    state: State<'_, ClientState>
) -> Result<HashMap<String, RawSpace>, String> {
    // get the client
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    let client = client_handler.get_client();

    // collect all the futures so we can join on all of them at once afterward.
    let tasks = client.joined_space_rooms().into_iter().map(|space| {
        let space_id = space.room_id().to_string();
        let state_clone = state.clone();
        async move {
            // use the get_space_tree function to fetch the entire hierarchy for this space,
            // if it fails for any reason, log the error and skip this space instead of failing the
            // whole function, since we want to be resilient to individual spaces having issues
            match get_space_tree(space_id.clone(), state_clone).await {
                Ok(tree) => {
                    Some(RawSpace {
                        raw_room: RawRoom {
                            id: space_id,
                            name: space.name(),
                            topic: space.topic(),
                            avatar_url: space.avatar_url().map(|m| m.to_string()),
                            is_space: space.is_space(),
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

/// Fetches the entire hierarchy of rooms under a given space, including the parent-child relationships.
/// This is used for building the space tree view in the UI, where we need to know not just the
/// rooms under a space, but also how they are nested within subspaces.
///
/// # Arguments
/// * `space_id` - The ID of the space to fetch the hierarchy for.
/// * `state` - The client state containing the Matrix client to fetch rooms from.
///
/// ### Returns
/// A list of `SpaceRoom` objects representing all rooms in the hierarchy under the given space,
/// with their parent spaces listed in order from immediate parent to root space. The list includes
/// the space itself as the first item, followed by all descendant rooms and spaces.
#[tauri::command]
pub async fn get_space_tree(
    space_id: String,
    state: State<'_, ClientState>
) -> Result<Vec<SpaceRoom>, String> {
    // get client
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    let client = client_handler.get_client();

    // try turn the space id into an [`OwnedRoomId`] and fetch the room, if any of that fails, return an error
    let space_room_id = OwnedRoomId::try_from(space_id).map_err(|e| e.to_string())?;
    let room = client.get_room(&*space_room_id);

    // some basic error handling, if no room then say not found, if room isn't a space then say that instead
    let Some(room) = room else {
        return Err("Space not found".to_string());
    };
    if !room.is_space() {
        return Err("Given space ID does not correspond to a space room".to_string());
    }


    // construct and send the get_hierarchy request for the given space, this will return a list of all rooms in the hierarchy under that space
    let request = get_hierarchy::v1::Request::new(space_room_id.clone());
    let response: get_hierarchy::v1::Response = client.send(request).await.map_err(|e| e.to_string())?;

    debug!("Space hierarchy returned {} rooms", response.rooms.len());

    // we want to get the rooms, as well as the child to parent relationships
    let mut raw_rooms: Vec<RawRoom> = Vec::new();
    let mut child_to_parent: HashMap<String, String> = HashMap::new();
    let mut id_to_name: HashMap<String, String> = HashMap::new();

    for room_summary in response.rooms.iter().skip(1) {
        let room_id = room_summary.summary.room_id.to_string();
        let name = room_summary.summary.name.clone();
        let is_space = client.get_room(&*room_summary.summary.room_id).map(|r| r.is_space()).unwrap_or(false);
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

        raw_rooms.push(RawRoom { id: room_id, name, topic, avatar_url, is_space });
    }

    // we use this to build the parent path for each room, we look up the parent of the room in the
    // child_to_parent map, then look up the name of that parent room in the id_to_name map, and
    // repeat until we reach the root space (which has no parent)
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

    // finally, build them all up into a Vec<SpaceRoom>
    let mut rooms: Vec<SpaceRoom> = Vec::new();
    for raw in raw_rooms {
        let parent_spaces = build_parent_path(&raw.id);

        rooms.push(SpaceRoom {
            base: RawRoom {
                id: raw.id,
                name: raw.name,
                topic: raw.topic,
                avatar_url: raw.avatar_url,
                is_space: raw.is_space,
            },
            parent_spaces,
        });
    }

    Ok(rooms)
}

/// Get the DM rooms, this gets both the 1:1 rooms (ones marked with `m.direct`) and any group DM
/// rooms.
///
/// The fetching of group dms is handled by [`get_orphaned_rooms()`], read its doc to understand more
/// about the logic behind marking a room as a "group dm" room.
///
/// # Arguments
/// * `state` - The client state containing the Matrix client to fetch rooms from.
#[tauri::command]
pub async fn get_dm_rooms(
    state: State<'_, ClientState>
) -> Result<Vec<DmRoom>, String> {
    // get the client
    let state_r = state.0.read().await;
    let client_handler = state_r.as_ref().unwrap();
    let client = client_handler.get_client();
    // final dm rooms vec we will return
    let mut dm_rooms: Vec<DmRoom> = Vec::new();

    // fetch rooms with `m.direct` type account data, these are guaranteed to be 1:1 rooms, so we can directly map the room id to the user id of the other person in the dm and construct our DmRoom objects
    let direct_rooms = client
        .state_store()
        .get_account_data_event(GlobalAccountDataEventType::Direct)
        .await
        .map_err(|e| e.to_string())?;
    // try unwrap the direct rooms we got back
    if let Some(direct_rooms) = direct_rooms {
        // try deserializing it
        if let Ok(deserialized) = direct_rooms.deserialize() {
           match deserialized {
               AnyGlobalAccountDataEvent::Direct(direct_data) => {
                   let mut dm_room_user_map: HashMap<OwnedRoomId, Vec<OwnedDirectUserIdentifier>> = HashMap::new();
                   for (user_id, room_ids) in direct_data.content {
                       for room_id in room_ids {
                           // for each room, map the users in that room, we will use this later to construct the DmRoom objects with the correct members
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
                                   is_space: false,
                               },
                               members: user_ids.into_iter().map(|u| u.to_string()).collect(),
                           });
                       }
                   }
               }
               // we should have only gotten direct data, so if we got something else, log an error
               _ => error!("Unexpected account data event type, how"),
            }
        } else {
            error!("Failed to deserialize direct rooms data")
        }
    } else {
        error!("No direct message rooms found")
    }
    // also get orphaned rooms (group dms, probably) and add it to the list before returning it
    let mut other_rooms = get_orphaned_rooms(client).await?;
    dm_rooms.append(&mut other_rooms);

    Ok(dm_rooms)
}

/// Fetches rooms that the client is joined to which are not spaces and do not have a
/// parent space via m.space.parent events.
/// This is used for catching DM rooms that may not be listed in the m.direct account data (group
/// chats aren't technically `m.direct` because it's not 1:1), as well as any other non-space rooms
/// that are not properly linked to a parent space, for example legacy rooms that were made before spaces existed.
/// This is a fallback to ensure we don't miss any DM rooms, but ideally all DM rooms should be
/// discoverable via the m.direct account data and this function should return an empty list.
///
/// # Arguments
/// * `client` - The Matrix client to use for fetching rooms and their state.
async fn get_orphaned_rooms(client: &Client) -> Result<Vec<DmRoom>, String> {
    let non_space_rooms = client
        .joined_rooms()
        .into_iter()
        .filter(|room| !room.is_space());

    let mut room_futures = Vec::new();

    for room in non_space_rooms {
        room_futures.push(async move {
            // Exclude rooms that have a parent space via m.space.parent state events
            let has_parent_space = room
                .get_state_events(StateEventType::SpaceParent)
                .await
                .map(|events| !events.is_empty())
                .unwrap_or(false);

            if has_parent_space {
                return None;
            }

            // extract the room details and members to construct a DmRoom
            let room_id = room.room_id().to_string();
            let name = room.name();
            let topic = room.topic();
            let avatar_url = room.avatar_url().map(|u| u.to_string());
            let members = room
                .members(RoomMemberships::all())
                .await
                .unwrap_or_default()
                .iter()
                .filter_map(|u| u.display_name().map(|n| n.to_string()))
                .collect();
            Some(DmRoom {
                base: RawRoom { id: room_id, name, topic, avatar_url, is_space: false },
                members,
            })
        });
    }

    // wait for all the dm room futures to complete and collect the results, collecting it into a Vec<DmRoom>
    let other_rooms = join_all(room_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    Ok(other_rooms)
}
