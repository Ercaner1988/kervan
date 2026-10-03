use kervan_direnc::{DevreKesiciYapilandirma, DirencYoneticisi, TestSaati};
use kervan_yukari::{Hedef, Yukari, YukariHatasi};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn baslat_sahte_sunucu_sayacli(
    durum_kodu: u16,
    durum_metni: &'static str,
    yanit_govdesi: String,
    ek_basliklar: Vec<String>,
) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("TcpListener açılamadı");
    let addr = listener.local_addr().expect("Adres alınamadı");
    let istek_sayaci = Arc::new(AtomicUsize::new(0));
    let sayac_klon = istek_sayaci.clone();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            sayac_klon.fetch_add(1, Ordering::SeqCst);

            let mut buffer = Vec::new();
            let mut tmp = [0u8; 1024];
            let mut body_start = None;
            let mut content_length = None;

            loop {
                let Ok(n) = s.read(&mut tmp) else {
                    break;
                };
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

            let mut baslik_satirlari = String::new();
            for b in &ek_basliklar {
                baslik_satirlari.push_str(b);
                baslik_satirlari.push_str("\r\n");
            }

            let response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                durum_kodu,
                durum_metni,
                yanit_govdesi.len(),
                baslik_satirlari,
                yanit_govdesi
            );
            let _ = s.write_all(response.as_bytes());
            let _ = s.flush();
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    });

    (format!("http://{}", addr), istek_sayaci)
}

