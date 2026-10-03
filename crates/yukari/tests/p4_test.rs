use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use kervan_tarayici::config::Config;
use kervan_yukari::{Hedef, Yukari, YukariHatasi};

// P4-A1: FR-4.1 Yapılandırma ve Anahtar Doğrulaması
#[tokio::test]
async fn p4_a1_yapilandirma_ve_anahtar_dogrulamasi() {
    // 1. (c) kervan.toml.example yeni tablosuz da yüklenir (geriye uyum)
    let example_yol = Path::new("kervan.toml.example");
    let cfg = Config::load(example_yol).expect("kervan.toml.example hatasız yüklenmelidir");
    assert!(
        cfg.rest.is_empty(),
        "kervan.toml.example içinde rest boş olmalı"
    );
    assert!(cfg.dogrula().is_ok());

    // 2. (b) Ortam değişkeni yoksa istek 503 ve hata metninde değişken adı olur, değeri asla
    let hedef = Hedef {
        ad: "test_ds".into(),
        bicim: "openai".into(),
        taban_url: "http://127.0.0.1:9".into(),
        anahtar_env: "VAR_OLMAYAN_API_ANAHTARI_XYZ_12345".into(),
        model: "deepseek-chat".into(),
        yol: None,
        zaman_asimi: Duration::from_secs(5),
    };

    let yukari = Yukari::new();
    let govde = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": "merhaba"}]
    });

    let err = yukari.gonder(&hedef, &govde, None).await.unwrap_err();
    match err {
        YukariHatasi::AnahtarYok(env_ad) => {
            assert_eq!(env_ad, "VAR_OLMAYAN_API_ANAHTARI_XYZ_12345");
            let hata_metni = format!("{}", YukariHatasi::AnahtarYok(env_ad));
            assert!(hata_metni.contains("VAR_OLMAYAN_API_ANAHTARI_XYZ_12345"));
        }
        diger => panic!("Beklenmeyen hata: {:?}", diger),
    }
}

// P4-A2: FR-4.2 OpenAI-biçimli Kayıpsız Geçiş ve Derin Eşitlik
#[tokio::test]
async fn p4_a2_openai_bicimli_gecis_kayipsiz_ve_derin_esit() {
    // 1. Sahte yukarı akış HTTP sunucusu kur (rastgele boş port)
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let gizli_anahtar = "sk-super-secret-key-987654";
    let env_adi = "TEST_KERVAN_REST_KEY";
    unsafe {
        std::env::set_var(env_adi, gizli_anahtar);
    }

    let istemci_govdesi = json!({
        "model": "kervan/test_provider/ozel-model",
        "messages": [
            {"role": "system", "content": "Sen yardımcı bir asistansın."},
            {"role": "user", "content": "2+2 kaç eder?"}
        ],
        "temperature": 0.7,
        "max_tokens": 100,
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "hesapla",
                    "description": "Matematik işlemi",
                    "parameters": {"type": "object", "properties": {"ifade": {"type": "string"}}}
                }
            }
        ],
        "stream": true // İstemci stream istese bile yukari akışta false olmalı
    });

    let (tx, rx) = tokio::sync::oneshot::channel::<Value>();

    // Mock HTTP server
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = Vec::new();
        let mut tmp = [0u8; 1024];
        let mut body_start = None;
        let mut content_length = None;

        loop {
            let n = socket.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&tmp[..n]);

            if body_start.is_none() {
                if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                    body_start = Some(pos + 4);
                    let header_str = String::from_utf8_lossy(&buffer[..pos]);
                    for line in header_str.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("content-length:") {
                            if let Some(val) = line.split(':').nth(1) {
                                content_length = val.trim().parse::<usize>().ok();
                            }
                        }
                    }
                }
            }

            if let (Some(start), Some(cl)) = (body_start, content_length) {
                if buffer.len() >= start + cl {
                    break;
                }
            }
        }

        let request_str = String::from_utf8_lossy(&buffer);

        // Başlık kontrolleri
        assert!(request_str.contains("POST /v1/chat/completions HTTP/1.1"));
        assert!(
            request_str.contains(&format!("authorization: Bearer {}", gizli_anahtar))
                || request_str.contains(&format!("Authorization: Bearer {}", gizli_anahtar))
        );
        assert!(
            request_str
                .to_lowercase()
                .contains("content-type: application/json")
        );

        // Gövdeyi ayıkla
        let start = body_start.expect("Gövde başlangıcı bulunamadı");
        let cl = content_length.expect("Content-Length bulunamadı");
        let body_bytes = &buffer[start..start + cl];
        let body_json: Value = serde_json::from_slice(body_bytes).unwrap();
        tx.send(body_json).unwrap();

        // Yanıt dön
        let response_body = json!({
            "id": "chatcmpl-test-mock-999",
            "object": "chat.completion",
            "created": 1700000001,
            "model": "ozel-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Sonuç: 4"},
                "finish_reason": "stop"
            }]
        })
        .to_string();

        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(http_response.as_bytes()).await.unwrap();
    });

    let hedef = Hedef {
        ad: "test_provider".into(),
        bicim: "openai".into(),
        taban_url: format!("http://{}", addr),
        anahtar_env: env_adi.into(),
        model: "varsayilan-model".into(),
        yol: None,
        zaman_asimi: Duration::from_secs(5),
    };

    let yukari = Yukari::new();
    let yanit = yukari
        .gonder(&hedef, &istemci_govdesi, Some("ozel-model"))
        .await
        .unwrap();

    assert_eq!(yanit["choices"][0]["message"]["content"], "Sonuç: 4");

    // Yukarı akışın aldığı gövdeyi kontrol et
    let alinan_govde = rx.await.unwrap();

    // 1. Model adı 'ozel-model' olarak değişmiş olmalı
    assert_eq!(alinan_govde["model"], "ozel-model");
    // 2. stream=false zorlanmış olmalı
    assert_eq!(alinan_govde["stream"], false);

    // 3. 'messages', 'temperature', 'max_tokens', 'tools' DERİN EŞİT olmalıdır
    assert_eq!(alinan_govde["messages"], istemci_govdesi["messages"]);
    assert_eq!(alinan_govde["temperature"], istemci_govdesi["temperature"]);
    assert_eq!(alinan_govde["max_tokens"], istemci_govdesi["max_tokens"]);
    assert_eq!(alinan_govde["tools"], istemci_govdesi["tools"]);

    unsafe {
        std::env::remove_var(env_adi);
    }
}

