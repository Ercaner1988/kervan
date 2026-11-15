//! `eklenti` modülünün birim testleri.
//!
//! NEDEN AYRI DOSYA (2026-11-15): `eklenti.rs` 1500 fiziksel satır sınırını
//! aşıyordu (kapi "dosya sınırı" kırmızısı: 2086 → 2186 satır). Testler gövdeden
//! ayrıldı; davranış ve test adları (`eklenti::tests::…`) değişmedi.

use super::*;
use kervan_secim::ModelSecenegi;
#[test]
fn test_giden_mesaj_sor_dosyasiz_geriye_uyumluluk() {
    // Eski eklenti/istek formatı (dosyalar alanı yok)
    let json = r#"{"action":"sor","id":"req-1","provider":"gemini","istem":"Merhaba"}"#;
    let msg: GidenMesaj = serde_json::from_str(json).expect("deserialize edilmeli");
    match msg {
        GidenMesaj::Sor {
            id,
            provider,
            istem,
            dosyalar,
            ..
        } => {
            assert_eq!(id, "req-1");
            assert_eq!(provider, "gemini");
            assert_eq!(istem, "Merhaba");
            assert!(dosyalar.is_empty(), "dosyalar varsayılan boş vektör olmalı");
        }
        _ => panic!("GidenMesaj::Sor bekleniyordu"),
    }
}

#[test]
fn test_giden_mesaj_sor_dosyali_ser_de() {
    let msg = GidenMesaj::Sor {
        id: "req-2".into(),
        provider: "gemini".into(),
        profile: None,
        istem: "Bu resmi incele".into(),
        dosyalar: vec![DosyaEki {
            ad: "test.png".into(),
            mime_tipi: "image/png".into(),
            veri_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==".into(),
        }],
    };

    let json = serde_json::to_string(&msg).expect("serialize edilmeli");
    assert!(json.contains("\"dosyalar\""));
    assert!(json.contains("test.png"));
    assert!(json.contains("image/png"));

    let deserialized: GidenMesaj = serde_json::from_str(&json).expect("deserialize edilmeli");
    match deserialized {
        GidenMesaj::Sor { dosyalar, .. } => {
            assert_eq!(dosyalar.len(), 1);
            assert_eq!(dosyalar[0].ad, "test.png");
            assert_eq!(dosyalar[0].mime_tipi, "image/png");
        }
        _ => panic!("GidenMesaj::Sor bekleniyordu"),
    }
}

/// Zaman aşımında biriken metin atılmaz: `YarimYanit` olarak döner ve üretim durdurulur.
#[tokio::test]
async fn zaman_asiminda_yarim_metin_doner() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;
    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
            panic!("önce Sor gelmeli")
        };
        let parca = GelenMesaj::Parca {
            id,
            delta: "Yarım cümle".into(),
            done: false,
        };
        h.mesaji_isle(parca).await;
        rx.recv().await
    });
    let hata = hub
        .sor("gemini", "soru", std::time::Duration::from_millis(300))
        .await
        .unwrap_err();
    let yarim = hata
        .downcast::<YarimYanit>()
        .expect("YarimYanit bekleniyordu");
    assert_eq!(yarim.0, "Yarım cümle");
    assert!(matches!(
        besleyici.await.unwrap(),
        Some(GidenMesaj::Durdur { .. })
    ));
}

/// İki profil bağlıyken hesap seçimi en-az-kullanılanı izler: 4 seçimde
/// her profil 2 kez. Aynı sağlayıcının profilleri birbirini ezmez.
#[tokio::test]
async fn en_az_kullanilan_hesap_seciliyor() {
    let hub = EklentiHub::new();
    let (tx_a, _rx_a) = mpsc::channel(8);
    let (tx_b, _rx_b) = mpsc::channel(8);
    hub.sekme_kaydet("gemini".into(), "a".into(), tx_a).await;
    hub.sekme_kaydet("gemini".into(), "b".into(), tx_b).await;

    let mut secilenler = Vec::new();
    for _ in 0..4 {
        let (_tx, profile) = hub.hesap_sec("gemini", None).await.unwrap();
        secilenler.push(profile);
    }
    secilenler.sort();
    assert_eq!(secilenler, vec!["a", "a", "b", "b"]);
    assert_eq!(hub.bagli_hesaplar().await, vec!["gemini:a", "gemini:b"]);
    assert_eq!(hub.bagli_sekmeler().await, vec!["gemini"]);
}

/// Eski eklenti `profile` alanı göndermez; ayrıştırma düşmemeli ve sunucu
/// `Sor` içinde profili `None` bırakmalı (eklenti kendi varsayılanına düşer).
#[test]
fn eski_eklenti_profilsiz_calismaya_devam_eder() {
    let json = r#"{"action":"sor","id":"req-9","provider":"gemini","istem":"Merhaba"}"#;
    let msg: GidenMesaj = serde_json::from_str(json).expect("profil alanı zorunlu değil");
    match msg {
        GidenMesaj::Sor { profile, .. } => assert_eq!(profile, None),
        _ => panic!("Sor bekleniyordu"),
    }
}

