//! Hesap bazlı ÖLÇÜM defteri.
//!
//! NEDEN VAR: `kervan.toml`daki model listesi sağlayıcı GENELİDİR ve tek bir
//! ölçümden gelir. Oysa siteler model menüsünü hesaba göre değiştiriyor:
//! aynı Gemini menüsünde "3.1 Pro" bir hesapta seçilebiliyor, başka hesapta
//! tıklanınca seçilmiyor (ücretli plan). Bu fark `kervan.toml`a YAZILMAZ;
//! sayfadan ÖLÇÜLÜR ve burada saklanır.
//!
//! Kural: bu dosyadaki bir kayıt yalnız `/kervan/olc` ölçümünden gelir. Elle
//! yazılmış katalog burada barınamaz — kaynağı ölçüm olmayan veri uydurmadır.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Tek bir hesabın ölçülmüş model menüsü.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HesapOlcumu {
    pub saglayici: String,
    pub hesap: String,
    /// Ölçüm anı (unix saniye). Eski ölçüm olduğu gizlenmez.
    pub olculdu: u64,
    #[serde(default)]
    pub modeller: Vec<OlcuModel>,
    /// Araç menüsündeki özelliklerin ölçülmüş hâli. Eski defterlerde bu alan
    /// yoktur; `default` ile okunur, boşsa yazılmaz — "ölçülmedi" ile
    /// "ölçüldü ve boş" ayrımı böylece dosyada görünür kalır.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ozellikler: Vec<OlcuOzellik>,
}

/// Araç menüsünde görülen tek özelliğin ölçülmüş hâli.
///
/// Yalnız ÖLÇÜLEN yazılır: öğenin adı, tam metni ve işaret durumu. "Kilitli"
/// hükmü buraya yazılmaz çünkü kilit göstergesi menü metninden güvenilir
/// biçimde okunamıyor; kilit iddiası ancak tıklama denemesiyle (model
/// ölçümündeki `secilebilir` gibi) kanıtlanır.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OlcuOzellik {
    pub kod: String,
    pub etiket: String,
    /// Ölçüm anında işaretli miydi (açık/kapalı).
    pub isaretli: bool,
    /// Öğenin tam metni (ad + açıklama); eşleştirmede son çare.
    #[serde(default)]
    pub tam: String,
}

/// Menüde görülen tek öğenin ölçülmüş hali.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OlcuModel {
    /// Kervan kimlik kodu (site etiketinden üretilir, ölçümde sabitlenir).
    pub kod: String,
    /// Sitedeki görünen ad.
    pub etiket: String,
    /// Bu hesapta GERÇEKTEN seçilebiliyor mu? "Menüde var" yetmez: tıklanıp
    /// yeniden okunmadan bu alan yazılmaz.
    pub secilebilir: bool,
}

fn dosya_yolu() -> PathBuf {
    // Test ve tanı: defter yolunu ortam değişkeniyle ayırmak, gerçek kullanıcı
    // defterine dokunmadan oku/yaz sınaması yapmayı sağlar. Üretimde bu değişken
    // yoktur; varsayılan yol kullanılır.
    if let Some(p) = std::env::var_os("KERVAN_OLCUM_DOSYA")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(p).join("kervan").join("olcum.json");
    }
    PathBuf::from(".").join("kervan").join("olcum.json")
}

/// Defterin tam yolu — ölçüm yanıtında gösterilir ki "yazdım" iddiası
/// denetlenebilir olsun.
pub fn dosya_yolu_goster() -> String {
    dosya_yolu().display().to_string()
}

/// Site etiketinden kararlı kimlik kodu üretir: "3.1 Pro" → "3-1-pro".
///
/// Türkçe harfler ASCII'ye katlanır (ı→i, ş→s, ğ→g, ü→u, ö→o, ç→c); aksi hâlde
/// aynı model iki farklı kodla listelenir. Kod istemciyle yapılan sözleşmedir,
/// etiket ise siteye aittir ve değişebilir.
pub fn slug(etiket: &str) -> String {
    let mut s = String::with_capacity(etiket.len());
    for ch in etiket.chars() {
        let k = match ch {
            'ı' | 'İ' => 'i',
            'ş' | 'Ş' => 's',
            'ğ' | 'Ğ' => 'g',
            'ü' | 'Ü' => 'u',
            'ö' | 'Ö' => 'o',
            'ç' | 'Ç' => 'c',
            'â' | 'Â' => 'a',
            'î' | 'Î' => 'i',
            'û' | 'Û' => 'u',
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
            _ => '-',
        };
        s.push(k);
    }
    s.trim_matches('-').replace("--", "-")
}

