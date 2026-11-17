//! Hesap kasası: CLI kimlik dosyalarını (Claude Code, Codex, Gemini CLI) saklar
//! ve aralarında geçiş yapar.
//!
//! NEDEN BU SANDIK VAR: `openchamber/extensions/claude-accounts` yalnız Claude
//! Code dosyalarını tanıyor ve kimliği `accountUuid.organizationUuid` çiftinden
//! türetiyor. Takım/kurum hesabında bu çift var; **ücretsiz (kişisel) hesapta
//! `organizationUuid` yok**, bu yüzden uzantı o hesapları "unidentified" sayıp
//! listeye almıyor. Buradaki kimlik çıkarımı kademelidir: kurum çifti → yalnız
//! `accountUuid` → eposta → token parmak izi. Böylece ücretsiz hesaplar da aynı
//! kasaya girer ve aynı geçiş yolundan kullanılır.
//!
//! İKİNCİ FARK: aynı mekanizma tek araçla sınırlı değil. Kasa `Cesit` başına
//! ayrı dosya tutar; Claude, Codex ve Gemini aynı `gecis` çağrısıyla değişir.
//!
//! YAZMA KURALI: kimlik dosyasının **tanımadığımız alanlarına dokunulmaz**.
//! Claude'da `.credentials.json` içindeki `mcpOAuth` ve `~/.claude.json` içindeki
//! onlarca üst anahtar (machineID, mcpServers, …) korunur; yalnız kimlik taşıyan
//! alt nesne değişir. Biçim de korunur (girinti, CRLF, sondaki satır sonu):
//! aksi halde `git diff` gibi araçlar ve insan gözü dosyayı baştan yazılmış sanır.
//!
//! GÜVENLİK: dosyalar kullanıcı profilinin altında, depo dışında durur; kasa da
//! `%LOCALAPPDATA%\kervan\kasa` altında. Sır HTTP yanıtına çıkmaz — panel yalnız
//! eposta/plan/kimlik görür.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Kasa dosya biçimi sürümü. Okuma sırasında farklı sürüm reddedilir: eski bir
/// şemayı sessizce yorumlamak, yanlış alanı kimlik sanmak demek olurdu.
pub const SURUM: u32 = 1;

/// Hangi aracın kimlik dosyaları. Her biri kendi kasa dosyasını taşır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cesit {
    Claude,
    Codex,
    Gemini,
}

impl Cesit {
    pub fn ad(&self) -> &'static str {
        match self {
            Cesit::Claude => "claude",
            Cesit::Codex => "codex",
            Cesit::Gemini => "gemini",
        }
    }

    pub fn from_ad(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" | "claude-code" | "anthropic" => Some(Cesit::Claude),
            "codex" | "openai" | "chatgpt" => Some(Cesit::Codex),
            "gemini" | "google" => Some(Cesit::Gemini),
            _ => None,
        }
    }

    pub fn tumu() -> [Cesit; 3] {
        [Cesit::Claude, Cesit::Codex, Cesit::Gemini]
    }

    /// Kimlik dosyaları. `ev` kullanıcı ana dizini (`%USERPROFILE%`).
    pub fn dosyalar(&self, ev: &Path) -> Vec<PathBuf> {
        match self {
            Cesit::Claude => vec![
                ev.join(".claude").join(".credentials.json"),
                ev.join(".claude.json"),
            ],
            Cesit::Codex => vec![ev.join(".codex").join("auth.json")],
            Cesit::Gemini => vec![
                ev.join(".gemini").join("oauth_creds.json"),
                ev.join(".gemini").join("google_accounts.json"),
            ],
        }
    }
}

/// Kasadaki tek hesap. `veri` geri yazılacak kimlik yükünün tamamıdır; panel bu
/// alanı hiç görmez.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kayit {
    pub kimlik: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eposta: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ad: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Ücretsiz mi? `None` = bilinmiyor (dosya plan alanı taşımıyor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ucretsiz: Option<bool>,
    pub kaydedildi: u64,
    pub veri: Value,
}

impl Kayit {
    /// Kimlik yükünün parmak izi — aynı hesabın yenilenmiş token'ını ayırt etmek
    /// için. Sır sızdırmaz: FNV-1a özeti.
    pub fn parmak_izi(&self) -> u64 {
        let metin = serde_json::to_string(&self.veri).unwrap_or_default();
        fnv1a(&metin)
    }

    /// Panelin göreceği özet: sır yok.
    pub fn ozet(&self) -> Value {
        json!({
            "kimlik": self.kimlik,
            "eposta": self.eposta,
            "ad": self.ad,
            "plan": self.plan,
            "ucretsiz": self.ucretsiz,
            "kaydedildi": self.kaydedildi,
        })
    }
}

/// Disk üzerindeki bir kimlik dosyasının okunmuş hâli. `ham` biçimi korumak için
/// saklanır; geri yazma bu metne benzetilerek yapılır.
#[derive(Debug, Clone)]
pub struct Dosya {
    pub yol: PathBuf,
    pub var: bool,
    pub ham: String,
}

impl Dosya {
    pub fn deger(&self) -> Result<Value> {
        if !self.var {
            return Ok(Value::Null);
        }
        serde_json::from_str(&self.ham)
            .with_context(|| format!("kimlik dosyasi bozuk JSON: {}", self.yol.display()))
    }
}