#[test]
fn host_denetimi_yalniz_geri_donguye_izin_verir() {
    assert!(host_gecerli_mi("127.0.0.1:20129"));
    assert!(host_gecerli_mi("localhost:20129"));
    assert!(host_gecerli_mi("[::1]:20129"));
    assert!(host_gecerli_mi("LOCALHOST"));
    assert!(!host_gecerli_mi("kervan.example.com:20129"));
    assert!(!host_gecerli_mi("127.0.0.1.nip.io:20129"));
    assert!(!host_gecerli_mi(""));
}

#[test]
fn origin_denetimi_web_sayfasini_reddeder() {
    assert!(origin_gecerli_mi(None));
    assert!(origin_gecerli_mi(Some("chrome-extension://abcdef")));
    assert!(origin_gecerli_mi(Some("moz-extension://1111-2222")));
    assert!(!origin_gecerli_mi(Some("https://evil.example")));
    assert!(!origin_gecerli_mi(Some("http://127.0.0.1:20129")));
    assert!(!origin_gecerli_mi(Some("null")));
}

#[test]
fn token_karsilastirmasi_uzunluk_ve_icerik_duyarli() {
    assert!(token_esit_mi("abc123", "abc123"));
    assert!(!token_esit_mi("abc123", "abc124"));
    assert!(!token_esit_mi("abc", "abc123"));
    assert!(!token_esit_mi("", "abc123"));
}

/// İstemci kopunca (`sor` future'ı düşünce) iki kayıt da silinmeli ve
/// eklentiye `Durdur` gitmeli. Guard olmadan bu kayıtlar süreç ömrü boyunca
/// kalıyordu (S4).
#[tokio::test]
async fn istemci_kopunca_kayit_silinir_ve_durdur_gonderilir() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let gorev = tokio::spawn({
        let h = hub.clone();
        async move {
            h.sor("gemini", "soru", std::time::Duration::from_secs(60))
                .await
        }
    });

    assert!(matches!(rx.recv().await, Some(GidenMesaj::Sor { .. })));
    assert_eq!(hub.bekleyen_istekler.lock().await.len(), 1);
    assert_eq!(hub.biriken_metinler.lock().await.len(), 1);

    gorev.abort();
    let _ = gorev.await;

    let durdur = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("temizlik görevi çalışmalı")
        .expect("kanal açık kalmalı");
    assert!(matches!(durdur, GidenMesaj::Durdur { .. }));
    assert!(hub.bekleyen_istekler.lock().await.is_empty());
    assert!(hub.biriken_metinler.lock().await.is_empty());
}

fn deneme_menusu() -> ModelMenusu {
    ModelMenusu {
        dugme: "button#m".into(),
        oge: "[role=menuitem]".into(),
        modeller: vec![
            ModelSecenegi {
                kod: "hizli".into(),
                etiket: "Hızlı".into(),
                kilitli: false,
                kilit_nedeni: None,
            },
            ModelSecenegi {
                kod: "yuksek".into(),
                etiket: "Yüksek".into(),
                kilitli: false,
                kilit_nedeni: None,
            },
        ],
    }
}

fn oge(metin: &str, isaretli: bool) -> OgeDurum {
    OgeDurum {
        metin: metin.into(),
        tam: metin.into(),
        isaretli,
        kanit: String::new(),
    }
}

/// Tel biçimi sabittir: eklenti bu adları bekler (`action` + alanlar).
#[test]
fn secim_turu_tel_bicimi() {
    let oku = serde_json::to_value(GidenMesaj::OgeOku {
        id: "tur-1".into(),
        provider: "gemini".into(),
        profile: Some("p".into()),
        secici: "[role=menuitem]".into(),
        dugme: Some("button#m".into()),
    })
    .unwrap();
    assert_eq!(oku["action"], "oge_oku");
    assert_eq!(oku["indeks"], serde_json::Value::Null);

    let bas = serde_json::to_value(GidenMesaj::OgeBas {
        id: "tur-2".into(),
        provider: "gemini".into(),
        profile: None,
        secici: ".toggle".into(),
        indeks: 3,
        dugme: None,
    })
    .unwrap();
    assert_eq!(bas["action"], "oge_bas");
    assert_eq!(bas["indeks"], 3);
    assert!(bas.get("dugme").is_none(), "boş dugme tele yazılmamalı");

    let yanit: GelenMesaj = serde_json::from_str(
        r#"{"action":"ogeler","id":"tur-2","ogeler":[{"metin":"Yüksek","tam":"Yüksek","isaretli":true}]}"#,
    )
    .expect("eklenti yanıtı çözülmeli");
    match yanit {
        GelenMesaj::Ogeler { id, ogeler } => {
            assert_eq!(id, "tur-2");
            assert!(ogeler[0].isaretli);
            assert_eq!(ogeler[0].metin, "Yüksek");
        }
        _ => panic!("Ogeler bekleniyordu"),
    }
}

