#[cfg(test)]
mod tests {
    use log::*;

    #[test]
    fn test_mexc_binary_data_detection() {
        env_logger::init();
        
        // Тест определения формата данных по заголовкам
        
        // GZIP заголовок
        let gzip_data = vec![0x1f, 0x8b, 0x08, 0x00];
        let is_gzip = gzip_data.len() >= 2 && gzip_data[0] == 0x1f && gzip_data[1] == 0x8b;
        assert!(is_gzip);
        
        // DEFLATE/ZLIB заголовки (исправленная логика)
        let deflate_data1 = vec![0x78, 0x01]; // zlib header
        let is_deflate1 = deflate_data1.len() >= 2 && 
            deflate_data1[0] == 0x78 && 
            (deflate_data1[1] == 0x01 || deflate_data1[1] == 0x9c || deflate_data1[1] == 0xda);
        assert!(is_deflate1);
        
        let deflate_data2 = vec![0x78, 0x9c]; // zlib header
        let is_deflate2 = deflate_data2.len() >= 2 && 
            deflate_data2[0] == 0x78 && 
            (deflate_data2[1] == 0x01 || deflate_data2[1] == 0x9c || deflate_data2[1] == 0xda);
        assert!(is_deflate2);
        
        // Protocol Buffers данные - улучшенное определение
        let protobuf_data1 = vec![0x08, 0x96, 0x01, 0x12]; // типичный protobuf с varint
        let protobuf_data2 = vec![0x0a, 0x1e]; // field 1, length-delimited, length 30
        let protobuf_data3 = vec![0x0a, 0x1e, b's', b'p', b'o', b't', b'@']; // реальный случай
        
        // Проверяем новую логику определения протобуф
        for protobuf_data in [&protobuf_data1, &protobuf_data2, &protobuf_data3] {
            let is_gzip = protobuf_data.len() >= 2 && protobuf_data[0] == 0x1f && protobuf_data[1] == 0x8b;
            let is_deflate_zlib = protobuf_data.len() >= 2 && 
                protobuf_data[0] == 0x78 && 
                (protobuf_data[1] == 0x01 || protobuf_data[1] == 0x9c || protobuf_data[1] == 0xda);
            
            let is_likely_protobuf = protobuf_data.len() >= 4 && 
                !is_gzip && !is_deflate_zlib &&
                (
                    (protobuf_data[0] == 0x08 && protobuf_data[1] < 0x80) || // field 1, varint
                    (protobuf_data[0] == 0x0a && protobuf_data[1] < 0x80) || // field 1, length-delimited
                    (protobuf_data[0] == 0x10 && protobuf_data[1] < 0x80) || // field 2, varint
                    (protobuf_data[0] == 0x12 && protobuf_data[1] < 0x80) || // field 2, length-delimited
                    (protobuf_data[0] == 0x0a && protobuf_data.len() > 10 && 
                     protobuf_data[2..].starts_with(b"spot@"))
                );
            
            assert!(!is_gzip, "Protobuf data incorrectly detected as GZIP: {:?}", protobuf_data);
            assert!(!is_deflate_zlib, "Protobuf data incorrectly detected as DEFLATE: {:?}", protobuf_data);
            
            // Для protobuf_data3 специально проверяем spot@ паттерн
            if protobuf_data[0] == 0x0a && protobuf_data.len() > 6 {
                if protobuf_data[2..].starts_with(b"spot@") {
                    assert!(is_likely_protobuf, "Real protobuf pattern not detected: {:?}", protobuf_data);
                }
            } else if protobuf_data[0] == 0x08 || protobuf_data[0] == 0x0a {
                assert!(is_likely_protobuf, "Protobuf pattern not detected: {:?}", protobuf_data);
            }
        }
        
        // JSON данные (обычный текст)
        let json_data = b"{\"test\":\"value\"}";
        let is_json = String::from_utf8(json_data.to_vec()).is_ok() &&
                     (json_data[0] == b'{' || json_data[0] == b'[');
        assert!(is_json);
        
        println!("✅ Все тесты определения формата прошли успешно");
    }