#[tokio::test]
async fn test_p7_ardisik_5xx_kesiciyi_acar() {
    let (url, istek_sayaci) = baslat_sahte_sunucu_sayacli(
        500,
        "Internal Server Error",
        r#"{"error": "Sunucu içi kritik arıza"}"#.to_string(),
        vec![],
    );

    let test_saati = Arc::new(TestSaati::yeni(Instant::now()));
    let direnc = DirencYoneticisi::yeni(
        test_saati.clone(),
        Some(DevreKesiciYapilandirma {
            hata_esigi: 3,
            bozulma_esigi: 2,
            soguma_suresi: Duration::from_secs(30),
            yari_acik_izin: 1,
        }),
    );
    let yukari = Yukari::yeni_direncli(direnc);

    let env_key = "TEST_P7_BREAKER_KEY";
    unsafe { std::env::set_var(env_key, "sk-test-breaker-key-123") };

    let hedef = Hedef {
        ad: "test-breaker-hedef".to_string(),
        bicim: "openai".to_string(),
        taban_url: url,
        anahtar_env: env_key.to_string(),
        model: "gpt-4o-mini".to_string(),
        yol: Some("/v1/chat/completions".to_string()),
        zaman_asimi: Duration::from_secs(5),
    };

    let govde = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "test"}]
    });

    // 1. İstek -> 500 hatası, sayaç = 1
    let res1 = yukari.gonder(&hedef, &govde, None).await;
    assert!(matches!(res1, Err(YukariHatasi::HttpHatasi(500, _, _))));
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 1);

    // 2. İstek -> 500 hatası, sayaç = 2
    let res2 = yukari.gonder(&hedef, &govde, None).await;
    assert!(matches!(res2, Err(YukariHatasi::HttpHatasi(500, _, _))));
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 2);

    // 3. İstek -> 500 hatası, sayaç = 3 (Eşik 3 doldu, devre kesici AÇILDI!)
    let res3 = yukari.gonder(&hedef, &govde, None).await;
    assert!(matches!(res3, Err(YukariHatasi::HttpHatasi(500, _, _))));
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 3);

    // 4. İstek -> Devre kesici AÇIK! İstek yukarı akışa GİTMEDEN reddedilmeli!
    let res4 = yukari.gonder(&hedef, &govde, None).await;
    match res4 {
        Err(YukariHatasi::DevreAcik { ad, .. }) => {
            assert_eq!(ad, "test-breaker-hedef");
        }
        other => panic!("4. istekte DevreAcik bekleniyordu, dönen: {:?}", other),
    }

    // Sahte sunucu sayacı 3 kalmalıdır! (4. istek yukarı akışa hiç gitmedi)
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_p7_429_retry_after_ve_soguma() {
    let (url, istek_sayaci) = baslat_sahte_sunucu_sayacli(
        429,
        "Too Many Requests",
        r#"{"error": "Hız limiti aşıldı"}"#.to_string(),
        vec!["Retry-After: 5".to_string()],
    );

    let test_saati = Arc::new(TestSaati::yeni(Instant::now()));
    let direnc = DirencYoneticisi::yeni(test_saati.clone(), None);
    let yukari = Yukari::yeni_direncli(direnc);

    let env_key = "TEST_P7_RETRY_KEY";
    unsafe { std::env::set_var(env_key, "sk-test-retry-key-456") };

    let hedef = Hedef {
        ad: "test-retry-hedef".to_string(),
        bicim: "openai".to_string(),
        taban_url: url,
        anahtar_env: env_key.to_string(),
        model: "gpt-4o-mini".to_string(),
        yol: Some("/v1/chat/completions".to_string()),
        zaman_asimi: Duration::from_secs(5),
    };

    let govde = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "rate limit test"}]
    });

    // 1. İstek -> 429 hatası, Retry-After: 5, sayaç = 1
    let res1 = yukari.gonder(&hedef, &govde, None).await;
    match res1 {
        Err(YukariHatasi::HttpHatasi(429, _, Some(sn))) => {
            assert_eq!(sn, 5);
        }
        other => panic!("429 Retry-After: 5 bekleniyordu, dönen: {:?}", other),
    }
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 1);

    // 2. İstek hemen sonrasında -> Soğumada! Yukarı akışa GİTMEDEN reddedilmeli!
    let res2 = yukari.gonder(&hedef, &govde, None).await;
    match res2 {
        Err(YukariHatasi::Sogumada {
            ad,
            retry_after_saniye,
        }) => {
            assert_eq!(ad, "test-retry-hedef");
            assert_eq!(retry_after_saniye, 5);
        }
        other => panic!("Sogumada hatası bekleniyordu, dönen: {:?}", other),
    }

    // Sayaç hala 1 kalmalı (2. istek yukarı akışa hiç gitmedi)
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 1);

    // Saati 6 saniye ilerletelim (soğuma süresi biter, gerçek uyuma yok!)
    test_saati.ilerlet(Duration::from_secs(6));

    // 3. İstek -> Soğuma bittiği için istek yukarı akışa tekrar gider!
    let res3 = yukari.gonder(&hedef, &govde, None).await;
    assert!(matches!(res3, Err(YukariHatasi::HttpHatasi(429, _, _))));
    // Sayaç 2 olmalı!
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_p7_4xx_hata_kesiciyi_etkilemez() {
    let (url, istek_sayaci) = baslat_sahte_sunucu_sayacli(
        400,
        "Bad Request",
        r#"{"error": "Geçersiz istemci parametresi"}"#.to_string(),
        vec![],
    );

    let test_saati = Arc::new(TestSaati::yeni(Instant::now()));
    let direnc = DirencYoneticisi::yeni(
        test_saati.clone(),
        Some(DevreKesiciYapilandirma {
            hata_esigi: 3,
            bozulma_esigi: 2,
            soguma_suresi: Duration::from_secs(30),
            yari_acik_izin: 1,
        }),
    );
    let yukari = Yukari::yeni_direncli(direnc);

    let env_key = "TEST_P7_4XX_KEY";
    unsafe { std::env::set_var(env_key, "sk-test-4xx-key-789") };

    let hedef = Hedef {
        ad: "test-4xx-hedef".to_string(),
        bicim: "openai".to_string(),
        taban_url: url,
        anahtar_env: env_key.to_string(),
        model: "gpt-4o-mini".to_string(),
        yol: Some("/v1/chat/completions".to_string()),
        zaman_asimi: Duration::from_secs(5),
    };

    let govde = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "bad request test"}]
    });

    // 400 hatasından 5 kez gönderelim
    for _ in 0..5 {
        let res = yukari.gonder(&hedef, &govde, None).await;
        assert!(matches!(res, Err(YukariHatasi::HttpHatasi(400, _, _))));
    }

    // 4xx hataları kesiciyi beslemediği için tüm 5 istek de yukarı akışa gitti!
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn test_p7_hata_temizligi_ve_anahtar_sizdirmazligi() {
    let gizli_anahtar = "sk-super-gizli-anahtar-xyz-998877";
    let env_key = "TEST_P7_SECRET_KEY_ENV";
    unsafe { std::env::set_var(env_key, gizli_anahtar) };

    // Sahte sunucu hata gövdesinde anahtarı ve iç dosya yolunu yankılar
    let sahte_hata_govdesi = format!(
        r#"{{"error": "Unauthorized key: {}", "internal_trace": "File 'C:\\Users\\admin\\kervan\\config.rs' line 42, Bearer {}"}}"#,
        gizli_anahtar, gizli_anahtar
    );

    let (url, _) = baslat_sahte_sunucu_sayacli(401, "Unauthorized", sahte_hata_govdesi, vec![]);

    let yukari = Yukari::new();
    let hedef = Hedef {
        ad: "test-leak-hedef".to_string(),
        bicim: "openai".to_string(),
        taban_url: url,
        anahtar_env: env_key.to_string(),
        model: "gpt-4o-mini".to_string(),
        yol: Some("/v1/chat/completions".to_string()),
        zaman_asimi: Duration::from_secs(5),
    };

    let govde = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "leak test"}]
    });

    let res = yukari.gonder(&hedef, &govde, None).await;
    match res {
        Err(YukariHatasi::HttpHatasi(401, temizlenmis_govde, _)) => {
            // 1. Gizli anahtar asla yer almamalıdır!
            assert!(
                !temizlenmis_govde.contains(gizli_anahtar),
                "Gizli anahtar hata gövdesine sızdı! Gövde: {}",
                temizlenmis_govde
            );
            assert!(
                temizlenmis_govde.contains("[GIZLENDI]"),
                "Maskelenen alan '[GIZLENDI]' içermiyor: {}",
                temizlenmis_govde
            );

            // 2. İç dosya yolları sızmamalıdır!
            assert!(
                !temizlenmis_govde.contains("C:\\Users\\admin"),
                "İç Windows dosya yolu sızdı! Gövde: {}",
                temizlenmis_govde
            );
            assert!(
                temizlenmis_govde.contains("[GIZLI_YOL]"),
                "Maskelenen yol '[GIZLI_YOL]' içermiyor: {}",
                temizlenmis_govde
            );
        }
        other => panic!("401 HttpHatasi bekleniyordu, dönen: {:?}", other),
    }
}
