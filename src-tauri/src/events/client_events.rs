use ruma::events::room::message::SyncRoomMessageEvent;
use tauri::{AppHandle, Emitter};
use serde::Serialize;
use tracing::{error, trace};

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
}