/// Mutlu yol: menü okunur, DOĞRU indekse tıklanır, seçim doğrulanır ve
/// istem AYNI hesaba gider (seçim başka sekmeye kaçmaz).
#[tokio::test]
async fn secim_turu_dogru_indekse_tiklar_ve_ayni_hesaba_sorar() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::OgeOku {
            id,
            provider,
            profile,
            secici,
            dugme,
        }) = rx.recv().await
        else {
            panic!("önce OgeOku gelmeli")
        };
        assert_eq!(provider, "gemini");
        assert_eq!(profile.as_deref(), Some("p"));
        assert_eq!(secici, "[role=menuitem]");
        assert_eq!(dugme.as_deref(), Some("button#m"));
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", true), oge("Yüksek", false)],
        })
        .await;

        let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
            panic!("sonra OgeBas gelmeli")
        };
        assert_eq!(indeks, 1, "Yüksek ikinci sırada");
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
        })
        .await;

        let Some(GidenMesaj::Sor { id, profile, .. }) = rx.recv().await else {
            panic!("en son Sor gelmeli")
        };
        assert_eq!(profile.as_deref(), Some("p"));
        h.mesaji_isle(GelenMesaj::Parca {
            id: id.clone(),
            delta: "yanıt".into(),
            done: false,
        })
        .await;
        h.mesaji_isle(GelenMesaj::Parca {
            id,
            delta: String::new(),
            done: true,
        })
        .await;
    });

    let menu = deneme_menusu();
    let yanit = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(menu.clone()),
                model_kodu: Some("yuksek".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_secs(5),
        )
        .await
        .expect("seçim ve istem tamamlanmalı");
    assert_eq!(yanit, "yanıt");
    besleyici.await.unwrap();
}

/// Menüde zaten seçiliyse TIKLAMA YAPILMAZ (gereksiz tıklama seçimi bozabilir).
#[tokio::test]
async fn secili_model_icin_tiklama_yapilmaz() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
            panic!("OgeOku gelmeli")
        };
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
        })
        .await;
        let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
            panic!("tıklama olmadan Sor gelmeli")
        };
        h.mesaji_isle(GelenMesaj::Parca {
            id,
            delta: "tamam".into(),
            done: true,
        })
        .await;
    });

    let menu = deneme_menusu();
    let yanit = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(menu.clone()),
                model_kodu: Some("yuksek".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(yanit, "tamam");
    besleyici.await.unwrap();
}

/// ALT DÜĞÜM İŞARETİ GÜÇLÜ KANITI GÖLGELEMEZ (2026-10-24 canlı ölçümü).
///
/// Menü "3.6 Flash*[sinif:selected] | 3.6 Düşünen*[torun:aria-checked=true]"
/// okunmuştu. Hedef "Düşünen" olsaydı eski kod onu işaretli sanıp tıklamayı
/// atlar, istek sessizce Flash ile giderdi. Güçlü kanıt varken zayıf kanıt
/// yok hükmündedir; hedef tıklanır.
#[tokio::test]
async fn alt_dugum_isareti_guclu_kaniti_golgelemez() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
            panic!("OgeOku gelmeli")
        };
        let kanitli = |ad: &str, kanit: &str| OgeDurum {
            metin: ad.into(),
            tam: ad.into(),
            isaretli: true,
            kanit: kanit.into(),
        };
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![
                kanitli("Hızlı", "sinif:selected"),
                kanitli("Yüksek", "torun:aria-checked=true"),
            ],
        })
        .await;
        let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
            panic!("güçlü kanıt varken hedefe tıklanmalı")
        };
        assert_eq!(indeks, 1);
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
        })
        .await;
        let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
            panic!("doğrulama sonrası Sor gelmeli")
        };
        h.mesaji_isle(GelenMesaj::Parca {
            id,
            delta: "tamam".into(),
            done: true,
        })
        .await;
    });

    let yanit = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(deneme_menusu()),
                model_kodu: Some("yuksek".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(yanit, "tamam");
    besleyici.await.unwrap();
}

/// Zayıf kanıt tıklamayı ATLAMAYA yetmez; tıklama sonrası doğrulamaya yeter
/// (yalnız listede zayıf kanıtlı tek öğe ise).
#[test]
fn zayif_kanit_atlamaya_yetmez_dogrulamaya_yeter() {
    let kanitli = |ad: &str, kanit: &str| OgeDurum {
        metin: ad.into(),
        tam: ad.into(),
        isaretli: true,
        kanit: kanit.into(),
    };
    let liste = vec![
        oge("Hızlı", false),
        kanitli("Yüksek", "torun:aria-checked=true"),
    ];
    assert!(!zaten_secili(&liste, 1), "zayıf kanıt atlamaya yetmez");
    assert!(secim_dogrulandi(&liste, 1), "tek zayıf işaret hedefte");
}

