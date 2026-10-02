//! Field-Level Encryption — AES-256-GCM
//!
//! OmniRoute `src/lib/db/encryption.ts` mimarisinden saf Rust ile uyarlanmıştır.
//!
//! Format: `enc:v1:<iv_hex>:<ciphertext_hex>:<authTag_hex>`
//! Anahtar Türetme: scrypt (salt: "omniroute-field-encryption-v1", key_length: 32)
//! Şifreleme: AES-256-GCM (16 bayt IV, 16 bayt Auth Tag)

use aes_gcm::AesGcm;
use aes_gcm::aead::generic_array::typenum::U16;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::Aes256;
use rand::Rng;

pub const ON_EK: &str = "enc:v1:";
pub const SABIT_TUZ: &[u8] = b"omniroute-field-encryption-v1";

/// 16-bayt IV (Nonce) ve 16-bayt Auth Tag kullanan AES-256-GCM tipi
pub type Aes256Gcm16 = AesGcm<Aes256, U16>;

#[derive(Debug, Clone)]
pub struct Sifreleyici {
    anahtar: Option<[u8; 32]>,
}

impl Sifreleyici {
    /// Verilen gizli anahtardan (`STORAGE_ENCRYPTION_KEY`) scrypt ile 32 baytlık anahtar türetir.
    pub fn yeni(gizli_anahtar: Option<&str>) -> Self {
        let anahtar = gizli_anahtar.and_then(|sir| {
            let sir = sir.trim();
            if sir.is_empty() {
                return None;
            }
            let mut derived_key = [0u8; 32];
            let params = scrypt::Params::new(14, 8, 1, 32).ok()?; // N=16384, r=8, p=1
            scrypt::scrypt(sir.as_bytes(), SABIT_TUZ, &params, &mut derived_key).ok()?;
            Some(derived_key)
        });

        Self { anahtar }
    }

    /// Passthrough (şifrelemesiz) modda mı?
    pub fn passthrough_mu(&self) -> bool {
        self.anahtar.is_none()
    }

    /// Değerin şifreli olup olmadığını kontrol eder (`enc:v1:` ile başlıyor mu?)
    pub fn sifreli_mi(deger: &str) -> bool {
        deger.starts_with(ON_EK)
    }

    /// Düz metni şifreler. Anahtar yoksa düz metni olduğu gibi döner.
    pub fn sifrele(&self, duz_metin: &str) -> Result<String, String> {
        let key = match &self.anahtar {
            Some(k) => k,
            None => return Ok(duz_metin.to_string()),
        };

        if Self::sifreli_mi(duz_metin) {
            return Ok(duz_metin.to_string());
        }

        let cipher =
            Aes256Gcm16::new_from_slice(key).map_err(|e| format!("Cipher başlatılamadı: {}", e))?;

        let mut iv = [0u8; 16];
        rand::rng().fill_bytes(&mut iv);

        let nonce = aes_gcm::aead::generic_array::GenericArray::from_slice(&iv);
        let ciphertext_with_tag = cipher
            .encrypt(nonce, duz_metin.as_bytes())
            .map_err(|e| format!("Şifreleme hatası: {}", e))?;

        if ciphertext_with_tag.len() < 16 {
            return Err("Geçersiz şifreleme çıktısı".to_string());
        }

        let ayrac = ciphertext_with_tag.len() - 16;
        let ciphertext = &ciphertext_with_tag[..ayrac];
        let auth_tag = &ciphertext_with_tag[ayrac..];

        let iv_hex = hex::encode(iv);
        let ciphertext_hex = hex::encode(ciphertext);
        let tag_hex = hex::encode(auth_tag);

        Ok(format!(
            "{}{}:{}:{}",
            ON_EK, iv_hex, ciphertext_hex, tag_hex
        ))
    }

    /// Şifreli metni çözer. Şifreli değilse veya anahtar yoksa olduğu gibi döner.
    pub fn coz(&self, sifreli_metin: &str) -> Result<String, String> {
        if !Self::sifreli_mi(sifreli_metin) {
            return Ok(sifreli_metin.to_string());
        }

        let key = match &self.anahtar {
            Some(k) => k,
            None => return Err("STORAGE_ENCRYPTION_KEY tanımlı değil, çözülemez".to_string()),
        };

        let govde = &sifreli_metin[ON_EK.len()..];
        let parcalar: Vec<&str> = govde.split(':').collect();
        if parcalar.len() != 3 {
            return Err("Bozuk şifreli metin formatı".to_string());
        }

        let iv = hex::decode(parcalar[0]).map_err(|e| format!("Geçersiz IV hex: {}", e))?;
        let ciphertext =
            hex::decode(parcalar[1]).map_err(|e| format!("Geçersiz ciphertext hex: {}", e))?;
        let auth_tag = hex::decode(parcalar[2]).map_err(|e| format!("Geçersiz tag hex: {}", e))?;

        if iv.len() != 16 || auth_tag.len() != 16 {
            return Err("Geçersiz IV veya tag uzunluğu".to_string());
        }

        let mut birlestirilmis = ciphertext;
        birlestirilmis.extend_from_slice(&auth_tag);

        let cipher =
            Aes256Gcm16::new_from_slice(key).map_err(|e| format!("Cipher başlatılamadı: {}", e))?;

        let nonce = aes_gcm::aead::generic_array::GenericArray::from_slice(&iv);
        let duz_baytlar = cipher
            .decrypt(nonce, birlestirilmis.as_ref())
            .map_err(|e| format!("Çözümleme başarısız (Auth tag doğrulanamadı): {}", e))?;

        String::from_utf8(duz_baytlar).map_err(|e| format!("UTF-8 ayrıştırma hatası: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sifrele_ve_coz_dongusu() {
        let gizli = "test-secret-key-32-bytes-long-kervan!";
        let sifreleyici = Sifreleyici::yeni(Some(gizli));

        let orijinal = "sk-ant-api03-kervan-ve-omniroute-parite-testi-12345";
        let sifreli = sifreleyici
            .sifrele(orijinal)
            .expect("Şifreleme başarılı olmalı");

        assert!(sifreli.starts_with(ON_EK));
        let cozulmus = sifreleyici
            .coz(&sifreli)
            .expect("Çözümleme başarılı olmalı");
        assert_eq!(cozulmus, orijinal);
    }

    #[test]
    fn test_omniroute_test_vektoru_cozumu() {
        // Bun ile üretilen kanonik test vektörü
        let gizli = "test-secret-key-32-bytes-long-kervan!";
        let sifreleyici = Sifreleyici::yeni(Some(gizli));

        let omniroute_ciphertext = "enc:v1:250c7e92f084c15706bbb60099f4d359:dad9f6975dcf2fbc20320412faf6c843c2a1af067cb4b5c9978850e2c4aa43e1651c83e15c4b2b0a99832015841a481d974e1d:1f1a3285c8179f253c5b16fef578b711";
        let cozulmus = sifreleyici
            .coz(omniroute_ciphertext)
            .expect("OmniRoute vektörü çözülebilmeli");

        assert_eq!(
            cozulmus,
            "sk-ant-api03-kervan-ve-omniroute-parite-testi-12345"
        );
    }

    #[test]
    fn test_passthrough_modu() {
        let sifreleyici = Sifreleyici::yeni(None);
        assert!(sifreleyici.passthrough_mu());

        let metin = "acik-metin-anahtar";
        let sonuc = sifreleyici.sifrele(metin).unwrap();
        assert_eq!(sonuc, metin);
    }
}
