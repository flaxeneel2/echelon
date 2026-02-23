use anyhow::Ok;
use ruma::events::room::message::SyncRoomMessageEvent;
use tauri::{AppHandle, Emitter};
use serde::Serialize;
use tracing::{error, trace};
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::TransactionId;
use ruma::RoomId;

pub struct ClientEvents;

#[derive(Clone, Serialize)]
struct MessagePayload {
    sender: String,
    room_id: String,
    body: String,
    event_id: String,
}

impl ClientEvents {
    pub fn register_events(client: &matrix_sdk::Client, app_handle: AppHandle) {
        client.add_event_handler(move |event: SyncRoomMessageEvent| {
            let app = app_handle.clone();
            async move {
                Self::on_message(event, app).await;
            }
        });
    }

    async fn on_message(event: SyncRoomMessageEvent, app_handle: AppHandle) {
        trace!("Received message: {:?}", event);

        // Get the content based on event type
        let (sender, body, event_id) = match event {
            SyncRoomMessageEvent::Original(original) => {
                let body = original.content.body().to_string();
                (
                    original.sender.to_string(),
                    body,
                    original.event_id.to_string(),
                )
            },
            SyncRoomMessageEvent::Redacted(redacted) => {
                (
                    redacted.sender.to_string(),
                    "[Redacted message]".to_string(),
                    redacted.event_id.to_string(),
                )
            },
        };

        // Extract message details
        let payload = MessagePayload {
            sender,
            room_id: "unknown".to_string(), // Room ID not directly available in sync events
            body,
            event_id,
        };

        // Emit event to frontend
        if let Err(e) = app_handle.emit("matrix:message", payload) {
            error!("Failed to emit message event: {}", e);
        }
    }

    /// This function sends a message to a specified room. It constructs the message content 
    /// and uses the Matrix client to send it. The transaction ID is generated for tracking 
    /// the message sending process.
    /// 
    /// currently it only supports plain text. (as this is not complete, it is not added to the invoke handler yet)
    pub async fn send_message(room_id: &RoomId, content: String, client: &matrix_sdk::Client) -> bool {

        // Message content and transaction ID
        let content = RoomMessageEventContent::text_plain(content);
        let txn_id = TransactionId::new();

        // Here we attempt to get the room.
        if let Some(room) = client.get_room(&room_id) {

            // We attempt to send the message to the room.
            let result = room.send(content).with_transaction_id(txn_id);
            
            // If we failed to send the message, we log the error and return false.
            if let Err(e) = result.await {
                error!("Failed to send message: {}", e);
                return false;
            }

        } else {
            // if the room is not found, we log an error and return false.
            error!("Room with ID {} not found", room_id);
            return false;
        }

        // If the message is sent successfully, we return true.
        return true;
    }
}

