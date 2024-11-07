use std::collections::HashMap;
use std::fs;
use teloxide::Bot;
use crate::models::UserData;

pub struct Config {
    bot: Bot,
    file_path: String,
    users: HashMap<i64, UserData>,
}

impl Config {
    pub async fn new() -> Self {
        let bot_token = std::env::var("TELOXIDE_TOKEN").expect("BOT_TOKEN must be set");
        let bot = Bot::new(bot_token);
        let file_path = "users.json".to_string();
        
        // Загружаем пользователей при создании конфига
        let users = match fs::read_to_string(&file_path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => HashMap::new(),
        };
        
        Config { 
            bot,
            file_path,
            users,
        }
    }

    pub fn get_bot(&self) -> &Bot { &self.bot }

    pub fn get_or_create_user(&mut self, user_id: i64) -> &mut UserData {
        if !self.users.contains_key(&user_id) {
            self.users.insert(user_id, UserData::new(user_id));
            self.save_users();
        }
        self.users.get_mut(&user_id).unwrap()
    }

    pub fn get_user(&self, user_id: i64) -> Option<&UserData> {
        self.users.get(&user_id)
    }

    pub fn update_user(&mut self, user_id: i64, update_fn: impl FnOnce(&mut UserData)) {
        if let Some(user) = self.users.get_mut(&user_id) {
            update_fn(user);
            self.save_users();
        }
    }

    // Получить всех пользователей (для команды top)
    pub fn get_all_users(&self) -> &HashMap<i64, UserData> {
        &self.users
    }

    fn save_users(&self) {
        if let Ok(data) = serde_json::to_string_pretty(&self.users) {
            let _ = fs::write(&self.file_path, data);
        }
    }
}