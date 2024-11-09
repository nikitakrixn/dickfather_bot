use rand::seq::SliceRandom;
use teloxide::types::ChatId;
use teloxide::Bot;
use teloxide::macros::BotCommands;
use teloxide::prelude::{Message, Requester};
use chrono::{DateTime, Local, Utc};
use rand::Rng;
use reqwest::Client;
use scraper::{Html, Selector};
use crate::config::Config;
use crate::loader::Error;
use crate::models::{TrainingExercise, get_training_exercises};

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
    Anekdot,
    #[command(description = "Тренировка твоего писюна")]
    Train,
    #[command(description = "Погода")]
    Weather,
    #[command(description = "Случайный мем")]
    Meme
}

pub(crate) async fn command_handler(bot: Bot, msg: Message, cmd: Command) -> Result<(), Error> {
    let mut config = Config::new().await;
    match cmd {
        Command::Pisun => pisun_handler(bot, msg, &mut config).await,
        Command::Size => size_handler(bot, msg, &mut config).await,
        Command::Top => top_handler(bot, msg, &mut config).await,
        Command::Anekdot => joke_handler(bot, msg).await,
        Command::Train => train_handler(bot, msg, &mut config).await,
        Command::Weather => weather_handler(bot, msg, &mut config).await,
        Command::Meme => meme_handler(bot, msg).await
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

async fn train_handler(bot: Bot, msg: Message, config: &mut Config) -> Result<(), Error> {
    let user_id = msg.from.clone().map(|user| user.id.0 as i64).unwrap_or(0);
    let mut user = config.get_or_create_user(user_id).clone();
    
    let now = Utc::now();
    let can_train = now.date_naive() > user.last_train.date_naive();

    if can_train {
        let (exercise, result) = generate_training_exercise();
        let (change, message) = process_training_result(result, user.pisun);

        user.pisun = (user.pisun + change).max(0);
        user.last_train = now;

        let response = format!(
            "{}\n\n{}",
            exercise.description,
            message
        );

        bot.send_message(msg.chat.id, response).await?;
        config.update_user(user_id, |u| *u = user);
    } else {
        bot.send_message(msg.chat.id, "Ты уже тренировался сегодня! Возвращайся завтра 💪🍆").await?;
    }
    
    Ok(())
}
async fn weather_handler(bot: Bot, msg: Message, config: &mut Config) -> Result<(), Error> {
    let url = "https://api.open-meteo.com/v1/forecast?latitude=55&longitude=73.70&current=temperature_2m,relative_humidity_2m,apparent_temperature,is_day,precipitation,rain,showers,snowfall,weathercode,windspeed_10m&hourly=temperature_2m,precipitation_probability,weathercode&daily=temperature_2m_max,temperature_2m_min,sunrise,sunset&wind_speed_unit=ms&timeformat=unixtime&timezone=auto&forecast_days=3";

    let response = reqwest::get(url).await.unwrap();

    let weather_info: serde_json::Value = match response.status().is_success() {
        true => {
            let text = response.text().await.unwrap();

            let data = serde_json::from_str(&text).unwrap();
            data
        }
        _ => {
            panic!("Oh no! A wild error appears: {:?}", response.status());
        }
    };

    let current = &weather_info["current"];
    let daily = &weather_info["daily"];
    let hourly = &weather_info["hourly"];

    let current_temp = current["temperature_2m"].as_f64().unwrap();
    let apparent_temp = current["apparent_temperature"].as_f64().unwrap();
    let humidity = current["relative_humidity_2m"].as_f64().unwrap();
    let is_day = current["is_day"].as_i64().unwrap();
    let wind_speed = current["windspeed_10m"].as_f64().unwrap();
    let weather_code = current["weathercode"].as_i64().unwrap();

    let weather_description = get_weather_description(weather_code);
    let weather_emoji = get_weather_emoji(is_day, weather_code);

    let now: DateTime<Local> = Local::now();

    let max_temp = daily["temperature_2m_max"][0].as_f64().unwrap();
    let min_temp = daily["temperature_2m_min"][0].as_f64().unwrap();
    let sunrise = DateTime::from_timestamp(daily["sunrise"][0].as_i64().unwrap(), 0).unwrap().with_timezone(&Local);
    let sunset = DateTime::from_timestamp(daily["sunset"][0].as_i64().unwrap(), 0).unwrap().with_timezone(&Local);

    let clothing_recommendation = get_clothing_recommendation(current_temp);

    let precipitation_prob = hourly["precipitation_probability"]
        .as_array()
        .unwrap()
        .iter()
        .take(24)
        .map(|v| v.as_i64().unwrap())
        .max()
        .unwrap();

    let mut weather_message = format!(
        "{} Погода в Омске на {}\n\n\
        Текущая температура: {:.1}°C (ощущается как {:.1}°C)\n\
        {}\n\
        Влажность: {}%\n\
        Скорость ветра: {:.1} м/с\n\
        Вероятность осадков: {}%\n\n\
        Максимальная температура сегодня: {:.1}°C\n\
        Минимальная температура сегодня: {:.1}°C\n\
        Восход солнца: {}\n\
        Закат солнца: {}\n\n\
        Рекомендация по одежде: {}\n\n\
        Прогноз на ближайшие дни:\n{}",
        weather_emoji,
        now.format("%d.%m.%Y %H:%M").to_string(),
        current_temp,
        apparent_temp,
        weather_description,
        humidity,
        wind_speed,
        precipitation_prob,
        max_temp,
        min_temp,
        sunrise.format("%H:%M").to_string(),
        sunset.format("%H:%M").to_string(),
        clothing_recommendation,
        get_forecast(daily, hourly)
    );

    if rand::thread_rng().gen_bool(0.1) {
        let user_id = msg.from.clone().map(|user| user.id.0 as i64).unwrap_or(0);
        let mut user = config.get_or_create_user(user_id).clone();
        
        let pisun_change = calculate_pisun_change(current_temp);
        user.pisun = (user.pisun + pisun_change).max(0);
        
        let pisun_message = if pisun_change > 0 {
            format!("\n\nНеожиданно! Из-за погоды твой писюн вырос на {} см!", pisun_change)
        } else if pisun_change < 0 {
            format!("\n\nОй-ой! Из-за погоды твой писюн уменьшился на {} см!", pisun_change.abs())
        } else {
            "\n\nПогода не повлияла на размер твоего писюна.".to_string()
        };

        weather_message.push_str(&pisun_message);
        weather_message.push_str(&format!("\nТекущий размер твоего писюна: {} см.", user.pisun));

        config.update_user(user_id, |u| *u = user);
    }

    bot.send_message(msg.chat.id, weather_message).await?;

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
            "Ахахахах, неудачник. Твой огрызок стал меньше на целых {} см! 🍆📉", 
            abs_change
        ),
        -6..=-3 => format!(
            "Твой, и не без того маленький пенис, стал меньше аж на {} см. 😔🍆🤏", 
            abs_change
        ),
        -2..=-1 => format!(
            "Мои спутники зафиксировали уменьшение твоего полового органа на {} см. 😕🍆", 
            abs_change
        ),
        0 => "Нуууу, что тут можно ещё сказать... Твоя пипирка сегодня не выросла 🤔🍆".to_string(),
        1..=3 => format!(
            "Поздравляю! Твой член сегодня вырос на целых {} см. 💪🍆", 
            abs_change
        ),
        4..=7 => format!(
            "Все тяночки вокруг в шоке! Твой гигантский половой орган стал больше на {} см. 🚀🍆", 
            abs_change
        ),
        8..=9 => format!(
            "*Ах ты читер!* Каким-то образом ты смог увеличить свой писюн на {} см. 👑🍆🏆", 
            abs_change
        ),
        10 => format!(
            "🎉🎉🎉 Поздравляю! +{} см! Ты настоящий гигант! 💪🍆👑", 
            abs_change
        ),
        _ => "Что-то пошло не так...".to_string(),
    }
}

