//! `kervan.toml` — hesap listesi ve hiz butcesi.
//!
//! Yol aranir (bkz. [`adaylar`]): `KERVAN_CONFIG` verilmisse yalniz orasi; yoksa
//! calisma dizini, ikilinin yani, sonra kurulum klasoru. Hicbiri yoksa tek hesapli
//! makul bir varsayilanla calisir — bu sayede ilk kullanimda yapilandirma yazmak
//! gerekmez — ama hangi yerlere bakildigini YUKSEK SESLE soyler. Eskiden sessizce
//! varsayilana dusuyordu; tasinmis bir kurulumda (ya da cwd degisince) hesap havuzu
//! gorunmez olurdu ve bunu soyleyen kimse yoktu.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Kurulum klasoru (`tazele.cmd` hedefi), `USERPROFILE`'a gore; `kervan.toml`
/// altinda. el-Fihrist `konum.rs` ile ayni desen: sabit yol, sessiz cwd tahmini yok.
const KURULUM: &str = r"Desktop\mcp-tools\kervan";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Bir tarayici kac dakika bosta kalinca kapatilir. 0 = hic kapatma.
    ///
    /// Hesap sayisinin tavani kod degil bellek: olculdu, kimlik basina ~344 MB.
    /// Bosta kapatma olmadan 20 hesap ~7 GB tutar; bununla yalniz o an calisanlar
    /// acik kalir. Oturum diskte oldugu icin kapatmak veri kaybetmez.
    #[serde(default = "varsayilan_bosta_dakika")]
    pub bosta_dakika: u64,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub pacing: Pacing,
    #[serde(default)]
    pub kota: Kota,
    /// Saglayici basina secici/adres ezmeleri. Site arayuzu degisince kod degil
    /// bu bolum degisir.
    #[serde(default)]
    pub providers: std::collections::HashMap<String, SaglayiciAyar>,
    /// REST yukarı akış sağlayıcıları ([rest.<ad>]).
    #[serde(default)]
    pub rest: std::collections::HashMap<String, RestAyar>,
    /// Sağlayıcı seçim stratejisi ([yonlendirme]).
    #[serde(default)]
    pub yonlendirme: Yonlendirme,
}

/// Sağlayıcı seçim stratejisi ([yonlendirme]).
///
/// Ad doğrulaması burada YAPILMAZ: geçerli adların tek kaynağı
/// `kervan_havuz::router::RoutingStrategy`'dir ve bu crate ona bağımlı değil.
/// Bilinmeyen ad sunucu açılışında yüksek sesle reddedilir (main.rs).
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Yonlendirme {
    /// `lkgp` (varsayılan), `round-robin`, `least-used`, `p2c`, `priority`, …
    #[serde(default = "varsayilan_strateji")]
    pub strateji: String,
    /// Aynı istek başarısız olursa sıradaki adaya geçilsin mi?
    #[serde(default = "varsayilan_yedek")]
    pub yedek_deneme: bool,
    /// Tek istekte denenecek azami aday sayısı (yedek_deneme açıkken).
    #[serde(default = "varsayilan_azami_deneme")]
    pub azami_deneme: u32,
}

fn varsayilan_strateji() -> String {
    "lkgp".into()
}

fn varsayilan_yedek() -> bool {
    true
}

fn varsayilan_azami_deneme() -> u32 {
    3
}

impl Default for Yonlendirme {
    fn default() -> Self {
        Self {
            strateji: varsayilan_strateji(),
            yedek_deneme: varsayilan_yedek(),
            azami_deneme: varsayilan_azami_deneme(),
        }
    }
}

/// REST yukarı akış sağlayıcı yapılandırması ([rest.<ad>]).
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RestAyar {
    pub bicim: String,
    pub taban_url: String,
    pub anahtar_env: String,
    pub model: String,
    #[serde(default)]
    pub yol: Option<String>,
}

impl RestAyar {
    pub fn dogrula(&self, ad: &str) -> Result<()> {
        match self.bicim.trim() {
            "openai" | "claude" | "gemini" => {}
            diger => anyhow::bail!(
                "Geçersiz REST sağlayıcı biçimi: '{}' ([rest.{}])",
                diger,
                ad
            ),
        }
        let url = self.taban_url.trim();
        if url.is_empty() || !(url.starts_with("http://") || url.starts_with("https://")) {
            anyhow::bail!(
                "[rest.{}] taban_url boş ya da http(s):// ile başlamıyor: '{}'",
                ad,
                self.taban_url
            );
        }
        if self.model.trim().is_empty() {
            anyhow::bail!("[rest.{}] model boş olamaz", ad);
        }
        let env = self.anahtar_env.trim();
        if env.is_empty() || !env.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            anyhow::bail!(
                "[rest.{}] anahtar_env geçerli bir ortam değişkeni adı değil: '{}'",
                ad,
                self.anahtar_env
            );
        }
        Ok(())
    }
}

