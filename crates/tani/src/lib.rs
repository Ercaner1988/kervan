//! # kervan-tani
//! Kervan tanı ve yetenek algılama altyapısı.
//! PRD: KERVAN-TANI-ISTEMI.md (G1-G4, FR-1...FR-7).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Tanı sınıfları (katmanlı, fail-closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sinif {
    OturumDusto,
    DogrulamaDuvari,
    Cloudflare,
    Kota,
    AgHatasi,
    SeciciKirildi,
    Yuklenmedi,
    Bilinmeyen,
}

impl Sinif {
    pub fn etiket(self) -> &'static str {
        match self {
            Self::OturumDusto => "oturum_dustu",
            Self::DogrulamaDuvari => "dogrulama_duvari",
            Self::Cloudflare => "cloudflare",
            Self::Kota => "kota",
            Self::AgHatasi => "ag_hatasi",
            Self::SeciciKirildi => "secici_kirildi",
            Self::Yuklenmedi => "yuklenmedi",
            Self::Bilinmeyen => "bilinmeyen",
        }
    }
}

/// Sınıflandırma güven derecesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guven {
    Kural,
    Ocr,
    Model,
}

/// Kanıt kaydı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kanit {
    pub tur: String,
    pub metin: String,
}

/// Çözüm önerisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Oneri {
    pub eylem: String,
    pub ayrinti: String,
}

/// Olay başına üretilen tam Tanı Kartı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaniKarti {
    pub zaman: String,
    pub hesap: String,
    pub saglayici: String,
    pub tetik: String,
    pub sinif: Sinif,
    pub guven: Guven,
    pub kanit: Vec<Kanit>,
    pub goruntu: Option<PathBuf>,
    pub oneri: Oneri,
}

/// Sağlayıcı tekil yetenek kaydı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Yetenek {
    pub ad: String,
    pub var: bool,
    pub kanit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degerler: Vec<String>,
}

/// Sağlayıcı ilk bağlantı Yetenek Kartı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YetenekKarti {
    pub saglayici: String,
    pub zaman: String,
    pub yetenekler: Vec<Yetenek>,
}

/// Kural tanımı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kural {
    pub sinif: String,
    #[serde(default)]
    pub url_desenleri: Vec<String>,
    #[serde(default)]
    pub dom_desenleri: Vec<String>,
    #[serde(default)]
    pub ocr_desenleri: Vec<String>,
    pub eylem: String,
    pub oneri_metni: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaniKurallari {
    pub kural: Vec<Kural>,
}

impl TaniKurallari {
    pub fn toml_metninden(metin: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(metin)
    }

    pub fn varsayilan() -> Self {
        const METIN: &str = include_str!("../tani-kurallari.toml");
        Self::toml_metninden(METIN).expect("tani-kurallari.toml geçerli toml olmalı")
    }
}

/// Saf sınıflandırma işlevi (tarayıcısız, fail-closed).
/// `(dom_ozeti, url, ocr_metni, kurallar) -> (Sinif, Guven, Vec<Kanit>, Oneri)`
pub fn siniflandir(
    dom_ozeti: &str,
    url: &str,
    ocr_metni: Option<&str>,
    kurallar: &TaniKurallari,
) -> (Sinif, Guven, Vec<Kanit>, Oneri) {
    let dom_katlanmis = katla::katla(dom_ozeti);
    let url_katlanmis = katla::katla(url);

    // 1. Önce DOM ve URL tabanlı deterministik kurallar
    for k in &kurallar.kural {
        for u in &k.url_desenleri {
            let u_kat = katla::katla(u);
            if !u_kat.is_empty() && url_katlanmis.contains(&u_kat) {
                let sinif = sinif_coz(&k.sinif);
                let kanit = vec![Kanit {
                    tur: "url".to_string(),
                    metin: format!("URL '{}' desenini içeriyor: {}", u, url),
                }];
                let oneri = Oneri {
                    eylem: k.eylem.clone(),
                    ayrinti: k.oneri_metni.clone(),
                };
                return (sinif, Guven::Kural, kanit, oneri);
            }
        }

        for d in &k.dom_desenleri {
            let d_kat = katla::katla(d);
            if !d_kat.is_empty() && dom_katlanmis.contains(&d_kat) {
                let sinif = sinif_coz(&k.sinif);
                let kanit = vec![Kanit {
                    tur: "dom".to_string(),
                    metin: format!("DOM '{}' desenini içeriyor", d),
                }];
                let oneri = Oneri {
                    eylem: k.eylem.clone(),
                    ayrinti: k.oneri_metni.clone(),
                };
                return (sinif, Guven::Kural, kanit, oneri);
            }
        }
    }

    // 2. DOM'da bulunamadıysa OCR metnini sına
    if let Some(ocr) = ocr_metni {
        let ocr_katlanmis = katla::katla(ocr);
        for k in &kurallar.kural {
            for o in &k.ocr_desenleri {
                let o_kat = katla::katla(o);
                if !o_kat.is_empty() && ocr_katlanmis.contains(&o_kat) {
                    let sinif = sinif_coz(&k.sinif);
                    let kanit = vec![Kanit {
                        tur: "ocr".to_string(),
                        metin: format!("OCR '{}' desenini yakaladı", o),
                    }];
                    let oneri = Oneri {
                        eylem: k.eylem.clone(),
                        ayrinti: k.oneri_metni.clone(),
                    };
                    return (sinif, Guven::Ocr, kanit, oneri);
                }
            }
        }
    }

    // 3. Hiçbir kural eşleşmediyse: fail-closed -> Bilinmeyen
    (
        Sinif::Bilinmeyen,
        Guven::Kural,
        vec![Kanit {
            tur: "kod".to_string(),
            metin: "Hiçbir deterministik kural veya OCR deseni eşleşmedi".to_string(),
        }],
        Oneri {
            eylem: "elle_bak".to_string(),
            ayrinti: "Ekran görüntüsü ve DOM özetini elle inceleyin".to_string(),
        },
    )
}