/// Defteri diskten okur. Dosya yoksa boş defter (ölçüm hiç yapılmamış).
pub fn yukle() -> Vec<HesapOlcumu> {
    let Ok(metin) = std::fs::read_to_string(dosya_yolu()) else {
        return Vec::new();
    };
    serde_json::from_str(&metin).unwrap_or_else(|e| {
        eprintln!("olcum        : uyari: defter okunamadi ({e}); bos sayildi");
        Vec::new()
    })
}

/// Defteri diske yazar. Aynı (sağlayıcı, hesap) için eski kayıt EZİLİR:
/// defter "son ölçüm"dür, ölçüm geçmişi değil.
///
/// İSTİSNA (alan bazlı): model ölçümü ile özellik ölçümü AYRI turlarda
/// yapılabiliyor. Yeni kayıt bir alanda boşsa eski alan korunur; aksi hâlde
/// yalnız özellik ölçen bir tur, o hesabın model listesini sessizce silerdi.
/// Boş alan "ölçüldü ve hiç yok" demek değildir — model ölçümü hiçbir öğe
/// deneyemediğinde zaten kayıt üretmiyor (`tumunu_dene`).
pub fn kaydet(yeni: &[HesapOlcumu]) -> std::io::Result<()> {
    let yol = dosya_yolu();
    if let Some(dizin) = yol.parent() {
        std::fs::create_dir_all(dizin)?;
    }
    let mut harita: BTreeMap<(String, String), HesapOlcumu> = yukle()
        .into_iter()
        .map(|o| ((o.saglayici.clone(), o.hesap.clone()), o))
        .collect();
    for o in yeni {
        let anahtar = (o.saglayici.clone(), o.hesap.clone());
        let kayit = birlestir(harita.get(&anahtar), o);
        harita.insert(anahtar, kayit);
    }
    let liste: Vec<HesapOlcumu> = harita.into_values().collect();
    let metin = serde_json::to_string_pretty(&liste)?;
    std::fs::write(yol, metin)
}

/// Eski kayıtla yeni ölçümü alan bazında birleştirir (bkz. `kaydet`).
///
/// Ayrı fonksiyon: disk ve ortam değişkenine dokunmadan sınanabilsin.
fn birlestir(eski: Option<&HesapOlcumu>, yeni: &HesapOlcumu) -> HesapOlcumu {
    let Some(eski) = eski else {
        return yeni.clone();
    };
    if !yeni.modeller.is_empty() && !yeni.ozellikler.is_empty() {
        return yeni.clone();
    }
    HesapOlcumu {
        saglayici: yeni.saglayici.clone(),
        hesap: yeni.hesap.clone(),
        olculdu: if yeni.modeller.is_empty() && yeni.ozellikler.is_empty() {
            eski.olculdu
        } else {
            yeni.olculdu
        },
        modeller: if yeni.modeller.is_empty() {
            eski.modeller.clone()
        } else {
            yeni.modeller.clone()
        },
        ozellikler: if yeni.ozellikler.is_empty() {
            eski.ozellikler.clone()
        } else {
            yeni.ozellikler.clone()
        },
    }
}

/// Belirli hesabın ölçülmüş menüsü.
pub fn hesap(defter: &[HesapOlcumu], saglayici: &str, hesap: &str) -> Option<HesapOlcumu> {
    defter
        .iter()
        .find(|o| o.saglayici == saglayici && o.hesap == hesap)
        .cloned()
}

/// Menüdeki DÜŞÜNME ekseni: model DEĞİL, aynı menüdeki ikinci eksen (radyo).
///
/// 2026-10-25 ölçümü: Gemini model menüsünde "Düşünme düzeyi / Düşük / Orta /
/// Yüksek" öğeleri model listesiyle AYNI menüde çıkıyor. Bunları model saymak,
/// "Orta"yı bir model kodu gibi sunmak demekti — panelin en büyük yanlışı buydu.
/// Geri kurma sırasında da bunlara dokunulmaz.
const DUSUNME: [&str; 4] = ["Düşünme düzeyi", "Düşük", "Orta", "Yüksek"];

