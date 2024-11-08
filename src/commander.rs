use teloxide::types::ChatId;
use teloxide::Bot;
use teloxide::macros::BotCommands;
use teloxide::prelude::{Message, Requester};
use chrono::{DateTime, Utc};
use rand::Rng;
use reqwest::Client;
use scraper::{Html, Selector};
use crate::config::Config;
use crate::loader::Error;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Эти команды доступны:")]
pub enum Command {
    #[command(description = "Измеряет твою пипирку")]
    Pisun,
    #[command(description = "Показывает текущий размер")]
    Size,
    #[command(description = "Показывает топ 10 пользователей")]
    Top,
    #[command(description = "Случайный анекдот")]
    Anekdot
}

pub(crate) async fn command_handler(bot: Bot, msg: Message, cmd: Command) -> Result<(), Error> {
    let mut config = Config::new().await;
    match cmd {
        Command::Pisun => pisun_handler(bot, msg, &mut config).await,
        Command::Size => size_handler(bot, msg, &mut config).await,
        Command::Top => top_handler(bot, msg, &mut config).await,
        Command::Anekdot => joke_handler(bot, msg).await,
    }
}

async fn pisun_handler(bot: Bot, msg: Message, config: &mut Config) -> Result<(), Error> {
    let user_id = msg.from.clone().map(|user| user.id.0 as i64).unwrap_or(0);
    let mut user = config.get_or_create_user(user_id).clone();
    
    if can_use_command(user.last_command) {
        let change = match user.pisun {
            0 => generate_random_change(0, 10),
            _ => generate_random_change(-10, 10),
        };
        let message = get_roll_message(change);
        
        user.pisun += change;
        user.last_command = Utc::now();

        if user.pisun < 0 {
            user.pisun = 0;
            bot.send_message(msg.chat.id, "Мои соболезнования. Сегодня у тебя произошла страшная трагедия, твой писюн отпал.").await?;
        } else {
            bot.send_message(msg.chat.id, message).await?;
        }

        config.update_user(user_id, |u| *u = user);
    } else {
        send_cooldown_message(&bot, msg.chat.id).await?;
    }
    
    Ok(())
}

async fn size_handler(bot: Bot, msg: Message, config: &mut Config) -> Result<(), Error> {
    let user_id = msg.from.clone().map(|user| user.id.0 as i64).unwrap_or(0);
    let user = config.get_or_create_user(user_id);
    
    let message = match user.pisun {
        0 => "На данный момент у тебя нет писюна, неудачник!".to_string(),
        _ => format!("Текущий размер твоего писюна аж {} см.", user.pisun),
    };
    bot.send_message(msg.chat.id, message).await?;
    
    Ok(())
}

async fn top_handler(bot: Bot, msg: Message, config: &Config) -> Result<(), Error> {
    let users = config.get_all_users();
    let mut users: Vec<_> = users.values().collect();
    users.sort_by_key(|u| std::cmp::Reverse(u.pisun));
    
    let top = users.iter()
        .take(10)
        .enumerate()
        .map(|(i, u)| format!("{}. {} см.", i + 1, u.pisun))
        .collect::<Vec<_>>()
        .join("\n");
    
    let message = format!("Топ 10 самых больших писюнов:\n{}", top);
    bot.send_message(msg.chat.id, message).await?;
    
    Ok(())
}

async fn joke_handler(bot: Bot, msg: Message) -> Result<(), Error> {
    match get_random_joke().await {
        Ok(joke) => {
            bot.send_message(msg.chat.id, joke).await?;
        }
        Err(e) => {
            eprintln!("Ошибка при получении анекдота: {}", e);
            bot.send_message(msg.chat.id, "Не удалось получить анекдот").await?;
        }
    }
    Ok(())
}


fn can_use_command(last_command: DateTime<Utc>) -> bool {
    let now = chrono::Local::now();
    let last_command_date = last_command.date_naive();
    let current_date = now.date_naive();

    current_date > last_command_date 
}

fn generate_random_change(a: i32, b: i32) -> i32 {
    rand::thread_rng().gen_range(a..=b)
}

async fn send_cooldown_message(bot: &Bot, chat_id: ChatId) -> Result<Message, Error> {
    bot.send_message(
        chat_id,
        "Ты уже измерял свой огрызок сегодня! Попробуй завтра 😊"
    ).await.map_err(|e| e.into())
}

fn get_roll_message(change: i32) -> String {
    let abs_change = change.abs();
    
    match change {
        -10..=-7 => format!(
            "Ахахахах, неудачник. Твой огрызок стал меньше на целых {} см.", 
            abs_change
        ),
        -6..=-3 => format!(
            "Твой, и не без того маленький пенис, стал меньше аж на {} см.", 
            abs_change
        ),
        -2..=-1 => format!(
            "Мои спутники зафиксировали уменьшение твоего полового органа на {} см.", 
            abs_change
        ),
        0 => "Нуууу, что тут можно ещё сказать... Твоя пипирка сегодня не выросла".to_string(),
        1..=3 => format!(
            "Поздравляю! Твой член сегодня вырос на целых {} см.", 
            abs_change
        ),
        4..=7 => format!(
            "Все тяночки вокруг в шоке! Твой гигантский половой орган стал больше на {} см.", 
            abs_change
        ),
        8..=10 => format!(
            "*Ах ты читер!* Каким-то образом ты смог увеличить свой писюн на {} см.", 
            abs_change
        ),
        _ => "Что-то пошло не так...".to_string(),
    }
}

async fn get_random_joke() -> Result<String, reqwest::Error> {
    let client = Client::new();
    let url = "https://baneks.ru/random";
    let response = client.get(url).send().await?;
    let body = response.text().await?;

    let document = Html::parse_document(&body);
    let selector = Selector::parse("article p").unwrap();

    if let Some(element) = document.select(&selector).next() {
        let joke = element.text().collect::<Vec<_>>().join("\n");
        Ok(joke)
    } else {
        Ok("Не удалось найти анекдот".to_string())
    }
}