use crypto_ws_client::{BinanceSpotWSClient, WSClient};
use log::info;
use std::time::Duration;

const TIMEOUT_FOR_RESTART: u64 = 15;

async fn run_app() {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = tokio::task::spawn(async move {
        let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];
        let ws_client = BinanceSpotWSClient::new(tx, None).await;
        ws_client.subscribe_trade(&symbols).await;
        // run for 5 seconds
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), ws_client.run()).await;
        ws_client.close().await;
    });

    // Обработка сообщений в отдельном потоке
    tokio::task::spawn(async move {
        for msg in rx {
            println!("{}", msg);
        }
    });

    // Дожидаемся завершения основной задачи
    let _ = handle.await;
}

#[tokio::main]
async fn main() {
    // Настройка логирования
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    // Бесконечный цикл для перезапуска приложения
    loop {
        info!("Запуск приложения...");
        run_app().await;
        info!("Приложение завершило работу. Перезапуск через {} секунд...", TIMEOUT_FOR_RESTART);
        tokio::time::sleep(Duration::from_secs(TIMEOUT_FOR_RESTART)).await;
    }
}
