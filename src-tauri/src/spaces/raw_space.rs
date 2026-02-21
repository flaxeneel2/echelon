use serde::{Deserialize, Serialize};
use crate::rooms::room_types::SpaceRoom;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSpace {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) topic: Option<String>,
    pub(crate) avatar_url: Option<String>,
    pub(crate) rooms: Vec<SpaceRoom>,
}