/// İKİ EKSENLİ MENÜ (2026-10-24 incelemesinin blocker'ı): mod ekseni güçlü
/// işaretli, düşünme düzeyi hedefi zayıf işaretli. Kural liste genelinde
/// "tam bir işaret" arasaydı bu menüde seçim HİÇ doğrulanamaz, her istek
/// düşerdi. Tıklanır ve hedefin kendi kanıtıyla doğrulanır.
#[test]
fn iki_eksenli_menude_hedef_dogrulanir() {
    let kanitli = |ad: &str, kanit: &str| OgeDurum {
        metin: ad.into(),
        tam: ad.into(),
        isaretli: true,
        kanit: kanit.into(),
    };
    let liste = vec![
        kanitli("Otomatik", "sinif:selected"),
        kanitli("Yüksek", "torun:aria-checked=true"),
    ];
    assert!(
        !zaten_secili(&liste, 1),
        "iki eksenli menüde atlama hükmü verilmez"
    );
    assert!(
        secim_dogrulandi(&liste, 1),
        "hedef tek zayıf işaretli öğeyse doğrulanır"
    );
}

/// Gemini'nin gerçek işareti `aria-label="Seçildi"` (güçlü), yanlış
/// eşleşme ise çıplak `aria-checked` (zayıf). Güçlü kanıtlı hedef, başka
/// satır işaretli olsa da doğrulanır; zayıf kanıtlı satır atlamaya yetmez.
#[test]
fn secildi_etiketi_cikplak_aria_checked_den_ustundur() {
    let kanitli = |ad: &str, kanit: &str| OgeDurum {
        metin: ad.into(),
        tam: ad.into(),
        isaretli: true,
        kanit: kanit.into(),
    };
    let liste = vec![
        kanitli("3.6 Flash", "torun:secildi"),
        kanitli("3.6 Düşünen", "torun:aria-checked=true"),
    ];
    assert!(
        secim_dogrulandi(&liste, 0),
        "güçlü kanıtlı öğe seçili sayılmalı"
    );
    assert!(!zaten_secili(&liste, 1), "zayıf kanıt atlamaya yetmez");
}

/// ÇOK İŞARETLİ MENÜ (2026-10-24 canlı ölçümü: "3.6 Flash* | 3.6 Düşünen*").
///
/// Hedef "işaretli" görünse bile atlama hükmü verilmez (atlamak için güçlü
/// ve TEK işaret gerekir); tıklanır ve hedefin kendi kanıtıyla doğrulanır.
#[tokio::test]
async fn cok_isaretli_menude_tiklama_atlanmaz() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
            panic!("OgeOku gelmeli")
        };
        // Bozuk okuma: iki öğe birden işaretli, hedef de aralarında.
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", true), oge("Yüksek", true)],
        })
        .await;
        let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
            panic!("çok işaretli menüde OgeBas gelmeli")
        };
        assert_eq!(indeks, 1, "hedef öğe (Yüksek) tıklanmalı");
        // Tıklama sonrası sağlıklı okuma: tek işaret, o da hedefte.
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
        })
        .await;
        let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
            panic!("doğrulama sonrası Sor gelmeli")
        };
        h.mesaji_isle(GelenMesaj::Parca {
            id,
            delta: "tamam".into(),
            done: true,
        })
        .await;
    });

    let yanit = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(deneme_menusu()),
                model_kodu: Some("yuksek".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(yanit, "tamam");
    besleyici.await.unwrap();
}

/// İKİ EKSENLİ MENÜ UÇTAN UCA (2026-10-24 incelemesinin blocker'ı).
///
/// Gerçek gemini menüsü: mod ekseni `sinif:selected` ile GÜÇLÜ işaretli,
/// düşünme düzeyi hedefi (`Yüksek`) alt düğüm `aria-checked` ile ZAYIF
/// işaretli. Eski kural liste genelinde "tam bir işaret" aradığı için bu
/// menüde seçimi hiç doğrulayamaz, her isteği düşürürdü. Doğru davranış:
/// tıkla, sonra hedefin kendi kanıtına bak (zayıf işaret tek zayıfsa yeter)
/// ve isteği gönder — mod ekseninin güçlü işareti hedefi geçersiz kılmaz.
#[tokio::test]
async fn iki_eksenli_menude_istem_gonderilir() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let kanitli = |ad: &str, kanit: &str| OgeDurum {
        metin: ad.into(),
        tam: ad.into(),
        isaretli: true,
        kanit: kanit.into(),
    };
    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
            panic!("OgeOku gelmeli")
        };
        // Mod ekseni güçlü + hedef zayıf: tıklanmalı (atlanmamalı).
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![
                kanitli("Otomatik", "sinif:selected"),
                kanitli("Yüksek", "torun:aria-checked=true"),
            ],
        })
        .await;
        let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
            panic!("iki eksenli menüde hedefe tıklanmalı")
        };
        assert_eq!(indeks, 1, "hedef öğe (Yüksek) tıklanmalı");
        // Tıklama sonrası menü yine iki eksenli; hedef zayıf işaretli.
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![
                kanitli("Otomatik", "sinif:selected"),
                kanitli("Yüksek", "torun:aria-checked=true"),
            ],
        })
        .await;
        let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
            panic!("iki eksenli menüde doğrulama sonrası Sor gelmeli")
        };
        h.mesaji_isle(GelenMesaj::Parca {
            id,
            delta: "tamam".into(),
            done: true,
        })
        .await;
    });

    let yanit = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(deneme_menusu()),
                model_kodu: Some("yuksek".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(yanit, "tamam");
    besleyici.await.unwrap();
}

