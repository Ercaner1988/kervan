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
//! ```text
//! kervan <hesap> --saglayici claude --headless --dom
//! ```
//!
//! "secici adaylari" bolumu sayfada ne oldugunu sayar; dogru olani `kervan.toml`'a
//! yazarsin. Yanlis secici sessizce yanlis yanit DONDURMEZ — `ask` istem kutusunu
//! bulamazsa gurultulu hata verir.
//!
//! GIRIS ADRESLERI (`giris_kalibi`) 2026-08-16'da tek tek olculdu — `--url ... --dom`
//! ile bos bir profilde acilip nereye dustugune ve sayfada ne oldugua bakildi:
//!
//! | saglayici    | adres                      | sonuc                                  |
//! |--------------|----------------------------|----------------------------------------|
//! | gemini       | AccountChooser?Email=...   | e-posta ON-DOLU (olculdu: identifier)  |
//! | claude       | /login                     | giris sayfasi + login-with-google      |
//! | deepseek     | /sign_in                   | giris formu (password alani var)       |
//! | qwen         | /auth?action=signin        | giris formu (password alani var)       |
//! | copilot-m365 | kok                        | login.microsoftonline.com'a yonleniyor |
//! | z.ai         | /auth                      | DOGRULANAMADI — sayfa cizilmedi (CF)   |
//! | kimi         | —                          | STATIK GIRIS YOLU YOK, modal           |
//! | copilot      | —                          | kok anonim uygulamayi aciyor           |
//!
//! Cikan sonuc: yalnizca Gemini'de e-posta on-doldurma var. kimi ve copilot'ta giris
//! ne baglanti ne de metinli dugme — ikonla acilan bir kalip. Onlarda "giris adresi"
//! diye bir sey uydurmak yerine kok adres birakildi.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, bail};

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
        cloudflare: false,
    }
}

