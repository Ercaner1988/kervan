//! # ekran5_ayarlar
//! Ekran 5: Ayarlar (Görünüm, kilim-tema, Güvenlik, Sistem Bilgileri)
//!
//! REST anahtarı kartı yalnız DURUM gösteriyordu: tanımsız bir anahtar
//! kırmızı yazıyla kalıyor, eklemek için hiçbir yol yoktu (m02007). Bu ekran
//! artık anahtarı ekleyip kaldırabilecek bir form barındırır; eylemi kendisi
//! UYGULAMAZ, `AnahtarFormu.bekleyen` alanına bırakır. Gerekçe: bu sandık
//! (`kervan-pano-ekran`) oturum kasasını görmüyor, gizli değeri elinde
//! tutmuyor; yazma tek kapıdan (`crates/gui`) yapılır.

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::Ozet;
use kilim_tema::TemaTercihi;

/// Ayarlar ekranının anahtar formu durumu.
#[derive(Debug, Default, Clone)]
pub struct AnahtarFormu {
    /// Formun hedeflediği ortam değişkeni adı (ör. `OPENROUTER_API_KEY`).
    pub secili_env: String,
    /// Henüz gönderilmemiş anahtar değeri.
    ///
    /// Değer yalnız burada, bellekte durur; hiçbir yere yazılmaz, günlüğe
    /// düşmez, `Ozet`'e girmez. Gönderimden sonra temizlenir.
    pub deger: String,
    /// Kullanıcıya gösterilecek sonuç: (mesaj, başarılı mı).
    pub mesaj: Option<(String, bool)>,
    /// Çizim sırasında oluşan, GUI katmanının uygulayacağı eylem.
    pub bekleyen: Option<AnahtarEylemi>,
}

/// Anahtar kartından çıkan eylem. Uygulama yeri `crates/gui/src/gui.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnahtarEylemi {
    /// Bu oturumun kasasına anahtar koy.
    Ekle { ad: String, deger: String },
    /// Bu oturumda girilmiş anahtarı kaldır (ortam değişkenini açığa çıkarır).
    Sil { ad: String },
}

pub fn render(ui: &mut Ui, ozet: &Ozet, tema_tercihi: &mut TemaTercihi, form: &mut AnahtarFormu) {
    ui.vertical(|ui| {
        ui.heading("Ayarlar & Sistem Masası");
        ui.separator();

        ui.horizontal(|ui| {
            // SOL: TEMA VE GÖRÜNÜM
            ui.vertical(|ui| {
                ui.set_width(380.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Görünüm & Tema (kilim-tema)").strong());
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Tema Seçici:");
                        kilim_tema::tema_secici(ui, tema_tercihi);
                    });

                    ui.add_space(8.0);
                    ui.label(RichText::new("Kâğıt lifi zemin ve kilim kenarlık paleti devrede.").size(11.0).color(Color32::GRAY));
                });

                ui.add_space(8.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Sistem ve Çalışma Zamanı (/health)").strong());
                    ui.separator();
                    ui.label(format!("Kervan Sürümü: v{}", ozet.saglik.surum));
                    ui.label(format!("Render Arka Ucu: {}", ozet.saglik.backend));
                    ui.label(format!("Çalışma Süresi: {} sn", ozet.saglik.calisma_suresi_sn));
                });
            });

            ui.add_space(12.0);

            // SAĞ: GÜVENLİK VE SAĞLAYICI ANAHTARLARI
            ui.vertical(|ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("REST Sağlayıcı Anahtarları ([rest.*])").strong());
                    ui.label(
                        RichText::new("Güvenlik kuralı: Anahtar değerleri ekranda ve günlükte asla görüntülenmez.")
                            .size(11.0)
                            .color(Color32::from_rgb(192, 79, 66)),
                    );
                    ui.separator();

                    if ozet.kimlikler.is_empty() {
                        ui.label(RichText::new("Yapılandırılmış REST anahtarı bulunamadı.").italics());
                    } else {
                        for k in &ozet.kimlikler {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&k.ad).strong());
                                if k.tanimli {
                                    // Panelden girilen anahtar süreç ömrüyle sınırlı;
                                    // bu ayrım yazılmazsa kullanıcı ertesi gün
                                    // "anahtar kayboldu" sanır.
                                    let etiket = if k.oturumda {
                                        "● Bu oturumda girildi (kapanınca silinir)"
                                    } else {
                                        "● Ortam Değişkeninde Tanımlı"
                                    };
                                    ui.label(
                                        RichText::new(etiket)
                                            .size(11.0)
                                            .color(Color32::from_rgb(46, 125, 79)),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("○ Tanımsız — anahtar yok")
                                            .size(11.0)
                                            .color(Color32::from_rgb(192, 79, 66)),
                                    );
                                }
                                if k.oturumda {
                                    ui.label(
                                        RichText::new("— yalnız bu oturum")
                                            .size(10.0)
                                            .color(Color32::GRAY),
                                    );
                                }
                            });
                        }
                    }

                    ui.add_space(8.0);
                    anahtar_formu(ui, ozet, form);
                });

                ui.add_space(8.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(
                        RichText::new("Kervan'da henüz yok — DALGA-4: Webhooks, Profil & API Manager Belirteçleri")
                            .size(11.0)
                            .color(Color32::from_rgb(180, 80, 50)),
                    );
                });
            });
        });
    });
}

