//! Kervan Çift Yönlü Araç / Fonksiyon Çağrısı ve Ajan Köprüsü Motoru.
//!
//! Bu modül, OmniRoute'un `translator/helpers`, `toolCallRepair` ve `agentRouterProtocol`
//! yeteneklerini saf Rust olarak Kervan'a kazandırır.
//!
//! GÖREVİ: Web arayüzlerinde (Gemini, Mistral, ChatGPT) koşan modellerin yerel dosyaları
//! okuyup yazabilmesi için ürettiği araç çağrılarını (tool-calls) ayrıştırmak, standartlaştırmak
//! ve Maşa Döngüsü (Agent Loop) için sisteme geri bildirmek.

use serde::{Deserialize, Serialize};

/// Ayrıştırılan araç çağrısı türleri
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AracCagrisi {
    /// Yerel dosya okuma isteği: `[KERVAN_OKU: c:\yol\dosya.docx]`
    DosyaOku { dosya_yolu: String },
    /// Yerel dosya yazma/güncelleme isteği: `[KERVAN_YAZ: c:\yol\dosya.docx] ... [/KERVAN_YAZ]`
    DosyaYaz { dosya_yolu: String, icerik: String },
    /// Yerel çalışma dizinini listeleme isteği: `[KERVAN_DIZIN: c:\yol]`
    DizinListele { dizin_yolu: String },
    /// Standart JSON biçimli fonksiyon çağrısı
    GenelFonksiyon {
        ad: String,
        parametreler_json: String,
    },
}

/// JSON fonksiyon çağrısı şeması (OpenAI / Gemini uyumlu)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Ham model yanıtından araç çağrılarını ve temiz metni ayıklar
pub struct AyiklamaSonucu {
    pub temiz_metin: String,
    pub cagrilar: Vec<AracCagrisi>,
}

/// Web modellerinin ürettiği metinleri tarayan ayıklayıcı
pub struct AracCagrisiAyiklayici;

impl AracCagrisiAyiklayici {
    /// Ham model çıktısını analiz eder
    pub fn ayikla(ham_yanit: &str) -> AyiklamaSonucu {
        let mut cagrilar = Vec::new();
        let mut temiz_metin = ham_yanit.to_string();

        // 1. [KERVAN_YAZ: yol] icerik [/KERVAN_YAZ] kalıbını tara
        while let Some(baslangic) = temiz_metin.find("[KERVAN_YAZ:") {
            if let Some(yol_sonu) = temiz_metin[baslangic..].find(']') {
                let mutlak_yol_sonu = baslangic + yol_sonu;
                let dosya_yolu = temiz_metin[baslangic + 12..mutlak_yol_sonu]
                    .trim()
                    .to_string();

                if let Some(bitis) = temiz_metin[mutlak_yol_sonu..].find("[/KERVAN_YAZ]") {
                    let mutlak_bitis = mutlak_yol_sonu + bitis;
                    let icerik = temiz_metin[mutlak_yol_sonu + 1..mutlak_bitis]
                        .trim()
                        .to_string();

                    cagrilar.push(AracCagrisi::DosyaYaz { dosya_yolu, icerik });

                    // Blok temiz metinden çıkarılır
                    let son_nokta = mutlak_bitis + 14;
                    temiz_metin.replace_range(baslangic..son_nokta, "[Dosya Yazıldı]");
                    continue;
                }
            }
            break;
        }

        // 2. [KERVAN_OKU: yol] kalıbını tara
        while let Some(baslangic) = temiz_metin.find("[KERVAN_OKU:") {
            if let Some(bitis) = temiz_metin[baslangic..].find(']') {
                let mutlak_bitis = baslangic + bitis;
                let dosya_yolu = temiz_metin[baslangic + 12..mutlak_bitis].trim().to_string();

                cagrilar.push(AracCagrisi::DosyaOku { dosya_yolu });
                temiz_metin.replace_range(baslangic..=mutlak_bitis, "[Dosya Okuma İsteği]");
            } else {
                break;
            }
        }

        // 3. [KERVAN_DIZIN: yol] kalıbını tara
        while let Some(baslangic) = temiz_metin.find("[KERVAN_DIZIN:") {
            if let Some(bitis) = temiz_metin[baslangic..].find(']') {
                let mutlak_bitis = baslangic + bitis;
                let dizin_yolu = temiz_metin[baslangic + 14..mutlak_bitis].trim().to_string();

                cagrilar.push(AracCagrisi::DizinListele { dizin_yolu });
                temiz_metin.replace_range(baslangic..=mutlak_bitis, "[Dizin Listeleme İsteği]");
            } else {
                break;
            }
        }

        // 4. Standart ```json { "name": ... } kod bloklarında araç çağrısı arama
        // Kapanış açılıştan SONRA aranır (eskiden açılışın kendisini bulup panikliyordu).
        // Araç çağrısı olan blok metinden çıkarılır; olmayan blok (sıradan JSON) kalır.
        let mut ara = 0;
        while let Some(b) = temiz_metin[ara..].find("```json").map(|i| ara + i) {
            let Some(s) = temiz_metin[b + 7..].find("```").map(|i| b + 7 + i) else {
                break;
            };
            match serde_json::from_str::<JsonToolCall>(temiz_metin[b + 7..s].trim()) {
                Ok(jtc) => {
                    cagrilar.push(AracCagrisi::GenelFonksiyon {
                        ad: jtc.name,
                        parametreler_json: jtc.arguments.to_string(),
                    });
                    temiz_metin.replace_range(b..s + 3, "");
                    ara = b;
                }
                Err(_) => ara = s + 3,
            }
        }

        AyiklamaSonucu {
            temiz_metin: temiz_metin.trim().to_string(),
            cagrilar,
        }
    }

