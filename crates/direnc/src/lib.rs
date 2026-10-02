//! Kervan 3 Katmanlı Dayanıklılık (Resilience) Kütüphanesi
//!
//! 1. Devre Kesici (Circuit Breaker: CLOSED → DEGRADED → OPEN → HALF_OPEN → CLOSED)
//! 2. Bağlantı ve Sağlayıcı Soğuma (Cooldown / Exponential Backoff / Retry-After)
//! 3. Model Kilidi (Model Lockout / Exclusion)

pub mod devre_kesici;
pub mod kilit;
pub mod soguma;

pub use devre_kesici::{DevreKesici, DevreKesiciYapilandirma, DirencHatasi, Durum};
pub use kilit::ModelKilidi;
pub use soguma::SogumaTakipcisi;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_devre_kesici_temel_akisi() {
        let ayar = DevreKesiciYapilandirma {
            hata_esigi: 3,
            bozulma_esigi: 2,
            soguma_suresi: Duration::from_millis(50),
            yari_acik_izin: 1,
        };

        let mut kesici = DevreKesici::yeni("openai-test", Some(ayar));
        assert_eq!(kesici.durum(), Durum::Kapali);
        assert!(kesici.istek_izni_al().is_ok());

        // 1. hata -> Kapalı kalır
        kesici.hata_kaydet();
        assert_eq!(kesici.durum(), Durum::Kapali);

        // 2. hata -> Bozulmuş (DEGRADED) durumuna geçer
        kesici.hata_kaydet();
        assert_eq!(kesici.durum(), Durum::Bozulmus);
        assert!(kesici.istek_izni_al().is_ok());

        // 3. hata -> Devre AÇILIR (OPEN)
        kesici.hata_kaydet();
        assert_eq!(kesici.durum(), Durum::Acik);

        // İstek reddedilmeli
        match kesici.istek_izni_al() {
            Err(DirencHatasi::DevreAcik { ad, .. }) => assert_eq!(ad, "openai-test"),
            _ => panic!("Açık devrede istek reddedilmeliydi"),
        }

        // Soğuma süresi kadar bekleyelim (55ms)
        std::thread::sleep(Duration::from_millis(55));

        // Soğuma bitince Yarı Açık (HALF_OPEN) olmalı
        assert_eq!(kesici.durum(), Durum::YariAcik);

        // İlk deneme izni başarılı olmalı
        assert!(kesici.istek_izni_al().is_ok());
        // İkinci istek (yarı açık limit 1 olduğu için) reddedilmeli
        match kesici.istek_izni_al() {
            Err(DirencHatasi::YariAcikDolu(_)) => {}
            _ => panic!("Yarı açık limit dolunca reddedilmeli"),
        }

        // Başarılı deneme sonrası devre tekrar KAPALI olmalı
        kesici.basari_kaydet();
        assert_eq!(kesici.durum(), Durum::Kapali);
    }

    #[test]
    fn test_devre_kesici_yari_acikta_hata_alinca_tekrar_acilir() {
        let ayar = DevreKesiciYapilandirma {
            hata_esigi: 1,
            bozulma_esigi: 1,
            soguma_suresi: Duration::from_millis(20),
            yari_acik_izin: 1,
        };

        let mut kesici = DevreKesici::yeni("gemini-test", Some(ayar));
        kesici.hata_kaydet();
        assert_eq!(kesici.durum(), Durum::Acik);

        std::thread::sleep(Duration::from_millis(25));
        assert_eq!(kesici.durum(), Durum::YariAcik);

        // Yarı açıkken tekrar hata alırsa derhal AÇIK duruma dönmeli
        kesici.hata_kaydet();
        assert_eq!(kesici.durum(), Durum::Acik);
    }

    #[test]
    fn test_soguma_takipcisi_ve_ustel_geri_cekilme() {
        let mut soguma = SogumaTakipcisi::yeni(Duration::from_millis(100), Duration::from_secs(5));
        assert!(!soguma.sogumada_mi("claude-hesap-1"));

        // 1. hata (100ms)
        let sure1 = soguma.soguma_baslat("claude-hesap-1", Some("429"), None);
        assert_eq!(sure1, Duration::from_millis(100));
        assert!(soguma.sogumada_mi("claude-hesap-1"));

        // 2. hata (üstel: 200ms)
        let sure2 = soguma.soguma_baslat("claude-hesap-1", Some("429"), None);
        assert_eq!(sure2, Duration::from_millis(200));

        // Başarı gelince sıfırlanmalı
        soguma.basari_bildir("claude-hesap-1");
        assert!(!soguma.sogumada_mi("claude-hesap-1"));
    }

    #[test]
    fn test_soguma_takipcisi_retry_after() {
        let mut soguma = SogumaTakipcisi::default();
        let sure = soguma.soguma_baslat("openai-hesap-2", Some("429"), Some(15));
        assert_eq!(sure, Duration::from_secs(15));
        assert!(soguma.sogumada_mi("openai-hesap-2"));
    }

    #[test]
    fn test_model_kilidi() {
        let mut kilit = ModelKilidi::yeni();
        let anahtar = "deepseek::model-xyz";

        assert!(!kilit.kilitli_mi(anahtar));

        // Geçici kilit (50ms)
        kilit.gecici_kilitle(
            anahtar,
            "model-xyz",
            Duration::from_millis(50),
            "404 Not Found",
        );
        assert!(kilit.kilitli_mi(anahtar));

        std::thread::sleep(Duration::from_millis(60));
        assert!(!kilit.kilitli_mi(anahtar));

        // Kalıcı kilit
        kilit.kalici_kilitle(anahtar, "model-xyz", "Model kaldırıldı");
        assert!(kilit.kilitli_mi(anahtar));

        // Kilidi aç
        kilit.kilidi_ac(anahtar);
        assert!(!kilit.kilitli_mi(anahtar));
    }
}