/// Cloudflare arkasinda oldugu OLCULEN siteler icin isaret.
///
/// Ayri bir sarmalayici, `p()`'ye yedinci bir `bool` parametresi eklemekten iyi:
/// cagri yerinde `cf(p("claude", ...))` ne dedigi belli, `p("claude", ..., true)`
/// ise okurken hangi bayrak oldugu belirsiz.
fn cf(mut s: Provider) -> Provider {
    s.cloudflare = true;
    s
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
        // bir oturumda `--dom` ile olculmeli.
        //
        // `/login` OLCULDU: gercek giris sayfasi aciliyor ve sayfada
        // `data-testid='login-with-google'` ile `data-testid='email'` var. Eposta
        // on-doldurma YOK. Duvar kalkinca giris tek tikla Google'a baglanabilir;
        // otomatik tiklatma simdilik yazilmadi, giris zaten hesap basina bir kez.
        cf(p(
            "claude",
            "https://claude.ai/",
            "https://claude.ai/login",
            "div[contenteditable='true']",
            "a[href*='/login'], button[data-testid='login-button']",
            ".font-claude-message, [data-testid='conversation-turn']",
        )),
        // KISMEN OLCULDU: istem alani `textarea` DEGIL contenteditable (olcumde
        // textarea 0, contenteditable 1). Giris isareti ve yanit secicisi
        // DOGRULANMADI. Ayrica perplexity girissiz de calisiyor, yani "girisli mi"
        // sorusu burada dogal olarak bulanik — hesap basina kota takibi icin girisin
        // dogrulanmasi gerekir.
        cf(p(
            "perplexity",
            "https://www.perplexity.ai/",
            "https://www.perplexity.ai/",
            "div[contenteditable='true']",
            "a[href*='/api/auth/signin'], button[data-testid='signin-button']",
            ".prose",
        )),
        // KISMEN OLCULDU (cikis yapilmis profilde). Kisisel Microsoft hesabinin
        // Copilot'u (hotmail/outlook/live) — is/okul hesabininkinden AYRI urun:
        // ayri adres, ayri kota, ayri oturum.
        //
        // Olcum: `textarea` 1, `div[contenteditable]` 0 — istem kutusu textarea.
        // Cikisli sayfada `data-testid='anonymous-block-page-title'` var ve HIC
        // giris BAGLANTISI yok; giris isareti bu yuzden bag degil o baslik. Bu
        // olcumden once detect_login cikisli profile "GIRISLI" diyordu — kutuyu
        // buluyor, giris isaretini bulamiyordu.
        //
        // Girisli oturumda da OLCULDU ve ucdan uca calisti (23x4 -> 132, 4.7 sn):
        //   kutu   : sayfada 2 textarea var, dogrusu `composer-input`. Sirasiz
        //            `textarea` calisiyordu ama yanlis olani ilk gelirse sessizce
        //            bosluga yazardik.
        //   yanit  : `[data-content='ai-message']` tuttu ama metni "Copilot said /
        //            93 / Edit in a page" seklinde kabukla geliyordu; ayni turun
        //            `ai-message-body` cocugu tam olarak "93" veriyor.
        //   `.ac-textBlock` hicbir olcumde eslesmedi, atildi.
        p(
            "copilot",
            "https://copilot.microsoft.com/",
            "https://copilot.microsoft.com/",
            "textarea[data-testid='composer-input']",
            "[data-testid='anonymous-block-page-title']",
            "[data-testid='ai-message-body']",
        ),
        // KISMEN OLCULDU (cikis yapilmis profilde). Is/okul hesabinin Copilot'u.
        //
        // Adi `copilot-m365`: `model_saglayicisi` ICERME ile eslesiyor ve en UZUN
        // ad kazaniyor, boylece "kervan/copilot" kisisele, "kervan/copilot-m365"
        // buraya gidiyor. Ad "m365" olsaydi "kervan/m365-copilot" iki adi da
        // tasiyip yanlis saglayiciya — yani yanlis kotaya — dusebilirdi.
        //
        // Olcum: cikisliyken adres login.microsoftonline.com'a YONLENIYOR, yani
        // sayfada giris BAGI yok, giris FORMU var. Isaret bu yuzden e-posta alani.
        //
        // Girisli oturumda olculdu: `textarea` 0, `div[contenteditable='true']` 0,
        // ama `[contenteditable]` TAM 1 (aria-label "Message Copilot"). Yani kutu
        // div degil ve/veya degeri "true" degil. Nitelik adiyla eslesmek hem dile
        // hem karisik sinif adlarina (`fai-EditorInput__input r18fti29 ...`)
        // bagimsiz; aria-label kullansaydik arayuz dili degisince kirilirdi.
        //
        // Yanit da olculdu (27x3 -> 81). Uc aday vardi ve secim onemliydi:
        //   chatOutput                -> SORUNUN kendisi. Bunu secseydik kendi
        //                                istemimizi cevap sanip geri dondururduk.
        //   copilot-message-reply-div -> "Copilot said:\n81" (kabuklu)
        //   markdown-reply            -> "81" (temiz) <- secilen
        p(
            "copilot-m365",
            "https://m365.cloud.microsoft/chat/",
            "https://m365.cloud.microsoft/chat/",
            "[contenteditable]",
            "input[type='email'], input[name='loginfmt']",
            "[data-testid='markdown-reply']",
        ),
        // Seciciler OLCULMEDI (giris yapilamadigi icin): deepseek ve qwen Cloudflare
        // duvarina takiliyor. Giris ADRESLERI ise olculdu ve dogru: ikisi de gercek
        // giris formu aciyor (`input[type='password']` var). E-posta alani
        // `input[type='email']` DEGIL `input[type='text']`; giris isareti password
        // uzerinden tuttugu icin degistirilmedi.
        cf(p(
            "deepseek",
            "https://chat.deepseek.com/",
            "https://chat.deepseek.com/sign_in",
            "textarea, div[contenteditable='true'], [contenteditable]",
            "input[type='password'], input[type='email']",
            ".ds-markdown",
        )),
        // OLCULDU ve ucdan uca calisti (23x4 -> 92, 11.3 sn). Kutu contenteditable,
        // textarea yok. Giris isareti OLCULEMEDI: kimi girissiz de cevapliyor, yani
        // "girisli mi" sorusu burada dogal olarak bulanik (perplexity gibi).
        //
        // Giris adresi de yok: sayfadaki 20 baglantinin hicbiri girise gitmiyor ve
        // 9 dugmenin sekizi metinsiz (ikon). Giris bir kalipla aciliyor, yani
        // "giris adresi" uydurmak yerine kok adres birakildi.
        // ADRES OLCUMLE DUZELTILDI (2026-08-19): kimi.com DEGIL kimi.ai.
        // kimi.com Cin ici adres; disaridan acilir ama giris tasimaz. Olculdu:
        // `access_token` + `refresh_token` + `msh_user_id` localStorage'da YALNIZ
        // `https+++www.kimi.ai` altinda; kimi.com altinda hicbiri yok. Yani buradaki
        // her kimi istegi bugune kadar MISAFIR olarak gitti — kotasi, gecmisi ve
        // model secimi girisli hesabin degil. "kimi girissiz de cevapliyor" notu
        // bu hatayi gorunmez kilmisti: calisiyor gorunuyordu, cunku gercekten
        // cevap geliyordu.
        p(
            "kimi",
            "https://www.kimi.ai/",
            "https://www.kimi.ai/",
            "div[contenteditable='true']",
            "input[type='password'], input[type='email']",
            ".markdown",
        ),
        cf(p(
            "qwen",
            "https://chat.qwen.ai/",
            "https://chat.qwen.ai/auth?action=signin",
            "textarea, div[contenteditable='true'], [contenteditable]",
            "input[type='password'], input[type='email']",
            ".markdown-content",
        )),
        // Ad "zai" degil "z.ai": model kimligi ICERME ile eslesiyor ve "zai"
        // baska adlarin icinde gecebilir.
        //
        // OLCULDU: kutu `textarea`. Yanit icin `.markdown-prose` YANLIS — hem
        // kullanici hem asistan balonu o sinifi tasiyor (`chat-user ...
        // markdown-prose` / `chat-assistant ... markdown-prose`), yani kendi
        // istemimizi cevap sanabilirdik. Dogrusu `.chat-assistant`.
        cf(p(
            "z.ai",
            "https://chat.z.ai/",
            "https://chat.z.ai/auth",
            "textarea",
            "input[type='password'], input[type='email']",
            ".chat-assistant",
        )),
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
            if let Some(v) = a.cloudflare {
                sag.cloudflare = v;
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
            bail!(
                "bilinmeyen saglayici '{ad}'. Taninanlar: {}",
                adlar.join(", ")
            )
        }
    }
}

