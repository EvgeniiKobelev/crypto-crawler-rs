# Анализ ошибки "Failed to send shutdown signal to ping task: channel closed"

## Что происходило

Ошибка возникала из-за **дублирования механизмов shutdown** в коде. Было два разных способа остановки ping задач:

1. **Старый механизм** (источник ошибки): Использовал `Arc<Mutex<Option<Sender>>>` и отдельную задачу для мониторинга флага `reconnect_in_progress`
2. **Новый механизм** (правильный): Использует `tokio::sync::watch::channel` с `shutdown_rx.changed()`

## Источник ошибки

**Файл:** `crypto-ws-client/src/common/ws_client_internal.rs`  
**Строка:** ~609

```rust
// СТАРЫЙ КОД (удалён)
if let Err(err) = tx.send(true) {
    error!("Failed to send shutdown signal to ping task: {}", err);
}
```

Этот код пытался отправить сигнал через уже закрытый канал, что приводило к ошибке.

## Что происходит после ошибки?

### 1. Ping задача продолжает работать
- Ping задача использует **новый механизм** (`shutdown_rx.changed()`)
- Старая ошибка **НЕ влияет** на работу ping задачи
- Ping задача корректно останавливается через новый механизм

### 2. WebSocket делает переподключение
**ДА, WebSocket автоматически переподключается!**

Логика переподключения:
```rust
// В методе run()
'connection_loop: loop {
    while let Some(msg) = message_rx.recv().await {
        // Обработка сообщений
    }
    
    // Когда message_rx.recv() возвращает None (соединение закрыто)
    if !self.reconnect_in_progress.load(Ordering::SeqCst) {
        info!("Message queue closed, attempting to reconnect...");
        
        // Вызывается метод reconnect()
        if let Some(new_message_rx) = self.reconnect(handler_clone, tx.clone()).await {
            message_rx = new_message_rx;
            continue 'connection_loop; // Продолжаем работу
        } else {
            error!("Failed to reconnect after multiple attempts, exiting...");
            break 'connection_loop; // Выходим только после исчерпания попыток
        }
    }
}
```

### 3. Процесс переподключения

1. **Обнаружение разрыва**: `message_rx.recv()` возвращает `None`
2. **Инициация переподключения**: Вызывается `self.reconnect()`
3. **Попытки подключения**: До 5 попыток с экспоненциальным backoff
4. **Восстановление подписок**: Автоматически восстанавливаются все активные подписки
5. **Продолжение работы**: Цикл `'connection_loop` продолжается с новым соединением

## Исправление

**Удалён старый механизм shutdown**, который вызывал ошибку:

```rust
// УДАЛЕНО
tokio::task::spawn(async move {
    let mut check_interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        check_interval.tick().await;
        if reconnect_in_progress_clone.load(Ordering::Acquire) {
            let mut guard = shutdown_tx.lock().unwrap();
            if let Some(tx) = guard.take() {
                if let Err(err) = tx.send(true) {
                    error!("Failed to send shutdown signal to ping task: {}", err); // ← ОШИБКА
                }
                break;
            }
        }
    }
});
```

**Оставлен только новый механизм**:
```rust
// В ping задаче
tokio::select! {
    // ... ping логика ...
    
    // Корректный shutdown через watch channel
    _ = shutdown_rx.changed() => {
        if *shutdown_rx.borrow() {
            info!("Ping task received shutdown signal, stopping");
            break;
        }
    }
}
```

## Результат

✅ **Ошибка устранена**: Удалён дублирующий код, вызывавший ошибку  
✅ **Переподключение работает**: WebSocket автоматически переподключается при разрыве  
✅ **Ping задачи корректно завершаются**: Используется только надёжный механизм shutdown  
✅ **Подписки восстанавливаются**: Все активные подписки автоматически восстанавливаются после переподключения

## Логи после исправления

Вместо ошибки теперь будут появляться информационные сообщения:
```
[INFO] Message queue closed, attempting to reconnect...
[INFO] Successfully reconnected after 1 attempts
[INFO] Restoring 3 subscriptions...
[INFO] Ping task received shutdown signal, stopping
```

## Мониторинг

Для мониторинга состояния соединения теперь доступны:
- `ConnectionState` - текущее состояние соединения
- `ConnectionMetrics` - метрики подключений и ошибок  
- `HealthStatus` - полная информация о здоровье соединения
- Структурированное логирование событий соединения

---

**Вывод**: Ошибка была косметической и не влияла на функциональность. WebSocket клиент **всегда делал переподключение** корректно, просто логировал ложную ошибку из-за дублирующего кода.
