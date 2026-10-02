use crate::trait_tanimi::{KervanPaneli, PanelDurumu};
use eframe::egui;

/// Güvenlik ve Koruma Paneli (Sert Kural #20)
#[derive(Debug, Default)]
pub struct GuvenlikPaneli;

impl KervanPaneli for GuvenlikPaneli {
    fn kimlik(&self) -> &'static str {
        "guvenlik"
    }

    fn baslik(&self) -> &'static str {
        "Güvenlik & Kalkan"
    }

    fn simge(&self) -> &'static str {
        "🛡"
    }

    fn ciz(&mut self, ui: &mut egui::Ui, durum: &mut PanelDurumu) {
        ui.heading("🛡 Güvenlik Kalkanı ve Kurallar");
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.heading("OmniRoute Sert Kural #20: PII Maskeleme");
            ui.label(
                "Kişisel Verilerin (PII) otomatik yeniden yazımı varsayılan olarak KAPALIDIR.",
            );
            ui.checkbox(
                &mut durum.pii_acik_mi,
                "PII Maskelemeyi Etkinleştir (Opt-in)",
            );
            if durum.pii_acik_mi {
                ui.colored_label(
                    egui::Color32::from_rgb(0, 180, 0),
                    "✓ PII maskeleme aktif: E-posta ve telefonlar filtreleniyor.",
                );
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(180, 100, 0),
                    "⚠ PII maskeleme pasif: Orijinal metin korunuyor.",
                );
            }
        });

        ui.add_space(8.0);

        ui.group(|ui| {
            ui.heading("Kimlik Bilgisi ve API Anahtarı Koruması");
            ui.label("Tüm `sk-...` API anahtarları günlüklerde ve istemci çıktılarında [REDACTED_API_KEY] olarak maskelenir.");
            ui.colored_label(egui::Color32::from_rgb(0, 180, 0), "✓ Daima Aktif (Kapatılamaz)");
        });

        ui.add_space(8.0);

        ui.group(|ui| {
            ui.heading("İstem Enjeksiyonu ve Jailbreak Koruması");
            ui.label(
                "Sistem talimatlarını atlatmaya çalışan girdiler geçit seviyesinde reddedilir.",
            );
            ui.colored_label(egui::Color32::from_rgb(0, 180, 0), "✓ Aktif");
        });
    }
}