/// Yerlesik saglayici tanimini kismen ezer; verilmeyen alan varsayilanda kalir.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct SaglayiciAyar {
    pub url: Option<String>,
    pub giris_kalibi: Option<String>,
    pub prompt_input: Option<String>,
    pub login_hint: Option<String>,
    pub response: Option<String>,
    /// Bu saglayiciya ayni anda kac istek gidebilir. Verilmezse 1.
    ///
    /// Olculdu: ayni siteye ayni anda yedi profille gitmek Cloudflare'i interaktif
    /// sinamaya gecirdi. Cok hesapli olmanin amaci es zamanlilik degil SUREKLILIK,
    /// o yuzden varsayilan sirali. Cloudflare arkasinda olmayan bir saglayicida
    /// hiz gerekiyorsa burada yukseltilir.
    pub eszamanli: Option<u32>,
    /// Site Cloudflare arkasinda mi? Verilirse yerlesik OLCUMU ezer.
    ///
    /// Tek etkisi: bu saglayicinin sekmesi acikken tarayici bosta kapatilmaz.
    pub cloudflare: Option<bool>,
}

/// Kota ogrenicinin ayarlari.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Kota {
    /// Saglayicinin limit mesajinda gecen ifadeler. Varsayilanlar makul TAHMIN;
    /// gercek metni gorunce buraya eklenir — kod degistirmeden, yeniden derlemeden.
    #[serde(default = "varsayilan_isaretler")]
    pub isaretler: Vec<String>,
    /// Limit gorulunce, pencere daha ogrenilmemisken ne kadar beklenir (dakika).
    #[serde(default = "varsayilan_kota_bekleme")]
    pub bilinmeyen_pencere_dk: u64,
}

fn varsayilan_isaretler() -> Vec<String> {
    vec![
        "reached your limit".into(),
        "sınırına ulaştınız".into(),
        "limitine ulaştınız".into(),
        "kotanız doldu".into(),
    ]
}

fn varsayilan_kota_bekleme() -> u64 {
    60
}

impl Default for Kota {
    fn default() -> Self {
        Self {
            isaretler: varsayilan_isaretler(),
            bilinmeyen_pencere_dk: varsayilan_kota_bekleme(),
        }
    }
}

/// Bir KIMLIK: profil dizini ve o profilde oturum acilmis saglayicilar.
///
/// Bir Google oturumu profildeki tum saglayicilara yetiyor (olculdu: Gemini acikken
/// Perplexity'ye "Continue with Google" ile sifresiz girildi), o yuzden hesap basina
/// ayri profil degil, KIMLIK basina tek profil tutuluyor.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Account {
    /// Profil dizini adi. Ayni ad = ayni oturum.
    pub name: String,
    /// Tekil bicim — eski yapilandirmalar kirilmasin diye korunuyor.
    #[serde(default)]
    pub provider: Option<String>,
    /// Bu profilde acik olan saglayicilar. Her biri kendi sekmesinde oturur.
    #[serde(default)]
    pub providers: Vec<String>,
}

impl Account {
    pub fn saglayicilar(&self) -> Vec<String> {
        if !self.providers.is_empty() {
            return self.providers.clone();
        }
        vec![self.provider.clone().unwrap_or_else(varsayilan_saglayici)]
    }
}

fn varsayilan_saglayici() -> String {
    "gemini".into()
}

/// Bir tarayiciyi yeniden acmak ~5 sn; 15 dk once kullanilmis bir hesabin hemen
/// tekrar gelmesi beklenmez. Kisa tutmak surekli ac/kapa, uzun tutmak bosuna
/// bellek demek olurdu.
fn varsayilan_bosta_dakika() -> u64 {
    15
}

/// Hiz butcesi — plandaki "insan hizinda davranmak" listesinin kod karsiligi.
/// Bot tespitini yenmek DEGIL, gercekten normal bir kullanici gibi davranmak:
/// yuku azaltir, hesabi yasatir.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Pacing {
    /// Ayni hesaba ardisik iki istek arasindaki en az sure.
    pub min_delay_ms: u64,
    /// Uzerine eklenen rastgele sapma. Sabit araliklarla istek atmak, insan
    /// davranisindan en kolay ayirt edilen oruntudur.
    pub jitter_ms: u64,
    /// Hesap basina saatlik tavan. 0 = sinirsiz.
    pub hourly_cap: u32,
}