/// Seçim DOĞRULANAMAZSA istem hiç gönderilmez: sessizce yanlış modelle
/// cevap vermek en pahalı hata türü.
#[tokio::test]
async fn dogrulanmayan_secim_istemi_dusurur() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
            panic!("OgeOku gelmeli")
        };
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", true), oge("Yüksek", false)],
        })
        .await;
        let Some(GidenMesaj::OgeBas { id, .. }) = rx.recv().await else {
            panic!("OgeBas gelmeli")
        };
        // Tıklama işe yaramadı: işaret yerinde değil.
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![oge("Hızlı", true), oge("Yüksek", false)],
        })
        .await;
        // Sonraki mesaj Sor OLSAYDI test kırılırdı; 1 sn bekleyip None döner.
        tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .ok()
            .flatten()
    });

    let menu = deneme_menusu();
    let hata = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(menu.clone()),
                model_kodu: Some("yuksek".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_secs(5),
        )
        .await
        .expect_err("doğrulanmayan seçim hata vermeli");
    assert!(
        format!("{hata:#}").contains("dogrulanamadi"),
        "hata doğrulamayı söylemeli: {hata:#}"
    );
    assert!(
        besleyici.await.unwrap().is_none(),
        "istem gönderilmemeliydi"
    );
    assert!(hub.bekleyen_istekler.lock().await.is_empty());
}

/// Tur yanıtı gelmezse bekleyen kayıt DÜŞÜRÜLÜR; sızıntı süreç ömrü boyunca kalırdı.
#[tokio::test]
async fn zaman_asiminda_bekleyen_tur_silinir() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let h = hub.clone();
    tokio::spawn(async move {
        assert!(matches!(rx.recv().await, Some(GidenMesaj::OgeOku { .. })));
        // Yanıt yok: zaman aşımı bekleniyor.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let _ = &h;
    });

    let menu = deneme_menusu();
    let hata = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(menu.clone()),
                model_kodu: Some("yuksek".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_millis(300),
        )
        .await
        .expect_err("tur zaman aşımına uğramalı");
    assert!(
        format!("{hata:#}").contains("zaman asimina"),
        "hata zaman aşımını söylemeli: {hata:#}"
    );
    assert!(
        hub.bekleyen_istekler.lock().await.is_empty(),
        "zaman aşımından sonra bekleyen kayıt kalmamalı"
    );
}

/// Tanımsız model kodu seçim turuna ÇIKMADAN hata verir (menüde arama yok).
#[tokio::test]
async fn tanimsiz_kod_tura_cikmadan_hata_verir() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;
    let menu = deneme_menusu();
    let hata = hub
        .sor_secimli(
            "gemini",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: Some(menu.clone()),
                model_kodu: Some("yok-boyle".into()),
                anahtarlar: Vec::new(),
            },
            std::time::Duration::from_secs(2),
        )
        .await
        .expect_err("tanımsız kod hata vermeli");
    assert!(format!("{hata:#}").contains("tanimsiz model kodu"));
    assert!(
        rx.try_recv().is_err(),
        "eklentiye hiçbir mesaj gitmemeliydi"
    );
}

