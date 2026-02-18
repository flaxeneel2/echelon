use ruma::events::room::message::SyncRoomMessageEvent;

pub struct ClientEvents;

impl ClientEvents {
    pub fn register_events(client: &matrix_sdk::Client) {
        client.add_event_handler(Self::on_message);
    }

    async fn on_message(event: SyncRoomMessageEvent) {
        println!("Received message: {:?}", event);
    }
}