impl Default for Pacing {
    fn default() -> Self {
        // Muhafazakar varsayilanlar: bir insanin sohbet temposu.
        Self {
            min_delay_ms: 3000,
            jitter_ms: 2000,
            hourly_cap: 30,
        }
    }
}

impl Config {
    pub fn load(yol: &Path) -> Result<Self> {
        if !yol.exists() {
            // `KERVAN_CONFIG` acikca verilmisse tahmin yurutme: ya odur ya hata.
            if std::env::var_os("KERVAN_CONFIG").is_some() {
                anyhow::bail!("KERVAN_CONFIG bir dosyayi gostermiyor: {}", yol.display());
            }
            // Sessiz varsayilan, tasinmis bir kurulumda hesap havuzunu gorunmez
            // yapardi. Bakilan yerleri soyle, sonra yine de varsayilanla basla.
            eprintln!(
                "kervan: uyari: kervan.toml bulunamadi; yerlesik varsayilan \
                 (tek hesap: {}) kullanilacak.\nBakilan yerler:\n  {}\n\
                 KERVAN_CONFIG ile acikca gosterebilirsin.",
                Self::varsayilan().accounts[0].name,
                adaylar()
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            );
            return Ok(Self::varsayilan());
        }
        let metin = std::fs::read_to_string(yol)
            .with_context(|| format!("yapilandirma okunamadi: {}", yol.display()))?;
        let mut c: Config = toml::from_str(&metin)
            .with_context(|| format!("yapilandirma ayristirilamadi: {}", yol.display()))?;
        if c.accounts.is_empty() {
            c.accounts = Self::varsayilan().accounts;
        }
        c.dogrula()?;
        Ok(c)
    }

    pub fn dogrula(&self) -> Result<()> {
        if self.yonlendirme.azami_deneme == 0 {
            anyhow::bail!("[yonlendirme] azami_deneme 0 olamaz");
        }
        for (ad, r) in &self.rest {
            r.dogrula(ad)?;
        }
        // `kervan/<ad>` önce REST'te aranır: aynı adlı [rest.X], X tarayıcı sağlayıcısını
        // sessizce gölgelerdi. Çakışma açık hatadır.
        //
        // Küme `saglayicilar()` üzerinden kurulur: `provider`/`providers` alanı boş
        // bırakılan hesabın varsayılanı da (`gemini`) gölgelemeye karşı korunmalı.
        let tarayici: std::collections::BTreeSet<String> = self
            .accounts
            .iter()
            .flat_map(|a| a.saglayicilar())
            .chain(self.providers.keys().cloned())
            .collect();
        // "auto" rezervedir: `[rest.auto]` yazılırsa `model:"auto"` REST'e gider ve
        // akıllı yönlendirme tamamen kapanırdı. Bu sessiz bir davranış değişikliği
        // olduğu için açılışta hata sayılır.
        if self.rest.contains_key("auto") {
            anyhow::bail!(
                "[rest.auto] kullanılamaz: 'auto' akıllı yönlendirmeye ayrılmış bir addır"
            );
        }
        // Eklentinin sabit adları (eklenti/background.js SAĞLAYICILAR) `[rest.X]`
        // yazılınca gölgelenir. Bu bilinçli bir tercih olabilir (tarayıcı yerine
        // doğrudan API), ama sessiz kalmamalı.
        const EKLENTI_ADLARI: [&str; 9] = [
            "qwen",
            "kimi",
            "gemini",
            "deepseek",
            "chatgpt",
            "copilot",
            "z.ai",
            "perplexity",
            "mistral",
        ];
        let mut cakisan: Vec<&str> = self
            .rest
            .keys()
            .map(String::as_str)
            .filter(|ad| tarayici.contains(*ad))
            .collect();
        for ad in self.rest.keys() {
            if EKLENTI_ADLARI.contains(&ad.as_str()) && !tarayici.contains(ad) {
                eprintln!(
                    "uyari: [rest.{ad}] eklentinin '{ad}' sekmesini golgeler; istekler REST'e gider"
                );
            }
        }
        cakisan.sort_unstable();
        if !cakisan.is_empty() {
            anyhow::bail!(
                "[rest.*] adları tarayıcı sağlayıcılarıyla çakışıyor: {} (kervan/<ad> REST'e giderdi)",
                cakisan.join(", ")
            );
        }
        Ok(())
    }