/// Aç/kapa doğrulaması "yokluk = kapalı" saymamalı.
///
/// 2026-10-24 incelemesi: tıklamadan sonra öğe listeden kaybolduğunda
/// `unwrap_or(false)` devreye giriyor, istenen durum `false` ise SAHTE
/// BAŞARI üretiyor ve istem doğrulanmamış seçimle siteye gidiyordu.
#[tokio::test]
async fn kaybolan_anahtar_sahte_basari_vermez() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("deepseek".into(), "p".into(), tx).await;
    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
            panic!("OgeOku bekleniyordu")
        };
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: vec![OgeDurum {
                metin: "DeepThink".into(),
                tam: "DeepThink".into(),
                isaretli: true,
                kanit: String::new(),
            }],
        })
        .await;
        let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
            panic!("OgeBas bekleniyordu")
        };
        assert_eq!(indeks, 0);
        // Tıklamadan sonra öğe sayfadan kayboldu: liste boş.
        h.mesaji_isle(GelenMesaj::Ogeler {
            id,
            ogeler: Vec::new(),
        })
        .await;
        // Buradan sonra istem GELMEMELİ.
        rx.try_recv()
    });

    let hata = hub
        .sor_secimli(
            "deepseek",
            "soru",
            Vec::new(),
            &EklentiSecimi {
                menu: None,
                model_kodu: None,
                anahtarlar: vec![(
                    Anahtar {
                        kod: "derin".into(),
                        dugme: ".ds-toggle-button".into(),
                        etiket: "DeepThink".into(),
                    },
                    false,
                )],
            },
            std::time::Duration::from_secs(2),
        )
        .await
        .expect_err("kaybolan öğe doğrulanamadı sayılmalı");
    assert!(
        format!("{hata:#}").contains("durum dogrulanamadi"),
        "beklenen hata: {hata:#}"
    );
    assert!(
        besleyici.await.unwrap().is_err(),
        "doğrulanmayan seçimde istem eklentiye gitmemeliydi"
    );
}

/// AYNI HESABA EŞZAMANLI İKİ İSTEK İÇ İÇE GEÇMEZ (2026-10-24 incelemesi).
///
/// Seçim turları üç ayrı gidiş-dönüştür (oku → tıkla → doğrula). Kilit
/// yokken A'nın doğrulaması bittikten sonra B'nin tıklaması sayfadaki
/// modeli değiştiriyor, A'nın istemi **B'nin modeliyle** gidiyordu; üstelik
/// seçim "doğrulanmış" göründüğü için arıza sessiz kalıyordu.
///
/// Ölçüt: eklentiye giden mesajlarda bir isteğin kimliği bitmeden
/// diğerinin kimliği başlamamalı (bitişiklik).
#[tokio::test]
async fn es_zamanli_istekler_ayni_hesapta_ic_ice_gecmez() {
    let hub = EklentiHub::new();
    let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

    let h = hub.clone();
    let besleyici = tokio::spawn(async move {
        // Sahte sayfa: seçili modelin indeksi. Tıklama bunu değiştirir.
        let mut secili = 0usize;
        let mut sira: Vec<(String, &'static str)> = Vec::new();
        let mut biten = 0usize;
        while let Some(mesaj) = rx.recv().await {
            match mesaj {
                GidenMesaj::OgeOku { id, .. } => {
                    sira.push((id.clone(), "oku"));
                    h.mesaji_isle(GelenMesaj::Ogeler {
                        id,
                        ogeler: vec![oge("Hızlı", secili == 0), oge("Yüksek", secili == 1)],
                    })
                    .await;
                }
                GidenMesaj::OgeBas { id, indeks, .. } => {
                    sira.push((id.clone(), "bas"));
                    secili = indeks;
                    h.mesaji_isle(GelenMesaj::Ogeler {
                        id,
                        ogeler: vec![oge("Hızlı", secili == 0), oge("Yüksek", secili == 1)],
                    })
                    .await;
                }
                GidenMesaj::Sor { id, .. } => {
                    sira.push((id.clone(), "sor"));
                    // Yayın sürüyor: kilit olmasa diğer istek TAM bu boşlukta
                    // seçim turlarına girerdi.
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    h.mesaji_isle(GelenMesaj::Parca {
                        id: id.clone(),
                        delta: "ok".into(),
                        done: false,
                    })
                    .await;
                    sira.push((id.clone(), "bitti"));
                    h.mesaji_isle(GelenMesaj::Parca {
                        id,
                        delta: String::new(),
                        done: true,
                    })
                    .await;
                    biten += 1;
                    if biten == 2 {
                        break;
                    }
                }
                _ => {}
            }
        }
        sira
    });

    let menu = deneme_menusu();
    let istek = |k: &'static str| {
        let hub = hub.clone();
        let menu = menu.clone();
        tokio::spawn(async move {
            hub.sor_secimli(
                "gemini",
                k,
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(menu),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(5),
            )
            .await
        })
    };
    let (a, b) = (istek("bir"), istek("iki"));
    assert_eq!(a.await.unwrap().unwrap(), "ok");
    assert_eq!(b.await.unwrap().unwrap(), "ok");

    let sira = besleyici.await.unwrap();
    // Her istek "oku" ile başlar; bitişiklik ölçütü: bir isteğin grubu
    // "bitti" ile kapanmadan yeni bir "oku" gelmemeli.
    let mut gruplar: Vec<Vec<&str>> = Vec::new();
    for (_, tur) in &sira {
        if *tur == "oku" {
            gruplar.push(Vec::new());
        }
        gruplar.last_mut().expect("ilk mesaj oku olmalı").push(*tur);
    }
    assert_eq!(gruplar.len(), 2, "iki istek iki grup açmalı: {sira:?}");
    for (i, grup) in gruplar.iter().enumerate() {
        assert_eq!(grup.first(), Some(&"oku"), "{i}. grup: {sira:?}");
        assert_eq!(
            grup.last(),
            Some(&"bitti"),
            "{i}. grup kapanmadan diğer istek başladı (kilit tutmuyor): {sira:?}"
        );
        assert_eq!(grup.iter().filter(|t| **t == "sor").count(), 1);
    }
}

