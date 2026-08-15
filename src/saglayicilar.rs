//! Saglayici tanimlari ve kayit defteri.
//!
//! SECICILER KODA GOMULU DEGIL: buradakiler yalnizca VARSAYILAN. Site arayuzu
//! degistiginde ya da bir secici yanlis ciktiginda `kervan.toml` duzenlenir, yeniden
//! derleme gerekmez. Planin "seciciler kodda degil yapilandirmada" maddesi bu.
//!
//! DOGRULUK NOTU — bilerek acik: gemini secicileri ORNEK UZERINDE OLCULDU. claude ve
//! perplexity secicileri OLCULMEDI; bu sitelerde hesap acmadan DOM'a bakilamiyor.
//! Asagidakiler makul baslangic degerleri, kesin degil. Dogru degerleri bulmanin yolu
//! tahmin degil olcum:
//!
//!     kervan <hesap> --saglayici claude --headless --dom
//!
//! "secici adaylari" bolumu sayfada ne oldugunu sayar; dogru olani `kervan.toml`'a
//! yazarsin. Yanlis secici sessizce yanlis yanit DONDURMEZ — `ask` istem kutusunu
//! bulamazsa gurultulu hata verir.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Result};

use crate::browser::{Provider, Selectors};
use crate::config::SaglayiciAyar;

fn p(ad: &str, url: &str, giris: &str, istem: &str, giris_isareti: &str, yanit: &str) -> Provider {
    Provider {
        ad: ad.to_string(),
        url: url.to_string(),
        giris_kalibi: giris.to_string(),
        selectors: Selectors {
            prompt_input: istem.to_string(),
            login_hint: giris_isareti.to_string(),
            response: yanit.to_string(),
        },
    }
}

/// Yerlesik varsayilanlar.
pub fn varsayilanlar() -> Vec<Provider> {
    vec![
        // OLCULDU: istem kutusu sarmalayici `rich-textarea` degil icindeki
        // contenteditable div. `ServiceLogin` bagi yalniz cikis yapilmisken var
        // (girisli 0, cikisli 1); `accounts.google.com` ayirt etmiyor.
        // AccountChooser adresi eposta alanini on-dolduruyor.
        p(
            "gemini",
            "https://gemini.google.com/app",
            "https://accounts.google.com/AccountChooser?Email={eposta}\
             &continue=https://gemini.google.com/app",
            "div[contenteditable='true']",
            "a[href*='ServiceLogin']",
            "model-response, message-content",
        ),
        // KISMEN OLCULDU (cikis yapilmis halde). `/new` cikisliyken `/logout`'a
        // dusuyordu, o yuzden taban adres kullaniliyor; girisliyken yeni sohbet
        // zaten aciliyor. Giris isareti ve yanit secicisi DOGRULANMADI — girisli
        // bir oturumda `--dom` ile olculmeli. Giris adresi eposta on-doldurmayi
        // desteklemiyor; duz giris sayfasi veriliyor.
        p(
            "claude",
            "https://claude.ai/",
            "https://claude.ai/login",
            "div[contenteditable='true']",
            "a[href*='/login'], button[data-testid='login-button']",
            ".font-claude-message, [data-testid='conversation-turn']",
        ),
        // KISMEN OLCULDU: istem alani `textarea` DEGIL contenteditable (olcumde
        // textarea 0, contenteditable 1). Giris isareti ve yanit secicisi
        // DOGRULANMADI. Ayrica perplexity girissiz de calisiyor, yani "girisli mi"
        // sorusu burada dogal olarak bulanik — hesap basina kota takibi icin girisin
        // dogrulanmasi gerekir.
        p(
            "perplexity",
            "https://www.perplexity.ai/",
            "https://www.perplexity.ai/",
            "div[contenteditable='true']",
            "a[href*='/api/auth/signin'], button[data-testid='signin-button']",
            ".prose",
        ),
    ]
}

