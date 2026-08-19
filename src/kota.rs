//! Kota ogrenici: kullanim kaydi ve **gozlemden** limit cikarimi.
//!
//! NEDEN VAR: web saglayicilari kalan kotayi soylemiyor. "Tam kullanim hakkim ne?"
//! sorusu ancak olculerek cevaplanir. Kervan her istegin sonucunu kaydeder, limite
//! carpma anlarini isaretler ve iki sayiyi gozlemden cikarir: pencere basina kac
//! istek, pencere kac saat.
//!
//! NEDEN SQLite DEGIL: kayit tek bicimli bir olay akisi, toplama isi duz sayim,
//! hacim ayda hesap basina ~20 bin satir. Ekleme-yalnizca JSONL bunu karsiliyor,
//! yeni bir C bagimliligi ve veritabani bozulma riski getirmiyor. Yarim yazilmis
//! son satir atlanir, dosya kendini onarir. Sorgular karmasiklasirsa SQLite'a
//! gecmek acik yol.
//!
//! 2026-08-19 — YUKARIDAKI KARAR OLCULDU VE KORUNDU (Turso gocu degerlendirmesi):
//!
//! | defter satiri | `ac()` | dosya |
//! |---|---|---|
//! | 88 (bugunku gercek) | 18,7 ms | 6 KB |
//! | 20.000 (modulun kendi aylik ongorusu) | 29,0 ms | 1,2 MB |
//! | 1.000.000 (yillarca kullanim) | 350,6 ms | 61 MB |
//!
//! Surec basina BIR kez odenen maliyet bu. "Dogrusal okuma acitiyor" gerekcesi
//! bu sayilarla dusuyor.
//!
//! Ikinci gerekce — eszamanli yazma — de olculdu ve dustu: ikinci bir kervan
//! ornegi `TcpListener::bind` asamasinda `os error 10048` ile cikiyor (olculdu:
//! cikis kodu 1, defter 88 satirda degismedi) ve defter yazmalari yalniz istek
//! servis edilirken olur. Yani PORT BIR MUTEX gibi calisiyor: hicbir zaman iki
//! yazici olmuyor.
//!
//! Gecis sarti degismedi ve hala saglanmis degil: "sorgular karmasiklasirsa".
//! Bugunku sorgu bir suzme + uc sayim + bir ortanca.
//!
//! UYDURMA YOK: gozlem olmadan tahmin uretmez. Kanit sayisi her zaman birlikte
//! raporlanir; "henuz bilmiyorum" gecerli ve dogru bir cevaptir.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sonuc {
    Basari,
    /// Saglayici kota/limit bildirdi — ogrenicinin asil beslendigi olay.
    Kota,
    Hata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Olay {
    /// Unix saniye.
    pub t: u64,
    pub hesap: String,
    /// Hangi saglayici. **Kota hesabin degil, hesap+saglayici ciftinin ozelligidir:**
    /// ayni Google hesabinda gemini'nin limiti ile kimi'nin limiti ayri ayri isler.
    ///
    /// Bu alan yokken 88 olay tek havuza yaziliyordu ve `tahmin_et` bunlari karistiriyordu;
    /// somut sonucu su idi: gemini'de olculmus 120 dakikalik pencere, kimi kota
    /// bildirdiginde kimi'nin bekleme suresi olarak uygulaniyordu (server.rs'te
    /// `ozet()` -> `pencere_dk` -> `kota_doldu`). Yani yanlis saglayicidan ogrenilen
    /// sayi baska bir saglayiciya uygulaniyordu.
    ///
    /// `None` = alan eklenmeden once yazilmis ESKI kayit. Atfedilemez oldugu icin
    /// tahmine sokulmaz; `Ozet.eski_kayit` ile sayisi acikca raporlanir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saglayici: Option<String>,
    pub sonuc: Sonuc,
    /// Kota olaylarinda saglayicinin metninden kisa bir kesit; gercek ifadeyi
    /// ogrenmek icin. Basarili yanitlarin icerigi ASLA yazilmaz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not: Option<String>,
}