    /// Araç çağrısının icra edilmesinin ardından modele geri gönderilecek Maşa Döngüsü yanıtını üretir
    pub fn sistem_geri_bildirimi_uret(cagri: &AracCagrisi, sonuc_metni: &str) -> String {
        match cagri {
            AracCagrisi::DosyaOku { dosya_yolu } => {
                format!(
                    "[SİSTEM YANITI — DOSYA OKUNDU: {}]\n{}\n[Şimdi bu içeriğe göre çalışmana devam et.]",
                    dosya_yolu, sonuc_metni
                )
            }
            AracCagrisi::DosyaYaz { dosya_yolu, .. } => {
                format!(
                    "[SİSTEM YANITI — DOSYA YAZILDI: {}]\nDosya başarıyla diske kaydedildi. Sıradaki adıma geçebilirsin.",
                    dosya_yolu
                )
            }
            AracCagrisi::DizinListele { dizin_yolu } => {
                format!(
                    "[SİSTEM YANITI — DİZİN LİSTESİ: {}]\n{}\n[İncelemene devam et.]",
                    dizin_yolu, sonuc_metni
                )
            }
            AracCagrisi::GenelFonksiyon { ad, .. } => {
                format!(
                    "[SİSTEM YANITI — FONKSİYON SONUCU ({}):]\n{}",
                    ad, sonuc_metni
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kervan_oku_ayiklama() {
        let ham = "Tezin ilgili bölümünü incelemem gerekiyor. [KERVAN_OKU: c:\\tez\\bolum1.docx] Lütfen bekleyin.";
        let res = AracCagrisiAyiklayici::ayikla(ham);

        assert_eq!(res.cagrilar.len(), 1);
        match &res.cagrilar[0] {
            AracCagrisi::DosyaOku { dosya_yolu } => {
                assert_eq!(dosya_yolu, "c:\\tez\\bolum1.docx");
            }
            _ => panic!("DosyaOku çağrısı bekleniyordu"),
        }
        assert!(res.temiz_metin.contains("[Dosya Okuma İsteği]"));
    }

    #[test]
    fn test_kervan_yaz_ayiklama() {
        let ham = "Düzeltilmiş metin aşağıdadır:\n[KERVAN_YAZ: c:\\tez\\cikti.md]\n# Giriş\nBu onaylı metindir.\n[/KERVAN_YAZ]\nİşlem tamamlandı.";
        let res = AracCagrisiAyiklayici::ayikla(ham);

        assert_eq!(res.cagrilar.len(), 1);
        match &res.cagrilar[0] {
            AracCagrisi::DosyaYaz { dosya_yolu, icerik } => {
                assert_eq!(dosya_yolu, "c:\\tez\\cikti.md");
                assert_eq!(icerik, "# Giriş\nBu onaylı metindir.");
            }
            _ => panic!("DosyaYaz çağrısı bekleniyordu"),
        }
    }

    #[test]
    fn test_sistem_geri_bildirimi() {
        let cagri = AracCagrisi::DosyaOku {
            dosya_yolu: "weber.docx".to_string(),
        };
        let bildirim =
            AracCagrisiAyiklayici::sistem_geri_bildirimi_uret(&cagri, "Örnek tez metni...");
        assert!(bildirim.contains("[SİSTEM YANITI — DOSYA OKUNDU: weber.docx]"));
        assert!(bildirim.contains("Örnek tez metni..."));
    }

    /// Eskiden kapanış açılışın kendisinde bulunuyor, dilim tersine dönüp panikliyordu.
    #[test]
    fn json_blogu_ayiklanir_ve_metinden_cikar() {
        let ham = "Bakıyorum.\n```json\n{\"name\": \"hava\", \"arguments\": {\"il\": \"İzmir\"}}\n```\nSonra\n```json\n[1, 2]\n```";
        let res = AracCagrisiAyiklayici::ayikla(ham);
        assert_eq!(
            res.cagrilar,
            vec![AracCagrisi::GenelFonksiyon {
                ad: "hava".into(),
                parametreler_json: r#"{"il":"İzmir"}"#.into()
            }]
        );
        assert!(!res.temiz_metin.contains("hava"), "{}", res.temiz_metin);
        assert!(
            res.temiz_metin.contains("[1, 2]"),
            "araç olmayan JSON kalmalı"
        );
    }
}
