use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, Duration};


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserData {
    pub user_id: i64,
    pub pisun: i32,
    pub last_command: DateTime<Utc>,
}

impl UserData {
    pub fn new(user_id: i64) -> Self {
        Self {
            user_id,
            pisun: 0,
            last_command: Utc::now() - Duration::days(1),
        }
    }
}