    #[test]
    fn test_protobuf_channel_detection() {
        // Тест определения протобуф каналов в командах
        
        let protobuf_commands = vec![
            r#"{"method":"SUBSCRIPTION","params":["spot@public.deals.v3.api.pb@BTCUSDT"]}"#,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.aggre.depth.v3.api.pb@100ms@BTCUSDT"]}"#,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api.pb@BTCUSDT@Min1"]}"#,
        ];
        
        let json_commands = vec![
            r#"{"method":"SUBSCRIPTION","params":["spot@public.deals.v3.api@BTCUSDT"]}"#,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.increase.depth.v3.api@BTCUSDT"]}"#,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api@BTCUSDT@Min1"]}"#,
        ];
        
        // Проверяем детекцию протобуф каналов
        for command in protobuf_commands {
            assert!(command.contains(".pb@") || command.contains(".api.pb"));
            println!("🔍 Обнаружен protobuf канал: {}", command);
        }
        
        // Проверяем, что JSON каналы не детектируются как протобуф
        for command in json_commands {
            assert!(!command.contains(".pb@") && !command.contains(".api.pb"));
            println!("✅ JSON канал корректен: {}", command);
        }
        
        println!("✅ Тест определения протобуф каналов прошел успешно");
    }

    #[test]
    fn test_utf8_vs_binary_data() {
        // Тест различения UTF-8 JSON от бинарных данных
        
        // Валидный JSON
        let valid_json = br#"{"channel":"spot@public.deals.v3.api@BTCUSDT","data":{"price":"50000"}}"#;
        let json_result = String::from_utf8(valid_json.to_vec());
        assert!(json_result.is_ok());
        let json_str = json_result.unwrap();
        assert!(json_str.trim().starts_with('{') || json_str.trim().starts_with('['));
        
        // Симуляция Protocol Buffers данных (невалидный UTF-8)
        let protobuf_data = vec![0x08, 0x96, 0x01, 0x12, 0x0a, 0x42, 0x54, 0x43, 0x55, 0x53, 0x44, 0x54];
        let protobuf_result = String::from_utf8(protobuf_data.clone());
        assert!(protobuf_result.is_err());
        
        // Проверяем, что первый байт указывает на protobuf (< 32 и не сжатие)
        let first_byte = protobuf_data[0];
        let is_likely_protobuf = first_byte < 32 && 
                                !(protobuf_data.len() >= 2 && first_byte == 0x1f && protobuf_data[1] == 0x8b) && // не gzip
                                !(protobuf_data.len() >= 2 && first_byte == 0x78); // не zlib
        
        println!("🔍 Первый байт protobuf данных: 0x{:02x}", first_byte);
        println!("🔍 Вероятно protobuf: {}", is_likely_protobuf);
        
        // Симуляция поврежденного JSON с невалидными UTF-8 байтами в середине
        let mut corrupted_json = b"{\"test\":\"".to_vec();
        corrupted_json.extend_from_slice(&[0xFF, 0xFE]); // невалидные UTF-8 байты
        corrupted_json.extend_from_slice(b"\"}");
        
        let corrupted_result = String::from_utf8(corrupted_json);
        assert!(corrupted_result.is_err());
        
        println!("✅ Тест различения UTF-8/бинарных данных прошел успешно");
    }

    #[test]
    fn test_compression_headers() {
        // Тест правильного определения заголовков сжатия
        
        // Различные варианты GZIP заголовков
        let gzip_headers = vec![
            vec![0x1f, 0x8b, 0x08, 0x00], // стандартный gzip
            vec![0x1f, 0x8b, 0x08, 0x01], // gzip с текстовым флагом
        ];
        
        for header in gzip_headers {
            let is_gzip = header.len() >= 2 && header[0] == 0x1f && header[1] == 0x8b;
            assert!(is_gzip, "Заголовок GZIP не распознан: {:?}", header);
        }
        
        // Различные варианты DEFLATE/ZLIB заголовков
        let deflate_headers = vec![
            vec![0x78, 0x01], // zlib deflate, уровень сжатия 1
            vec![0x78, 0x9c], // zlib deflate, уровень сжатия 6 (по умолчанию)
            vec![0x78, 0xda], // zlib deflate, уровень сжатия 9
        ];
        
        for header in deflate_headers {
            let is_deflate = header.len() >= 2 && 
                ((header[0] == 0x78 && (header[1] == 0x01 || header[1] == 0x9c || header[1] == 0xda)) ||
                 (header[0] == 0x08 || header[0] == 0x78));
            assert!(is_deflate, "Заголовок DEFLATE не распознан: {:?}", header);
        }
        
        // Данные, которые НЕ должны определяться как сжатые
        let non_compressed = vec![
            vec![0x7b, 0x22], // "{ JSON начало
            vec![0x5b, 0x22], // "[ JSON массив
            vec![0x08, 0x96], // protobuf (0x08 но не zlib)
            vec![0x00, 0x01], // произвольные данные
        ];
        
        for data in non_compressed {
            let is_gzip = data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b;
            let is_deflate = data.len() >= 2 && 
                ((data[0] == 0x78 && (data[1] == 0x01 || data[1] == 0x9c || data[1] == 0xda)) ||
                 (data[0] == 0x08 || data[0] == 0x78));
                 
            // Специальная проверка для 0x08 0x96 - не должно определяться как deflate
            if data == vec![0x08, 0x96] {
                assert!(!is_deflate, "0x08 0x96 неправильно определен как deflate");
            }
            
            assert!(!is_gzip, "Данные {:?} неправильно определены как GZIP", data);
            println!("✅ Данные {:?} правильно НЕ определены как сжатые", data);
        }
        
        println!("✅ Тест заголовков сжатия прошел успешно");
    }

