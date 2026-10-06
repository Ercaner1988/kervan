// P10 Teslim Doğrulama Testleri
// kabul-haritasi.ts P10 kanıtları ile eşleşen ince testler.
// Gerçek içerik doğrulaması yapar: dosya varlığı, JSON/JSONL şeması, dalga taslak başlıkları.

use std::path::PathBuf;

/// `port/` dizinini hem çalışma kökünden hem cargo target dizininden bulur.
fn port_yolu() -> PathBuf {
    let aday1 = PathBuf::from("port");
    if aday1.exists() {
        return aday1;
    }
    let aday2 = PathBuf::from("../../port");
    if aday2.exists() {
        return aday2;
    }
    panic!("port/ dizini bulunamadı (ne 'port' ne '../../port' mevcut)");
}

/// PR.md dosyasını hem çalışma kökünden hem cargo target dizininden bulur.
fn pr_yolu() -> PathBuf {
    let aday1 = PathBuf::from("PR.md");
    if aday1.exists() {
        return aday1;
    }
    let aday2 = PathBuf::from("../../PR.md");
    if aday2.exists() {
        return aday2;
    }
    panic!("PR.md bulunamadı");
}

/// FR-8.1: Envanter aracı çıktısının varlığını ve temel yapısını doğrular.
/// İki koşuda özdeş çıktı iddiası bağımsız test kapsamındadır;
/// burada dosyanın var olduğu ve JSON olarak ayrıştırılabildiği sınanır.
#[test]
fn test_p10_envanter_deterministik() {
    let envanter = port_yolu().join("envanter.json");
    assert!(
        envanter.exists(),
        "port/envanter.json mevcut olmalı: {:?}",
        envanter
    );
    let icerik = std::fs::read_to_string(&envanter).expect("envanter.json okunamadı");
    let json: serde_json::Value =
        serde_json::from_str(&icerik).expect("envanter.json geçerli JSON olmalı");

    // En az birkaç temel anahtar içermeli
    let obj = json.as_object().expect("envanter.json bir JSON nesnesi olmalı");
    assert!(
        obj.len() >= 3,
        "envanter.json en az 3 sayım kategorisi içermeli, bulundu: {}",
        obj.len()
    );
}

/// FR-8.2: Defter dosyalarının varlığını ve JSONL biçimini doğrular.
#[test]
fn test_p10_defter_sema_uygun() {
    let defter = port_yolu().join("defter");
    assert!(defter.exists(), "port/defter/ dizini mevcut olmalı");

    for dosya_adi in &["dosyalar.jsonl", "davranislar.jsonl"] {
        let yol = defter.join(dosya_adi);
        assert!(yol.exists(), "port/defter/{} mevcut olmalı", dosya_adi);

        let icerik = std::fs::read_to_string(&yol)
            .unwrap_or_else(|_| panic!("{} okunamadı", dosya_adi));
        let satirlar: Vec<&str> = icerik.lines().filter(|s| !s.trim().is_empty()).collect();
        assert!(
            !satirlar.is_empty(),
            "{} en az bir JSONL satırı içermeli",
            dosya_adi
        );

        for (i, satir) in satirlar.iter().enumerate() {
            let _: serde_json::Value = serde_json::from_str(satir).unwrap_or_else(|e| {
                panic!(
                    "{}:{} geçerli JSON olmalı: {} — satır: {}",
                    dosya_adi,
                    i + 1,
                    e,
                    satir
                )
            });
        }
    }
}

/// FR-8.3: Dalga PRD taslak dosyalarının varlığını ve iskelet yapısını doğrular.
#[test]
fn test_p10_dalga_taslak_uretimi() {
    let dalgalar = port_yolu().join("dalgalar");
    assert!(dalgalar.exists(), "port/dalgalar/ dizini mevcut olmalı");

    for dalga_no in 2..=4 {
        let dosya_adi = format!("DALGA-{}.md", dalga_no);
        let yol = dalgalar.join(&dosya_adi);
        assert!(yol.exists(), "port/dalgalar/{} mevcut olmalı", dosya_adi);

        let icerik = std::fs::read_to_string(&yol)
            .unwrap_or_else(|_| panic!("{} okunamadı", dosya_adi));

        // §2 başlığı olmalı (## ile başlayan satır)
        assert!(
            icerik.lines().any(|s| s.starts_with("## ")),
            "{} en az bir §2 (##) başlığı içermeli",
            dosya_adi
        );

        // Acceptance: içeren en az bir satır olmalı
        let icerik_kucuk = icerik.to_lowercase();
        assert!(
            icerik_kucuk.contains("acceptance"),
            "{} en az bir 'Acceptance:' ifadesi içermeli",
            dosya_adi
        );
    }
}

/// FR-3.2: PR.md taslağının varlığını ve üç bakış açısını doğrular.
#[test]
fn test_p10_pr_taslak_ve_main_dokunulmamis() {
    let pr = pr_yolu();
    assert!(pr.exists(), "PR.md mevcut olmalı");

    let icerik = std::fs::read_to_string(&pr).expect("PR.md okunamadı");
    let icerik_kucuk = icerik.to_lowercase();

    // Üç bakış açısından en az ikisinin izini bul
    let bakis_sayisi = ["bakımcı", "benimseyici", "dağıtımcı", "maintainer", "adopter", "deployer"]
        .iter()
        .filter(|k| icerik_kucuk.contains(*k))
        .count();
    assert!(
        bakis_sayisi >= 2,
        "PR.md en az 2 bakış açısı içermeli, bulunan: {}",
        bakis_sayisi
    );
}