/// Tazelik süzgeci: kayıt `SEKME_TAZELIK_ESIGI`'nı aşınca "canlı" sayılmaz.
///
/// NEDEN: eklenti eşleme değişmedikçe `kayit` yeniden göndermiyordu ve
/// sunucu ölü sekmeyi canlı sanıp isteği oraya yolluyordu (2026-11-14
/// ölçümü). Damga geçmişe çekilerek bayat kayıt taklit edilir; süzgeç
/// çalışmazsa `sekme_var_mi` true döner ve test kırmızı olur.
#[tokio::test]
async fn bayat_kayit_canli_sayilmaz() {
    let hub = EklentiHub::new();
    let (tx, _rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "varsayilan".into(), tx)
        .await;
    assert!(
        hub.sekme_var_mi("gemini").await,
        "taze kayıt canlı sayılmalı"
    );

    {
        let mut map = hub.sekmeler.lock().await;
        let kayit = map.get_mut("gemini:varsayilan").expect("kayıt var");
        // Damgayı eşiğin bir saniye ötesine çek: bayat.
        kayit.son_gorulme = Instant::now() - SEKME_TAZELIK_ESIGI - Duration::from_secs(1);
    }

    assert!(
        !hub.sekme_var_mi("gemini").await,
        "eşiği aşan kayıt canlı sayılmamalı"
    );
    // Süzgeç yalnız yanıtı değiştirmiyor, kaydı haritadan da düşürüyor.
    assert!(hub.bagli_hesaplar().await.is_empty(), "bayat kayıt düşmeli");
}

/// Tazeleme kaydı canlı tutar: yeni `kayit` damgayı ileri çeker.
#[tokio::test]
async fn tazeleme_kaydi_canli_tutar() {
    let hub = EklentiHub::new();
    let (tx, _rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "varsayilan".into(), tx.clone())
        .await;
    {
        let mut map = hub.sekmeler.lock().await;
        let kayit = map.get_mut("gemini:varsayilan").expect("kayıt var");
        kayit.son_gorulme = Instant::now() - SEKME_TAZELIK_ESIGI - Duration::from_secs(1);
    }
    // Eklentinin 10 sn'lik taraması gibi yeniden kaydet.
    hub.sekme_kaydet("gemini".into(), "varsayilan".into(), tx)
        .await;
    assert!(
        hub.sekme_var_mi("gemini").await,
        "tazelenen kayıt canlı olmalı"
    );
}

/// Bayat kayıt düşerken `kullanim` sayacı da silinir.
///
/// NEDEN: sayaç `sekmeler`ten AYRI bir haritada; yalnız `sekmeler` budanırsa
/// ölü profilin sayacı bellekte kalır ve aynı profil yeniden bağlandığında
/// yük dengelemesi eski değerle başlar (denetim, 2026-11-15).
#[tokio::test]
async fn bayat_kayit_kullanim_sayacini_da_temizler() {
    let hub = EklentiHub::new();
    let (tx_a, _rx_a) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    let (tx_b, _rx_b) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "a".into(), tx_a).await;
    hub.sekme_kaydet("gemini".into(), "b".into(), tx_b).await;
    // İki hesabın da sayacı dolu olsun.
    {
        let mut kullanim = hub.kullanim.lock().await;
        kullanim.insert("gemini:a".into(), 7);
        kullanim.insert("gemini:b".into(), 3);
    }
    // Yalnız "a" bayatlasın.
    {
        let mut map = hub.sekmeler.lock().await;
        let kayit = map.get_mut("gemini:a").expect("kayıt var");
        kayit.son_gorulme = Instant::now() - SEKME_TAZELIK_ESIGI - Duration::from_secs(1);
    }

    // Herhangi bir okuma yolu budamayı tetikler.
    assert!(hub.sekme_var_mi("gemini").await, "'b' hâlâ canlı");

    let kullanim = hub.kullanim.lock().await;
    assert!(
        !kullanim.contains_key("gemini:a"),
        "bayat kaydın sayacı silinmeli: {kullanim:?}"
    );
    assert_eq!(
        kullanim.get("gemini:b").copied(),
        Some(3),
        "canlı kaydın sayacı korunmalı"
    );
}

