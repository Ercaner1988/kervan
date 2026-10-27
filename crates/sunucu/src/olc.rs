//! ÖLÇÜM ucu: bağlı hesapların GERÇEK model menüsünü okur ve `kaydet=1` ise
//! sonucu kalıcı deftere yazar (`crate::olcum`).
//!
//! NEDEN AYRI UÇ: `kervan.toml` sağlayıcı geneli bir katalogdur ve hesap
//! farkını taşıyamaz. "Bu hesapta Pro var mı?" sorusunun cevabı ancak o hesabın
//! sayfasındaki menü tıklanarak ölçülür. Uç TAHMİN ÜRETMEZ: okunamayan hesap
//! `hata` alanıyla döner, menü uydurulmaz.
//!
//! `?secici=` verilirse yapılandırılmış model menüsü yerine o seçici okunur
//! (ör. `button[aria-label]` ile sayfadaki araç düğmelerini dökmek için).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::olcum::{HesapOlcumu, OlcuModel, OlcuOzellik, dusunme_mi, slug};
use crate::server::{AppState, simdi};
use kervan_secim::{OgeDurum, hedef_indeks};

#[derive(Debug, Deserialize)]
pub struct OlcSorgu {
    /// Yalnız bu sağlayıcının hesaplarını ölç.
    pub saglayici: Option<String>,
    /// Yalnız bu hesabı ölç (`provider` alanı olmadan da verilebilir).
    pub hesap: Option<String>,
    /// Yapılandırılmış menü yerine bu seçiciyi oku.
    pub secici: Option<String>,
    /// ARAÇ menüsü seçicisi: özellikler model menüsünden AYRI paneldedir.
    /// Verilirse o panel okunur ve `kaydet=1` ile deftere `ozellikler` olarak
    /// yazılır. Model ölçümüyle aynı turda da verilebilir.
    pub ozellik: Option<String>,
    /// Okumadan önce basılacak düğme (menü açıcı).
    pub dugme: Option<String>,
    /// DENE: yalnız bu ada sahip öğeye bas ve gerçekten seçiliyor mu ölç.
    pub dene: Option<String>,
    /// TÜM modelleri tek tek dene (kilitli/seçilebilir ayrımını çıkarır).
    #[serde(default, deserialize_with = "esnek_bayrak")]
    pub tum: Option<bool>,
    /// Ölçümü kalıcı deftere yaz.
    #[serde(default, deserialize_with = "esnek_bayrak")]
    pub kaydet: Option<bool>,
}

/// Bayrak alanlarını `true/false` yanında `1/0` ile de kabul eder.
///
/// NEDEN: `Option<bool>` serde'de yalnız `true`/`false` çözer; belgelerde ve elle
/// yazılan komutlarda `kaydet=1` doğal yazımdır ve sessizce 400 dönmesi ölçümün
/// hiç koşmamasına yol açar (yaşandı: defter yazılmadı, sebep görünmedi).
/// Tanınmayan değer sessizce yutulmaz — açık hata verir.
fn esnek_bayrak<'de, D>(d: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    let ham: Option<String> = Option::deserialize(d)?;
    let Some(v) = ham else {
        return Ok(None);
    };
    match v.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "evet" | "yes" | "on" => Ok(Some(true)),
        "0" | "false" | "hayir" | "no" | "off" => Ok(Some(false)),
        _ => Err(serde::de::Error::custom(format!(
            "gecersiz bayrak degeri: {v} (1/0/true/false)"
        ))),
    }
}

/// Bağlı her hesap için menüyü sayfadan okur.
///
/// TEK TEK ve sıralı: aynı siteye eşzamanlı menü açmak sekmeleri birbirine
/// karıştırır; ölçümün amacı hangi hesabın ne gördüğünü AYIRMAK.
/// Ölçüm turunun süre bütçesi.
///
/// ÖLÇÜLDÜ (2026-10-25): 12 sn yetmiyor. Menü açma üç stratejiyi sırayla
/// deniyor (her biri 2,5 sn) ve okuma boş listede üç kez yeniden açıyor;
/// gerçek bir tıklama turu 15-25 sn sürüyor. Kısa bütçe "menü yok" sanılan
/// ama aslında yalnız YAVAŞ olan ölçümler üretiyordu (dört aday seçici de
/// "zaman aşımı" döndü, hiçbiri gerçekte yoklanmadı). Ölçüm isteği değildir;
/// uzun sürmesi yük dengesini etkilemez.
const OLCUM_BUTCESI: std::time::Duration = std::time::Duration::from_secs(45);