/// Istenen model kimliginden saglayiciyi secer.
///
/// Esitlik degil ICERME araniyor, cunku model kimlikleri surum eki tasiyor
/// (`kervan/gemini-3.1-pro`). Boylece yeni saglayicilar eklenirken var olan
/// yapilandirmalar kirilmiyor.
pub fn model_saglayicisi(model: &str, defter: &HashMap<String, Arc<Provider>>) -> Option<String> {
    let m = model.to_ascii_lowercase();
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

    /// Eski yapilandirmalardaki model kimligi calismaya devam etmeli; yeni
    /// saglayici eklemek var olan kurulumu kirmamali.
    #[test]
    fn surum_ekli_model_kimligi_eslesir() {
        let d = defter();
        assert_eq!(
            model_saglayicisi("kervan/gemini-3.1-pro", &d).as_deref(),
            Some("gemini")
        );
        assert_eq!(
            model_saglayicisi("kervan/claude", &d).as_deref(),
            Some("claude")
        );
        assert_eq!(
            model_saglayicisi("kervan/perplexity", &d).as_deref(),
            Some("perplexity")
        );
        assert_eq!(model_saglayicisi("bilinmeyen", &d), None);
    }

    /// Iki Copilot AYRI urun (kisisel Microsoft hesabi vs is/okul M365): ayri
    /// adres, ayri kota. Kisa ad uzunu calarsa is hesabinin istegi kisisel
    /// hesaba duser ve yanlis kota harcanir.
    #[test]
    fn copilot_kisisel_ile_m365_karismaz() {
        let d = defter();
        assert_eq!(
            model_saglayicisi("kervan/copilot", &d).as_deref(),
            Some("copilot")
        );
        assert_eq!(
            model_saglayicisi("kervan/copilot-m365", &d).as_deref(),
            Some("copilot-m365")
        );
    }

    /// Yapilandirma secicileri ezebilmeli — site arayuzu degisince yeniden derleme
    /// degil tek satir duzenleme yetsin.
    #[test]
    fn yapilandirma_seciciyi_ezer() {
        let mut a = HashMap::new();
        a.insert(
            "claude".to_string(),
            SaglayiciAyar {
                response: Some(".yeni-secici".into()),
                ..Default::default()
            },
        );
        let d = kayit_defteri(&a);
        assert_eq!(d["claude"].selectors.response, ".yeni-secici");
        // ezilmeyen alanlar varsayilanda kalmali
        assert!(d["claude"].url.contains("claude.ai"));
        // olculdu: perplexity'de textarea yok, contenteditable var
        assert_eq!(
            d["perplexity"].selectors.prompt_input,
            "div[contenteditable='true']"
        );
        // baska saglayici etkilenmemeli
        assert_eq!(
            d["gemini"].selectors.response,
            "model-response, message-content"
        );
    }

    /// Cloudflare isareti OLCUME dayaniyor ve bosta kapatmayi yonetiyor. Yanlis
    /// tarafa dusmesi iki turlu zarar: isaretlenmeyen bir CF sitesi ac-kapa
    /// dongusune girip sinama davet eder, gereksiz isaretlenen bir site ise
    /// bosuna bellek tutar.
    #[test]
    fn cloudflare_isareti_olculene_uyar() {
        let d = defter();
        for ad in ["claude", "perplexity", "deepseek", "qwen", "z.ai"] {
            assert!(d[ad].cloudflare, "{ad} Cloudflare arkasinda olculdu");
        }
        for ad in ["gemini", "kimi", "copilot", "copilot-m365"] {
            assert!(!d[ad].cloudflare, "{ad} yedi hesapta da duvara takilmadi");
        }
    }

    /// Olcum degisirse yeniden derleme gerekmemeli.
    #[test]
    fn cloudflare_isareti_yapilandirmayla_ezilir() {
        let mut a = HashMap::new();
        a.insert(
            "claude".to_string(),
            SaglayiciAyar {
                cloudflare: Some(false),
                ..Default::default()
            },
        );
        assert!(!kayit_defteri(&a)["claude"].cloudflare);
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
