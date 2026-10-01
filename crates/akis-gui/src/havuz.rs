//! Sağlayıcı panelindeki hesap havuzu satırları.
//!
//! Doğruluk kaynağı sunucunun gerçek `Pool`'u. Her yenilemede onun anlık görüntüsü
//! `session_pool::OturumHavuzu`'na çevrilir; durum etiketi ve "en uygun hesap" (★)
//! seçimi oradan gelir. İkinci bir sayaç tutulmaz, havuz yalnız görünüm modelidir.

use std::time::{Duration, Instant};

use eframe::egui;
use kervan_akis::session_pool::{HesapOturumu, OturumDurumu, OturumHavuzu, Saglayici};
use kervan_havuz::pool::Pool;

use crate::renk;

#[derive(Debug, Clone, PartialEq)]
pub struct HesapSatiri {
    /// `Pool::slots` sırası; "Göster" bu numarayla çağrılır.
    pub index: usize,
    pub ad: String,
    pub saglayici: String,
    pub durum: OturumDurumu,
    pub son_saatte: u32,
    pub en_uygun: bool,
}

/// Havuzun anlık görüntüsü.
pub async fn goruntu(pool: &Pool) -> Vec<HesapSatiri> {
    let simdi = Instant::now();
    let mut havuz = OturumHavuzu::yeni();
    for (i, slot) in pool.slots.iter().enumerate() {
        let s = slot.state.lock().await;
        let durum = match (&s.paused_reason, s.not_before) {
            (Some(neden), _) => OturumDurumu::InsanKapisi {
                aciklama: neden.clone(),
            },
            (None, Some(t)) if t > simdi => OturumDurumu::Sogumada {
                deneme_sayisi: s.consecutive_errors,
                bitis: t,
            },
            _ => OturumDurumu::Hazir,
        };
        // id = sıra numarası (en uygun seçimi havuz sırasını izlesin), tab_id = yuva
        // numarası; kapsayıcı alanı bu görünümde hesap adını taşır.
        let mut o = HesapOturumu::yeni(
            format!("{i:04}"),
            Saglayici::parse(&slot.provider.ad),
            slot.hesap_adi.clone(),
            i as u32,
        );
        o.durum = durum;
        o.son_bir_saatteki_istekler = s.recent.clone();
        o.saatlik_tavan = pool.pacing.hourly_cap;
        o.asgari_bekleme = Duration::ZERO; // hız bütçesi zaten `not_before`'da
        o.basarili_istek_sayisi = u64::from(s.total);
        havuz.oturum_kaydet(o);
    }
    satirlar(havuz, simdi)
}

/// Görünüm modelinden satırlar; her sağlayıcının en uygun hesabı işaretlenir.
fn satirlar(mut havuz: OturumHavuzu, simdi: Instant) -> Vec<HesapSatiri> {
    let mut saglayicilar: Vec<Saglayici> = Vec::new();
    for o in havuz.oturumlar.values() {
        if !saglayicilar.contains(&o.saglayici) {
            saglayicilar.push(o.saglayici.clone());
        }
    }
    let en_uygunlar: Vec<String> = saglayicilar
        .iter()
        .filter_map(|s| havuz.en_uygun_hesabi_sec(s, simdi).map(|o| o.id))
        .collect();
    let mut liste: Vec<HesapSatiri> = havuz
        .oturumlar
        .values_mut()
        .map(|o| {
            o.istekleri_buda(simdi);
            HesapSatiri {
                index: o.tab_id as usize,
                ad: o.cookie_store_id.clone(),
                saglayici: o.saglayici.as_str().to_string(),
                durum: o.durum.clone(),
                son_saatte: o.son_bir_saatteki_istekler.len() as u32,
                en_uygun: en_uygunlar.contains(&o.id),
            }
        })
        .collect();
    liste.sort_by_key(|s| s.index);
    liste
}

/// Seçili sağlayıcının hesaplarını çizer; "Göster"e basılan hesabın numarasını döndürür.
pub fn ciz(ui: &mut egui::Ui, satirlar: &[HesapSatiri], saglayici: &str) -> Option<usize> {
    let bunlar: Vec<_> = satirlar
        .iter()
        .filter(|s| s.saglayici == saglayici)
        .collect();
    if bunlar.is_empty() {
        return None;
    }
    let mut goster = None;
    ui.group(|ui| {
        ui.colored_label(
            renk::SOLUK,
            format!("Hesap havuzu · {} hesap", bunlar.len()),
        );
        for s in bunlar {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let ad = if s.en_uygun {
                        format!("★ {}", s.ad)
                    } else {
                        s.ad.clone()
                    };
                    ui.label(egui::RichText::new(ad).strong());
                    let (yazi, r) = durum_etiketi(&s.durum);
                    ui.colored_label(r, egui::RichText::new(yazi).small());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if matches!(s.durum, OturumDurumu::InsanKapisi { .. }) {
                        if ui
                            .button("Göster")
                            .on_hover_text("Doğrulamayı çözmek için pencereyi öne getir")
                            .clicked()
                        {
                            goster = Some(s.index);
                        }
                    } else {
                        ui.colored_label(renk::SOLUK, format!("{} istek/sa", s.son_saatte));
                    }
                });
            });
        }
    });
    goster
}

pub fn durum_etiketi(d: &OturumDurumu) -> (String, egui::Color32) {
    let kalan = |t: &Instant| {
        let sn = t.saturating_duration_since(Instant::now()).as_secs();
        format!("{:02}:{:02}", sn / 60, sn % 60)
    };
    match d {
        OturumDurumu::Hazir => ("✓ müsait".into(), renk::BASARI),
        OturumDurumu::Sogumada { bitis, .. } => {
            (format!("⏳ soğumada {}", kalan(bitis)), renk::UYARI)
        }
        OturumDurumu::KotaliBekleme { acilis_zamani } => {
            (format!("⏳ kota {}", kalan(acilis_zamani)), renk::UYARI)
        }
        OturumDurumu::InsanKapisi { .. } => ("⚠ doğrulama bekliyor".into(), renk::HATA),
        OturumDurumu::DevreDisi => ("devre dışı".into(), renk::SOLUK),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oturum(i: u32, ad: &str, durum: OturumDurumu) -> HesapOturumu {
        let mut o = HesapOturumu::yeni(format!("{i:04}"), Saglayici::Gemini, ad, i);
        o.durum = durum;
        o.asgari_bekleme = Duration::ZERO;
        o
    }

    /// Soğumadaki ilk hesap atlanır, ★ müsait olan ilk hesaba gider; sıra korunur.
    #[test]
    fn en_uygun_musait_ilk_hesaptir() {
        let simdi = Instant::now();
        let mut h = OturumHavuzu::yeni();
        let bitis = simdi + Duration::from_secs(60);
        h.oturum_kaydet(oturum(
            0,
            "a",
            OturumDurumu::Sogumada {
                deneme_sayisi: 1,
                bitis,
            },
        ));
        h.oturum_kaydet(oturum(1, "b", OturumDurumu::Hazir));
        h.oturum_kaydet(oturum(2, "c", OturumDurumu::Hazir));
        let s = satirlar(h, simdi);
        assert_eq!(
            s.iter().map(|x| x.ad.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(
            s.iter().map(|x| x.en_uygun).collect::<Vec<_>>(),
            [false, true, false]
        );
    }
}