async fn meme_handler(bot: Bot, msg: Message) -> Result<(), Error> {
    match get_random_meme().await {
        Ok(meme_url) => {
            let url = reqwest::Url::parse(&meme_url).expect("Неверный URL");
            bot.send_photo(msg.chat.id, teloxide::types::InputFile::url(url)).await?;
        }
        Err(e) => {
            eprintln!("Ошибка при получении мема: {}", e);
            bot.send_message(msg.chat.id, "Не удалось получить мем").await?;
        }
    }
    Ok(())
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

fn get_weather_emoji(is_day: i64, weather_code: i64) -> String {
    match weather_code {
        0 => if is_day == 1 { "☀️" } else { "🌙" },
        1..=3 => if is_day == 1 { "🌤️" } else { "☁️" },
        45 | 48 => "🌫️",
        51..=55 | 61..=65 | 80..=82 => "🌧️",
        56..=57 | 66..=67 => "🌨️",
        71..=75 | 85..=86 => "❄️",
        77 => "🌨️",
        95..=99 => "⛈️",
        _ => "❓",
    }.to_string()
}

fn get_weather_description(code: i64) -> String {
    match code {
        0 => "Ясно",
        1..=3 => "Переменная облачность",
        45 | 48 => "Туман",
        51..=55 => "Морось",
        56..=57 => "Ледяная морось",
        61..=65 => "Дождь",
        66..=67 => "Ледяной дождь",
        71..=75 => "Снег",
        77 => "Снежные зерна",
        80..=82 => "Ливневые дожди",
        85..=86 => "Снежный ливень",
        95 => "Гроза",
        96..=99 => "Гроза с градом",
        _ => "Неизвестные погодные условия",
    }.to_string()
}

fn get_forecast(daily: &serde_json::Value, hourly: &serde_json::Value) -> String {
    let mut forecast = String::new();
    for i in 1..3 {
        let date = DateTime::from_timestamp(daily["time"][i].as_i64().unwrap(), 0).unwrap().with_timezone(&Local);
        let max_temp = daily["temperature_2m_max"][i].as_f64().unwrap();
        let min_temp = daily["temperature_2m_min"][i].as_f64().unwrap();
        let weather_code = hourly["weathercode"][i * 24].as_i64().unwrap();
        let weather_emoji = get_weather_emoji( 1, weather_code);
        
        forecast.push_str(&format!(
            "{} {}: от {:.1}°C до {:.1}°C, {}\n",
            date.format("%d.%m").to_string(),
            weather_emoji,
            min_temp,
            max_temp,
            get_weather_description(weather_code)
        ));
    }
    forecast
}

fn calculate_pisun_change(temperature: f64) -> i32 {
    let mut rng = rand::thread_rng();
    if temperature > 30.0 {
        rng.gen_range(-2..=0)
    } else if temperature > 20.0 {
        rng.gen_range(0..=2)
    } else if temperature > 10.0 {
        rng.gen_range(1..=3)
    } else if temperature > 0.0 {
        rng.gen_range(0..=2)
    } else if temperature > -10.0 {
        rng.gen_range(-1..=1)
    } else {
        rng.gen_range(-2..=0)
    }
}

fn get_clothing_recommendation(temperature: f64) -> String {
    if temperature > 30.0 {
        "Надевай самую легкую одежду и не забудь солнцезащитный крем!"
    } else if temperature > 20.0 {
        "Можно надеть шорты и футболку. Не забудь взять легкую кофту на вечер."
    } else if temperature > 10.0 {
        "Время для джинсов и кофты. Легкая куртка не помешает."
    } else if temperature > 0.0 {
        "Надевай куртку потеплее, шапку и перчатки."
    } else if temperature > -10.0 {
        "Пора доставать зимнюю куртку, теплую шапку и перчатки."
    } else {
        "Одевайся как можно теплее! Зимняя куртка, шарф, теплая шапка и перчатки обязательны."
    }.to_string()
}

fn generate_training_exercise() -> (TrainingExercise, bool) {
    let exercises = get_training_exercises();
    let exercise = exercises.choose(&mut rand::thread_rng()).unwrap().clone();
    let success = rand::thread_rng().gen_bool(exercise.success_rate);

    (exercise, success)
}

fn process_training_result(success: bool, current_size: i32) -> (i32, String) {
    let mut rng = rand::thread_rng();

    if success {
        let change = rng.gen_range(1..=3);
        (change, format!("Успех! Твой писюн вырос на {} см. 🎉", change))
    } else {
        let change = if current_size > 5 {
            -rng.gen_range(1..=2)
        } else {
            0
        };
        (change, format!("Неудача! {} 😔", if change < 0 { format!("Твой писюн уменьшился на {} см.", change.abs()) } else { "Но твой писюн не пострадал.".to_string() }))
    }
}

async fn get_random_meme() -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();
    let url = "https://pda.anekdot.ru/random/mem/";
    let response = client.get(url).send().await?;
    
    if response.status().is_success() {
        let body = response.text().await?;

        let document = scraper::Html::parse_document(&body);
        let selector = scraper::Selector::parse(".content img").unwrap();

        let image_element = document.select(&selector).next();
        if let Some(element) = image_element {
            let image_url = element.value().attr("src").unwrap_or("");
            Ok(image_url.to_string())
        } else {
            Ok("Не удалось найти изображение".to_string())
        }
    } else {
        Ok(format!("Ошибка при получении мема: {}", response.status()))
    }
}