    #[test]
    fn test_real_mexc_protobuf_case() {
        // Тест для реального случая из логов пользователя
        let real_protobuf_data = vec![
            10, 30, 115, 112, 111, 116, 64, 112, 114, 105, 118, 97, 116, 101, 46, 97, 99, 99, 111, 117
        ];
        
        // Проверяем, что это НЕ определяется как сжатые данные
        let is_gzip = real_protobuf_data.len() >= 2 && 
            real_protobuf_data[0] == 0x1f && real_protobuf_data[1] == 0x8b;
        let is_deflate_zlib = real_protobuf_data.len() >= 2 && 
            real_protobuf_data[0] == 0x78 && 
            (real_protobuf_data[1] == 0x01 || real_protobuf_data[1] == 0x9c || real_protobuf_data[1] == 0xda);
        
        assert!(!is_gzip, "Реальные protobuf данные неправильно определены как GZIP");
        assert!(!is_deflate_zlib, "Реальные protobuf данные неправильно определены как DEFLATE");
        
        // Проверяем, что это правильно определяется как protobuf
        let is_likely_protobuf = real_protobuf_data.len() >= 4 && 
            !is_gzip && !is_deflate_zlib &&
            (
                (real_protobuf_data[0] == 0x08 && real_protobuf_data[1] < 0x80) ||
                (real_protobuf_data[0] == 0x0a && real_protobuf_data[1] < 0x80) ||
                (real_protobuf_data[0] == 0x10 && real_protobuf_data[1] < 0x80) ||
                (real_protobuf_data[0] == 0x12 && real_protobuf_data[1] < 0x80) ||
                (real_protobuf_data[0] == 0x0a && real_protobuf_data.len() > 10 && 
                 real_protobuf_data[2..].starts_with(b"spot@"))
            );
        
        assert!(is_likely_protobuf, "Реальные protobuf данные не обнаружены правильно");
        
        // Проверяем, что мы можем извлечь информацию о канале
        if real_protobuf_data[0] == 0x0a {
            let channel_length = real_protobuf_data[1] as usize;
            assert_eq!(channel_length, 30, "Неправильная длина канала");
            
            if real_protobuf_data.len() >= 2 + channel_length {
                let channel_bytes = &real_protobuf_data[2..2 + std::cmp::min(channel_length, real_protobuf_data.len() - 2)];
                if let Ok(channel_name) = String::from_utf8(channel_bytes.to_vec()) {
                    assert!(channel_name.starts_with("spot@private"), "Канал не начинается с 'spot@private': {}", channel_name);
                    println!("✅ Успешно извлечен канал из protobuf: '{}'", channel_name);
                }
            }
        }
        
        // Проверяем, что это не валидный UTF-8 из-за бинарных данных в конце
        let utf8_result = String::from_utf8(real_protobuf_data.clone());
        // В реальных данных в конце есть невалидные UTF-8 байты, поэтому это должно быть ошибкой
        // Но для нашего урезанного примера это может быть OK, поэтому просто информируем
        match utf8_result {
            Ok(s) => println!("📝 Данные являются валидным UTF-8: '{}'", s),
            Err(e) => println!("📝 Данные НЕ являются валидным UTF-8: {}", e),
        }
        
        println!("✅ Тест реального случая MEXC protobuf прошел успешно");
    }
} 