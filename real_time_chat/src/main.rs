mod db;

use futures_util::{stream::SplitSink, StreamExt, SinkExt};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use warp::Filter;
use tokio::sync::broadcast;
use sqlx::SqlitePool;
use db::{save_message, load_messages};
use warp::ws::Message;
use warp::http::StatusCode;
use serde::Deserialize;

type Users = Arc<Mutex<HashSet<String>>>;

#[derive(Deserialize)]
struct UserRegistration {
    username: String,
    password: String,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = Arc::new(SqlitePool::connect(&database_url).await.unwrap());
    let (tx, _rx) = broadcast::channel(100);
    let active_users: Users = Arc::new(Mutex::new(HashSet::new()));

    let pool_clone1 = pool.clone(); // Клонируем pool для первого маршрута
    let pool_clone2 = pool.clone(); // Клонируем pool для второго маршрута

    // Маршруты чата
    let chat_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(warp::any().map(move || tx.clone()))
        .and(warp::any().map(move || pool_clone1.clone()))
        .and(warp::any().map(move || active_users.clone()))
        .map(|ws: warp::ws::Ws, params: std::collections::HashMap<String, String>, tx, pool, users| {
            let username = params.get("username").cloned().unwrap_or_default();
            let password = params.get("password").cloned().unwrap_or_default();
            ws.on_upgrade(move |socket| handle_socket(socket, tx, pool, users, username, password))
        });

    // Маршрут для регистрации
    let register_route = warp::path("register")
        .and(warp::post())
        .and(warp::body::json::<UserRegistration>())
        .and(warp::any().map(move || pool_clone2.clone()))
        .and_then(register_user);

    // Маршрут для статических файлов
    let static_files = warp::fs::dir("./static");

    // Объединяем маршруты
    let routes = static_files.or(chat_route).or(register_route);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

// Регистрация пользователя
async fn register_user(
    user_data: UserRegistration,
    pool: Arc<SqlitePool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let result = sqlx::query!(
        "INSERT INTO users (username, password) VALUES (?, ?)",
        user_data.username,
        user_data.password
    )
        .execute(&*pool)
        .await;

    match result {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(e) => {
            eprintln!("Error registering user: {}", e);
            Ok(StatusCode::CONFLICT) // Пользователь уже существует
        }
    }
}

// Обработка WebSocket-соединений
async fn handle_socket(
    socket: warp::ws::WebSocket,
    tx: broadcast::Sender<String>,
    pool: Arc<SqlitePool>,
    active_users: Users,
    username: String,
    _password: String,
) {
    let (mut user_ws_tx, mut user_ws_rx) = socket.split();

    {
        let mut users = active_users.lock().unwrap();
        users.insert(username.clone());
    }

    let join_message = format!("{} joined the chat", username);
    tx.send(join_message.clone()).unwrap();

    if let Ok(messages) = load_messages(&pool, &username).await {
        for (msg_username, msg_text, msg_time, msg_recipient) in messages {
            let formatted_message = if let Some(recipient) = msg_recipient {
                if recipient == username || msg_username == username {
                    format!(
                        "[{}] [PM] From {} to {}: {}",
                        msg_time, msg_username, recipient, msg_text
                    )
                } else {
                    continue;
                }
            } else {
                format!("[{}] [{}] {}", msg_time, msg_username, msg_text)
            };

            if let Err(e) = user_ws_tx.send(Message::text(formatted_message)).await {
                eprintln!("Failed to send message to {}: {}", username, e);
            }
        }
    } else {
        eprintln!("Failed to load messages for user: {}", username);
    }

    send_active_users_to_client(&active_users, &mut user_ws_tx).await;
    broadcast_active_users(&active_users, &tx).await;

    let tx_for_disconnect = tx.clone();
    let tx_for_messages = tx.clone();
    let active_users_clone = active_users.clone();
    let username_clone = username.clone();
    let pool_clone = pool.clone();

    tokio::spawn(async move {
        while let Some(result) = user_ws_rx.next().await {
            if let Ok(msg) = result {
                if msg.is_text() {
                    let text = msg.to_str().unwrap();
                    let timestamp = chrono::Utc::now().format("%H:%M").to_string();

                    if text.starts_with('@') {
                        let mut parts = text.splitn(2, ' ');
                        let recipient = parts.next().unwrap().trim_start_matches('@');
                        let message_body = parts.next().unwrap_or("").to_string();
                        let private_message = format!(
                            "[{}] [PM] From {} to {}: {}",
                            timestamp, username_clone, recipient, message_body
                        );

                        save_message(&pool_clone, &username_clone, &message_body, Some(recipient))
                            .await;
                        tx_for_messages.send(private_message).unwrap();
                    } else {
                        let public_message =
                            format!("[{}] [{}] {}", timestamp, username_clone, text);
                        save_message(&pool_clone, &username_clone, text, None).await;
                        tx_for_messages.send(public_message).unwrap();
                    }
                }
            }
        }

        {
            let mut users = active_users_clone.lock().unwrap();
            users.remove(&username_clone);
        }

        tx_for_disconnect
            .send(format!("{} left the chat", username_clone))
            .unwrap();

        broadcast_active_users(&active_users_clone, &tx_for_disconnect).await;
    });

    tokio::spawn(async move {
        let mut rx = tx.subscribe();
        while let Ok(msg) = rx.recv().await {
            if let Err(e) = user_ws_tx.send(Message::text(msg)).await {
                eprintln!("Failed to send message: {}", e);
                break;
            }
        }
    });
}

async fn send_active_users_to_client(users: &Users, ws_tx: &mut SplitSink<warp::ws::WebSocket, Message>) {
    let users_list: Vec<String> = users.lock().unwrap().iter().cloned().collect();
    let message = format!("ACTIVE_USERS: {}", serde_json::to_string(&users_list).unwrap());
    if let Err(e) = ws_tx.send(Message::text(message)).await {
        eprintln!("Failed to send active users to client: {}", e);
    }
}

async fn broadcast_active_users(users: &Users, tx: &broadcast::Sender<String>) {
    let users_list: Vec<String> = users.lock().unwrap().iter().cloned().collect();
    let message = format!("ACTIVE_USERS: {}", serde_json::to_string(&users_list).unwrap());
    tx.send(message).unwrap();
    println!("Broadcasting active users: {}", serde_json::to_string(&users_list).unwrap());
}
