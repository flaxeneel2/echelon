use serde::{Deserialize, Serialize};
use crate::rooms::room_info::RoomInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceInfo {
    pub id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub avatar_url: Option<String>,
    pub parent_spaces: Vec<String>,
    pub child_rooms: Option<Vec<RoomInfo>>,
}