pub async fn olc(
    State(st): State<Arc<AppState>>,
    Query(q): Query<OlcSorgu>,
) -> Json<serde_json::Value> {
    let mut hesaplar = Vec::new();
    let mut yazilacak: Vec<HesapOlcumu> = Vec::new();
    for anahtar in st.eklenti_hub.bagli_hesaplar().await {
        let Some((saglayici, profil)) = anahtar.split_once(':') else {
            continue;
        };
        if let Some(istenen) = q.saglayici.as_deref()
            && istenen != saglayici
        {
            continue;
        }
        if let Some(istenen) = q.hesap.as_deref()
            && istenen != profil
        {
            continue;
        }
        let yapilandirma = st.saglayicilar.get(saglayici);
        let secici = q
            .secici
            .clone()
            .or_else(|| yapilandirma.and_then(|p| p.model.as_ref().map(|m| m.oge.clone())));
        let dugme = q
            .dugme
            .clone()
            .or_else(|| yapilandirma.and_then(|p| p.model.as_ref().map(|m| m.dugme.clone())));
        let Some(secici) = secici else {
            hesaplar.push(serde_json::json!({
                "hesap": anahtar,
                "hata": "model menusu tanimli degil; ?secici= ile olculebilir",
            }));
            continue;
        };
        let sonuc = st
            .eklenti_hub
            .menu_oku_hesap(saglayici, profil, &secici, dugme.as_deref(), OLCUM_BUTCESI)
            .await;
        let once = match sonuc {
            Ok(o) => o,
            Err(e) => {
                hesaplar.push(serde_json::json!({
                    "hesap": anahtar,
                    "secici": secici,
                    "hata": format!("{e:#}"),
                }));
                continue;
            }
        };
        let mut kayit = serde_json::json!({
            "hesap": anahtar,
            "secici": secici,
            "menu": liste(&once),
        });

        // Özellik (araç) paneli: model menüsünden AYRI. Panel metni okunur ve
        // ham kanıt (ad, tam metin, işaret) saklanır; "kilitli/açık" hükmü bu
        // ölçümden ÇIKARILMAZ çünkü kilit göstergesi metinden güvenilir değil.
        let mut olculen_ozellikler: Option<Vec<OlcuOzellik>> = None;
        if let Some(ozellik_secici) = q.ozellik.as_deref() {
            match st
                .eklenti_hub
                .menu_oku_hesap(
                    saglayici,
                    profil,
                    ozellik_secici,
                    dugme.as_deref(),
                    OLCUM_BUTCESI,
                )
                .await
            {
                Ok(oge) => {
                    let mut gorulen: Vec<OlcuOzellik> = Vec::new();
                    for x in &oge {
                        let etiket = x.metin.trim();
                        let kod = slug(etiket);
                        if kod.is_empty() || gorulen.iter().any(|o| o.kod == kod) {
                            continue;
                        }
                        gorulen.push(OlcuOzellik {
                            kod,
                            etiket: etiket.to_string(),
                            isaretli: x.isaretli,
                            tam: x.tam.clone(),
                        });
                    }
                    kayit["ozellikler"] = serde_json::json!({
                        "secici": ozellik_secici,
                        "menu": liste(&oge),
                        "olculen": gorulen,
                    });
                    if !gorulen.is_empty() {
                        olculen_ozellikler = Some(gorulen);
                    }
                }
                Err(e) => {
                    kayit["ozellikler"] = serde_json::json!({
                        "secici": ozellik_secici,
                        "hata": format!("{e:#}"),
                    });
                }
            }
        }

        // Tek öğe denemesi: "menüde var" ile "seçilebiliyor" ayrımını kanıtlar.
        if let Some(aranan) = q.dene.as_deref() {
            kayit["dene"] = dene(
                &st,
                saglayici,
                profil,
                &secici,
                dugme.as_deref(),
                &once,
                aranan,
            )
            .await;
        }

        // Tüm modeller: her öğe için bas → yeniden oku → geri kur.
        let mut olculen_modeller: Option<Vec<OlcuModel>> = None;
        if q.tum.unwrap_or(false) {
            let (olculen, kanit) =
                tumunu_dene(&st, saglayici, profil, &secici, dugme.as_deref(), &once).await;
            kayit["olcum"] = serde_json::json!(kanit);
            olculen_modeller = olculen;
        }
        if q.kaydet.unwrap_or(false) && (olculen_modeller.is_some() || olculen_ozellikler.is_some())
        {
            yazilacak.push(HesapOlcumu {
                saglayici: saglayici.to_string(),
                hesap: profil.to_string(),
                olculdu: simdi(),
                modeller: olculen_modeller.unwrap_or_default(),
                ozellikler: olculen_ozellikler.unwrap_or_default(),
            });
        }
        hesaplar.push(kayit);
    }

    let mut kayit_durumu = serde_json::Value::Null;
    if q.kaydet.unwrap_or(false) && !yazilacak.is_empty() {
        kayit_durumu = match crate::olcum::kaydet(&yazilacak) {
            Ok(()) => serde_json::json!({
                "yazildi": yazilacak.len(),
                "dosya": crate::olcum::dosya_yolu_goster(),
            }),
            Err(e) => serde_json::json!({ "hata": format!("{e:#}") }),
        };
    }
    Json(serde_json::json!({
        "olculdu": simdi(),
        "hesaplar": hesaplar,
        "kayit": kayit_durumu,
    }))
}

