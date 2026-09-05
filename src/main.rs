use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::{
    collections::HashSet,
    env,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    sessions: Arc<Mutex<HashSet<String>>>,
    admin_password: String,
    telegram_bot_token: String,
    telegram_chat_id: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
struct Service {
    id: i64,
    name: String,
    price: i64,
    duration_min: i64,
}

#[derive(Deserialize)]
struct CreateBookingRequest {
    client_name: String,
    phone: String,
    service_id: i64,
    booking_date: String,
    booking_time: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct BookingDetail {
    id: i64,
    client_name: String,
    phone: String,
    service_name: String,
    price: i64,
    booking_date: String,
    booking_time: String,
    status: String,
    created_at: String,
}

#[derive(Deserialize)]
struct UpdateStatusRequest {
    status: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Загружаем переменные из .env
    dotenvy::dotenv().ok();

    let admin_password = env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin123".to_string());
    let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
    let telegram_chat_id = env::var("TELEGRAM_CHAT_ID").unwrap_or_default();

    let db_url = "sqlite://booking.db?mode=rwc";
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    init_db(&pool).await?;

    let state = AppState {
        db: pool,
        sessions: Arc::new(Mutex::new(HashSet::new())),
        admin_password,
        telegram_bot_token,
        telegram_chat_id,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/services", get(get_services))
        .route("/api/bookings", post(create_booking))
        .route("/api/admin/login", post(admin_login))
        .route("/api/admin/bookings", get(get_all_bookings))
        .route("/api/admin/bookings/:id/status", put(update_booking_status))
        .nest_service("/", ServeDir::new("static"))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    println!("🚀 Сервис записи запущен!");
    println!("💻 Локальный адрес: http://127.0.0.1:3001");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn verify_auth(headers: &HeaderMap, state: &AppState) -> Result<(), StatusCode> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.replace("Bearer ", ""))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if state.sessions.lock().unwrap().contains(&token) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn get_services(State(state): State<AppState>) -> Result<Json<Vec<Service>>, StatusCode> {
    let services = sqlx::query_as::<_, Service>("SELECT id, name, price, duration_min FROM services")
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(services))
}

// Отправка сообщения в Telegram
async fn send_telegram_notification(bot_token: String, chat_id: String, text: String) {
    if bot_token.is_empty() || chat_id.is_empty() {
        eprintln!("⚠️ Telegram token или chat_id не заданы в .env!");
        return;
    }

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

    let client = reqwest::Client::new();
    let params = [
        ("chat_id", chat_id.as_str()),
        ("text", text.as_str()),
        ("parse_mode", "Markdown"),
    ];

    if let Err(e) = client.post(&url).form(&params).send().await {
        eprintln!("❌ Ошибка отправки в Telegram: {:?}", e);
    } else {
        println!("📩 Уведомление успешно отправлено в Telegram!");
    }
}

// Создание записи + отправка в Telegram
async fn create_booking(
    State(state): State<AppState>,
    Json(payload): Json<CreateBookingRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    println!("📥 Получена новая запись от клиента: {}", payload.client_name);

    // 1. Сохранение в базу
    let insert_result = sqlx::query(
        "INSERT INTO bookings (client_name, phone, service_id, booking_date, booking_time, status) VALUES (?, ?, ?, ?, ?, 'Новая')"
    )
    .bind(&payload.client_name)
    .bind(&payload.phone)
    .bind(payload.service_id)
    .bind(&payload.booking_date)
    .bind(&payload.booking_time)
    .execute(&state.db)
    .await;

    if let Err(e) = insert_result {
        eprintln!("❌ Ошибка записи в БД: {:?}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // 2. Получение названия услуги
    let service_info: Option<(String, i64)> = sqlx::query_as(
        "SELECT name, price FROM services WHERE id = ?"
    )
    .bind(payload.service_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let (s_name, s_price) = service_info.unwrap_or(("Услуга".to_string(), 0));

    // 3. Форматирование текста
    let message_text = format!(
        "🔔 *Новая запись на сайте!*\n\n👤 *Имя:* {}\n📞 *Телефон:* {}\n✂️ *Услуга:* {} ({} ₸)\n📅 *Дата:* {}\n⏰ *Время:* {}",
        payload.client_name,
        payload.phone,
        s_name,
        s_price,
        payload.booking_date,
        payload.booking_time
    );

    // 4. Фоновая отправка в Telegram
    let token = state.telegram_bot_token.clone();
    let chat_id = state.telegram_chat_id.clone();
    tokio::spawn(async move {
        send_telegram_notification(token, chat_id, message_text).await;
    });

    Ok((StatusCode::CREATED, Json("Вы успешно записались!")))
}

async fn admin_login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    if payload.password == state.admin_password {
        let token = Uuid::new_v4().to_string();
        state.sessions.lock().unwrap().insert(token.clone());
        Ok(Json(LoginResponse { token }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn get_all_bookings(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<BookingDetail>>, StatusCode> {
    verify_auth(&headers, &state)?;

    let bookings = sqlx::query_as::<_, BookingDetail>(
        r#"
        SELECT 
            b.id, b.client_name, b.phone, 
            s.name as service_name, s.price, 
            b.booking_date, b.booking_time, b.status, b.created_at
        FROM bookings b
        JOIN services s ON b.service_id = s.id
        ORDER BY b.booking_date DESC, b.booking_time DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(bookings))
}

async fn update_booking_status(
    Path(id): Path<i64>,
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<UpdateStatusRequest>,
) -> Result<StatusCode, StatusCode> {
    verify_auth(&headers, &state)?;

    sqlx::query("UPDATE bookings SET status = ? WHERE id = ?")
        .bind(payload.status)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS services (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            duration_min INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS bookings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            client_name TEXT NOT NULL,
            phone TEXT NOT NULL,
            service_id INTEGER NOT NULL,
            booking_date TEXT NOT NULL,
            booking_time TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'Новая',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(pool)
    .await?;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM services")
        .fetch_one(pool)
        .await?;

    if count.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO services (name, price, duration_min) VALUES 
            ('Мужская стрижка', 5000, 45),
            ('Стрижка и оформление бороды', 8000, 60),
            ('Комплексный уход', 12000, 90);
            "#,
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}