pub fn dusunme_mi(ad: &str) -> bool {
    DUSUNME
        .iter()
        .any(|d| d.eq_ignore_ascii_case(ad) || slug(d) == slug(ad))
}

/// Ölçülmüş menüyü seçim katmanının anladığı `ModelMenusu`ya çevirir.
///
/// NEDEN: kilit kararı `coz_istek` içinde TEK yerde verilsin. Ölçüm varsa
/// otorite ölçümdür; `kervan.toml`daki sağlayıcı geneli liste yalnız o hesap
/// için ölçüm YOKKEN kullanılır.
pub fn menu_olarak(
    defter: &[HesapOlcumu],
    saglayici: &str,
    profil: &str,
    dugme: String,
    oge: String,
) -> Option<kervan_secim::ModelMenusu> {
    let h = hesap(defter, saglayici, profil)?;
    Some(kervan_secim::ModelMenusu {
        dugme,
        oge,
        modeller: h
            .modeller
            .iter()
            .map(|o| kervan_secim::ModelSecenegi {
                kod: o.kod.clone(),
                etiket: o.etiket.clone(),
                kilitli: !o.secilebilir,
                kilit_nedeni: (!o.secilebilir)
                    .then(|| "bu hesapta seçilemedi (ölçüldü)".to_string()),
            })
            .collect(),
    })
}

/// `/kervan/secenekler` yanıtındaki sağlayıcı kaydını ölçüm defteriyle zenginleştirir.
///
/// `kayit` yapılandırmadan gelen varsayım (model kodları + anahtarlar); `olculen`
/// o sağlayıcının ölçülmüş hesapları. Ölçüm yoksa kayıt OLDUĞU GİBİ döner —
/// hiç ölçüm yapılmamış bir sağlayıcıya boş `olcum` alanı eklemek "ölçüldü ve
/// hiçbir şey yok" izlenimi verirdi. Ölçüm VARSA yapılandırma boş olsa bile
/// kayıt üretilir: panelin görmesi gereken şey ölçümdür, `kervan.toml` değil.
pub fn panel_kaydi(
    kayit: Option<serde_json::Value>,
    olculen: &[HesapOlcumu],
) -> Option<serde_json::Value> {
    if olculen.is_empty() {
        return kayit;
    }
    let mut kayit =
        kayit.unwrap_or_else(|| serde_json::json!({ "modeller": [], "anahtarlar": [] }));
    let mut hesaplar = serde_json::Map::new();
    for o in olculen {
        let modeller: Vec<serde_json::Value> = o
            .modeller
            .iter()
            .map(|m| {
                serde_json::json!({
                    "kod": m.kod,
                    "etiket": m.etiket,
                    "kilitli": !m.secilebilir,
                    "kilit_nedeni": (!m.secilebilir)
                        .then_some("bu hesapta seçilemedi (ölçüldü)"),
                })
            })
            .collect();
        let ozellikler: Vec<serde_json::Value> = o
            .ozellikler
            .iter()
            .map(|z| {
                serde_json::json!({
                    "kod": z.kod,
                    "etiket": z.etiket,
                    "isaretli": z.isaretli,
                })
            })
            .collect();
        hesaplar.insert(
            o.hesap.clone(),
            serde_json::json!({
                "olculdu": o.olculdu,
                "modeller": modeller,
                "ozellikler": ozellikler,
            }),
        );
    }
    kayit["kaynak"] = serde_json::json!("yapilandirma");
    kayit["olcum"] = serde_json::json!({
        "dosya": dosya_yolu_goster(),
        "hesaplar": hesaplar,
    });
    Some(kayit)
}

