use axum::{
    extract::{
        State, // Экстрактор State используется для получения доступа к текущему состоянию
        Json // Экстрактор Json используется для десериализации Json-объектов
    }, 
    http::StatusCode, // Используется для формирования статуса ответа клиенту
    response::{
        Html, // Позволяет возвращать html-ответ на запрос
        IntoResponse // Трейт IntoResponse используется для формирования HTTP-ответа
    },
    Router, // Структура Router применяется для определения структуры веб-приложения (маршрут -> функция)
    routing::{get, post} // Модуль routing применяется для обработки GET и POST запросов
};
use serde::{Deserialize}; // Библиотека serde содержит трейты для сериализации и десериализации данных
use std::net::SocketAddr; // Структура SocketAddr применяется для задания адреса и порта сервера
use std::env; // Модуль env применяется для настройки отображения сообщений логирования  
use std::sync::{
    Arc, // Тип Arc применяется для доступа нескольским владельцам к одним и тем же данным
    Mutex // Тип Mutex применяется для предотвращения состояния гонки 
};
use std::collections::HashMap;

// Определяем структуру, содержащую данные о товаре
#[derive(Deserialize, Clone)]
struct ItemData {
    brand: String,
    name: String,
    price: i64,
    id: String,
}

// Определяем структуры, содержащую данные, которые можно безопасно разделять между потоками
#[derive(Clone)]
struct AppState {
    data: Arc<Mutex<Option<ItemData>>>,
    data_map: Arc<Mutex<HashMap<String, ItemData>>>,
    index: Arc<Mutex<usize>>, // Начальный индекс
    keys: Arc<Mutex<Vec<String>>>, // Начальный пустой массив ключей
}

#[tokio::main]
async fn main() {
    
    env::set_var("RUST_LOG", "info"); // Включаем отображение сообщений лога
    env_logger::init(); // Инициализируем систему логирования env_logger
    log::info!("Запуск сервера...");

    // Создаем начальное состояние, данные отсутствуют
    let shared_state = AppState {
        data: Arc::new(Mutex::new(None)),
        data_map: Arc::new(Mutex::new(HashMap::new())),
        index: Arc::new(Mutex::new(0)), // Начальный индекс
        keys: Arc::new(Mutex::new(Vec::new())), // Начальный пустой массив ключей
    };

    // Определяем структуру приложения (маршрут -> функция)
    let app = Router::new()
        .route("/post", post(receive_data)) // Принимаем данные
        .route("/previous", get(|state| navigate(state, NavigationDirection::Previous))) // Выводим предыдущие данные
        .route("/next", get(|state| navigate(state, NavigationDirection::Next))) // Выводим следующие данные
        .route("/first", get(|state| navigate(state, NavigationDirection::First))) // Выводим последние данные
        .route("/last", get(|state| navigate(state, NavigationDirection::Last))) // Выводим последние данные
        .route("/", get(show_data)) // Выводим данные на страницу
        .with_state(shared_state); // Передаем текущее состояние
    
    // Задаем адрес и порт сервера: http://127.0.0.1:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    log::info!("Сервер доступен по ссылке: http://{}", addr);

    // Запускаем сервер, контролируя Ok и Err
    match axum::Server::bind(&addr)
        .serve(app.into_make_service()) // Обрабатываем входящие запросы в соответствии с определенной ранее структурой в app
        .await // Ожидаем завершения операции
        {
            Ok(()) => (),
            Err(e) => log::error!("Ошибка при инициализации сервера: {}", e),
        }
}

// Функция отображения данных, хранящихся в состоянии
async fn show_data(
    State(state): State<AppState> // Получаем доступ к состоянию
) -> impl IntoResponse { // Формируем ответ
    let index = *state.index.lock().unwrap();
    let keys = state.keys.lock().unwrap();
    let data_map = state.data_map.lock().unwrap();
    log::info!("Получен запрос на показ данных.");

    // Проверяем есть ли данные в data и формируем html-ответ
    if index > 0 && index <= keys.len() {
        if let Some(item) = data_map.get(&keys[index - 1]) {
            log::info!("Данные отображены.");
            return Html(format!(
                "<h1>Данные заказа №{}</h1>
                <p>Фирма: {}</p>
                <p>Название: {}</p>
                <p>Стоимость: {}</p>
                <p>ID: {}</p>",
                index, item.brand, item.name, item.price, item.id
            ));
        }
    }

    log::info!("Данные отсутствуют.");
    Html("<h1>Данные отсутствуют.</h1>".to_string())
}

// Функция приема данных и обновления состояния
async fn receive_data(
    State(state): State<AppState>, // Получаем доступ к состоянию
    Json(payload): Json<ItemData>, // Десериализуем данные
) -> impl IntoResponse { // Формируем ответ
    let mut data = state.data.lock().unwrap(); // Получаем доступ к данным состояния
    *data = Some(payload.clone()); // Обновляем данные
    
    // Добавляем объект в хеш-таблицу
    let mut data_map = state.data_map.lock().unwrap();
    data_map.insert(payload.id.clone(), payload.clone());

    // Добавляем ключ в массив
    let mut keys = state.keys.lock().unwrap();
    keys.push(payload.id.clone());

    // Обновляем текущий индекс
    let mut index = state.index.lock().unwrap();
    *index = keys.len();
    log::info!("Приняты новые данные.");

    StatusCode::OK // Успешное выполнение операции
}

enum NavigationDirection {
    Previous,
    Next,
    First,
    Last,
}

// Функция навигации
async fn navigate(
    State(state): State<AppState>,
    direction: NavigationDirection,
) -> impl IntoResponse {
    let mut index = state.index.lock().unwrap();
    let keys = state.keys.lock().unwrap();
    let data_map = state.data_map.lock().unwrap();

    match direction {
        NavigationDirection::Previous => {
            if *index > 1 {
                *index -= 1;
            }
            log::info!("Переход на предыдущие данные.");
        }
        NavigationDirection::Next => {
            if *index < keys.len() {
                *index += 1;
            }
            log::info!("Переход на следующие данные.");
        }
        NavigationDirection::First => {
            if keys.len() > 0 {
                *index = 1;
                log::info!("Переход на первые данные.");
            }
        }
        NavigationDirection::Last => {
            *index = keys.len();
            log::info!("Переход на последние данные.");
        }
    }

    if *index > 0 {
        if let Some(item) = data_map.get(&keys[*index-1]) {
            return Html(format!(
                "<h1>Данные</h1>
                <p>Фирма: {}</p>
                <p>Название: {}</p>
                <p>Стоимость: {}</p>
                <p>ID: {}</p>",
                item.brand, item.name, item.price, item.id
            ));
        }
    }

    Html("<h1>Данные отсутствуют.</h1>".to_string())
}