/// Bayatlayıp düşen hesap yeniden bağlanınca sayaç sıfırdan başlar.
///
/// NEDEN: eski sayaç devralınırsa `hesap_sec` en-az-kullanılan seçiminde
/// yeni bağlanan hesabı haksız biçimde geri plana atar.
#[tokio::test]
async fn yeniden_baglanan_hesap_sayaci_sifirdan_baslar() {
    let hub = EklentiHub::new();
    let (tx_a, _rx_a) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "a".into(), tx_a.clone())
        .await;
    {
        let mut kullanim = hub.kullanim.lock().await;
        kullanim.insert("gemini:a".into(), 9);
    }
    {
        let mut map = hub.sekmeler.lock().await;
        let kayit = map.get_mut("gemini:a").expect("kayıt var");
        kayit.son_gorulme = Instant::now() - SEKME_TAZELIK_ESIGI - Duration::from_secs(1);
    }
    assert!(!hub.sekme_var_mi("gemini").await, "bayat kayıt düşmeli");

    hub.sekme_kaydet("gemini".into(), "a".into(), tx_a).await;
    let _ = hub.hesap_sec("gemini", Some("a")).await.expect("seçim");
    let kullanim = hub.kullanim.lock().await;
    assert_eq!(
        kullanim.get("gemini:a").copied(),
        Some(1),
        "yeni bağlanan hesap 1'den başlamalı, 10'dan değil: {kullanim:?}"
    );
}

/// Aynı sağlayıcının BAŞKA profili canlıyken ayrılan profilin sayacı silinir.
///
/// NEDEN: sayaç silme kararı anahtar (`provider:profile`) düzeyinde
/// verilmeliydi; koruma testi sağlayıcı düzeyinde kalırsa `gemini:b` canlı
/// olduğu sürece `gemini:a` sayacı silinmez, kayıt da haritadan çıktığı için
/// `bayatlari_buda` bir daha onu üretemez — kalıcı sızıntı (denetim, 2026-11-15).
#[tokio::test]
async fn ayrilan_profilin_sayaci_kardes_profil_canliyken_de_silinir() {
    let hub = EklentiHub::new();
    let (tx_a, _rx_a) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    let (tx_b, _rx_b) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "a".into(), tx_a.clone())
        .await;
    hub.sekme_kaydet("gemini".into(), "b".into(), tx_b).await;
    {
        let mut kullanim = hub.kullanim.lock().await;
        kullanim.insert("gemini:a".into(), 7);
        kullanim.insert("gemini:b".into(), 3);
    }

    hub.sekme_ayril_eger("gemini", "a", &tx_a).await;

    assert!(hub.sekme_var_mi("gemini").await, "'b' hâlâ canlı olmalı");
    let kullanim = hub.kullanim.lock().await;
    assert!(
        !kullanim.contains_key("gemini:a"),
        "ayrılan profilin sayacı silinmeli: {kullanim:?}"
    );
    assert_eq!(
        kullanim.get("gemini:b").copied(),
        Some(3),
        "kardeş profilin sayacı korunmalı"
    );
}

/// Başka bir soketten gelen `ayril`, bu soketin kaydını düşürmez.
///
/// NEDEN: aynı hesabın iki sekmesi açıkken `tabs.onRemoved` bir sekme için
/// `ayril` gönderir; kimlik kapısı olmadan canlı hesap sunucudan düşüyordu
/// (denetim, 2026-11-15). Aynı göndericiden gelen `ayril` ise düşürmeli.
#[tokio::test]
async fn baska_soketten_gelen_ayril_canli_kaydi_dusurmez() {
    let hub = EklentiHub::new();
    let (tx_canli, _rx_canli) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    let (tx_yabanci, _rx_yabanci) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "a".into(), tx_canli.clone())
        .await;

    hub.mesaji_isle_gonderici(
        GelenMesaj::Ayril {
            provider: "gemini".into(),
            profile: Some("a".into()),
        },
        &tx_yabanci,
    )
    .await;
    assert!(
        hub.sekme_var_mi("gemini").await,
        "yabancı soketin ayrılması canlı kaydı düşürmemeli"
    );

    hub.mesaji_isle_gonderici(
        GelenMesaj::Ayril {
            provider: "gemini".into(),
            profile: Some("a".into()),
        },
        &tx_canli,
    )
    .await;
    assert!(
        !hub.sekme_var_mi("gemini").await,
        "kaydı tutan soketin ayrılması düşürmeli"
    );
}

/// Kimliksiz `mesaji_isle` yolundan gelen `ayril` hiçbir kaydı düşürmez.
///
/// NEDEN: kimi düşüreceğimiz bilinmiyorsa silmek yerine yok saymak
/// güvenli taraftır; sessiz silme canlı hesabı kaybettirir.
#[tokio::test]
async fn kimliksiz_ayril_kaydi_dusurmez() {
    let hub = EklentiHub::new();
    let (tx, _rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
    hub.sekme_kaydet("gemini".into(), "a".into(), tx).await;

    hub.mesaji_isle(GelenMesaj::Ayril {
        provider: "gemini".into(),
        profile: Some("a".into()),
    })
    .await;

    assert!(
        hub.sekme_var_mi("gemini").await,
        "kimliksiz ayrılma kaydı düşürmemeli"
    );
}