/// Model kimliğine gömülü hesabı çözer: `kervan/gemini/3-1-pro@eposta`.
///
/// Dönüş: (hesapsız kimlik, profil). Profil yoksa `None`.
///
/// NEDEN KİMLİKTE: OpenCode/OpenChamber istekte YALNIZ `model` alanını
/// gönderiyor; hesap seçimi başka hiçbir alanla taşınamaz. Hesap kimliğe
/// gömülmezse "hangi hesap" sorusu sessizce rastgele cevaplanır.
pub fn hesap_eki(model: &str) -> (String, Option<String>) {
    let govde = model.trim();
    match govde.split_once('@') {
        Some((sol, sag)) if !sag.trim().is_empty() && !sol.trim().is_empty() => {
            (sol.trim().to_string(), Some(sag.trim().to_string()))
        }
        _ => (govde.to_string(), None),
    }
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn slug_turkce_harfleri_katlar() {
        assert_eq!(slug("3.1 Pro"), "3-1-pro");
        assert_eq!(slug("Yüksek"), "yuksek");
        assert_eq!(slug("3.6 Düşünen"), "3-6-dusunen");
        assert_eq!(slug("4 Argon"), "4-argon");
        assert_eq!(slug("3.5 Flash-Lite"), "3-5-flash-lite");
    }

    #[test]
    fn slug_bos_ve_tekrar_eden_ayirici() {
        assert_eq!(slug("  "), "");
        assert_eq!(slug("a  b"), "a-b");
    }

    /// Eski defter (yalnız `modeller`) yeni şemayla okunmalı: alan eklemek
    /// var olan ölçümü geçersiz kılmamalı.
    #[test]
    fn eski_defter_ozellik_alani_olmadan_okunur() {
        let metin = r#"[
            {"saglayici":"gemini","hesap":"a@b","olculdu":1760000000,
             "modeller":[{"kod":"3-1-pro","etiket":"3.1 Pro","secilebilir":true}]}
        ]"#;
        let defter: Vec<HesapOlcumu> = serde_json::from_str(metin).expect("eski defter okunmalı");
        assert_eq!(defter.len(), 1);
        assert_eq!(defter[0].modeller.len(), 1);
        assert!(defter[0].ozellikler.is_empty());
    }

    /// Özellik ölçümü gidiş-dönüş sağlam olmalı ve boş liste dosyaya
    /// YAZILMAMALI: "ölçülmedi" ile "ölçüldü, hiç özellik yok" ayrımı korunur.
    #[test]
    fn ozellikler_gidis_donus_ve_bos_yazilmaz() {
        let k = HesapOlcumu {
            saglayici: "gemini".into(),
            hesap: "a@b".into(),
            olculdu: 1760000000,
            modeller: vec![OlcuModel {
                kod: "otomatik".into(),
                etiket: "Otomatik".into(),
                secilebilir: true,
            }],
            ozellikler: vec![OlcuOzellik {
                kod: "derin-arastirma".into(),
                etiket: "Derin Araştırma".into(),
                isaretli: false,
                tam: "Derin Araştırma Ayrıntılı yanıtlar".into(),
            }],
        };
        let metin = serde_json::to_string(&k).expect("yazılmalı");
        let geri: HesapOlcumu = serde_json::from_str(&metin).expect("okunmalı");
        assert_eq!(geri, k);

        let bos = HesapOlcumu {
            ozellikler: Vec::new(),
            ..k
        };
        let metin = serde_json::to_string(&bos).expect("yazılmalı");
        assert!(
            !metin.contains("ozellikler"),
            "boş özellik listesi dosyaya yazılmamalı: {metin}"
        );
    }

    fn olcu(modeller: &[&str], ozellikler: &[&str]) -> HesapOlcumu {
        HesapOlcumu {
            saglayici: "gemini".into(),
            hesap: "a@b".into(),
            olculdu: 1760000000,
            modeller: modeller
                .iter()
                .map(|k| OlcuModel {
                    kod: (*k).into(),
                    etiket: (*k).into(),
                    secilebilir: true,
                })
                .collect(),
            ozellikler: ozellikler
                .iter()
                .map(|k| OlcuOzellik {
                    kod: (*k).into(),
                    etiket: (*k).into(),
                    isaretli: false,
                    tam: String::new(),
                })
                .collect(),
        }
    }

    /// Yalnız özellik ölçen tur, model listesini SİLMEMELİ; tersi de geçerli.
    #[test]
    fn birlestirme_olculmemis_alani_korur() {
        let eski = olcu(&["otomatik", "3-1-pro"], &[]);
        let yeni = HesapOlcumu {
            olculdu: 1760000999,
            ..olcu(&[], &["derin-arastirma"])
        };
        let b = birlestir(Some(&eski), &yeni);
        assert_eq!(b.modeller, eski.modeller);
        assert_eq!(b.ozellikler, yeni.ozellikler);
        assert_eq!(b.olculdu, 1760000999, "yeni ölçüm anı yazılmalı");

        let b = birlestir(Some(&eski), &olcu(&["4-argon"], &[]));
        assert_eq!(b.modeller.len(), 1);
        assert_eq!(b.modeller[0].kod, "4-argon");
        assert!(b.ozellikler.is_empty());
    }

    /// İki alan da doluysa yeni kayıt tümüyle geçerlidir (ezme kuralı).
    #[test]
    fn birlestirme_iki_alan_doluysa_yeniyi_secer() {
        let eski = olcu(&["otomatik"], &["eski-ozellik"]);
        let yeni = olcu(&["4-argon"], &["derin-arastirma"]);
        let b = birlestir(Some(&eski), &yeni);
        assert_eq!(b, yeni);
    }

    #[test]
    fn birlestirme_eski_kayit_yoksa_yeniyi_aynen_alir() {
        let yeni = olcu(&["otomatik"], &[]);
        assert_eq!(birlestir(None, &yeni), yeni);
    }

    /// Ölçüm varsa panel kaydı hem varsayımı hem ölçümü taşır: "kaynak" alanı
    /// yapılandırma listesinin VARSAYIM olduğunu açıkça söyler, hesap kataloğu
    /// ayrı verilir. İkisini tek listeye karıştırmak yanlış model riskidir.
    #[test]
    fn panel_kaydi_olcumu_ayri_tasir() {
        let varsayim = serde_json::json!({
            "modeller": [{"kod": "4-argon", "etiket": "4 Argon", "kilitli": true}],
            "anahtarlar": [],
        });
        let mut o = olcu(&["otomatik"], &["derin-arastirma"]);
        o.modeller[0].secilebilir = true;
        let v = panel_kaydi(Some(varsayim), &[o]).expect("kayit donmeli");

        assert_eq!(v["kaynak"], "yapilandirma");
        assert_eq!(
            v["modeller"].as_array().map(|a| a.len()),
            Some(1),
            "varsayim listesi korunmali: {v}"
        );
        let hesap = &v["olcum"]["hesaplar"]["a@b"];
        assert_eq!(hesap["modeller"][0]["kod"], "otomatik");
        assert_eq!(hesap["modeller"][0]["kilitli"], false);
        assert_eq!(hesap["ozellikler"][0]["kod"], "derin-arastirma");
        assert_eq!(hesap["ozellikler"][0]["isaretli"], false);
        assert_eq!(hesap["olculdu"], 1760000000);
    }

    /// Hiç ölçüm yoksa `olcum` alanı EKLENMEZ: boş alan "ölçüldü, bir şey yok"
    /// diye okunurdu.
    #[test]
    fn panel_kaydi_olcum_yoksa_alan_eklemez() {
        let varsayim = serde_json::json!({ "modeller": [], "anahtarlar": [] });
        let v = panel_kaydi(Some(varsayim), &[]).expect("kayit donmeli");
        assert!(v.get("olcum").is_none(), "olcum alani olmamali: {v}");
        assert!(v.get("kaynak").is_none(), "kaynak alani olmamali: {v}");
    }

    /// Ölçüm VARSA yapılandırma boş olsa bile kayıt üretilir; yoksa `None`
    /// (panelde konfigürasyonu da ölçümü de olmayan sağlayıcı görünmemeli).
    #[test]
    fn panel_kaydi_yapilandirma_yoksa_olcumu_yine_verir() {
        let v =
            panel_kaydi(None, &[olcu(&["otomatik"], &[])]).expect("ölçüm varsa kayıt üretilmeli");
        assert_eq!(
            v["olcum"]["hesaplar"]["a@b"]["modeller"][0]["kod"],
            "otomatik"
        );
        assert_eq!(
            v["modeller"].as_array().map(|a| a.len()),
            Some(0),
            "yapılandırma yoksa boş varsayım listesi: {v}"
        );
        assert!(
            panel_kaydi(None, &[]).is_none(),
            "ne ölçüm ne ayar: kayıt yok"
        );
    }

    /// Kilit kararı ÖLÇÜMDEN gelir: `secilebilir = false` → `kilitli: true`.
    #[test]
    fn panel_kaydi_kilit_olcumden_gelir() {
        let mut o = olcu(&["3-1-pro"], &[]);
        o.modeller[0].secilebilir = false;
        let v = panel_kaydi(
            Some(serde_json::json!({ "modeller": [], "anahtarlar": [] })),
            &[o],
        )
        .expect("kayit donmeli");
        assert_eq!(
            v["olcum"]["hesaplar"]["a@b"]["modeller"][0]["kilitli"],
            true
        );
    }
}