pub struct Defter {
    yol: PathBuf,
    olaylar: Mutex<Vec<Olay>>,
}

impl Defter {
    pub fn ac(yol: PathBuf) -> Result<Self> {
        let mut olaylar = Vec::new();
        if yol.exists() {
            let metin = std::fs::read_to_string(&yol)
                .with_context(|| format!("kota defteri okunamadi: {}", yol.display()))?;
            let mut bozuk = 0usize;
            for satir in metin.lines().filter(|s| !s.trim().is_empty()) {
                match serde_json::from_str::<Olay>(satir) {
                    Ok(o) => olaylar.push(o),
                    // Yarim yazilmis son satir ya da elle bozulmus girdi: atla ama
                    // sessizce degil. Kayit kaybini gizlemek ogrenmeyi yaniltir.
                    Err(_) => bozuk += 1,
                }
            }
            if bozuk > 0 {
                eprintln!("kota defterinde {bozuk} okunamayan satir atlandi");
            }
            olaylar.sort_by_key(|o| o.t);
        }
        if let Some(ust) = yol.parent() {
            std::fs::create_dir_all(ust).ok();
        }
        Ok(Self { yol, olaylar: Mutex::new(olaylar) })
    }

    pub async fn yaz(&self, hesap: &str, saglayici: &str, sonuc: Sonuc, not: Option<String>) {
        let olay = Olay {
            t: simdi(),
            hesap: hesap.to_string(),
            saglayici: Some(saglayici.to_string()),
            sonuc,
            not,
        };
        // Diske yazamamak ogrenmeyi durdurur ama istegi durdurmamali: uyar, devam et.
        match self.ekle(&olay) {
            Ok(()) => {}
            Err(e) => eprintln!("kota defterine yazilamadi: {e:#}"),
        }
        self.olaylar.lock().await.push(olay);
    }