    fn varsayilan() -> Self {
        Self {
            bosta_dakika: varsayilan_bosta_dakika(),
            accounts: vec![Account {
                name: "ercan-gemini-1".into(),
                provider: None,
                providers: vec![varsayilan_saglayici()],
            }],
            pacing: Pacing::default(),
            kota: Kota::default(),
            providers: Default::default(),
            rest: Default::default(),
            yonlendirme: Yonlendirme::default(),
        }
    }
}

/// `kervan.toml` icin bakilacak yerler, sirayla. `KERVAN_CONFIG` verilmisse yalniz
/// o doner (acik tercih, arama yok) — boylece yanlis bir yol sessizce yok sayilmaz.
pub fn adaylar() -> Vec<PathBuf> {
    if let Some(v) = std::env::var_os("KERVAN_CONFIG") {
        return vec![PathBuf::from(v)];
    }
    let mut a = vec![PathBuf::from("kervan.toml")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        a.push(dir.join("kervan.toml"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        a.push(PathBuf::from(home).join(KURULUM).join("kervan.toml"));
    }
    a
}

/// Adaylar icinde ilk var olani; hicbiri yoksa ilk adayi doner ki cagiran
/// "bulunamadi" diyebilsin.
fn ilk_var_olan(adaylar: &[PathBuf]) -> PathBuf {
    adaylar
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .unwrap_or_else(|| adaylar[0].clone())
}

pub fn varsayilan_yol() -> PathBuf {
    ilk_var_olan(&adaylar())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hesaplar_ve_hiz_butcesi_okunur() {
        let t = r#"
            [[accounts]]
            name = "a"
            [[accounts]]
            name = "b"
            provider = "gemini"
            [pacing]
            min_delay_ms = 1000
            jitter_ms = 500
            hourly_cap = 10
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(c.accounts.len(), 2);
        assert_eq!(c.accounts[0].name, "a");
        // provider verilmezse varsayilan dolmali
        assert_eq!(c.accounts[0].saglayicilar(), vec!["gemini".to_string()]);
        assert_eq!(c.pacing.hourly_cap, 10);
    }

    /// pacing blogu hic yoksa muhafazakar varsayilan gelmeli — yapilandirma
    /// eksikligi "sinirsiz hiz" anlamina GELMEMELI.
    #[test]
    fn eksik_pacing_guvenli_varsayilana_duser() {
        let c: Config = toml::from_str("[[accounts]]\nname = \"a\"\n").unwrap();
        assert!(c.pacing.min_delay_ms >= 1000);
        assert!(c.pacing.hourly_cap > 0);
    }

    #[test]
    fn gecerli_rest_yapilandirmasi_okunur() {
        let t = r#"
            [rest.deepseek]
            bicim = "openai"
            taban_url = "https://api.deepseek.com"
            anahtar_env = "DEEPSEEK_API_KEY"
            model = "deepseek-chat"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(c.rest.len(), 1);
        let ds = c.rest.get("deepseek").unwrap();
        assert_eq!(ds.bicim, "openai");
        assert_eq!(ds.taban_url, "https://api.deepseek.com");
        assert_eq!(ds.anahtar_env, "DEEPSEEK_API_KEY");
        assert_eq!(ds.model, "deepseek-chat");
        assert!(c.dogrula().is_ok());
    }

    /// [yonlendirme] yoksa varsayılan lkgp + yedek deneme açık gelmeli.
    #[test]
    fn yonlendirme_varsayilani_lkgp() {
        let c: Config = toml::from_str("[[accounts]]\nname = \"a\"\n").unwrap();
        assert_eq!(c.yonlendirme.strateji, "lkgp");
        assert!(c.yonlendirme.yedek_deneme);
        assert_eq!(c.yonlendirme.azami_deneme, 3);
    }

    #[test]
    fn yonlendirme_okunur() {
        let t = r#"
            [yonlendirme]
            strateji = "round-robin"
            yedek_deneme = false
            azami_deneme = 5
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(c.yonlendirme.strateji, "round-robin");
        assert!(!c.yonlendirme.yedek_deneme);
        assert_eq!(c.yonlendirme.azami_deneme, 5);
        assert!(c.dogrula().is_ok());
    }

    /// azami_deneme = 0 "hiç aday denenmez" demek; sessizce kabul edilemez.
    #[test]
    fn yonlendirme_azami_deneme_sifir_reddedilir() {
        let t = r#"
            [yonlendirme]
            azami_deneme = 0
        "#;
        let c: Config = toml::from_str(t).unwrap();
        let hata = c.dogrula().unwrap_err().to_string();
        assert!(hata.contains("azami_deneme"), "{hata}");
    }

    #[test]
    fn gecersiz_rest_bicimi_hata_verir() {
        let t = r#"
            [rest.gecersiz]
            bicim = "bilinmeyen_bicim"
            taban_url = "https://api.example.com"
            anahtar_env = "API_KEY"
            model = "model-1"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert!(c.dogrula().is_err());
    }

    #[test]
    fn rest_adi_tarayici_saglayicisini_golgeleyemez() {
        let t = r#"
            [[accounts]]
            name = "a"
            providers = ["gemini"]
            [rest.gemini]
            bicim = "gemini"
            taban_url = "https://generativelanguage.googleapis.com"
            anahtar_env = "GEMINI_API_KEY"
            model = "gemini-2.5-flash"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        let hata = c.dogrula().unwrap_err().to_string();
        assert!(hata.contains("çakışıyor: gemini"), "{hata}");
    }

    /// `provider`/`providers` yazılmamış hesap `gemini`ye düşer; `[rest.gemini]`
    /// bu varsayılanı da gölgeleyemez (çakışma kümesi `saglayicilar()`den kurulur).
    #[test]
    fn rest_adi_varsayilan_hesabi_da_golgeleyemez() {
        let t = r#"
            [[accounts]]
            name = "a"
            [rest.gemini]
            bicim = "gemini"
            taban_url = "https://generativelanguage.googleapis.com"
            anahtar_env = "GEMINI_API_KEY"
            model = "gemini-2.5-flash"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        let hata = c.dogrula().unwrap_err().to_string();
        assert!(hata.contains("çakışıyor: gemini"), "{hata}");
    }

    #[test]
    fn rest_auto_rezerve_reddedilir() {
        let t = r#"
            [rest.auto]
            bicim = "openai"
            taban_url = "https://api.example.com"
            anahtar_env = "API_KEY"
            model = "m"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        let hata = c.dogrula().unwrap_err().to_string();
        assert!(hata.contains("[rest.auto]"), "{hata}");
    }

    /// Bilinmeyen alan sessizce yutulmamalı: `[yonlendırme]` (Türkçe ı) yazım
    /// hatası fark edilmeden varsayılana düşerdi.
    #[test]
    fn bilinmeyen_alan_reddedilir() {
        let t = "[yonlendırme]\nstrateji = \"round-robin\"\n";
        assert!(toml::from_str::<Config>(t).is_err());
        let t2 = "[[accounts]]\nname = \"a\"\neszamanli = 2\n";
        assert!(toml::from_str::<Config>(t2).is_err());
    }

    #[test]
    fn rest_eksik_ya_da_gecersiz_alanlar_reddedilir() {
        let vakalar = [
            (
                "[rest.x]\nbicim=\"openai\"\ntaban_url=\"\"\nanahtar_env=\"K\"\nmodel=\"m\"\n",
                "taban_url",
            ),
            (
                "[rest.x]\nbicim=\"openai\"\ntaban_url=\"api.example.com\"\nanahtar_env=\"K\"\nmodel=\"m\"\n",
                "taban_url",
            ),
            (
                "[rest.x]\nbicim=\"openai\"\ntaban_url=\"https://a\"\nanahtar_env=\"\"\nmodel=\"m\"\n",
                "anahtar_env",
            ),
            (
                "[rest.x]\nbicim=\"openai\"\ntaban_url=\"https://a\"\nanahtar_env=\"K\"\nmodel=\"  \"\n",
                "model",
            ),
        ];
        for (metin, beklenen) in vakalar {
            let c: Config = toml::from_str(metin).unwrap();
            let hata = c.dogrula().unwrap_err().to_string();
            assert!(hata.contains(beklenen), "{beklenen} icin: {hata}");
        }
    }

    /// Tasinmis kurulumun cekirdegi: adaylar icinde VAR OLAN secilir; hicbiri
    /// yoksa ilk aday doner ki cagiran "bulunamadi" diyebilsin. Eskiden cwd'deki
    /// `kervan.toml` tek aday sayilir, yoksa sessizce varsayilana dusulurdu.
    #[test]
    fn ilk_var_olan_mevcut_adayi_secer_yoksa_ilkini_dondurur() {
        let dizin = std::env::temp_dir().join(format!("kervan-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dizin).unwrap();
        let yok1 = dizin.join("yok-1.toml");
        let var = dizin.join("var.toml");
        let yok2 = dizin.join("yok-2.toml");
        std::fs::write(&var, "[[accounts]]\nname = \"x\"\n").unwrap();
        assert_eq!(
            ilk_var_olan(&[yok1.clone(), var.clone(), yok2.clone()]),
            var
        );
        assert_eq!(ilk_var_olan(&[yok1.clone(), yok2]), yok1);
        let _ = std::fs::remove_dir_all(&dizin);
    }
}