pub fn oku(yol: &Path) -> Result<Dosya> {
    match std::fs::read(yol) {
        Ok(bayt) => Ok(Dosya {
            yol: yol.to_path_buf(),
            var: true,
            ham: String::from_utf8_lossy(&bayt).into_owned(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Dosya {
            yol: yol.to_path_buf(),
            var: false,
            ham: String::new(),
        }),
        Err(e) => Err(e).with_context(|| format!("kimlik dosyasi okunamadi: {}", yol.display())),
    }
}

/// JSON'u dosyanın kendi biçimine benzeterek yazar.
///
/// Girinti ham metinden okunur (`\n(\s+)"` yakalaması), CRLF ve sondaki satır
/// sonu korunur. Biçim korunmazsa araç "dosya tamamen değişti" sanır ve yedeği
/// boşa gider.
pub fn yaz_benzeri(d: &Dosya, deger: &Value) -> Result<()> {
    if let Some(ust) = d.yol.parent() {
        std::fs::create_dir_all(ust)
            .with_context(|| format!("kimlik dizini acilamadi: {}", ust.display()))?;
    }
    let metin = benzeri_metin(&d.ham, d.var, deger)?;
    std::fs::write(&d.yol, metin)
        .with_context(|| format!("kimlik dosyasi yazilamadi: {}", d.yol.display()))?;
    Ok(())
}

fn benzeri_metin(ham: &str, var: bool, deger: &Value) -> Result<String> {
    let cok_satirli = var && ham.contains('\n');
    let girinti = girinti_uzunlugu(ham);
    let mut s = if cok_satirli {
        serde_json::to_string_pretty(deger)?
    } else {
        serde_json::to_string(deger)?
    };
    if cok_satirli && girinti != 2 {
        // `to_string_pretty` iki boşluk kullanır; hedef girinti farklıysa ölçekle.
        s = s
            .split('\n')
            .map(|satir| {
                let bosluk = satir.len() - satir.trim_start_matches(' ').len();
                if bosluk == 0 {
                    satir.to_string()
                } else {
                    format!("{}{}", " ".repeat(bosluk / 2 * girinti), &satir[bosluk..])
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    let crlf = var && ham.contains("\r\n");
    if crlf {
        s = s.replace('\n', "\r\n");
    }
    if var && ham.ends_with('\n') && !s.ends_with('\n') {
        s.push_str(if crlf { "\r\n" } else { "\n" });
    }
    Ok(s)
}

fn girinti_uzunlugu(ham: &str) -> usize {
    for satir in ham.split('\n').skip(1) {
        let bosluk = satir.len() - satir.trim_start_matches(' ').len();
        if bosluk > 0 && satir.trim_start().starts_with('"') {
            return bosluk;
        }
    }
    2
}

fn fnv1a(metin: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in metin.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// base64url (JWT gövdesi) çözümü — ek sandık getirmemek için elde.
fn base64url(parca: &str) -> Option<Vec<u8>> {
    let mut cikti = Vec::with_capacity(parca.len() * 3 / 4);
    let mut birikim: u32 = 0;
    let mut bit: u32 = 0;
    for c in parca.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        birikim = (birikim << 6) | v as u32;
        bit += 6;
        if bit >= 8 {
            bit -= 8;
            cikti.push((birikim >> bit) as u8);
        }
    }
    Some(cikti)
}

/// JWT gövdesini JSON olarak döndürür. İmza doğrulanmaz — burada amaç kimliği
/// okumak, kimlik iddiasını doğrulamak değil (token zaten yerel dosyada).
pub fn jwt_govde(jwt: &str) -> Option<Value> {
    let govde = jwt.split('.').nth(1)?;
    let bayt = base64url(govde)?;
    serde_json::from_slice(&bayt).ok()
}

fn jwt_eposta(jwt: &str) -> Option<String> {
    jwt_govde(jwt)?.get("email")?.as_str().map(str::to_string)
}

fn alan<'a>(v: &'a Value, yol: &[&str]) -> Option<&'a Value> {
    let mut simdi = v;
    for k in yol {
        simdi = simdi.get(k)?;
    }
    Some(simdi)
}

fn metin(v: &Value, yol: &[&str]) -> Option<String> {
    alan(v, yol)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ---------------------------------------------------------------- sağlayıcılar

/// O an dosyalarda duran oturumun kaydı. `None` = giriş yok.
pub fn aktif(cesit: Cesit, ev: &Path) -> Result<Option<Kayit>> {
    let dosyalar: Vec<Dosya> = cesit
        .dosyalar(ev)
        .iter()
        .map(|y| oku(y))
        .collect::<Result<_>>()?;
    match cesit {
        Cesit::Claude => claude_kayit(&dosyalar),
        Cesit::Codex => codex_kayit(&dosyalar),
        Cesit::Gemini => gemini_kayit(&dosyalar),
    }
}

fn claude_kayit(dosyalar: &[Dosya]) -> Result<Option<Kayit>> {
    let kimlik_dosyasi = dosyalar[0].deger()?;
    let ayar = dosyalar
        .get(1)
        .map(Dosya::deger)
        .transpose()?
        .unwrap_or(Value::Null);

    let oauth = alan(&kimlik_dosyasi, &["claudeAiOauth"])
        .cloned()
        .unwrap_or(Value::Null);
    let hesap = alan(&ayar, &["oauthAccount"])
        .cloned()
        .unwrap_or(Value::Null);
    let token = metin(&oauth, &["accessToken"]);
    if token.is_none() && hesap.is_null() {
        return Ok(None);
    }

    let eposta = metin(&hesap, &["emailAddress"]);
    let organization = metin(&hesap, &["organizationUuid"]);
    let uuid = metin(&hesap, &["accountUuid"]);
    // KADEMELI KİMLİK — ücretsiz hesap buradan içeri girer: kurum çifti yoksa
    // yalnız `accountUuid`, o da yoksa eposta, o da yoksa token parmak izi.
    let kimlik = match (uuid, organization) {
        (Some(u), Some(o)) => format!("{u}.{o}"),
        (Some(u), None) => u,
        (None, _) => match &eposta {
            Some(e) => format!("mail:{}", e.to_ascii_lowercase()),
            None => format!("tok:{:016x}", fnv1a(token.as_deref().unwrap_or_default())),
        },
    };
    let plan = metin(&oauth, &["subscriptionType"]).or_else(|| metin(&oauth, &["rateLimitTier"]));
    let ucretsiz = plan.as_deref().and_then(ucretsiz_mi);

    Ok(Some(Kayit {
        kimlik,
        eposta: eposta.clone(),
        ad: metin(&hesap, &["fullName"]).or_else(|| metin(&hesap, &["displayName"])),
        plan,
        ucretsiz,
        kaydedildi: simdi_ms(),
        veri: json!({ "claudeAiOauth": oauth, "oauthAccount": hesap }),
    }))
}

fn codex_kayit(dosyalar: &[Dosya]) -> Result<Option<Kayit>> {
    let auth = dosyalar[0].deger()?;
    if auth.is_null() {
        return Ok(None);
    }
    let id_token = metin(&auth, &["tokens", "id_token"]);
    let account_id = metin(&auth, &["tokens", "account_id"]);
    let eposta = id_token.as_deref().and_then(jwt_eposta);
    let govde = id_token
        .as_deref()
        .and_then(jwt_govde)
        .unwrap_or(Value::Null);
    let plan = metin(
        &govde,
        &["https://api.openai.com/auth", "chatgpt_plan_type"],
    );

    let kimlik = if let Some(a) = &account_id {
        format!("acct:{a}")
    } else if let Some(s) = metin(&govde, &["sub"]) {
        format!("sub:{s}")
    } else if let Some(e) = &eposta {
        format!("mail:{}", e.to_ascii_lowercase())
    } else {
        return Ok(None);
    };
    // ChatGPT oturumu olmayan (yalnız API anahtarlı) dosya bir "hesap" değildir.
    if metin(&auth, &["auth_mode"]).as_deref() != Some("chatgpt") && eposta.is_none() {
        return Ok(None);
    }
    let ucretsiz = plan.as_deref().and_then(ucretsiz_mi);
    Ok(Some(Kayit {
        kimlik,
        eposta,
        ad: metin(&govde, &["name"]),
        plan,
        ucretsiz,
        kaydedildi: simdi_ms(),
        veri: auth,
    }))
}

fn gemini_kayit(dosyalar: &[Dosya]) -> Result<Option<Kayit>> {
    let kimlik_dosyasi = dosyalar[0].deger()?;
    let hesaplar = dosyalar
        .get(1)
        .map(Dosya::deger)
        .transpose()?
        .unwrap_or(Value::Null);
    if kimlik_dosyasi.is_null() && hesaplar.is_null() {
        return Ok(None);
    }
    // Gemini CLI: aktif hesap `google_accounts.json` içinde `active` alanıyla
    // işaretlenir; token `oauth_creds.json` içindedir.
    let aktif_eposta = metin(&hesaplar, &["active"]);
    let id_token = metin(&kimlik_dosyasi, &["id_token"]);
    let eposta = aktif_eposta.or_else(|| id_token.as_deref().and_then(jwt_eposta));
    let kimlik = match &eposta {
        Some(e) => format!("mail:{}", e.to_ascii_lowercase()),
        None => match metin(&kimlik_dosyasi, &["refresh_token"]) {
            Some(t) => format!("tok:{:016x}", fnv1a(&t)),
            None => return Ok(None),
        },
    };
    Ok(Some(Kayit {
        kimlik,
        eposta,
        ad: id_token
            .as_deref()
            .and_then(jwt_govde)
            .and_then(|g| metin(&g, &["name"])),
        plan: None,
        ucretsiz: None,
        kaydedildi: simdi_ms(),
        veri: json!({ "oauth_creds": kimlik_dosyasi, "google_accounts": hesaplar }),
    }))
}

fn ucretsiz_mi(plan: &str) -> Option<bool> {
    let p = plan.trim().to_ascii_lowercase();
    match p.as_str() {
        "" | "free" | "none" | "basic" | "default" => Some(true),
        _ => Some(false),
    }
}

/// Kimlik yükünü kimlik dosyalarına yazar. Yalnız tanınan alt nesneler değişir;
/// dosyanın geri kalanı ve biçimi korunur.
fn uygula(cesit: Cesit, ev: &Path, kayit: &Kayit) -> Result<()> {
    let yollar = cesit.dosyalar(ev);
    match cesit {
        Cesit::Claude => {
            let kimlik = oku(&yollar[0])?;
            let ayar = oku(&yollar[1])?;

            let mut kok = match kimlik.deger()? {
                Value::Object(m) => m,
                _ => Map::new(),
            };
            kok.insert(
                "claudeAiOauth".into(),
                alan(&kayit.veri, &["claudeAiOauth"])
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            yaz_benzeri(&kimlik, &Value::Object(kok))?;

            let oauth_hesap = alan(&kayit.veri, &["oauthAccount"]).cloned();
            if let Some(hesap) = oauth_hesap.filter(|h| !h.is_null()) {
                let mut ayar_kok = match ayar.deger()? {
                    Value::Object(m) => m,
                    _ => Map::new(),
                };
                ayar_kok.insert("oauthAccount".into(), hesap);
                yaz_benzeri(&ayar, &Value::Object(ayar_kok))?;
            }
        }
        Cesit::Codex => {
            let auth = oku(&yollar[0])?;
            yaz_benzeri(&auth, &kayit.veri)?;
        }
        Cesit::Gemini => {
            let kimlik = oku(&yollar[0])?;
            let hesaplar = oku(&yollar[1])?;
            let creds = alan(&kayit.veri, &["oauth_creds"])
                .cloned()
                .unwrap_or(Value::Null);
            if !creds.is_null() {
                yaz_benzeri(&kimlik, &creds)?;
            }
            // google_accounts.json'da `active` kasadan gelir ama `old` GEÇMİŞTİR:
            // kasadaki eski anlık görüntü geri konmaz, yoksa Gemini CLI'nin
            // oturum geçmişi her geçişte geriye sarılır. Şu anki aktif hesap
            // `old`un başına eklenir, hedef hesap `old`dan çıkarılır.
            let g = alan(&kayit.veri, &["google_accounts"])
                .cloned()
                .unwrap_or(Value::Null);
            if !g.is_null() {
                let hedef = alan(&g, &["active"]).cloned().unwrap_or(Value::Null);
                let mut kok = match hesaplar.deger()? {
                    Value::Object(m) => m,
                    _ => Map::new(),
                };
                let onceki = kok
                    .get("active")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let mut gecmis: Vec<String> = kok
                    .get("old")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(hedef_adi) = hedef.as_str() {
                    gecmis.retain(|a| a != hedef_adi);
                    if let Some(onceki) = onceki.filter(|p| p != hedef_adi) {
                        gecmis.retain(|a| a != &onceki);
                        gecmis.insert(0, onceki);
                    }
                }
                kok.insert("active".into(), hedef);
                kok.insert(
                    "old".into(),
                    Value::Array(gecmis.into_iter().map(Value::String).collect()),
                );
                yaz_benzeri(&hesaplar, &Value::Object(kok))?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------- kasa

/// Kasa kökü: `KERVAN_KASA` → `%LOCALAPPDATA%\kervan\kasa`.
pub fn varsayilan_yol() -> PathBuf {
    if let Some(p) = std::env::var_os("KERVAN_KASA") {
        return PathBuf::from(p);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kervan")
        .join("kasa")
}

/// Kullanıcı ana dizini.
pub fn ev_dizini() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Debug, Serialize, Deserialize)]
struct KasaDosyasi {
    surum: u32,
    #[serde(default)]
    kayitlar: Vec<Kayit>,
}

#[derive(Debug, Serialize)]
pub struct GecisSonucu {
    /// Değişti mi? Zaten aktifse `false`.
    pub degisti: bool,
    pub kimlik: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eposta: Option<String>,
    pub mesaj: String,
}

pub struct Kasa {
    kok: PathBuf,
    ev: PathBuf,
}

impl Kasa {
    pub fn ac(kok: impl Into<PathBuf>) -> Self {
        Kasa {
            kok: kok.into(),
            ev: ev_dizini(),
        }
    }

    /// Varsayılan kök ve ev diziniyle kasa.
    pub fn varsayilan() -> Self {
        Kasa::ac(varsayilan_yol())
    }

    /// Testler ve araçlar için: kök ile ev dizinini ayrı verebilmek gerekir.
    pub fn ac_ile(kok: impl Into<PathBuf>, ev: impl Into<PathBuf>) -> Self {
        Kasa {
            kok: kok.into(),
            ev: ev.into(),
        }
    }

    /// Kasa kökü — panel "hesaplar nerede duruyor" sorusunu bununla yanıtlar.
    /// Yol sırdır değil; dosya adları (`claude.json`) da sır değil, içerik sır.
    pub fn kok(&self) -> &Path {
        &self.kok
    }

    fn yol(&self, cesit: Cesit) -> PathBuf {
        self.kok.join(format!("{}.json", cesit.ad()))
    }

    fn oku_kasa(&self, cesit: Cesit) -> Result<KasaDosyasi> {
        let yol = self.yol(cesit);
        match std::fs::read(&yol) {
            Ok(b) => {
                let d: KasaDosyasi = serde_json::from_slice(&b)
                    .with_context(|| format!("kasa dosyasi bozuk: {}", yol.display()))?;
                if d.surum != SURUM {
                    bail!(
                        "kasa dosyasi surumu {} taninmiyor (beklenen {}): {}",
                        d.surum,
                        SURUM,
                        yol.display()
                    );
                }
                Ok(d)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(KasaDosyasi {
                surum: SURUM,
                kayitlar: Vec::new(),
            }),
            Err(e) => Err(e).with_context(|| format!("kasa dosyasi okunamadi: {}", yol.display())),
        }
    }

    fn yaz_kasa(&self, cesit: Cesit, d: &KasaDosyasi) -> Result<()> {
        std::fs::create_dir_all(&self.kok)
            .with_context(|| format!("kasa dizini acilamadi: {}", self.kok.display()))?;
        let yol = self.yol(cesit);
        let gecici = yol.with_extension("json.tmp");
        std::fs::write(&gecici, serde_json::to_vec_pretty(d)?)
            .with_context(|| format!("kasa dosyasi yazilamadi: {}", gecici.display()))?;
        // Önce geçici, sonra yerine: yarım yazılmış kasa dosyası tüm hesapları
        // kaybettirirdi.
        std::fs::rename(&gecici, &yol)
            .with_context(|| format!("kasa dosyasi yerine konulamadi: {}", yol.display()))?;
        Ok(())
    }

    pub fn liste(&self, cesit: Cesit) -> Result<Vec<Kayit>> {
        Ok(self.oku_kasa(cesit)?.kayitlar)
    }

    /// O an dosyalarda duran oturum (kasaya yazmadan).
    pub fn aktif(&self, cesit: Cesit) -> Result<Option<Kayit>> {
        aktif(cesit, &self.ev)
    }

    /// Kasayı dosyalarla uzlaştırır ve güncel listeyi döndürür.
    ///
    /// Token yenilenmesi (aynı kimlik, farklı `accessToken`) bir çakışma değil,
    /// aynı hesabın tazelenmesidir: yerinde güncellenir. `claude-accounts` burada
    /// `conflict` verip kullanıcıya soruyor; kervan'da tek yazar kasa olduğu için
    /// soru gereksiz.
    pub fn topla(&self, cesit: Cesit) -> Result<Vec<Kayit>> {
        let mut d = self.oku_kasa(cesit)?;
        if let Some(anlik) = self.aktif(cesit)? {
            match d.kayitlar.iter_mut().find(|k| k.kimlik == anlik.kimlik) {
                Some(k) => {
                    if k.parmak_izi() != anlik.parmak_izi() {
                        let kaydedildi = k.kaydedildi;
                        *k = Kayit {
                            kaydedildi,
                            ..anlik
                        };
                        self.yaz_kasa(cesit, &d)?;
                    }
                }
                None => {
                    d.kayitlar.push(anlik);
                    self.yaz_kasa(cesit, &d)?;
                }
            }
        }
        Ok(d.kayitlar)
    }

    /// Kaydı kasaya ekler (aynı kimlik varsa üzerine yazar).
    pub fn kaydet(&self, cesit: Cesit, kayit: Kayit) -> Result<()> {
        let mut d = self.oku_kasa(cesit)?;
        match d.kayitlar.iter_mut().find(|k| k.kimlik == kayit.kimlik) {
            Some(k) => *k = kayit,
            None => d.kayitlar.push(kayit),
        }
        self.yaz_kasa(cesit, &d)
    }

    pub fn sil(&self, cesit: Cesit, kimlik: &str) -> Result<bool> {
        let mut d = self.oku_kasa(cesit)?;
        let once = d.kayitlar.len();
        d.kayitlar.retain(|k| k.kimlik != kimlik);
        let silindi = d.kayitlar.len() != once;
        if silindi {
            self.yaz_kasa(cesit, &d)?;
        }
        Ok(silindi)
    }

    /// Hesabı değiştirir.
    ///
    /// Sıra `claude-accounts` ile aynı ve nedeni aynı: (1) dosyalar sakin mi,
    /// (2) hedef zaten aktif mi, (3) hedef kasada var mı, (4) yaz, (5) yazılanı
    /// geri okuyup doğrula, (6) olmadıysa özgün baytları geri koy.
    pub fn gecis(&self, cesit: Cesit, kimlik: &str) -> Result<GecisSonucu> {
        self.sakin_mi(cesit)?;

        let yollar = cesit.dosyalar(&self.ev);
        let onceki: Vec<Option<Vec<u8>>> = yollar.iter().map(|y| std::fs::read(y).ok()).collect();

        if let Some(anlik) = self.aktif(cesit)?
            && anlik.kimlik == kimlik
        {
            return Ok(GecisSonucu {
                degisti: false,
                kimlik: anlik.kimlik,
                eposta: anlik.eposta,
                mesaj: "Hesap zaten aktif.".to_string(),
            });
        }

        let kayit = self
            .liste(cesit)?
            .into_iter()
            .find(|k| k.kimlik == kimlik)
            .with_context(|| format!("kayitli hesap bulunamadi: {kimlik}. Listeyi tazele."))?;

        if let Err(e) = uygula(cesit, &self.ev, &kayit) {
            // Geri alma: yazma yarıda kaldıysa dosyalar karışık kalmasın.
            for (yol, eski) in yollar.iter().zip(onceki.iter()) {
                match eski {
                    Some(b) => {
                        let _ = std::fs::write(yol, b);
                    }
                    None => {
                        let _ = std::fs::remove_file(yol);
                    }
                }
            }
            return Err(e.context("gecis yazilamadi, onceki kimlik dosyalari geri konuldu"));
        }

        match self.aktif(cesit)? {
            Some(yeni) if yeni.kimlik == kimlik => Ok(GecisSonucu {
                degisti: true,
                kimlik: yeni.kimlik,
                eposta: yeni.eposta,
                mesaj: "Hesap degistirildi. Araci yeniden baslat.".to_string(),
            }),
            Some(yeni) => bail!(
                "gecis dogrulanamadi: dosyalarda '{}' gorunuyor, beklenen '{kimlik}'. \
                 Araç bu sirada yazmis olabilir; listeyi tazeleyip tekrar dene.",
                yeni.kimlik
            ),
            None => bail!("gecis dogrulanamadi: kimlik dosyalarinda oturum kalmadi"),
        }
    }

    /// Kimlik dosyaları sakin mi? Araç token yenilerken yazıyorsa geçiş yarışa
    /// girer ve iki taraf da birbirinin yazdığını ezer.
    fn sakin_mi(&self, cesit: Cesit) -> Result<()> {
        let yollar = cesit.dosyalar(&self.ev);
        let oku_ham = |y: &PathBuf| std::fs::read(y).ok();
        let ilk: Vec<_> = yollar.iter().map(oku_ham).collect();
        std::thread::sleep(std::time::Duration::from_millis(250));
        let ikinci: Vec<_> = yollar.iter().map(oku_ham).collect();
        if ilk != ikinci {
            bail!("kimlik dosyalari hala yaziliyor; birkac saniye sonra tekrar dene");
        }
        Ok(())
    }
}

pub fn simdi_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaz(yol: &Path, metin: &str) {
        std::fs::create_dir_all(yol.parent().unwrap()).unwrap();
        std::fs::write(yol, metin).unwrap();
    }

    const KURUM_CREDS: &str = "{\n  \"mcpOAuth\": {\n    \"x\": 1\n  },\n  \"claudeAiOauth\": {\n    \"accessToken\": \"sk-ant-oat-KURUM\",\n    \"refreshToken\": \"sk-ant-ort-KURUM\",\n    \"subscriptionType\": \"team\"\n  }\n}\n";

    fn kurum_ayar() -> String {
        serde_json::to_string_pretty(&json!({
            "numStartups": 42,
            "machineID": "abc",
            "oauthAccount": {
                "accountUuid": "28094d10-ac18-4764-b18a-8a1c5b604fc0",
                "organizationUuid": "7f000000-0000-0000-0000-000000000001",
                "emailAddress": "ercan.er@medeniyet.edu.tr",
                "organizationName": "IMU IT Law"
            }
        }))
        .unwrap()
    }

    #[test]
    fn bicim_korunur_girinti_crlf_ve_satir_sonu() {
        let ham = "{\r\n    \"a\": 1\r\n}\r\n";
        let s = benzeri_metin(ham, true, &json!({"a": 2, "b": {"c": 3}})).unwrap();
        assert!(s.contains("\r\n"), "CRLF korunmaliydi: {s:?}");
        assert!(
            s.contains("\r\n    \"a\""),
            "4 bosluk girinti korunmaliydi: {s:?}"
        );
        assert!(
            s.ends_with("\r\n"),
            "sondaki satir sonu korunmaliydi: {s:?}"
        );
        assert!(
            s.contains("\r\n        \"c\""),
            "ic ice girinti olceklenmeliydi: {s:?}"
        );
    }

    #[test]
    fn tek_satirli_dosya_tek_satir_kalir() {
        let s = benzeri_metin("{\"a\":1}", true, &json!({"a": 2, "b": 3})).unwrap();
        assert!(
            !s.contains('\n'),
            "tek satirli dosya cok satira cevrilmemeli: {s:?}"
        );
    }

    #[test]
    fn ucretsiz_claude_hesabi_kurum_olmadan_da_kimlik_alir() {
        // Uzantının listeye almadığı durum: organizationUuid YOK, subscriptionType
        // "free". Kimlik epostaya düşer ve hesap yine kasaya girer.
        let ev = tempfile::tempdir().unwrap();
        let creds = ev.path().join(".claude/.credentials.json");
        yaz(
            &creds,
            "{\n  \"claudeAiOauth\": {\n    \"accessToken\": \"sk-ant-oat-UCRETSIZ\",\n    \"refreshToken\": \"sk-ant-ort-UCRETSIZ\",\n    \"subscriptionType\": \"free\"\n  }\n}\n",
        );
        yaz(
            &ev.path().join(".claude.json"),
            &serde_json::to_string_pretty(&json!({
                "oauthAccount": {
                    "accountUuid": "11111111-2222-3333-4444-555555555555",
                    "emailAddress": "kişisel@gmail.com"
                }
            }))
            .unwrap(),
        );
        let k = aktif(Cesit::Claude, ev.path()).unwrap().unwrap();
        assert_eq!(k.kimlik, "11111111-2222-3333-4444-555555555555");
        assert_eq!(k.eposta.as_deref(), Some("kişisel@gmail.com"));
        assert_eq!(k.ucretsiz, Some(true));
    }

    #[test]
    fn kimliksiz_claude_dosyasi_hesap_sayilmaz() {
        let ev = tempfile::tempdir().unwrap();
        yaz(&ev.path().join(".claude.json"), "{\"numStartups\": 1}");
        assert!(aktif(Cesit::Claude, ev.path()).unwrap().is_none());
    }

    #[test]
    fn codex_kimligi_jwt_ve_account_id_den_gelir() {
        // {"email":"a@b.co"} base64url gövde
        let govde = "eyJlbWFpbCI6ImFAYi5jbyIsIm5hbWUiOiJBQiIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X3BsYW5fdHlwZSI6ImZyZWUifX0";
        let ev = tempfile::tempdir().unwrap();
        yaz(
            &ev.path().join(".codex/auth.json"),
            &serde_json::to_string_pretty(&json!({
                "auth_mode": "chatgpt",
                "OPENAI_API_KEY": null,
                "tokens": {
                    "id_token": format!("x.{govde}.y"),
                    "access_token": "at",
                    "refresh_token": "rt",
                    "account_id": "acct-123"
                }
            }))
            .unwrap(),
        );
        let k = aktif(Cesit::Codex, ev.path()).unwrap().unwrap();
        assert_eq!(k.kimlik, "acct:acct-123");
        assert_eq!(k.eposta.as_deref(), Some("a@b.co"));
        assert_eq!(k.plan.as_deref(), Some("free"));
        assert_eq!(k.ucretsiz, Some(true));
    }

    #[test]
    fn api_anahtarli_codex_dosyasi_hesap_sayilmaz() {
        let ev = tempfile::tempdir().unwrap();
        yaz(
            &ev.path().join(".codex/auth.json"),
            "{\"auth_mode\":\"apikey\",\"OPENAI_API_KEY\":\"sk-x\",\"tokens\":null}",
        );
        assert!(aktif(Cesit::Codex, ev.path()).unwrap().is_none());
    }

    #[test]
    fn gecis_hesabi_degistirir_ve_dokunulmamasi_gereken_alanlari_korur() {
        let ev = tempfile::tempdir().unwrap();
        let kasa_kok = ev.path().join("kasa");
        let kurum_creds = ev.path().join(".claude/.credentials.json");
        let ayar = ev.path().join(".claude.json");

        // 1) Kurum hesabıyla giriş yapılmış hâli kasaya al.
        yaz(&kurum_creds, KURUM_CREDS);
        yaz(&ayar, &kurum_ayar());
        let kasa = Kasa::ac_ile(&kasa_kok, ev.path());
        let liste = kasa.topla(Cesit::Claude).unwrap();
        assert_eq!(liste.len(), 1);
        assert_eq!(
            liste[0].kimlik,
            "28094d10-ac18-4764-b18a-8a1c5b604fc0.7f000000-0000-0000-0000-000000000001"
        );

        // 2) Ücretsiz hesaba geçilmiş hâli kasaya al (kimlik epostaya düşer).
        //    `mcpOAuth` burada da duruyor: Claude Code kendi yazımlarında onu
        //    koruyor, dolayısıyla geçişin de koruması gerekir.
        yaz(
            &kurum_creds,
            "{\n  \"mcpOAuth\": {\n    \"x\": 1\n  },\n  \"claudeAiOauth\": {\n    \"accessToken\": \"sk-ant-oat-UCRETSIZ\",\n    \"refreshToken\": \"sk-ant-ort-UCRETSIZ\",\n    \"subscriptionType\": \"free\"\n  }\n}\n",
        );
        yaz(
            &ayar,
            &serde_json::to_string_pretty(&json!({
                "numStartups": 77,
                "machineID": "abc",
                "oauthAccount": {
                    "accountUuid": "99999999-8888-7777-6666-555555555555",
                    "emailAddress": "ucretsiz@gmail.com"
                }
            }))
            .unwrap(),
        );
        let liste = kasa.topla(Cesit::Claude).unwrap();
        assert_eq!(liste.len(), 2);

        // 3) Kurum hesabına geri dön.
        let sonuc = kasa
            .gecis(
                Cesit::Claude,
                "28094d10-ac18-4764-b18a-8a1c5b604fc0.7f000000-0000-0000-0000-000000000001",
            )
            .unwrap();
        assert!(sonuc.degisti);

        let creds = std::fs::read_to_string(&kurum_creds).unwrap();
        let v: Value = serde_json::from_str(&creds).unwrap();
        assert_eq!(v["claudeAiOauth"]["accessToken"], "sk-ant-oat-KURUM");
        assert_eq!(v["mcpOAuth"]["x"], 1, "mcpOAuth korunmaliydi");

        let a: Value = serde_json::from_str(&std::fs::read_to_string(&ayar).unwrap()).unwrap();
        assert_eq!(
            a["oauthAccount"]["emailAddress"],
            "ercan.er@medeniyet.edu.tr"
        );
        assert_eq!(a["oauthAccount"]["organizationName"], "IMU IT Law");
        assert_eq!(
            a["machineID"], "abc",
            "kimlik disi ust anahtarlar korunmaliydi"
        );
        // Kimlik disi ust anahtarlar GECERLI dosyadan korunur: kasadaki anlik
        // goruntu eski bir `numStartups` tasiyor, gecis onu geri getirmemeli.
        assert_eq!(a["numStartups"], 77);
    }

    #[test]
    fn ayni_hesaba_gecis_yazmaz() {
        let ev = tempfile::tempdir().unwrap();
        let creds = ev.path().join(".claude/.credentials.json");
        yaz(&creds, KURUM_CREDS);
        yaz(&ev.path().join(".claude.json"), &kurum_ayar());
        let kasa = Kasa::ac_ile(ev.path().join("kasa"), ev.path());
        kasa.topla(Cesit::Claude).unwrap();
        let kimlik = kasa.liste(Cesit::Claude).unwrap()[0].kimlik.clone();
        let s = kasa.gecis(Cesit::Claude, &kimlik).unwrap();
        assert!(!s.degisti);
    }

    #[test]
    fn bilinmeyen_hesaba_gecis_hata_verir_ve_dosyaya_dokunmaz() {
        let ev = tempfile::tempdir().unwrap();
        let creds = ev.path().join(".claude/.credentials.json");
        yaz(&creds, KURUM_CREDS);
        yaz(&ev.path().join(".claude.json"), &kurum_ayar());
        let kasa = Kasa::ac_ile(ev.path().join("kasa"), ev.path());
        kasa.topla(Cesit::Claude).unwrap();
        let once = std::fs::read_to_string(&creds).unwrap();
        let hata = kasa
            .gecis(Cesit::Claude, "yok-boyle-bir-hesap")
            .unwrap_err();
        assert!(format!("{hata:#}").contains("kayitli hesap bulunamadi"));
        assert_eq!(std::fs::read_to_string(&creds).unwrap(), once);
    }

    #[test]
    fn token_yenilenmesi_cakisma_degil_guncellemedir() {
        let ev = tempfile::tempdir().unwrap();
        let creds = ev.path().join(".claude/.credentials.json");
        yaz(&creds, KURUM_CREDS);
        yaz(&ev.path().join(".claude.json"), &kurum_ayar());
        let kasa = Kasa::ac_ile(ev.path().join("kasa"), ev.path());
        kasa.topla(Cesit::Claude).unwrap();

        // Araç token'ı yeniledi: aynı hesap, yeni accessToken.
        let yenilenmis = KURUM_CREDS.replace("sk-ant-oat-KURUM", "sk-ant-oat-YENI");
        yaz(&creds, &yenilenmis);
        let liste = kasa.topla(Cesit::Claude).unwrap();
        assert_eq!(liste.len(), 1, "yenilenme yeni hesap yaratmamali");
        assert_eq!(
            liste[0].veri["claudeAiOauth"]["accessToken"],
            "sk-ant-oat-YENI"
        );
    }

    #[test]
    fn codex_kasasi_ve_gecisi_calisir() {
        let ev = tempfile::tempdir().unwrap();
        let govde_a = "eyJlbWFpbCI6ImFAYi5jbyJ9"; // {"email":"a@b.co"}
        let govde_b = "eyJlbWFpbCI6ImNAZC5lbyJ9"; // {"email":"c@d.eo"}
        let auth = ev.path().join(".codex/auth.json");
        yaz(
            &auth,
            &serde_json::to_string_pretty(&json!({
                "auth_mode": "chatgpt",
                "tokens": {"id_token": format!("x.{govde_a}.y"), "account_id": "A"}
            }))
            .unwrap(),
        );
        let kasa = Kasa::ac_ile(ev.path().join("kasa"), ev.path());
        kasa.topla(Cesit::Codex).unwrap();
        yaz(
            &auth,
            &serde_json::to_string_pretty(&json!({
                "auth_mode": "chatgpt",
                "tokens": {"id_token": format!("x.{govde_b}.y"), "account_id": "B"}
            }))
            .unwrap(),
        );
        kasa.topla(Cesit::Codex).unwrap();
        assert_eq!(kasa.liste(Cesit::Codex).unwrap().len(), 2);
        let s = kasa.gecis(Cesit::Codex, "acct:A").unwrap();
        assert!(s.degisti);
        assert_eq!(s.eposta.as_deref(), Some("a@b.co"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&auth).unwrap()).unwrap();
        assert_eq!(v["tokens"]["account_id"], "A");
    }

    #[test]
    fn gemini_ucretsiz_hesabi_kasaya_girer_ve_gecis_yapar() {
        // Gemini CLI'nin ücretsiz (Google AI Pro aboneliği olmayan) hesabı.
        // google_accounts.json `active` alanını taşır; kimlik buradan gelir.
        let ev = tempfile::tempdir().unwrap();
        let creds = ev.path().join(".gemini/oauth_creds.json");
        let hesaplar = ev.path().join(".gemini/google_accounts.json");
        yaz(
            &creds,
            "{\n  \"access_token\": \"ya29.A\",\n  \"refresh_token\": \"1//A\",\n  \"scope\": \"https://www.googleapis.com/auth/cloud-platform\",\n  \"token_type\": \"Bearer\",\n  \"expiry_date\": 1767225600000\n}\n",
        );
        yaz(
            &hesaplar,
            "{\n  \"active\": \"kisisel@gmail.com\",\n  \"old\": []\n}\n",
        );

        let kasa = Kasa::ac_ile(ev.path().join("kasa"), ev.path());
        let liste = kasa.topla(Cesit::Gemini).unwrap();
        assert_eq!(liste.len(), 1);
        assert_eq!(liste[0].kimlik, "mail:kisisel@gmail.com");
        assert_eq!(liste[0].eposta.as_deref(), Some("kisisel@gmail.com"));

        // İkinci hesap: aktif alanı değişir, eski hesap `old` altına düşer.
        yaz(
            &creds,
            "{\n  \"access_token\": \"ya29.B\",\n  \"refresh_token\": \"1//B\"\n}\n",
        );
        yaz(
            &hesaplar,
            "{\n  \"active\": \"is@gmail.com\",\n  \"old\": [\"kisisel@gmail.com\"]\n}\n",
        );
        assert_eq!(kasa.topla(Cesit::Gemini).unwrap().len(), 2);

        let s = kasa.gecis(Cesit::Gemini, "mail:kisisel@gmail.com").unwrap();
        assert!(s.degisti);
        assert_eq!(s.eposta.as_deref(), Some("kisisel@gmail.com"));
        assert_eq!(
            kasa.aktif(Cesit::Gemini).unwrap().unwrap().kimlik,
            "mail:kisisel@gmail.com"
        );

        // Biçim korunur: geri yazılan dosyada dokunulmaması gereken alanlar durur.
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&hesaplar).unwrap()).unwrap();
        assert_eq!(v["active"], "kisisel@gmail.com");
        assert_eq!(v["old"][0], "is@gmail.com");
        let c: Value = serde_json::from_str(&std::fs::read_to_string(&creds).unwrap()).unwrap();
        assert_eq!(c["refresh_token"], "1//A");
        assert_eq!(c["scope"], "https://www.googleapis.com/auth/cloud-platform");
    }

    #[test]
    fn kasa_dosyasi_depo_disinda_ve_surumlu() {
        let ev = tempfile::tempdir().unwrap();
        let kasa = Kasa::ac_ile(ev.path().join("kasa"), ev.path());
        yaz(&ev.path().join(".claude/.credentials.json"), KURUM_CREDS);
        yaz(&ev.path().join(".claude.json"), &kurum_ayar());
        kasa.topla(Cesit::Claude).unwrap();
        let ham = std::fs::read_to_string(ev.path().join("kasa/claude.json")).unwrap();
        let v: Value = serde_json::from_str(&ham).unwrap();
        assert_eq!(v["surum"], 1);
        assert_eq!(v["kayitlar"].as_array().unwrap().len(), 1);
    }
}
