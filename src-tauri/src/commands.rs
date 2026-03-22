#[path = "commands/account.rs"]
pub mod account;
#[path = "commands/auth.rs"]
pub mod auth;
#[path = "commands/dm.rs"]
pub mod dm;
#[path = "commands/rooms.rs"]
pub mod rooms;
#[path = "commands/spaces.rs"]
pub mod spaces;

pub use account::reset_account;
pub use auth::{login, logout, oauth_login, oauth_register, register, restore_session};
pub use dm::get_dm_rooms;
#[allow(deprecated)]
pub use rooms::get_rooms;
pub use spaces::{get_all_spaces_with_trees, get_space_tree, get_spaces};

