use rand::seq::SliceRandom;
use teloxide::payloads::SendPhotoSetters;
use teloxide::types::ChatId;
use teloxide::utils::markdown::escape;
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
    #[command(description = "Тренировка твоего писюна")]
    Train,
    #[command(description = "Показывает топ 10 пользователей")]
    Top,
    #[command(description = "Показывает текущий размер")]
    Size,
    #[command(description = "Случайный фильм на вечер")]
    RandomMovie,
    #[command(description = "Случайный анекдот")]
    Anekdot,
    #[command(description = "Погода")]
    Weather,
    #[command(description = "Случайный мем")]
    Meme,
    #[command(description = "Случайная мудрость")]
    Wisdom,
    #[command(description = "Совет, если ты с похмелья")]
    Hangover,
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
        Command::Meme => meme_handler(bot, msg).await,
        Command::Wisdom => wisdom_handler(bot, msg).await,
        Command::Hangover => hangover_handler(bot, msg).await,
        Command::RandomMovie => random_movie_handler(bot, msg).await,
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
    let user = config.get_or_create_user(user_id).clone();
    
    let all_users = config.get_all_users();
    let mut sorted_users: Vec<_> = all_users.values().collect();
    sorted_users.sort_by_key(|u| std::cmp::Reverse(u.pisun));

    let user_rank = sorted_users
        .iter()
        .position(|u| u.user_id == user_id)
        .map(|rank| rank + 1)
        .unwrap_or(sorted_users.len() + 1);

    let message = match user.pisun {
        0 => format!("На данный момент у тебя нет писюна, неудачник! Ты занимаешь {} место в рейтинге.", user_rank),
        _ => format!("Текущий размер твоего писюна аж {} см. Ты занимаешь {} место в рейтинге.", user.pisun, user_rank),
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
        -10..=-7 => {
            let messages = vec![
                format!(
                    "Ахахахах, неудачник. Твой огрызок стал меньше на целых {} см! 🍆📉", 
                    abs_change
                ),
                format!(
                    "Ахахахах, невдаха. Твій член став меншим на {} см! 🍆📉", 
                    abs_change
                )
            ];
                messages.choose(&mut rand::thread_rng()).unwrap().to_string()
        },
        -6..=-3 => {
            let messages = vec![
                format!(
                    "Твой, и не без того маленький пенис, стал меньше аж на {} см. 😔🍆🤏", 
                    abs_change
                ),
                format!(
                    "Твій, і не без того маленький член, став меншим аж на {} см. 😔🍆🤏", 
                    abs_change
                )
            ];
            messages.choose(&mut rand::thread_rng()).unwrap().to_string()
        },
        -2..=-1 => {
            let messages = vec![
                format!(
                    "Мои спутники зафиксировали уменьшение твоего полового органа на {} см. 😕🍆", 
                    abs_change
                ),
                format!(
                    "Мої супутники зафіксували зменшення твого статевого органу на {} см. 😕🍆", 
                    abs_change
                )
            ];
            messages.choose(&mut rand::thread_rng()).unwrap().to_string()
        },
        0 => {
            let messages = vec![
                "Нуууу, что тут можно ещё сказать... Твоя пипирка сегодня не выросла 🤔🍆".to_string(),
                "Нууууу, що тут ще можна сказати... Твій член сьогодні не виріс 🤔🍆".to_string()
            ];
            messages.choose(&mut rand::thread_rng()).unwrap().to_string()
        },
        1..=3 => {
            let messages = vec![
                format!(
                    "Отличный результат! Твой писюн увеличился на {} см. 🚀", 
                    abs_change
                ),
                format!(
                    "Вау! Твій член виріс на {} см. 🚀", 
                    abs_change
                )
            ];
            messages.choose(&mut rand::thread_rng()).unwrap().to_string()
        },
        4..=7 => {
            let messages = vec![
                format!(
                    "Все тяночки вокруг в шоке! Твой гигантский половой орган стал больше на {} см. 🚀🍆", 
                    abs_change
                ),
                format!(
                    "Твій член став значно більше на {} см. 💪",
                    abs_change
                )
            ];
            messages.choose(&mut rand::thread_rng()).unwrap().to_string()
        },
        8..=9 => {
            let messages = vec![
                format!(
                    "*Ах ты читер!* Каким-то образом ты смог увеличить свой писюн на {} см. 👑🍆🏆", 
                    abs_change
                ),
                format!(
                    "Новий рекорд! Твій член став {} см.",
                    abs_change
                )
            ];
            messages.choose(&mut rand::thread_rng()).unwrap().to_string()
        },
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

async fn wisdom_handler(bot: Bot, msg: Message) -> Result<(), Error> {
    let wisdoms = vec![
        "Если жизнь даёт тебе лимоны, сделай лимонад. А если водку — зови друзей. 🍋🍹",
        "Мудрость приходит с возрастом, но иногда возраст приходит один. 👴📜",
        "Не важно, сколько у тебя проблем — важно, сколько у тебя мемов. 🤣📱",
        "Деньги счастья не приносят, но с ними легче грустить в дорогой машине. 🚗💸",
        "Если тебя не понимают — значит, ты говоришь слишком умно. 🧠🧐",
        "Не откладывай на завтра то, что можно вообще не делать. 🛌✨",
        "Лучше быть смешным, чем скучным. Даже если никто не смеётся. 🤡",
        "Если упал — лежи. Земля — это тоже уютно. 🌍🛋️",
        "ТЫ ЧО ТУПОЙ?"
    ];

    let wisdom = wisdoms.choose(&mut rand::thread_rng()).unwrap();
    bot.send_message(msg.chat.id, wisdom.to_string()).await?;
    Ok(())
}

async fn hangover_handler(bot: Bot, msg: Message) -> Result<(), Error> {
    let tips = vec![
        "Вода, вода и еще раз вода! И никаких больше \"я только одну бутылочку\". 🍼🍺",
        "Съешь что-нибудь жирное. Или хотя бы посмотри на фотографию еды. 🍔📸",
        "Ибупрофен — твой новый лучший друг. Но не забудь про воду! 💊💧",
        "Поспи чуть-чуть. Или не чуть-чуть. Главное — не просыпайся до понедельника. 🛏️💤",
        "Контрастный душ. Или просто сиди в ванной и плачь. 🚿😢",
        "Солёный огурец и рассол — твоё спасение! 🥒💚",
        "Не забудь, что завтра ты снова скажешь \"больше не пью\". И это ложь. 🍷🚫",
    ];

    let tip = tips.choose(&mut rand::thread_rng()).unwrap();
    bot.send_message(msg.chat.id, tip.to_string()).await?;
    Ok(())
}

async fn random_movie_handler(bot: Bot, msg: Message) -> Result<(), Error> {
    match get_random_movie().await {
        Ok((text, poster_url))  => {
            bot.send_photo(msg.chat.id, teloxide::types::InputFile::url(reqwest::Url::parse(&poster_url).expect("Invalid URL"))).caption(text).parse_mode(teloxide::types::ParseMode::MarkdownV2).await?;
        },
        Err(e) => {
            eprintln!("Ошибка при получении фильма: {}", e);
            bot.send_message(msg.chat.id, "Не удалось получить фильм").await?;
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

async fn get_random_meme() -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = "https://pda.anekdot.ru/random/mem/";
    let response = client.get(url).send().await.map_err(|e | format!("Ошибка при получении мема: {}", e))?;
    
    if response.status().is_success() {
        let body = response.text().await.map_err(|e | format!("Ошибка при получении мема: {}", e))?;

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
        Err(format!("Ошибка при получении мема: {}", response.status()))
    }
}

async fn get_random_movie() -> Result<(String, String), String> {
    let api_key = std::env::var("TMDB_API_KEY").map_err(|e| format!("TMDB_API_KEY не установлен: {}", e))?;
    let url = format!("https://api.themoviedb.org/3/movie/top_rated?api_key={}&language=ru-RU&page=1", api_key);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await.map_err(|e| format!("Ошибка при запросе к TMDB: {}", e))?;

    if response.status().is_success() {
        let body = response.text().await.map_err(|e| format!("Ошибка при чтении ответа от TMDB: {}", e))?;
        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("Ошибка при парсинге JSON: {}", e))?;

        let results = json["results"].as_array().ok_or("В ответе TMDB нет поля 'results'")?;
        let movie = results.choose(&mut rand::thread_rng()).ok_or("Список фильмов пуст")?;

        let title = movie["title"].as_str().ok_or("В фильме нет поля 'title'")?.to_string();
        let overview = movie["overview"].as_str().ok_or("В фильме нет поля 'overview'")?.to_string();
        let poster_path = movie["poster_path"].as_str().ok_or("В фильме нет поля 'poster_path'")?.to_string();
        let movie_id = movie["id"].as_u64().ok_or("В фильме нет поля 'id'")?;

        let poster_url = format!("https://image.tmdb.org/t/p/w500{}", poster_path);
        let tmdb_url = format!("https://www.themoviedb.org/movie/{}", movie_id);

        let escaped_title = escape(&title);
        let escaped_overview = escape(&overview);
        let escaped_tmdb_url = escape(&tmdb_url);

        let text = format!("🎥 Сегодня рекомендуем посмотреть: *{}* \\({}\\)\n\n{}", escaped_title, escaped_tmdb_url, escaped_overview);

        Ok((text, poster_url))
    } else {
        Err(format!("Ошибка при запросе к TMDB: код {}", response.status()))
    }
}