fn liste(o: &[OgeDurum]) -> Vec<serde_json::Value> {
    o.iter()
        .map(|x| {
            serde_json::json!({
                "ad": x.metin,
                "tam": x.tam,
                "isaretli": x.isaretli,
                "kanit": x.kanit,
            })
        })
        .collect()
}

/// Menüdeki seçili öğeler (model + düşünme ekseni ayrımıyla).
fn secililer(o: &[OgeDurum]) -> Vec<String> {
    o.iter()
        .filter(|x| x.isaretli)
        .map(|x| x.metin.clone())
        .collect()
}

/// Ölçümden sonra menüyü ESKİ hâline getirir: önce seçili olup şimdi seçili
/// olmayan her öğeye yeniden basar. Ölçüm hesabın ayarını değiştirip bırakmaz.
async fn geri_kur(
    st: &Arc<AppState>,
    saglayici: &str,
    profil: &str,
    secici: &str,
    dugme: Option<&str>,
    once_secili: &[String],
    simdiki: &[OgeDurum],
) -> serde_json::Value {
    let ruzgar = OLCUM_BUTCESI;
    let mut geri: Vec<String> = Vec::new();
    let mut kalan: Vec<OgeDurum> = simdiki.to_vec();
    for ad in once_secili {
        let Some(i) = hedef_indeks(&kalan, ad) else {
            continue;
        };
        if kalan[i].isaretli {
            continue;
        }
        match st
            .eklenti_hub
            .menu_bas_hesap(saglayici, profil, secici, i, dugme, ruzgar)
            .await
        {
            Ok(yeni) => {
                kalan = yeni;
                geri.push(ad.clone());
            }
            Err(e) => {
                return serde_json::json!({
                    "geri_kurulan": geri,
                    "hata": format!("{e:#}"),
                });
            }
        }
    }
    serde_json::json!({ "geri_kurulan": geri })
}

/// Tüm model öğelerini tek tek dener: bas → yeniden oku → seçildi mi?
///
/// Düşünme ekseni atlanır (model değil). Sonunda menü eski hâline döner.
async fn tumunu_dene(
    st: &Arc<AppState>,
    saglayici: &str,
    profil: &str,
    secici: &str,
    dugme: Option<&str>,
    once: &[OgeDurum],
) -> (Option<Vec<OlcuModel>>, serde_json::Value) {
    let ruzgar = OLCUM_BUTCESI;
    let ilk_secili = secililer(once);
    let adaylar: Vec<(usize, String, String)> = once
        .iter()
        .enumerate()
        .filter(|(_, o)| !dusunme_mi(&o.metin) && !o.metin.trim().is_empty())
        .map(|(i, o)| (i, o.metin.clone(), o.tam.clone()))
        .collect();
    let mut modeller: Vec<OlcuModel> = Vec::new();
    let mut kanit: Vec<serde_json::Value> = Vec::new();
    let mut guncel: Vec<OgeDurum> = once.to_vec();
    let mut basarisiz = 0usize;

    for (_, etiket, tam) in adaylar {
        let kod = slug(&etiket);
        let Some(i) = hedef_indeks(&guncel, &etiket) else {
            kanit.push(serde_json::json!({
                "etiket": etiket, "sonuc": "menuden_kayboldu"
            }));
            continue;
        };
        if guncel[i].isaretli {
            modeller.push(OlcuModel {
                kod,
                etiket: etiket.clone(),
                secilebilir: true,
            });
            kanit.push(serde_json::json!({
                "etiket": etiket, "sonuc": "zaten_secili"
            }));
            continue;
        }
        match st
            .eklenti_hub
            .menu_bas_hesap(saglayici, profil, secici, i, dugme, ruzgar)
            .await
        {
            Ok(yeni) => {
                let secildi = hedef_indeks(&yeni, &etiket).is_some_and(|j| yeni[j].isaretli);
                modeller.push(OlcuModel {
                    kod: kod.clone(),
                    etiket: etiket.clone(),
                    secilebilir: secildi,
                });
                kanit.push(serde_json::json!({
                    "etiket": etiket,
                    "tam": tam,
                    "sonuc": if secildi { "secildi" } else { "secilemedi" },
                }));
                // Her denemeden sonra eski hâle dön ki sonraki deneme temiz olsun.
                let g = geri_kur(st, saglayici, profil, secici, dugme, &ilk_secili, &yeni).await;
                guncel = match st
                    .eklenti_hub
                    .menu_oku_hesap(saglayici, profil, secici, dugme, ruzgar)
                    .await
                {
                    Ok(o) => o,
                    Err(e) => {
                        kanit.push(serde_json::json!({
                            "etiket": etiket, "sonuc": "geri_okunamadi", "hata": format!("{e:#}")
                        }));
                        basarisiz += 1;
                        yeni
                    }
                };
                if g.get("hata").is_some() {
                    kanit.push(serde_json::json!({
                        "etiket": etiket, "sonuc": "geri_kurulamadi", "ayrinti": g
                    }));
                }
            }
            Err(e) => {
                kanit.push(serde_json::json!({
                    "etiket": etiket, "sonuc": "tiklanamadi", "hata": format!("{e:#}")
                }));
                basarisiz += 1;
            }
        }
    }

    let ozet = serde_json::json!({
        "denenen": kanit.len(),
        "basarisiz": basarisiz,
        "son_secili": secililer(&guncel),
        "adimlar": kanit,
    });
    // Hiçbir öğe denenemediyse ölçüm YOKTUR: boş listeyi "kaydedilebilir" sayma.
    if modeller.is_empty() || modeller.len() == basarisiz {
        (None, ozet)
    } else {
        (Some(modeller), ozet)
    }
}

