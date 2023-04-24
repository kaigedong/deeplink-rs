use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct UserId {
    user_id: String,
}