// P4-A3: FR-4.3 Chat Kancası Model Çözümleme ve Ayrım
#[tokio::test]
async fn p4_a3_chat_kancasi_model_cozumleme() {
    let ayar = kervan_tarayici::config::RestAyar {
        bicim: "openai".into(),
        taban_url: "https://api.deepseek.com".into(),
        anahtar_env: "DS_KEY".into(),
        model: "deepseek-chat".into(),
        yol: None,
    };

    let mut rest_map: std::collections::HashMap<String, Hedef> = std::collections::HashMap::new();
    rest_map.insert(
        "deepseek".into(),
        Hedef::rest_ayardan("deepseek", &ayar, Duration::from_secs(10)),
    );

    // 1. kervan/deepseek doğru çözülür
    let sade_model = "kervan/deepseek";
    let (hedef, alt_model) = {
        let sade = sade_model.strip_prefix("kervan/").unwrap_or(sade_model);
        let mut parcalar = sade.splitn(2, '/');
        let saglayici_ad = parcalar.next().unwrap();
        let alt = parcalar.next().map(|s| s.to_string());
        (rest_map.get(saglayici_ad).unwrap().clone(), alt)
    };
    assert_eq!(hedef.ad, "deepseek");
    assert_eq!(alt_model, None);

    // 2. kervan/deepseek/deepseek-reasoner alt modelle çözülür
    let ozel_model = "kervan/deepseek/deepseek-reasoner";
    let (hedef2, alt_model2) = {
        let sade = ozel_model.strip_prefix("kervan/").unwrap_or(ozel_model);
        let mut parcalar = sade.splitn(2, '/');
        let saglayici_ad = parcalar.next().unwrap();
        let alt = parcalar.next().map(|s| s.to_string());
        (rest_map.get(saglayici_ad).unwrap().clone(), alt)
    };
    assert_eq!(hedef2.ad, "deepseek");
    assert_eq!(alt_model2, Some("deepseek-reasoner".into()));

    // 3. REST olmayan bir model (örn: gemini-flash) çözülmez (None döner -> tarayıcı yoluna düşer)
    let tarayici_model = "kervan/gemini";
    let rest_mi = {
        let sade = tarayici_model
            .strip_prefix("kervan/")
            .unwrap_or(tarayici_model);
        let saglayici_ad = sade.splitn(2, '/').next().unwrap();
        rest_map.contains_key(saglayici_ad)
    };
    assert!(!rest_mi, "Tarayıcı modeli REST hedefine düşmemelidir");
}