/// "Menüde var" ≠ "bu hesapta seçilebiliyor". Tek kanıt: bas, yeniden oku.
#[allow(clippy::too_many_arguments, reason = "ölçüm bağlamı tek çağrıda")]
async fn dene(
    st: &Arc<AppState>,
    saglayici: &str,
    profil: &str,
    secici: &str,
    dugme: Option<&str>,
    once: &[OgeDurum],
    aranan: &str,
) -> serde_json::Value {
    let ruzgar = OLCUM_BUTCESI;
    let onceki = secililer(once);
    let Some(i) = hedef_indeks(once, aranan) else {
        return serde_json::json!({ "sonuc": "menude_yok", "onceki_secili": onceki });
    };
    if once[i].isaretli {
        return serde_json::json!({ "sonuc": "zaten_secili", "onceki_secili": onceki });
    }
    let sonra = match st
        .eklenti_hub
        .menu_bas_hesap(saglayici, profil, secici, i, dugme, ruzgar)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            return serde_json::json!({
                "sonuc": "tiklanamadi",
                "hata": format!("{e:#}"),
                "onceki_secili": onceki,
            });
        }
    };
    let secildi = hedef_indeks(&sonra, aranan).is_some_and(|j| sonra[j].isaretli);
    let mut kayit = serde_json::json!({
        "sonuc": if secildi { "secildi" } else { "secilemedi" },
        "onceki_secili": onceki,
        "sonra_secili": secililer(&sonra),
    });
    kayit["geri"] = geri_kur(st, saglayici, profil, secici, dugme, &onceki, &sonra).await;
    kayit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct Deneme {
        #[serde(default, deserialize_with = "esnek_bayrak")]
        tum: Option<bool>,
    }

    fn coz(json: &str) -> Option<bool> {
        serde_json::from_str::<Deneme>(json)
            .expect("bayrak cozulmeli")
            .tum
    }

    #[test]
    fn bayrak_bir_ve_sifir_kabul_edilir() {
        // Belgelerdeki dogal yazim: kaydet=1 / tum=1. Sessiz 400 uretmemeli.
        assert_eq!(coz(r#"{"tum":"1"}"#), Some(true));
        assert_eq!(coz(r#"{"tum":"0"}"#), Some(false));
    }

    #[test]
    fn bayrak_yer_ve_buyuk_harf_tanir() {
        assert_eq!(coz(r#"{"tum":" TRUE "}"#), Some(true));
        assert_eq!(coz(r#"{"tum":"False"}"#), Some(false));
    }

    #[test]
    fn bayrak_yoksa_none() {
        assert_eq!(coz("{}"), None);
    }

    #[test]
    fn bayrak_taninmayan_degeri_yutmaz() {
        // Sessiz yanlis anlam, yanlis olcumden kotudur: acik hata beklenir.
        let hata = serde_json::from_str::<Deneme>(r#"{"tum":"belki"}"#);
        assert!(hata.is_err(), "taninmayan deger hata vermeli");
    }
}