    fn ekle(&self, olay: &Olay) -> Result<()> {
        let satir = serde_json::to_string(olay)?;
        let mut d = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.yol)
            .with_context(|| format!("kota defteri acilamadi: {}", self.yol.display()))?;
        writeln!(d, "{satir}")?;
        Ok(())
    }

    /// Ozet **hesap + saglayici** cifti icin cikar.
    ///
    /// Yalniz hesaba bakmak yanlisti: bir hesabin gemini ve kimi olaylari tek
    /// akista karisiyor, `tahmin_et` birinden ogrendigi pencereyi digerine
    /// uyguluyordu. Saglayicisi bilinmeyen ESKI kayitlar tahmine sokulmaz —
    /// atfedilemezler — ama sayilari `eski_kayit` ile gorunur kalir; kayip
    /// kaydi gizlemek ogrenmeyi yaniltir.
    pub async fn ozet(&self, hesap: &str, saglayici: &str) -> Ozet {
        let hepsi = self.olaylar.lock().await;
        let kendi: Vec<Olay> = hepsi
            .iter()
            .filter(|o| o.hesap == hesap && o.saglayici.as_deref() == Some(saglayici))
            .cloned()
            .collect();
        let eski_kayit =
            hepsi.iter().filter(|o| o.hesap == hesap && o.saglayici.is_none()).count();
        let n = simdi();
        let say = |sn: u64, s: Sonuc| {
            kendi.iter().filter(|o| o.sonuc == s && n.saturating_sub(o.t) < sn).count()
        };
        Ozet {
            toplam: kendi.len(),
            son_saat: say(3600, Sonuc::Basari),
            son_gun: say(86_400, Sonuc::Basari),
            son_hafta: say(604_800, Sonuc::Basari),
            eski_kayit,
            tahmin: tahmin_et(&kendi),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Ozet {
    pub toplam: usize,
    pub son_saat: usize,
    pub son_gun: usize,
    pub son_hafta: usize,
    /// Bu hesapta saglayicisi bilinmeyen (alan eklenmeden once yazilmis) kayit sayisi.
    /// Tahmine girmezler; gizlenmemeleri icin sayilari burada durur.
    pub eski_kayit: usize,
    pub tahmin: Tahmin,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Tahmin {
    /// Pencere kac DAKIKA surdu (kota carpmasindan sonraki ilk basariya kadar).
    ///
    /// Saat degil dakika: saatin tek ondaligiyla kisa pencereler `0.0` gorunuyordu,
    /// yani "bilmiyorum" ile "sifir" ayni okunuyordu. Tamsayi dakika hem kisa hem
    /// uzun pencerede kayipsiz.
    pub pencere_dk: Option<u64>,
    /// Pencere basina kac basarili istek gecti.
    pub pencere_basina_istek: Option<u32>,
    /// Kac kota carpmasi gozlendi. 0 ise tahmin YOK — uydurulmaz.
    pub kanit: usize,
}

/// Olaylardan kota penceresini cikarir. Girdi zaman sirali olmali.
///
/// Yontem: iki kota carpmasi arasinda kac basarili istek gectigi kapasiteyi,
/// carpmadan sonraki ilk basariya kadar gecen sure pencere uzunlugunu verir.
/// Birden cok ornek varsa ortancasi alinir — tek bir aykiri gozlem tahmini
/// kacirmasin diye.
pub fn tahmin_et(olaylar: &[Olay]) -> Tahmin {
    let mut kapasiteler: Vec<u32> = Vec::new();
    let mut pencereler: Vec<u64> = Vec::new();
    let mut sayac: u32 = 0;
    let mut acik_kota: Option<u64> = None;

    for o in olaylar {
        match o.sonuc {
            Sonuc::Basari => {
                if let Some(kt) = acik_kota.take() {
                    // Kotadan sonraki ilk basari: pencere yenilendi.
                    pencereler.push(o.t.saturating_sub(kt));
                    sayac = 0;
                }
                sayac += 1;
            }
            Sonuc::Kota => {
                // Ust uste kota carpmalari tek pencereye ait; yalniz ilki sayilir.
                // Yoksa ikinci carpma kapasiteyi 0 diye kaydedip tahmini bozardi.
                if acik_kota.is_none() {
                    kapasiteler.push(sayac);
                    acik_kota = Some(o.t);
                }
            }
            Sonuc::Hata => {}
        }
    }

    Tahmin {
        pencere_dk: ortanca(&mut pencereler).map(|s| s / 60),
        pencere_basina_istek: ortanca(&mut kapasiteler),
        kanit: kapasiteler.len(),
    }
}

fn ortanca<T: Ord + Copy>(v: &mut [T]) -> Option<T> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}

/// Saglayici kota mi bildiriyor?
///
/// Isaret listesi YAPILANDIRMADAN gelir cunku ifadeyi henuz gozlemlemedik; varsayilanlar
/// makul tahminler, kesin degil. Gercek metni gorunce `kervan.toml`'a eklenir.
///
/// Kisa yanit sarti bilerek var: limit mesajlari kisadir, ama kullanici Gemini'ye
/// "kota limitleri nasil isler" diye sorarsa UZUN ve icinde ayni ifadeler gecen bir
/// yanit gelir. Onu kota sanmak ogrenicinin verisini bozardi — ki ogrendigi tek sey
/// bu veri.
pub fn kota_sinyali(metin: &str, isaretler: &[String]) -> bool {
    const EN_UZUN_LIMIT_MESAJI: usize = 300;
    if metin.chars().count() > EN_UZUN_LIMIT_MESAJI {
        return false;
    }
    let m = metin.to_lowercase();
    isaretler.iter().any(|i| m.contains(&i.to_lowercase()))
}

pub fn varsayilan_yol() -> PathBuf {
    if let Some(p) = std::env::var_os("KERVAN_KOTA") {
        return PathBuf::from(p);
    }
    yerel_veri().join("kervan").join("kota.jsonl")
}

fn yerel_veri() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn simdi() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Defter yolunu disaridan gormek icin (gunluge basmak amaciyla).
pub fn yol_goster(d: &Defter) -> &Path {
    &d.yol
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(t: u64, sonuc: Sonuc) -> Olay {
        Olay { t, hesap: "a".into(), saglayici: Some("p".into()), sonuc, not: None }
    }

    /// ESKI KAYIT UYUMU (§IV kapisi): `saglayici` alani eklenmeden once yazilmis
    /// satirlar ayristirilmaya DEVAM etmeli. Bozulursa dort gunluk gercek defter
    /// sessizce cope giderdi.
    #[test]
    fn saglayicisiz_eski_satir_hala_ayrisir() {
        let satir = r#"{"t":1786823552,"hesap":"ercan-gemini-2","sonuc":"basari"}"#;
        let o: Olay = serde_json::from_str(satir).expect("eski satir ayrisamadi");
        assert_eq!(o.hesap, "ercan-gemini-2");
        assert_eq!(o.sonuc, Sonuc::Basari);
        assert_eq!(o.saglayici, None, "eski kayit atfedilemez olmali");
    }

    /// Yeni yazilan satirda `saglayici` bulunmali; yoksa dun duzelttigimiz hata
    /// yarin sessizce geri gelir.
    #[test]
    fn yeni_satir_saglayici_tasir() {
        let o = Olay {
            t: 1,
            hesap: "a".into(),
            saglayici: Some("kimi".into()),
            sonuc: Sonuc::Basari,
            not: None,
        };
        let s = serde_json::to_string(&o).unwrap();
        assert!(s.contains(r#""saglayici":"kimi""#), "{s}");
    }

    /// ASIL HATA: bir hesabin iki saglayicisi ayri kota havuzudur.
    ///
    /// gemini'de 120 dakikalik pencere olculmusken kimi kota bildirirse, kimi'nin
    /// beklemesi gemini'nin sayisiyla kurulamaz. Karisik akista `tahmin_et` tek
    /// pencere uretiyordu; ayrildiktan sonra her saglayici kendi kanitini tasir.
    #[test]
    fn saglayicilar_ayri_kota_havuzudur() {
        let s = 3600;
        let ev = |t: u64, sag: &str, sonuc: Sonuc| Olay {
            t,
            hesap: "a".into(),
            saglayici: Some(sag.into()),
            sonuc,
            not: None,
        };
        // gemini: 2 istek sonra kota, 2 saat sonra acildi.
        // kimi: arada gecen, kotayla ilgisi olmayan basarilar.
        let hepsi = vec![
            ev(0, "gemini", Sonuc::Basari),
            ev(5, "kimi", Sonuc::Basari),
            ev(10, "gemini", Sonuc::Basari),
            ev(15, "kimi", Sonuc::Basari),
            ev(20, "gemini", Sonuc::Kota),
            ev(25, "kimi", Sonuc::Basari),
            ev(20 + 2 * s, "gemini", Sonuc::Basari),
        ];

        let sadece = |sag: &str| -> Vec<Olay> {
            hepsi.iter().filter(|o| o.saglayici.as_deref() == Some(sag)).cloned().collect()
        };

        let g = tahmin_et(&sadece("gemini"));
        assert_eq!(g.kanit, 1);
        assert_eq!(g.pencere_dk, Some(120), "gemini kendi penceresini bilir");

        let k = tahmin_et(&sadece("kimi"));
        assert_eq!(k.kanit, 0, "kimi hic kota gormedi");
        assert_eq!(k.pencere_dk, None, "kimi'ye gemini'nin penceresi uygulanamaz");

        // Ayrilmamis akista kimi'nin basarilari gemini'nin kapasitesine karisiyordu:
        // kota carpmasina kadar sayilan 4 istegin yalniz 2'si gemini'ye aitti.
        assert_eq!(tahmin_et(&hepsi).pencere_basina_istek, Some(4), "karisik akis sisirir");
        assert_eq!(g.pencere_basina_istek, Some(2), "gercek gemini kapasitesi");
    }

    /// Gozlem yoksa tahmin de OLMAMALI. Uydurulmus bir limit, olmayan bir limitten
    /// beterdir: havuz ona gore plan yapar.
    #[test]
    fn kanit_yoksa_tahmin_yok() {
        let olaylar: Vec<Olay> = (0..50).map(|i| o(i * 60, Sonuc::Basari)).collect();
        let t = tahmin_et(&olaylar);
        assert_eq!(t.kanit, 0);
        assert_eq!(t.pencere_basina_istek, None);
        assert_eq!(t.pencere_dk, None);
    }

    /// Iki temiz dongu: her pencerede 3 istek, pencere 2 saat.
    #[test]
    fn iki_dongude_pencere_ve_kapasite_cikar() {
        let s = 3600;
        let olaylar = vec![
            o(0, Sonuc::Basari),
            o(10, Sonuc::Basari),
            o(20, Sonuc::Basari),
            o(30, Sonuc::Kota),
            o(30 + 2 * s, Sonuc::Basari), // 2 saat sonra acildi
            o(30 + 2 * s + 10, Sonuc::Basari),
            o(30 + 2 * s + 20, Sonuc::Basari),
            o(30 + 2 * s + 30, Sonuc::Kota),
        ];
        let t = tahmin_et(&olaylar);
        assert_eq!(t.kanit, 2);
        assert_eq!(t.pencere_basina_istek, Some(3));
        assert_eq!(t.pencere_dk, Some(120));
    }

    /// Kisa pencere `0` degil gercek degeriyle raporlanmali; saat cinsinden tek
    /// ondalikla 1 dakikalik pencere "0.0" gorunuyordu, yani "bilmiyorum"la
    /// karisiyordu.
    #[test]
    fn kisa_pencere_sifira_yuvarlanmaz() {
        let olaylar = vec![
            o(0, Sonuc::Basari),
            o(10, Sonuc::Kota),
            o(10 + 300, Sonuc::Basari), // 5 dakika sonra acildi
        ];
        assert_eq!(tahmin_et(&olaylar).pencere_dk, Some(5));
    }

    /// Ust uste kota carpmasi tek pencereye ait; ikincisi kapasiteyi 0 yapmamali.
    #[test]
    fn ard_arda_kota_kapasiteyi_bozmaz() {
        let olaylar = vec![
            o(0, Sonuc::Basari),
            o(10, Sonuc::Basari),
            o(20, Sonuc::Kota),
            o(30, Sonuc::Kota),
            o(40, Sonuc::Kota),
        ];
        let t = tahmin_et(&olaylar);
        assert_eq!(t.kanit, 1, "uc carpma tek pencere");
        assert_eq!(t.pencere_basina_istek, Some(2));
    }

    /// Hatalar kapasiteyi tuketmis sayilmamali: istek saglayiciya ulasmadi.
    #[test]
    fn hatalar_kapasiteden_sayilmaz() {
        let olaylar = vec![
            o(0, Sonuc::Basari),
            o(10, Sonuc::Hata),
            o(20, Sonuc::Hata),
            o(30, Sonuc::Basari),
            o(40, Sonuc::Kota),
        ];
        assert_eq!(tahmin_et(&olaylar).pencere_basina_istek, Some(2));
    }

    /// Kullanici modele "limitler nasil isler" diye sorarsa gelen UZUN yanit kota
    /// sanilmamali; yoksa ogrenicinin verisi kendi sorularimizla kirlenir.
    #[test]
    fn uzun_yanit_kota_sayilmaz() {
        let isaretler = vec!["reached your limit".to_string()];
        assert!(kota_sinyali("You've reached your limit.", &isaretler));
        let uzun = format!("{} {}", "Kota limitleri soyle isler: ".repeat(20), "reached your limit");
        assert!(!kota_sinyali(&uzun, &isaretler), "uzun aciklama kota degildir");
    }

    #[test]
    fn bos_isaret_listesi_hicbir_seyi_kota_yapmaz() {
        assert!(!kota_sinyali("herhangi bir yanit", &[]));
    }
}