/// Varsayilanlari yapilandirmadaki degerlerle ezerek kayit defterini kurar.
pub fn kayit_defteri(ayarlar: &HashMap<String, SaglayiciAyar>) -> HashMap<String, Arc<Provider>> {
    let mut d: HashMap<String, Arc<Provider>> = HashMap::new();
    for mut sag in varsayilanlar() {
        if let Some(a) = ayarlar.get(&sag.ad) {
            if let Some(v) = &a.url {
                sag.url = v.clone();
            }
            if let Some(v) = &a.giris_kalibi {
                sag.giris_kalibi = v.clone();
            }
            if let Some(v) = &a.prompt_input {
                sag.selectors.prompt_input = v.clone();
            }
            if let Some(v) = &a.login_hint {
                sag.selectors.login_hint = v.clone();
            }
            if let Some(v) = &a.response {
                sag.selectors.response = v.clone();
            }
        }
        d.insert(sag.ad.clone(), Arc::new(sag));
    }
    d
}

pub fn bul(defter: &HashMap<String, Arc<Provider>>, ad: &str) -> Result<Arc<Provider>> {
    match defter.get(ad) {
        Some(s) => Ok(s.clone()),
        None => {
            let mut adlar: Vec<&str> = defter.keys().map(|s| s.as_str()).collect();
            adlar.sort_unstable();
            bail!("bilinmeyen saglayici '{ad}'. Taninanlar: {}", adlar.join(", "))
        }
    }
}

/// Istenen model kimliginden saglayiciyi secer.
///
/// Esitlik degil ICERME araniyor, cunku model kimlikleri surum eki tasiyor
/// (`kervan/gemini-3.1-pro`). Boylece yeni saglayicilar eklenirken var olan
/// yapilandirmalar — ornegin Hermes'inki — kirilmiyor.
pub fn model_saglayicisi(model: &str, defter: &HashMap<String, Arc<Provider>>) -> Option<String> {
    let m = model.to_lowercase();
    let mut eslesen: Vec<&String> = defter.keys().filter(|ad| m.contains(ad.as_str())).collect();
    // En uzun ad kazansin: ileride "claude" ve "claude-code" gibi adlar olursa
    // kisa olan uzunu calmasin.
    eslesen.sort_by_key(|a| std::cmp::Reverse(a.len()));
    eslesen.first().map(|s| (*s).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defter() -> HashMap<String, Arc<Provider>> {
        kayit_defteri(&HashMap::new())
    }

    /// Hermes'in yapilandirmasindaki eski kimlik calismaya devam etmeli; yeni
    /// saglayici eklemek var olan kurulumu kirmamali.
    #[test]
    fn surum_ekli_model_kimligi_eslesir() {
        let d = defter();
        assert_eq!(model_saglayicisi("kervan/gemini-3.1-pro", &d).as_deref(), Some("gemini"));
        assert_eq!(model_saglayicisi("kervan/claude", &d).as_deref(), Some("claude"));
        assert_eq!(model_saglayicisi("kervan/perplexity", &d).as_deref(), Some("perplexity"));
        assert_eq!(model_saglayicisi("bilinmeyen", &d), None);
    }

    /// Yapilandirma secicileri ezebilmeli — site arayuzu degisince yeniden derleme
    /// degil tek satir duzenleme yetsin.
    #[test]
    fn yapilandirma_seciciyi_ezer() {
        let mut a = HashMap::new();
        a.insert(
            "claude".to_string(),
            SaglayiciAyar { response: Some(".yeni-secici".into()), ..Default::default() },
        );
        let d = kayit_defteri(&a);
        assert_eq!(d["claude"].selectors.response, ".yeni-secici");
        // ezilmeyen alanlar varsayilanda kalmali
        assert!(d["claude"].url.contains("claude.ai"));
        // olculdu: perplexity'de textarea yok, contenteditable var
        assert_eq!(d["perplexity"].selectors.prompt_input, "div[contenteditable='true']");
        // baska saglayici etkilenmemeli
        assert_eq!(d["gemini"].selectors.response, "model-response, message-content");
    }

    /// Bilinmeyen saglayici SESSIZCE varsayilana dusmemeli; yanlis hesabi yanlis
    /// siteye baglamak en kotu hata bicimi olurdu.
    #[test]
    fn bilinmeyen_saglayici_acik_hata_verir() {
        let e = bul(&defter(), "yokboyle").unwrap_err().to_string();
        assert!(e.contains("bilinmeyen saglayici"), "{e}");
        assert!(e.contains("claude"), "taninanlar listelenmeli: {e}");
    }
}