fn sinif_coz(ad: &str) -> Sinif {
    match ad {
        "oturum_dustu" => Sinif::OturumDusto,
        "dogrulama_duvari" => Sinif::DogrulamaDuvari,
        "cloudflare" => Sinif::Cloudflare,
        "kota" => Sinif::Kota,
        "ag_hatasi" => Sinif::AgHatasi,
        "secici_kirildi" => Sinif::SeciciKirildi,
        "yuklenmedi" => Sinif::Yuklenmedi,
        _ => Sinif::Bilinmeyen,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sekiz_ornek_siniflandirma() {
        let kurallar = TaniKurallari::varsayilan();

        // 1. Cloudflare sınaması
        let (s, g, _, _) = siniflandir(
            "Just a moment... Enable JavaScript to run",
            "https://chatgpt.com",
            None,
            &kurallar,
        );
        assert_eq!(s, Sinif::Cloudflare);
        assert_eq!(g, Guven::Kural);

        // 2. Doğrulama duvarı / Captcha
        let (s, _, _, _) = siniflandir(
            "<div class='g-recaptcha'></div>",
            "https://auth.openai.com/verify",
            None,
            &kurallar,
        );
        assert_eq!(s, Sinif::DogrulamaDuvari);

        // 3. Oturum düştü
        let (s, _, _, _) = siniflandir(
            "Please Sign in to continue",
            "https://accounts.google.com/login",
            None,
            &kurallar,
        );
        assert_eq!(s, Sinif::OturumDusto);

        // 4. Kota sınırı
        let (s, _, _, _) = siniflandir(
            "You've reached the usage limit. Try again in 41 minutes.",
            "https://copilot.microsoft.com",
            None,
            &kurallar,
        );
        assert_eq!(s, Sinif::Kota);

        // 5. Ağ hatası (about:neterror)
        let (s, _, _, _) = siniflandir(
            "Server not found",
            "about:neterror?e=nssFailure",
            None,
            &kurallar,
        );
        assert_eq!(s, Sinif::AgHatasi);

        // 6. Seçici kırıldı
        let (s, _, _, o) = siniflandir(
            "prompt_input: 0 eşleşme, textarea bulunamadı",
            "https://gemini.google.com/app",
            None,
            &kurallar,
        );
        assert_eq!(s, Sinif::SeciciKirildi);
        assert_eq!(o.eylem, "secici_oner");

        // 7. Yüklenmedi / Boş sayfa
        let (s, _, _, _) = siniflandir("<html></html>", "about:blank", None, &kurallar);
        assert_eq!(s, Sinif::Yuklenmedi);

        // 8. Bilinmeyen (Eşleşmeyen arayüz)
        let (s, _, _, o) = siniflandir(
            "Rastgele sıradan bir sayfa metni",
            "https://example.com",
            None,
            &kurallar,
        );
        assert_eq!(s, Sinif::Bilinmeyen);
        assert_eq!(o.eylem, "elle_bak");
    }

    #[test]
    fn test_ocr_katmani_devreye_girer() {
        let kurallar = TaniKurallari::varsayilan();
        // DOM'da hiçbir şey yok ama OCR görseldeki metni yakaladı
        let (s, g, _, _) = siniflandir(
            "<div>Canvas veya resim</div>",
            "https://app.custom.ai",
            Some("Uyarı: Kullanım sınırına ulaşıldı (Rate limit reached)"),
            &kurallar,
        );
        assert_eq!(s, Sinif::Kota);
        assert_eq!(g, Guven::Ocr);
    }
}