/// Seçili hedefi listeye göre düzeltir.
///
/// Liste boşsa boş döner; seçim listede yoksa ilk hedefe düşer. Ayrı bir
/// işlev olmasının nedeni sınanabilirlik: form çizimi olmadan da bu karar
/// doğrulanabilsin.
fn hedef_sec(adlar: &[&str], secili: &str) -> String {
    if adlar.is_empty() {
        return String::new();
    }
    if adlar.contains(&secili) {
        return secili.to_string();
    }
    adlar[0].to_string()
}

/// Anahtar ekleme/kaldırma formu.
///
/// Tek bir hedef adı seçilir ve değer girilir; ilk `Ekle` basışında eylem
/// `form.bekleyen`e konur ve GUI onu uygular. Başarılı eklemede değer
/// temizlenir — gizli değerin ekranda kalması, ekran görüntüsüne girmesi
/// demektir.
fn anahtar_formu(ui: &mut Ui, ozet: &Ozet, form: &mut AnahtarFormu) {
    ui.separator();
    ui.label(RichText::new("Anahtar Ekle").strong().size(12.0));
    ui.label(
        RichText::new("Anahtar yalnız bellekte tutulur; Kervan kapanınca silinir. Diskten okunmaz, hiçbir dosyaya yazılmaz.")
            .size(10.5)
            .color(Color32::GRAY),
    );
    ui.add_space(4.0);

    if ozet.kimlikler.is_empty() {
        ui.label(
            RichText::new("Önce kervan.toml'da bir [rest.*] hedefi tanımlanmalı; anahtarın bağlanacağı ortam değişkeni yok.")
                .size(11.0)
                .color(Color32::GRAY),
        );
        return;
    }

    // Seçili hedef listede yoksa (yeniden yükleme, hedef kaldırma) ilkine düş:
    // boş bir seçimle düğmenin sessizce hiçbir şey yapmaması yanıltıcı olurdu.
    let adlar: Vec<&str> = ozet.kimlikler.iter().map(|k| k.ad.as_str()).collect();
    form.secili_env = hedef_sec(&adlar, &form.secili_env);

    ui.horizontal(|ui| {
        ui.label("Hedef:");
        egui::ComboBox::from_id_salt("ayar_anahtar_hedef")
            .selected_text(form.secili_env.clone())
            .show_ui(ui, |ui| {
                for ad in &adlar {
                    ui.selectable_value(&mut form.secili_env, (*ad).to_string(), *ad);
                }
            });
    });

    ui.horizontal(|ui| {
        ui.label("Değer:");
        ui.add(
            egui::TextEdit::singleline(&mut form.deger)
                .password(true)
                .desired_width(320.0)
                .hint_text("anahtarı yapıştır"),
        );
    });

    let hazir = !form.secili_env.trim().is_empty() && !form.deger.trim().is_empty();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(hazir, egui::Button::new("Anahtarı Ekle"))
            .on_disabled_hover_text("hedef ve değer dolu olmalı")
            .clicked()
        {
            form.bekleyen = Some(AnahtarEylemi::Ekle {
                ad: form.secili_env.trim().to_string(),
                deger: form.deger.trim().to_string(),
            });
            form.deger.clear();
        }

        // Kaldırma yalnız bu oturumda girilmiş bir anahtar varsa anlamlı:
        // ortam değişkeninden gelen bir anahtar buradan silinemez.
        let oturumdaki = ozet
            .kimlikler
            .iter()
            .find(|k| k.ad == form.secili_env)
            .is_some_and(|k| k.oturumda);
        if ui
            .add_enabled(oturumdaki, egui::Button::new("Oturum Anahtarını Kaldır"))
            .on_disabled_hover_text("bu hedef için oturumda girilmiş anahtar yok")
            .clicked()
        {
            form.bekleyen = Some(AnahtarEylemi::Sil {
                ad: form.secili_env.trim().to_string(),
            });
        }
    });

    if let Some((mesaj, basarili)) = &form.mesaj {
        ui.label(RichText::new(mesaj).size(11.0).color(if *basarili {
            Color32::from_rgb(46, 125, 79)
        } else {
            Color32::from_rgb(192, 79, 66)
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secili_hedef_listede_kalirsa_korunur() {
        assert_eq!(
            hedef_sec(&["A", "B", "C"], "B"),
            "B",
            "kullanıcının seçimi kendiliğinden değişmemeli"
        );
    }

    #[test]
    fn listede_olmayan_secim_ilkine_duser() {
        // Hedef kervan.toml'dan kaldırıldığında form boş seçimle kalırsa
        // "Ekle" düğmesi sessizce hiçbir şey yapmazdı.
        assert_eq!(hedef_sec(&["A", "B"], "SILINMIS"), "A");
        assert_eq!(hedef_sec(&["A", "B"], ""), "A");
    }

    #[test]
    fn bos_liste_bos_secim() {
        // Kimlik yoksa form zaten çizilmiyor; yine de paniklememeli.
        assert_eq!(hedef_sec(&[], "X"), "");
    }
}
