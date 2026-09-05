# ✂️ Rust Booking App — Сервис онлайн-записи и бронирования

Лаконичный, быстрый и асинхронный REST API сервис для записи на услуги (барбершоп / салон красоты / мастерские) с автоматическими уведомлениями в Telegram и встроенной админ-панелью.

Написан на **Rust** с использованием веб-фреймворка **Axum** и базы данных **SQLite**.

---

## 🚀 Особенности и функционал

- ⚡ **Высокая производительность:** Написан на асинхронном движке Tokio / Axum.
- 💾 **Легковесная база данных:** Автоматическая инициализация и работа с SQLite через `sqlx`.
- 📩 **Telegram-уведомления:** Мгновенные оповещения в Telegram-чат администратора при создании новой записи клиентом.
- 🔐 **Панель администратора:** Защищенная авторизация по токену сессии (`UUID v4`), просмотр всех записей и смена статусов заказов.
- 🌐 **CORS & Static Files:** Встроенная раздача статического фронтенда и поддержка CORS для работы с SPA.
- 🔒 **Безопасность конфигурации:** Все секреты, ключи API и логины изолированы в `.env`.

---

## 🛠️ Технологический стек

- **Язык:** Rust (Edition 2021)
- **Web Framework:** [Axum](https://github.com/tokio-rs/axum)
- **Async Runtime:** [Tokio](https://tokio.rs/)
- **Database / ORM:** [SQLx](https://github.com/launchbadge/sqlx) (SQLite)
- **HTTP Client:** [Reqwest](https://github.com/seanmonstar/reqwest) (Telegram Bot API)
- **Сериализация:** [Serde](https://serde.rs/) / `serde_json`
- **Аутентификация & Утилиты:** `uuid`, `dotenvy`, `tower-http`

---

## ⚙️ Установка и запуск

### 1. Клонирование репозитория
```bash
git clone [https://github.com/Eruxa001/rust-booking-app.git](https://github.com/Eruxa001/rust-booking-app.git)
cd rust-booking-app