use sqlx::{SqlitePool, Row}; // Импортируем нужные типы

// #[derive(Debug, FromRow)]
// struct MessageRow {
//     username: String,
//     message: String,
//     time: String,
//     recipient: Option<String>, // Обработка recipient для личных сообщений
// }

pub async fn save_message(
    pool: &SqlitePool,
    username: &str,
    message: &str,
    recipient: Option<&str>,
) {
    if let Err(e) = sqlx::query!(
        "INSERT INTO messages (username, message, recipient) VALUES (?, ?, ?)",
        username,
        message,
        recipient
    )
        .execute(pool)
        .await
    {
        eprintln!("Error saving message to database: {}", e);
    }
}

pub async fn load_messages(
    pool: &SqlitePool,
    username: &str,
) -> Result<Vec<(String, String, String, Option<String>)>, sqlx::Error> {
    let query = r#"
        SELECT
            username,
            message,
            strftime('%H:%M', timestamp) as time,
            recipient
        FROM messages
        WHERE (recipient IS NULL)  -- Публичные сообщения
           OR (recipient = ?)      -- Личные сообщения, где пользователь - получатель
           OR (username = ?)       -- Личные сообщения, где пользователь - отправитель
        ORDER BY timestamp ASC
    "#;

    let rows = sqlx::query(query)
        .bind(username) // recipient = username
        .bind(username) // username = username
        .fetch_all(pool)
        .await?;

    let messages = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("username"),
                row.get::<String, _>("message"),
                row.get::<String, _>("time"),
                row.try_get::<Option<String>, _>("recipient").ok().flatten(),
            )
        })
        .collect();

    Ok(messages)
}



// Новая функция для загрузки пользователей
// pub async fn load_users(pool: &SqlitePool) -> Vec<String> {
//     sqlx::query!("SELECT DISTINCT username FROM messages")
//         .fetch_all(pool)
//         .await
//         .unwrap_or_else(|_| vec![]) // Возвращаем пустой список пользователей в случае ошибки
//         .into_iter()
//         .map(|row| row.username)
//         .collect()
// }

// pub async fn init_db() -> SqlitePool {
//     let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
//
//     SqlitePool::connect(&database_url)
//         .await
//         .expect("Failed to connect to the database")
// }
