//! Kervan WebExtension ile yerel WebSocket köprüsü.
//!
//! NEDEN BU KÖPRÜ VAR:
//! WebDriver (geckodriver/marionette) kullanıldığında tarayıcı 'navigator.webdriver = true'
//! ile damgalanır ve Cloudflare / Turnstile / Kimi bot kalkanları sonsuz doğrulama
//! döngüsüne girer.
//!
//! Bu modül, tarayıcının sıradan bir insan gibi çalışmasını sağlar. Tarayıcı içine
//! yerleştirilen hafif Kervan Eklentisi, 127.0.0.1:20129/ws/eklenti adresine bağlanır.
//! İstemler bu WebSocket üzerinden güvenle iletilir, DOM gözlemcisi (MutationObserver)
//! ile yanıtlar canlı olarak Kervan'a geri akar.

use anyhow::{Context, Result, bail};
use kervan_secim::{
    Anahtar, AnahtarSonucu, ModelMenusu, OgeDurum, SecimSonucu, hedef_indeks, secim_dogrulandi,
    zaten_secili,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

/// Eklentiye giden komut kuyruğunun kapasitesi. Giden akış yalnız komut taşır
/// (token parçaları değil); yavaş bir sekme bu sınırı aşarsa yeni istek açık
/// hatayla reddedilir, bellek sınırsız şişmez.
pub const GIDEN_KUYRUK_KAPASITESI: usize = 64;

/// Eklentiden Kervan'a gelen mesaj tipleri.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GelenMesaj {
    /// Eklenti sekmesi açıldığında veya yenilendiğinde kayıt olur.
    Kayit {
        provider: String,
        profile: Option<String>,
        url: Option<String>,
    },
    /// Canlı akış (SSE) parçası veya nihai yanıt.
    Parca {
        id: String,
        delta: String,
        done: bool,
    },
    /// Sayfa içinde karşılaşılan bir hata.
    Hata { id: String, mesaj: String },
    /// Sekme kapandığında yalnız o hesabın kaydını düşürür. `profile` yoksa
    /// sağlayıcının varsayılan profili anlaşılır.
    Ayril {
        provider: String,
        profile: Option<String>,
    },
    /// İnsan kapısı / güvenlik uyarısı durumu (Turnstile, Cloudflare, vb.)
    Durum {
        provider: String,
        durum: String,
        detay: Option<String>,
    },
    /// Seçim turu yanıtı: sayfadan okunan ham öğe listesi (ad, tam metin, işaret).
    ///
    /// KATLAMA/EŞLEŞTİRME BURADA YAPILMAZ: eklenti yalnız ölçümü döndürür, karar
    /// `kervan-secim::hedef_indeks` ile Rust tarafında verilir. İki katlama
    /// ayrışınca var olan seçenek "menüde yok" sayılmıştı (2026-10-22 ölçümü).
    Ogeler { id: String, ogeler: Vec<OgeDurum> },
    /// Nabız kontrolü.
    Pong,
}

/// Gönderilmek üzere hazırlanan dosya eki (görsel, video, belge).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DosyaEki {
    pub ad: String,
    pub mime_tipi: String,
    pub veri_base64: String,
}

/// Kervan'dan Eklentiye giden emirler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GidenMesaj {
    /// Sayfadaki istem kutusuna yazıp gönder.
    Sor {
        id: String,
        provider: String,
        /// Seçilen hesap profili. Eski eklenti bu alanı yok sayar ve
        /// `provider` ile arar; yeni eklenti doğrudan `provider:profile`
        /// sekmesine gider.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
        istem: String,
        #[serde(default)]
        dosyalar: Vec<DosyaEki>,
    },
    /// Devam eden üretimi durdur.
    Durdur {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
    },
    /// Sayfadaki öğeleri oku ve ham listeyi döndür.
    ///
    /// `dugme` verilirse önce o düğmeye basılır (model menüsü); menü zaten
    /// açıksa basılmaz. Okuma sonrası menü ESCAPE ile kapatılır.
    OgeOku {
        id: String,
        provider: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
        secici: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dugme: Option<String>,
    },
    /// Sırasındaki öğeye bas ve GÜNCEL listeyi döndür (doğrulama çağıranda).
    OgeBas {
        id: String,
        provider: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
        secici: String,
        indeks: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dugme: Option<String>,
    },
    /// Nabız sorusu.
    Ping,
}

/// Devam eden bir istemin yanıt kanalı.
pub enum YanitKanali {
    /// Tek seferde tam yanıt bekleyen akış.
    Tekil(oneshot::Sender<Result<String>>),
    /// Canlı akış (streaming) bekleyen akış.
    Akan(mpsc::Sender<Result<String>>),
    /// Seçim turu: okunan ham öğe listesini bekleyen akış.
    Ogeler(oneshot::Sender<Result<Vec<OgeDurum>>>),
}

/// Bağlı bir eklenti oturumu. Birden çok hesap (profil) aynı sağlayıcıda
/// olabildiği için kimlik `provider:profile` anahtarıdır.
pub struct BagliSekme {
    pub provider: String,
    pub profile: String,
    pub gonderici: mpsc::Sender<GidenMesaj>,
}

/// Seçim turlarının gideceği hesap.
///
/// Tur mesajları sekme kanalına yazılsa da eklenti arka planı hangi sekmeye
/// ileteceğini bu alanlardan çözer; `sor` ile aynı çözüm kuralı geçerli.
struct Hedef {
    provider: String,
    profile: String,
}

/// Eklenti yolunda istemden ÖNCE uygulanacak seçimler.
///
/// NEDEN TEK YAPI: model menüsü, model kodu ve aç/kapa anahtarları ayrı
/// parametre olarak verildiğinde imza okunamaz hale geliyordu (clippy 8/7).
/// Üçü de "istemden önce, aynı sekmede" kuralına tabi olduğu için birlikte
/// taşınmaları doğal.
#[derive(Debug, Default, Clone)]
pub struct EklentiSecimi {
    /// Sağlayıcının ÖLÇÜLMÜŞ model menüsü; tanımlı değilse model seçilemez.
    pub menu: Option<ModelMenusu>,
    /// `kervan.toml`daki kimlik kodu ("orta"), site etiketi değil.
    pub model_kodu: Option<String>,
    /// Aç/kapa anahtarları: (anahtar, istenen durum).
    pub anahtarlar: Vec<(Anahtar, bool)>,
}

/// Tarayıcı eklentisinin bağlanırken kullanacağı paylaşılan token.
///
/// NEDEN GEREKLİ: `/ws/eklenti` yalnız 127.0.0.1'e bağlı olsa da herhangi bir
/// web sayfası oraya WebSocket açıp `kayit` gönderebilir ve istemleri okuyabilir.
/// Token dosyası yalnız yerel kullanıcı tarafından okunur; eklenti token'ı
/// `/eklenti/token` ucundan yalnız uzantı kaynaklı (`chrome-extension://`,
/// `moz-extension://`) isteklere alır.
pub fn token_dosyasi() -> PathBuf {
    if let Some(p) = std::env::var_os("KERVAN_EKLENTI_TOKEN_DOSYASI") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(p).join("kervan").join("eklenti.token");
    }
    if let Some(h) = std::env::var_os("HOME") {
        return PathBuf::from(h)
            .join(".local")
            .join("state")
            .join("kervan")
            .join("eklenti.token");
    }
    PathBuf::from(".").join("kervan").join("eklenti.token")
}

fn rastgele_token() -> String {
    format!(
        "{:032x}{:032x}",
        rand::random::<u128>(),
        rand::random::<u128>()
    )
}

/// Diskten okur; yoksa üretip yazar. Böylece Kervan yeniden başladığında
/// eklenti yeni token istemek zorunda kalmaz.
pub fn kalici_token() -> Result<String> {
    let yol = token_dosyasi();
    if let Ok(eski) = std::fs::read_to_string(&yol) {
        let t = eski.trim().to_string();
        if t.len() >= 32 && t.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(t);
        }
    }
    let token = rastgele_token();
    if let Some(dizin) = yol.parent() {
        std::fs::create_dir_all(dizin)
            .with_context(|| format!("token dizini oluşturulamadı: {}", dizin.display()))?;
    }
    std::fs::write(&yol, &token)
        .with_context(|| format!("token dosyası yazılamadı: {}", yol.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&yol, std::fs::Permissions::from_mode(0o600));
    }
    Ok(token)
}

/// `Host` başlığı yalnız geri döngü adlarını gösterebilir. DNS rebinding ile
/// saldırgan alan adı 127.0.0.1'e çözülse bile başlık kendi adını taşır.
pub fn host_gecerli_mi(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let ad = if let Some(kalan) = h.strip_prefix('[') {
        match kalan.split_once(']') {
            Some((ic, _)) => format!("[{ic}]"),
            None => return false,
        }
    } else if let Some((ad, port)) = h.rsplit_once(':') {
        if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            ad.to_string()
        } else {
            h.clone()
        }
    } else {
        h.clone()
    };
    matches!(ad.as_str(), "127.0.0.1" | "localhost" | "[::1]")
}

/// `Origin` yoksa (tarayıcı dışı yerel istemci) kabul; varsa yalnız uzantı
/// şeması kabul. `http(s)://` sayfalar reddedilir.
pub fn origin_gecerli_mi(origin: Option<&str>) -> bool {
    let Some(o) = origin else { return true };
    let o = o.to_ascii_lowercase();
    o.starts_with("chrome-extension://")
        || o.starts_with("moz-extension://")
        || o.starts_with("safari-web-extension://")
}

/// Uzunluk sızdırmayan sabit zamanlı karşılaştırma.
pub fn token_esit_mi(gelen: &str, beklenen: &str) -> bool {
    if gelen.len() != beklenen.len() {
        return false;
    }
    gelen
        .bytes()
        .zip(beklenen.bytes())
        .fold(0u8, |fark, (a, b)| fark | (a ^ b))
        == 0
}

/// Hata metninde menünün ne gösterdiğini insan okunur biçimde verir.
/// İşaretli öğe `*` ile ayrılır: "yanlış seçenek işaretli" tanısı tek bakışta görünür.
fn oge_listesi(ogeler: &[OgeDurum]) -> String {
    if ogeler.is_empty() {
        return "hic oge yok".to_string();
    }
    ogeler
        .iter()
        .map(|o| {
            if o.isaretli {
                if o.kanit.is_empty() {
                    format!("{}*", o.metin)
                } else {
                    format!("{}*[{}]", o.metin, o.kanit)
                }
            } else {
                o.metin.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Sekme anahtarı. Profil adı `:` içerebildiği için ayırıcı ilk `:`dir.
pub fn hesap_anahtari(provider: &str, profile: &str) -> String {
    format!("{provider}:{profile}")
}

/// Zaman aşımında elde kalan yarım yanıt. Üretim `Durdur` ile kesildiği için
/// çağıran devam istemiyle sürdürüp dikebilir (`stream_recovery`).
#[derive(Debug)]
pub struct YarimYanit(pub String);

impl std::fmt::Display for YarimYanit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "yanıt yarıda kesildi ({} karakter alındı)", self.0.len())
    }
}

impl std::error::Error for YarimYanit {}

/// İstemci (HTTP bağlantısı) kopunca `sor_gonder` future'ı düşer. Bu koruma
/// kayıtları siler ve eklentiye `Durdur` gönderir; aksi hâlde `bekleyen_istekler`
/// ile `biriken_metinler` süreç ömrü boyunca sızardı ve tarayıcı üretimi
/// boşuna sürerdi.
struct IstekKoruma {
    hub: EklentiHub,
    id: String,
    provider: String,
    profile: Option<String>,
    gonderici: mpsc::Sender<GidenMesaj>,
    etkin: bool,
}

impl IstekKoruma {
    /// Normal çıkışlarda temizliği yapan yol zaten çalıştı; guard'ı kapat.
    fn etkisizlestir(&mut self) {
        self.etkin = false;
    }
}

impl Drop for IstekKoruma {
    fn drop(&mut self) {
        if !self.etkin {
            return;
        }
        let Ok(tutamac) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let hub = self.hub.clone();
        let id = self.id.clone();
        let provider = self.provider.clone();
        let profile = self.profile.clone();
        let gonderici = self.gonderici.clone();
        tutamac.spawn(async move {
            hub.bekleyen_istekler.lock().await.remove(&id);
            hub.biriken_metinler.lock().await.remove(&id);
            // Kanal dolu ya da kapanmışsa durdurma düşer; soket zaten kopmuş
            // demektir, üretimi kesecek başka bir yol yok.
            drop(gonderici.try_send(GidenMesaj::Durdur {
                id,
                provider: Some(provider),
                profile,
            }));
        });
    }
}

/// Tüm bağlı eklenti sekmelerini ve bekleyen istekleri yöneten merkezi hub.
#[derive(Clone)]
pub struct EklentiHub {
    /// `provider:profile` anahtarına göre bağlı sekmeler.
    sekmeler: Arc<Mutex<HashMap<String, BagliSekme>>>,
    /// Hesap başına kullanım sayısı (en-az-kullanılan seçimi): anahtar -> sayaç.
    kullanim: Arc<Mutex<HashMap<String, u64>>>,
    /// İstek kimliğine göre yanıt bekleyen kanallar: id -> YanitKanali
    bekleyen_istekler: Arc<Mutex<HashMap<String, YanitKanali>>>,
    /// Her istek için biriken tam metin (tekil yanıtlar için): id -> String
    biriken_metinler: Arc<Mutex<HashMap<String, String>>>,
    /// Sağlayıcıların anlık durumları (örneğin insan kapısı uyarısı): provider -> String
    durumlar: Arc<Mutex<HashMap<String, String>>>,
    /// Eklenti bağlantısında istenen paylaşılan token.
    token: Arc<String>,
    /// Hesap başına seçim/istem kilidi: `provider:profile` -> kilit.
    ///
    /// NEDEN: seçim turları (oku → tıkla → doğrula) ile istem aynı sekmede
    /// üç ayrı gidiş-dönüştür. Aynı hesaba eşzamanlı iki istek gelirse A'nın
    /// doğrulamasından sonra B'nin tıklaması sayfadaki modeli değiştirir ve
    /// A'nın istemi **B'nin modeliyle** gider (2026-10-24 incelemesi). Bu,
    /// dilimin kapatmayı vaat ettiği "sessiz yanlış model" arızasının ta
    /// kendisidir; kilit seçim + istem + yanıtı tek kritik bölgeye alır.
    kilits: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl Default for EklentiHub {
    fn default() -> Self {
        Self {
            sekmeler: Arc::new(Mutex::new(HashMap::new())),
            kullanim: Arc::new(Mutex::new(HashMap::new())),
            bekleyen_istekler: Arc::new(Mutex::new(HashMap::new())),
            biriken_metinler: Arc::new(Mutex::new(HashMap::new())),
            durumlar: Arc::new(Mutex::new(HashMap::new())),
            token: Arc::new(rastgele_token()),
            kilits: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl EklentiHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Süreç ömrü boyunca sabit kalan token'ı diskten okur (yoksa üretir).
    /// Dosya yazılamazsa geçici token'a düşer ve bunu yüksek sesle söyler.
    pub fn kalici() -> Self {
        let mut hub = Self::default();
        match kalici_token() {
            Ok(t) => hub.token = Arc::new(t),
            Err(e) => eprintln!(
                "uyari: eklenti token dosyası kullanılamadı ({e:#}); geçici token üretildi. \
                 Kervan yeniden başlarsa eklenti yeniden bağlanmalı."
            ),
        }
        hub
    }

    /// Eklentinin bağlantıda göndermesi gereken token.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Hesap kilidini verir (`provider:profile`). Kilit seçim turları ile
    /// istemi tek kritik bölgede tutar; aynı sekmede iç içe seçim olmaz.
    ///
    /// Kilit TEMBEL kurulur: hesap başına bir tane, ilk istekte. Tablo küçük
    /// kalır (görülen hesap sayısı kadar) ve sekme ayrılınca TEMİZLENMEZ.
    /// Gerekçe (2026-10-24 incelemesi, karar): temizlik, kilidi tutan istek
    /// sürerken yeni bir `Mutex` üretir; sonraki istek o yeni kilidi alır ve
    /// eski istekle aynı hesapta iç içe girer — yani temizlik, kilidin
    /// engellediği arızayı geri getirir. Sınırsız büyüme de yok: anahtar
    /// `provider:profile`, sayısı yapılandırmadaki hesaplarla sınırlı.
    async fn hesap_kilidi(&self, anahtar: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut kilitler = self.kilits.lock().await;
        kilitler
            .entry(anahtar.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Yeni bir sekme bağlandığında kaydeder. Aynı `provider:profile` yeniden
    /// bağlanırsa (sayfa yenileme) eski kaydın yerine geçer; sayaç korunur.
    pub async fn sekme_kaydet(
        &self,
        provider: String,
        profile: String,
        gonderici: mpsc::Sender<GidenMesaj>,
    ) {
        let anahtar = hesap_anahtari(&provider, &profile);
        let mut map = self.sekmeler.lock().await;
        // Yalnız YENİ kayıt duyurulur: 10 sn'lik tarama aynı sekmeyi yeniden
        // bildiriyor ve log okunamaz hale geliyordu (2026-10-24).
        if map
            .insert(
                anahtar.clone(),
                BagliSekme {
                    provider,
                    profile,
                    gonderici,
                },
            )
            .is_none()
        {
            println!("eklenti      : '{anahtar}' sekmesi bağlandı");
        }
    }

    /// Sekme kapandığında kaydı siler. `profile` yoksa sağlayıcının tüm
    /// profilleri değil, yalnız "varsayilan" profili düşer.
    pub async fn sekme_ayril(&self, provider: &str, profile: Option<&str>) {
        let profile = profile.unwrap_or("varsayilan");
        let anahtar = hesap_anahtari(provider, profile);
        let mut map = self.sekmeler.lock().await;
        if map.remove(&anahtar).is_some() {
            println!("eklenti      : '{anahtar}' sekmesi ayrıldı");
        }
        drop(map);
        self.kullanim.lock().await.remove(&anahtar);
    }

    /// Yalnız belirli bir göndericiye ait kaydı düşürür. Eski bir soket
    /// kapanırken araya giren yeni bağlantının kaydını silmemek için.
    pub async fn sekme_ayril_eger(
        &self,
        provider: &str,
        profile: &str,
        gonderici: &mpsc::Sender<GidenMesaj>,
    ) {
        let anahtar = hesap_anahtari(provider, profile);
        let mut map = self.sekmeler.lock().await;
        if map
            .get(&anahtar)
            .is_some_and(|s| s.gonderici.same_channel(gonderici))
        {
            map.remove(&anahtar);
            println!("eklenti      : '{anahtar}' sekmesi ayrıldı");
        }
        drop(map);
        if !self.sekme_var_mi(provider).await {
            self.kullanim.lock().await.remove(&anahtar);
        }
    }

    /// Bir sağlayıcının eklenti sekmesi (herhangi bir profili) canlı mı?
    pub async fn sekme_var_mi(&self, provider: &str) -> bool {
        let onek = format!("{provider}:");
        let map = self.sekmeler.lock().await;
        map.keys().any(|k| k.starts_with(&onek))
    }

    /// Sağlayıcı için en az kullanılan profili seçer ve sayacını artırır.
    ///
    /// `istenen` verilirse O profil kullanılır (panelden hesap seçimi). Profil
    /// bağlı değilse sessizce başka hesaba düşmek YASAK: kullanıcı A hesabını
    /// seçtiğini sanırken istek B'ye giderdi. Açık hata döner.
    async fn hesap_sec(
        &self,
        provider: &str,
        istenen: Option<&str>,
    ) -> Result<(mpsc::Sender<GidenMesaj>, String)> {
        let onek = format!("{provider}:");
        let map = self.sekmeler.lock().await;
        let mut adaylar: Vec<(&String, &BagliSekme)> =
            map.iter().filter(|(k, _)| k.starts_with(&onek)).collect();
        if adaylar.is_empty() {
            bail!(
                "'{provider}' için bağlı bir eklenti sekmesi bulunamadı. Tarayıcıda ilgili sayfa açık mı?"
            );
        }
        if let Some(istenen) = istenen.filter(|p| !p.trim().is_empty()) {
            let aranan = hesap_anahtari(provider, istenen.trim());
            let Some(secilen) = adaylar.iter().find(|(k, _)| **k == aranan) else {
                let mut mevcut: Vec<String> = adaylar
                    .iter()
                    .map(|(k, _)| k.trim_start_matches(&onek).to_string())
                    .collect();
                mevcut.sort();
                bail!(
                    "'{aranan}' bağlı değil. Bağlı hesaplar: {}",
                    mevcut.join(", ")
                );
            };
            let (gonderici, profile) = (secilen.1.gonderici.clone(), secilen.1.profile.clone());
            let mut kullanim = self.kullanim.lock().await;
            *kullanim.entry(aranan).or_insert(0) += 1;
            return Ok((gonderici, profile));
        }
        // Eşitlikte anahtar sırası kararlı olsun: deterministik seçim.
        adaylar.sort_by(|a, b| a.0.cmp(b.0));
        let mut kullanim = self.kullanim.lock().await;
        let Some((anahtar, gonderici, profile)) = adaylar
            .into_iter()
            .min_by_key(|(k, _)| kullanim.get(*k).copied().unwrap_or(0))
            .map(|(k, s)| (k.clone(), s.gonderici.clone(), s.profile.clone()))
        else {
            // Yukaridaki bos kontrolu bunu zaten eliyor; yine de `expect`
            // yerine acik hata: secim mantigi degisirse sessiz panik olmasin.
            bail!("'{provider}' için aday hesap seçilemedi");
        };
        *kullanim.entry(anahtar).or_insert(0) += 1;
        Ok((gonderici, profile))
    }

    /// Bir hesabın GERÇEK menüsünü okur — seçim YAPMAZ, yalnız okur.
    ///
    /// NEDEN: `kervan.toml`daki model listesi tek bir ölçümden geliyor ve
    /// sağlayıcı geneli. "Bu hesabın menüsünde Pro var mı?" sorusunun cevabı
    /// ancak o hesabın sayfasından okunur; tahmin edilemez.
    ///
    /// KULLANIM SAYACINI ARTIRMAZ: ölçüm bir istek değildir, yük dengelemesini
    /// kaydırmamalı (aksi hâlde ölçülen hesap sonraki isteklerde geri plana düşer).
    pub async fn menu_oku_hesap(
        &self,
        provider: &str,
        profil: &str,
        secici: &str,
        dugme: Option<&str>,
        zaman_asimi: std::time::Duration,
    ) -> Result<Vec<OgeDurum>> {
        let anahtar = hesap_anahtari(provider, profil);
        let gonderici = {
            let map = self.sekmeler.lock().await;
            match map.get(&anahtar) {
                Some(s) => s.gonderici.clone(),
                None => bail!("'{anahtar}' bağlı değil"),
            }
        };
        let kilit = self.hesap_kilidi(&anahtar).await;
        let _bolge = kilit.lock().await;
        let hedef = Hedef {
            provider: provider.to_string(),
            profile: profil.to_string(),
        };
        self.oge_oku(&gonderici, &hedef, secici, dugme, zaman_asimi)
            .await
    }

    /// Menüdeki tek bir öğeye basar ve GÜNCEL listeyi döndürür (ölçüm için).
    ///
    /// Sayacı artırmaz: bu bir istek değil, "bu hesapta bu seçenek gerçekten
    /// seçilebiliyor mu" sorusunun ölçümüdür.
    pub async fn menu_bas_hesap(
        &self,
        provider: &str,
        profil: &str,
        secici: &str,
        indeks: usize,
        dugme: Option<&str>,
        zaman_asimi: std::time::Duration,
    ) -> Result<Vec<OgeDurum>> {
        let anahtar = hesap_anahtari(provider, profil);
        let gonderici = {
            let map = self.sekmeler.lock().await;
            match map.get(&anahtar) {
                Some(s) => s.gonderici.clone(),
                None => bail!("'{anahtar}' bağlı değil"),
            }
        };
        let kilit = self.hesap_kilidi(&anahtar).await;
        let _bolge = kilit.lock().await;
        let hedef = Hedef {
            provider: provider.to_string(),
            profile: profil.to_string(),
        };
        self.oge_bas(&gonderici, &hedef, secici, indeks, dugme, zaman_asimi)
            .await
    }

    /// Eklentiye ek dosyalarla birlikte soru sorar ve yanıtın tamamlanmasını bekler.
    /// Sağlayıcıda birden çok profil bağlıysa en-az-kullanılan seçilir.
    pub async fn sor_ekli(
        &self,
        provider: &str,
        istem: &str,
        dosyalar: Vec<DosyaEki>,
        timeout: std::time::Duration,
    ) -> Result<String> {
        self.sor_ekli_hesap(provider, None, istem, dosyalar, timeout)
            .await
    }

    /// `sor_ekli`nin hedef hesap alabildiği biçimi. `profil = None` eskisi gibi
    /// en-az-kullanılan hesabı seçer.
    pub async fn sor_ekli_hesap(
        &self,
        provider: &str,
        profil: Option<&str>,
        istem: &str,
        dosyalar: Vec<DosyaEki>,
        timeout: std::time::Duration,
    ) -> Result<String> {
        let (tx_giden, profile) = self.hesap_sec(provider, profil).await?;
        // Aynı hesaba eşzamanlı istek: seçim turları ve istem sırayla koşmalı.
        // Kilit yanıt bitene kadar tutulur; aynı sekmede akan iki yanıt zaten
        // birbirine karışır.
        let kilit = self.hesap_kilidi(&hesap_anahtari(provider, &profile)).await;
        let _bolge = kilit.lock().await;
        let id = format!("req-{}", rand::random::<u32>());
        self.sor_gonder(
            tx_giden,
            provider,
            Some(profile),
            id,
            istem,
            dosyalar,
            timeout,
        )
        .await
    }

    /// Seçilmiş bir hesaba (veya profilesiz eski yola) istek gönderir.
    ///
    /// Argüman sayısı bilinçli: hepsi tek bir tel mesajının alanları; ayrı bir
    /// yapıya sarmak çağrı yerlerinde aynı alanları tekrar dizmekten başka
    /// bir şey kazandırmaz.
    #[allow(clippy::too_many_arguments, reason = "tel mesajı alanları tek çağrıda")]
    async fn sor_gonder(
        &self,
        tx_giden: mpsc::Sender<GidenMesaj>,
        provider: &str,
        profile: Option<String>,
        id: String,
        istem: &str,
        dosyalar: Vec<DosyaEki>,
        timeout: std::time::Duration,
    ) -> Result<String> {
        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let mut bekleyen = self.bekleyen_istekler.lock().await;
            bekleyen.insert(id.clone(), YanitKanali::Tekil(resp_tx));
            let mut biriken = self.biriken_metinler.lock().await;
            biriken.insert(id.clone(), String::new());
        }

        // İstemci koparsa bu future düşer: kayıtlar silinmeli ve üretim
        // durdurulmalı. Normal çıkışlarda `etkisizlestir` ile kapatılır.
        let mut koruma = IstekKoruma {
            hub: self.clone(),
            id: id.clone(),
            provider: provider.to_string(),
            profile: profile.clone(),
            gonderici: tx_giden.clone(),
            etkin: true,
        };

        let send_sonucu = tx_giden.try_send(GidenMesaj::Sor {
            id: id.clone(),
            provider: provider.to_string(),
            profile: profile.clone(),
            istem: istem.to_string(),
            dosyalar,
        });
        if let Err(e) = send_sonucu {
            koruma.etkisizlestir();
            self.bekleyen_istekler.lock().await.remove(&id);
            self.biriken_metinler.lock().await.remove(&id);
            bail!("eklenti istemi gönderilemedi (kuyruk dolu ya da bağlantı kapalı): {e}");
        }

        // Yanıtı zaman aşımıyla bekle
        let sonuc = match tokio::time::timeout(timeout, resp_rx).await {
            Ok(Ok(sonuc)) => sonuc,
            Ok(Err(_)) => {
                self.bekleyen_istekler.lock().await.remove(&id);
                self.biriken_metinler.lock().await.remove(&id);
                Err(anyhow::anyhow!("istem kanalı beklenmedik şekilde kapandı"))
            }
            Err(_) => {
                self.bekleyen_istekler.lock().await.remove(&id);
                let yarim = self.biriken_metinler.lock().await.remove(&id);
                // Eklentiye iptal emri ilet: üretim durur, devam istemi güvenle gönderilebilir.
                drop(tx_giden.try_send(GidenMesaj::Durdur {
                    id,
                    provider: Some(provider.to_string()),
                    profile,
                }));
                match yarim.filter(|m| !m.trim().is_empty()) {
                    Some(m) => Err(anyhow::Error::new(YarimYanit(m))),
                    None => Err(anyhow::anyhow!(
                        "{timeout:?} süre içinde yanıt tamamlanamadı (zaman aşımı)"
                    )),
                }
            }
        };
        koruma.etkisizlestir();
        sonuc
    }

    /// Eklentiye bir soru sorar ve yanıtın tamamlanmasını bekler.
    pub async fn sor(
        &self,
        provider: &str,
        istem: &str,
        timeout: std::time::Duration,
    ) -> Result<String> {
        self.sor_ekli(provider, istem, Vec::new(), timeout).await
    }

    /// Model ve aç/kapa seçimlerini uygulayıp AYNI hesaba soru sorar.
    ///
    /// NEDEN TEK ÇAĞRI: seçim turları ile istem aynı sekmeye gitmeli. Hesap
    /// seçimi (`hesap_sec`) en-az-kullanılanı döndürdüğü için iki ayrı çağrı
    /// seçimi bir sekmeye, istemi başka bir hesaba gönderebilirdi.
    ///
    /// SIRA: model önce, aç/kapa sonra — tarayıcı yolundaki `ask` ile aynı;
    /// istem yazılmadan önce seçim yapılır (sayfa menü açıkken istemi yutuyor).
    pub async fn sor_secimli(
        &self,
        provider: &str,
        istem: &str,
        dosyalar: Vec<DosyaEki>,
        secim: &EklentiSecimi,
        timeout: std::time::Duration,
    ) -> Result<String> {
        self.sor_secimli_hesap(provider, None, istem, dosyalar, secim, timeout)
            .await
    }

    /// `sor_secimli`nin hedef hesap alabildiği biçimi: panelden seçilen profil
    /// hem model/anahtar tıklamalarını hem istemi AYNI sekmeye gönderir.
    #[allow(clippy::too_many_arguments, reason = "tel mesajı alanları tek çağrıda")]
    pub async fn sor_secimli_hesap(
        &self,
        provider: &str,
        profil: Option<&str>,
        istem: &str,
        dosyalar: Vec<DosyaEki>,
        secim: &EklentiSecimi,
        timeout: std::time::Duration,
    ) -> Result<String> {
        let (tx_giden, profile) = self.hesap_sec(provider, profil).await?;
        // SEÇİM + İSTEM TEK KRİTİK BÖLGE: kilit olmadan B'nin tıklaması A'nın
        // doğrulamasından sonra sayfadaki modeli değiştirir ve A'nın istemi
        // B'nin modeliyle gider (2026-10-24 incelemesi).
        let kilit = self.hesap_kilidi(&hesap_anahtari(provider, &profile)).await;
        let _bolge = kilit.lock().await;
        let hedef = Hedef {
            provider: provider.to_string(),
            profile: profile.clone(),
        };
        let tur_zamani = timeout.min(std::time::Duration::from_secs(45));
        if let Some(kod) = secim.model_kodu.as_deref() {
            let Some(menu) = secim.menu.as_ref() else {
                bail!("'{provider}' icin olculmus model menusu tanimli degil");
            };
            let etiket = menu.etiket(kod).with_context(|| {
                format!(
                    "'{provider}' icin tanimsiz model kodu '{kod}'. Tanimli: {}",
                    menu.kodlar().join(", ")
                )
            })?;
            let sonuc = self
                .model_turu(&tx_giden, &hedef, menu, etiket, tur_zamani)
                .await?;
            println!("model        : {etiket} ({sonuc:?}) [eklenti]");
        }
        for (anahtar, istenen) in &secim.anahtarlar {
            let sonuc = self
                .anahtar_turu(&tx_giden, &hedef, anahtar, *istenen, tur_zamani)
                .await?;
            println!("anahtar      : {} ({sonuc:?}) [eklenti]", anahtar.kod);
        }
        let id = format!("req-{}", rand::random::<u32>());
        self.sor_gonder(
            tx_giden,
            provider,
            Some(profile),
            id,
            istem,
            dosyalar,
            timeout,
        )
        .await
    }

    /// Model seçimini site menüsünde uygular ve seçimi HEDEF öğenin kendi
    /// işaretinden doğrular.
    ///
    /// TIKLAMAYI ATLAMA ŞARTI (2026-10-24 incelemesi): hedefin kendi işareti
    /// GÜÇLÜ ve menüde işaretli tek öğe o. Bunun dışında her hâlde tıklanır —
    /// hedef zaten seçiliyse tıklama radyoda durumu değiştirmez, seçili
    /// değilse doğru modeli kurar. Yani şüphe tıklamaya çözülür, "zaten
    /// seçili"ye değil.
    ///
    /// NEDEN LİSTE GENELİNDE "TAM BİR İŞARET" ARANMIYOR: menü iki eksen
    /// taşıyabiliyor (gemini: mod ekseni + düşünme düzeyi; `kervan.toml:57-62`
    /// ve `kervan_secim::model_sec` belgesi). O hâlde iki işaret NORMALDİR;
    /// doğrulamayı liste sayımına bağlamak, doğru seçilmiş modeli her turda
    /// "doğrulanamadı" diye düşürürdü.
    async fn model_turu(
        &self,
        tx_giden: &mpsc::Sender<GidenMesaj>,
        hedef: &Hedef,
        menu: &ModelMenusu,
        etiket: &str,
        zaman_asimi: std::time::Duration,
    ) -> Result<SecimSonucu> {
        let ogeler = self
            .oge_oku(tx_giden, hedef, &menu.oge, Some(&menu.dugme), zaman_asimi)
            .await?;
        let Some(i) = hedef_indeks(&ogeler, etiket) else {
            bail!(
                "'{etiket}' modeli sayfadaki menude bulunamadi. Menudekiler: {}",
                oge_listesi(&ogeler)
            );
        };
        if zaten_secili(&ogeler, i) {
            return Ok(SecimSonucu::ZatenSecili);
        }
        let sonra = self
            .oge_bas(
                tx_giden,
                hedef,
                &menu.oge,
                i,
                Some(&menu.dugme),
                zaman_asimi,
            )
            .await?;
        match hedef_indeks(&sonra, etiket) {
            Some(j) if secim_dogrulandi(&sonra, j) => Ok(SecimSonucu::Degisti),
            _ => bail!(
                "'{etiket}' tiklandi ama secim dogrulanamadi (simdiki menu: {}). Sessizce \
                 yanlis modelle devam etmemek icin istek dusuruldu.",
                oge_listesi(&sonra)
            ),
        }
    }

    /// Aç/kapa özelliğini istenen duruma getirir ve durumu yeniden okuyarak doğrular.
    async fn anahtar_turu(
        &self,
        tx_giden: &mpsc::Sender<GidenMesaj>,
        hedef: &Hedef,
        anahtar: &Anahtar,
        istenen: bool,
        zaman_asimi: std::time::Duration,
    ) -> Result<AnahtarSonucu> {
        let ogeler = self
            .oge_oku(tx_giden, hedef, &anahtar.dugme, None, zaman_asimi)
            .await?;
        let Some(i) = hedef_indeks(&ogeler, &anahtar.etiket) else {
            bail!(
                "'{}' ac/kapa ozelligi sayfada bulunamadi ('{}'). Sayfadakiler: {}",
                anahtar.kod,
                anahtar.dugme,
                oge_listesi(&ogeler)
            );
        };
        if ogeler[i].isaretli == istenen {
            return Ok(AnahtarSonucu::ZatenIstendigi);
        }
        let sonra = self
            .oge_bas(tx_giden, hedef, &anahtar.dugme, i, None, zaman_asimi)
            .await?;
        // DOGRULAMA: oge tiklamadan sonra listede YOKSA durum bilinmiyor demektir;
        // "kapali" diye yorumlanip istenen=false icin sahte basari uretiyordu
        // (2026-10-24 incelemesi: istem dogrulanmadan siteye gitti).
        let Some(j) = hedef_indeks(&sonra, &anahtar.etiket) else {
            bail!(
                "'{}' ac/kapa ozelligi tiklandiktan sonra listede bulunamadi; durum \
                 dogrulanamadi, istek dusuruldu. Simdiki liste: {}",
                anahtar.kod,
                oge_listesi(&sonra)
            );
        };
        let simdi = sonra[j].isaretli;
        if simdi == istenen {
            Ok(AnahtarSonucu::Degisti)
        } else {
            bail!(
                "'{}' tiklandi ama durum degismedi (istenen: {}, simdi: {}). Ozellik plan/kota \
                 nedeniyle kilitli olabilir; istek dusuruldu.",
                anahtar.kod,
                if istenen { "acik" } else { "kapali" },
                if simdi { "acik" } else { "kapali" }
            )
        }
    }

    /// Tek bir okuma turu: eklentiye `OgeOku` gönderir, ham listeyi bekler.
    async fn oge_oku(
        &self,
        tx_giden: &mpsc::Sender<GidenMesaj>,
        hedef: &Hedef,
        secici: &str,
        dugme: Option<&str>,
        zaman_asimi: std::time::Duration,
    ) -> Result<Vec<OgeDurum>> {
        let id = format!("tur-{}", rand::random::<u32>());
        let (tx_y, rx_y) = oneshot::channel();
        self.bekleyen_istekler
            .lock()
            .await
            .insert(id.clone(), YanitKanali::Ogeler(tx_y));
        if let Err(e) = tx_giden.try_send(GidenMesaj::OgeOku {
            id: id.clone(),
            provider: hedef.provider.clone(),
            profile: Some(hedef.profile.clone()),
            secici: secici.to_string(),
            dugme: dugme.map(str::to_string),
        }) {
            self.bekleyen_istekler.lock().await.remove(&id);
            bail!("eklenti okuma turu gonderilemedi: {e}");
        }
        self.tur_yaniti(rx_y, &id, zaman_asimi).await
    }

    /// Tek bir tıklama turu: öğeye basar, eklentinin döndürdüğü GÜNCEL listeyi verir.
    async fn oge_bas(
        &self,
        tx_giden: &mpsc::Sender<GidenMesaj>,
        hedef: &Hedef,
        secici: &str,
        indeks: usize,
        dugme: Option<&str>,
        zaman_asimi: std::time::Duration,
    ) -> Result<Vec<OgeDurum>> {
        let id = format!("tur-{}", rand::random::<u32>());
        let (tx_y, rx_y) = oneshot::channel();
        self.bekleyen_istekler
            .lock()
            .await
            .insert(id.clone(), YanitKanali::Ogeler(tx_y));
        if let Err(e) = tx_giden.try_send(GidenMesaj::OgeBas {
            id: id.clone(),
            provider: hedef.provider.clone(),
            profile: Some(hedef.profile.clone()),
            secici: secici.to_string(),
            indeks,
            dugme: dugme.map(str::to_string),
        }) {
            self.bekleyen_istekler.lock().await.remove(&id);
            bail!("eklenti tiklama turu gonderilemedi: {e}");
        }
        self.tur_yaniti(rx_y, &id, zaman_asimi).await
    }

    /// Tur yanıtını zaman aşımıyla bekler; süre dolunca kayıt DÜŞÜRÜLÜR.
    async fn tur_yaniti(
        &self,
        rx_y: oneshot::Receiver<Result<Vec<OgeDurum>>>,
        id: &str,
        zaman_asimi: std::time::Duration,
    ) -> Result<Vec<OgeDurum>> {
        match tokio::time::timeout(zaman_asimi, rx_y).await {
            Ok(Ok(sonuc)) => sonuc,
            Ok(Err(_)) => {
                self.bekleyen_istekler.lock().await.remove(id);
                bail!("eklenti secim turu kanali beklenmedik sekilde kapandi")
            }
            Err(_) => {
                self.bekleyen_istekler.lock().await.remove(id);
                bail!("eklenti secim turu zaman asimina ugradi ({zaman_asimi:?})")
            }
        }
    }

    /// Eklentiden gelen bir mesajı işler.
    pub async fn mesaji_isle(&self, mesaj: GelenMesaj) {
        match mesaj {
            GelenMesaj::Kayit {
                provider,
                profile,
                url: _,
            } => {
                // Kayıt işi genelde WS el sıkışmasında atanır, ancak dinamik güncelleme de desteklenir.
                let prof = profile.unwrap_or_else(|| "varsayilan".into());
                let mut durumlar = self.durumlar.lock().await;
                durumlar.insert(provider, format!("Hazır ({prof})"));
            }
            GelenMesaj::Parca { id, delta, done } => {
                let mut bitir = false;
                let mut son_metin = String::new();

                {
                    let mut biriken = self.biriken_metinler.lock().await;
                    if let Some(m) = biriken.get_mut(&id) {
                        m.push_str(&delta);
                        if done {
                            son_metin = m.clone();
                            bitir = true;
                        }
                    }
                }

                if bitir {
                    self.biriken_metinler.lock().await.remove(&id);
                    let mut bekleyen = self.bekleyen_istekler.lock().await;
                    if let Some(YanitKanali::Tekil(tx)) = bekleyen.remove(&id) {
                        // Alıcı düşmüşse yanıt kaybolur; bekleyen istek yok.
                        drop(tx.send(Ok(son_metin)));
                    }
                }
            }
            GelenMesaj::Hata { id, mesaj } => {
                self.biriken_metinler.lock().await.remove(&id);
                let mut bekleyen = self.bekleyen_istekler.lock().await;
                match bekleyen.remove(&id) {
                    Some(YanitKanali::Tekil(tx)) => {
                        // Alıcı düşmüşse hata kaybolur; bekleyen istek yok.
                        drop(tx.send(Err(anyhow::anyhow!("{mesaj}"))));
                    }
                    Some(YanitKanali::Ogeler(tx)) => {
                        drop(tx.send(Err(anyhow::anyhow!("{mesaj}"))));
                    }
                    _ => {}
                }
            }
            GelenMesaj::Ogeler { id, ogeler } => {
                let mut bekleyen = self.bekleyen_istekler.lock().await;
                if let Some(YanitKanali::Ogeler(tx)) = bekleyen.remove(&id) {
                    drop(tx.send(Ok(ogeler)));
                }
            }
            GelenMesaj::Durum {
                provider,
                durum,
                detay,
            } => {
                let text = match detay {
                    Some(d) => format!("{durum}: {d}"),
                    None => durum,
                };
                println!("eklenti-durum : [{provider}] {text}");
                let mut durumlar = self.durumlar.lock().await;
                durumlar.insert(provider, text);
            }
            GelenMesaj::Ayril { provider, profile } => {
                let anahtar = hesap_anahtari(&provider, profile.as_deref().unwrap_or("varsayilan"));
                let mut map = self.sekmeler.lock().await;
                if map.remove(&anahtar).is_some() {
                    println!("eklenti      : '{anahtar}' sekmesi ayrıldı");
                }
                drop(map);
                self.kullanim.lock().await.remove(&anahtar);
            }
            GelenMesaj::Pong => {}
        }
    }

    /// Bağlı tüm sekmelerin sağlayıcı adlarını (tekilleştirilmiş) döner.
    /// Yönlendirme adayları sağlayıcı düzeyindedir; profiller `bagli_hesaplar`
    /// ile ayrıca görünür.
    pub async fn bagli_sekmeler(&self) -> Vec<String> {
        let map = self.sekmeler.lock().await;
        let mut adlar: Vec<String> = map.values().map(|s| s.provider.clone()).collect();
        adlar.sort();
        adlar.dedup();
        adlar
    }

    /// Bağlı hesapların `provider:profile` anahtarlarını döner.
    pub async fn bagli_hesaplar(&self) -> Vec<String> {
        let map = self.sekmeler.lock().await;
        let mut anahtarlar: Vec<String> = map.keys().cloned().collect();
        anahtarlar.sort();
        anahtarlar
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kervan_secim::ModelSecenegi;

    #[test]
    fn test_giden_mesaj_sor_dosyasiz_geriye_uyumluluk() {
        // Eski eklenti/istek formatı (dosyalar alanı yok)
        let json = r#"{"action":"sor","id":"req-1","provider":"gemini","istem":"Merhaba"}"#;
        let msg: GidenMesaj = serde_json::from_str(json).expect("deserialize edilmeli");
        match msg {
            GidenMesaj::Sor {
                id,
                provider,
                istem,
                dosyalar,
                ..
            } => {
                assert_eq!(id, "req-1");
                assert_eq!(provider, "gemini");
                assert_eq!(istem, "Merhaba");
                assert!(dosyalar.is_empty(), "dosyalar varsayılan boş vektör olmalı");
            }
            _ => panic!("GidenMesaj::Sor bekleniyordu"),
        }
    }

    #[test]
    fn test_giden_mesaj_sor_dosyali_ser_de() {
        let msg = GidenMesaj::Sor {
            id: "req-2".into(),
            provider: "gemini".into(),
            profile: None,
            istem: "Bu resmi incele".into(),
            dosyalar: vec![DosyaEki {
                ad: "test.png".into(),
                mime_tipi: "image/png".into(),
                veri_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==".into(),
            }],
        };

        let json = serde_json::to_string(&msg).expect("serialize edilmeli");
        assert!(json.contains("\"dosyalar\""));
        assert!(json.contains("test.png"));
        assert!(json.contains("image/png"));

        let deserialized: GidenMesaj = serde_json::from_str(&json).expect("deserialize edilmeli");
        match deserialized {
            GidenMesaj::Sor { dosyalar, .. } => {
                assert_eq!(dosyalar.len(), 1);
                assert_eq!(dosyalar[0].ad, "test.png");
                assert_eq!(dosyalar[0].mime_tipi, "image/png");
            }
            _ => panic!("GidenMesaj::Sor bekleniyordu"),
        }
    }

    /// Zaman aşımında biriken metin atılmaz: `YarimYanit` olarak döner ve üretim durdurulur.
    #[tokio::test]
    async fn zaman_asiminda_yarim_metin_doner() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;
        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
                panic!("önce Sor gelmeli")
            };
            let parca = GelenMesaj::Parca {
                id,
                delta: "Yarım cümle".into(),
                done: false,
            };
            h.mesaji_isle(parca).await;
            rx.recv().await
        });
        let hata = hub
            .sor("gemini", "soru", std::time::Duration::from_millis(300))
            .await
            .unwrap_err();
        let yarim = hata
            .downcast::<YarimYanit>()
            .expect("YarimYanit bekleniyordu");
        assert_eq!(yarim.0, "Yarım cümle");
        assert!(matches!(
            besleyici.await.unwrap(),
            Some(GidenMesaj::Durdur { .. })
        ));
    }

    /// İki profil bağlıyken hesap seçimi en-az-kullanılanı izler: 4 seçimde
    /// her profil 2 kez. Aynı sağlayıcının profilleri birbirini ezmez.
    #[tokio::test]
    async fn en_az_kullanilan_hesap_seciliyor() {
        let hub = EklentiHub::new();
        let (tx_a, _rx_a) = mpsc::channel(8);
        let (tx_b, _rx_b) = mpsc::channel(8);
        hub.sekme_kaydet("gemini".into(), "a".into(), tx_a).await;
        hub.sekme_kaydet("gemini".into(), "b".into(), tx_b).await;

        let mut secilenler = Vec::new();
        for _ in 0..4 {
            let (_tx, profile) = hub.hesap_sec("gemini", None).await.unwrap();
            secilenler.push(profile);
        }
        secilenler.sort();
        assert_eq!(secilenler, vec!["a", "a", "b", "b"]);
        assert_eq!(hub.bagli_hesaplar().await, vec!["gemini:a", "gemini:b"]);
        assert_eq!(hub.bagli_sekmeler().await, vec!["gemini"]);
    }

    /// Eski eklenti `profile` alanı göndermez; ayrıştırma düşmemeli ve sunucu
    /// `Sor` içinde profili `None` bırakmalı (eklenti kendi varsayılanına düşer).
    #[test]
    fn eski_eklenti_profilsiz_calismaya_devam_eder() {
        let json = r#"{"action":"sor","id":"req-9","provider":"gemini","istem":"Merhaba"}"#;
        let msg: GidenMesaj = serde_json::from_str(json).expect("profil alanı zorunlu değil");
        match msg {
            GidenMesaj::Sor { profile, .. } => assert_eq!(profile, None),
            _ => panic!("Sor bekleniyordu"),
        }
    }

    #[test]
    fn host_denetimi_yalniz_geri_donguye_izin_verir() {
        assert!(host_gecerli_mi("127.0.0.1:20129"));
        assert!(host_gecerli_mi("localhost:20129"));
        assert!(host_gecerli_mi("[::1]:20129"));
        assert!(host_gecerli_mi("LOCALHOST"));
        assert!(!host_gecerli_mi("kervan.example.com:20129"));
        assert!(!host_gecerli_mi("127.0.0.1.nip.io:20129"));
        assert!(!host_gecerli_mi(""));
    }

    #[test]
    fn origin_denetimi_web_sayfasini_reddeder() {
        assert!(origin_gecerli_mi(None));
        assert!(origin_gecerli_mi(Some("chrome-extension://abcdef")));
        assert!(origin_gecerli_mi(Some("moz-extension://1111-2222")));
        assert!(!origin_gecerli_mi(Some("https://evil.example")));
        assert!(!origin_gecerli_mi(Some("http://127.0.0.1:20129")));
        assert!(!origin_gecerli_mi(Some("null")));
    }

    #[test]
    fn token_karsilastirmasi_uzunluk_ve_icerik_duyarli() {
        assert!(token_esit_mi("abc123", "abc123"));
        assert!(!token_esit_mi("abc123", "abc124"));
        assert!(!token_esit_mi("abc", "abc123"));
        assert!(!token_esit_mi("", "abc123"));
    }

    /// İstemci kopunca (`sor` future'ı düşünce) iki kayıt da silinmeli ve
    /// eklentiye `Durdur` gitmeli. Guard olmadan bu kayıtlar süreç ömrü boyunca
    /// kalıyordu (S4).
    #[tokio::test]
    async fn istemci_kopunca_kayit_silinir_ve_durdur_gonderilir() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let gorev = tokio::spawn({
            let h = hub.clone();
            async move {
                h.sor("gemini", "soru", std::time::Duration::from_secs(60))
                    .await
            }
        });

        assert!(matches!(rx.recv().await, Some(GidenMesaj::Sor { .. })));
        assert_eq!(hub.bekleyen_istekler.lock().await.len(), 1);
        assert_eq!(hub.biriken_metinler.lock().await.len(), 1);

        gorev.abort();
        let _ = gorev.await;

        let durdur = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("temizlik görevi çalışmalı")
            .expect("kanal açık kalmalı");
        assert!(matches!(durdur, GidenMesaj::Durdur { .. }));
        assert!(hub.bekleyen_istekler.lock().await.is_empty());
        assert!(hub.biriken_metinler.lock().await.is_empty());
    }

    fn deneme_menusu() -> ModelMenusu {
        ModelMenusu {
            dugme: "button#m".into(),
            oge: "[role=menuitem]".into(),
            modeller: vec![
                ModelSecenegi {
                    kod: "hizli".into(),
                    etiket: "Hızlı".into(),
                    kilitli: false,
                    kilit_nedeni: None,
                },
                ModelSecenegi {
                    kod: "yuksek".into(),
                    etiket: "Yüksek".into(),
                    kilitli: false,
                    kilit_nedeni: None,
                },
            ],
        }
    }

    fn oge(metin: &str, isaretli: bool) -> OgeDurum {
        OgeDurum {
            metin: metin.into(),
            tam: metin.into(),
            isaretli,
            kanit: String::new(),
        }
    }

    /// Tel biçimi sabittir: eklenti bu adları bekler (`action` + alanlar).
    #[test]
    fn secim_turu_tel_bicimi() {
        let oku = serde_json::to_value(GidenMesaj::OgeOku {
            id: "tur-1".into(),
            provider: "gemini".into(),
            profile: Some("p".into()),
            secici: "[role=menuitem]".into(),
            dugme: Some("button#m".into()),
        })
        .unwrap();
        assert_eq!(oku["action"], "oge_oku");
        assert_eq!(oku["indeks"], serde_json::Value::Null);

        let bas = serde_json::to_value(GidenMesaj::OgeBas {
            id: "tur-2".into(),
            provider: "gemini".into(),
            profile: None,
            secici: ".toggle".into(),
            indeks: 3,
            dugme: None,
        })
        .unwrap();
        assert_eq!(bas["action"], "oge_bas");
        assert_eq!(bas["indeks"], 3);
        assert!(bas.get("dugme").is_none(), "boş dugme tele yazılmamalı");

        let yanit: GelenMesaj = serde_json::from_str(
            r#"{"action":"ogeler","id":"tur-2","ogeler":[{"metin":"Yüksek","tam":"Yüksek","isaretli":true}]}"#,
        )
        .expect("eklenti yanıtı çözülmeli");
        match yanit {
            GelenMesaj::Ogeler { id, ogeler } => {
                assert_eq!(id, "tur-2");
                assert!(ogeler[0].isaretli);
                assert_eq!(ogeler[0].metin, "Yüksek");
            }
            _ => panic!("Ogeler bekleniyordu"),
        }
    }

    /// Mutlu yol: menü okunur, DOĞRU indekse tıklanır, seçim doğrulanır ve
    /// istem AYNI hesaba gider (seçim başka sekmeye kaçmaz).
    #[tokio::test]
    async fn secim_turu_dogru_indekse_tiklar_ve_ayni_hesaba_sorar() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::OgeOku {
                id,
                provider,
                profile,
                secici,
                dugme,
            }) = rx.recv().await
            else {
                panic!("önce OgeOku gelmeli")
            };
            assert_eq!(provider, "gemini");
            assert_eq!(profile.as_deref(), Some("p"));
            assert_eq!(secici, "[role=menuitem]");
            assert_eq!(dugme.as_deref(), Some("button#m"));
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", true), oge("Yüksek", false)],
            })
            .await;

            let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
                panic!("sonra OgeBas gelmeli")
            };
            assert_eq!(indeks, 1, "Yüksek ikinci sırada");
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
            })
            .await;

            let Some(GidenMesaj::Sor { id, profile, .. }) = rx.recv().await else {
                panic!("en son Sor gelmeli")
            };
            assert_eq!(profile.as_deref(), Some("p"));
            h.mesaji_isle(GelenMesaj::Parca {
                id: id.clone(),
                delta: "yanıt".into(),
                done: false,
            })
            .await;
            h.mesaji_isle(GelenMesaj::Parca {
                id,
                delta: String::new(),
                done: true,
            })
            .await;
        });

        let menu = deneme_menusu();
        let yanit = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(menu.clone()),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(5),
            )
            .await
            .expect("seçim ve istem tamamlanmalı");
        assert_eq!(yanit, "yanıt");
        besleyici.await.unwrap();
    }

    /// Menüde zaten seçiliyse TIKLAMA YAPILMAZ (gereksiz tıklama seçimi bozabilir).
    #[tokio::test]
    async fn secili_model_icin_tiklama_yapilmaz() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
                panic!("OgeOku gelmeli")
            };
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
            })
            .await;
            let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
                panic!("tıklama olmadan Sor gelmeli")
            };
            h.mesaji_isle(GelenMesaj::Parca {
                id,
                delta: "tamam".into(),
                done: true,
            })
            .await;
        });

        let menu = deneme_menusu();
        let yanit = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(menu.clone()),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(yanit, "tamam");
        besleyici.await.unwrap();
    }

    /// ALT DÜĞÜM İŞARETİ GÜÇLÜ KANITI GÖLGELEMEZ (2026-10-24 canlı ölçümü).
    ///
    /// Menü "3.6 Flash*[sinif:selected] | 3.6 Düşünen*[torun:aria-checked=true]"
    /// okunmuştu. Hedef "Düşünen" olsaydı eski kod onu işaretli sanıp tıklamayı
    /// atlar, istek sessizce Flash ile giderdi. Güçlü kanıt varken zayıf kanıt
    /// yok hükmündedir; hedef tıklanır.
    #[tokio::test]
    async fn alt_dugum_isareti_guclu_kaniti_golgelemez() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
                panic!("OgeOku gelmeli")
            };
            let kanitli = |ad: &str, kanit: &str| OgeDurum {
                metin: ad.into(),
                tam: ad.into(),
                isaretli: true,
                kanit: kanit.into(),
            };
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![
                    kanitli("Hızlı", "sinif:selected"),
                    kanitli("Yüksek", "torun:aria-checked=true"),
                ],
            })
            .await;
            let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
                panic!("güçlü kanıt varken hedefe tıklanmalı")
            };
            assert_eq!(indeks, 1);
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
            })
            .await;
            let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
                panic!("doğrulama sonrası Sor gelmeli")
            };
            h.mesaji_isle(GelenMesaj::Parca {
                id,
                delta: "tamam".into(),
                done: true,
            })
            .await;
        });

        let yanit = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(deneme_menusu()),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(yanit, "tamam");
        besleyici.await.unwrap();
    }

    /// Zayıf kanıt tıklamayı ATLAMAYA yetmez; tıklama sonrası doğrulamaya yeter
    /// (yalnız listede zayıf kanıtlı tek öğe ise).
    #[test]
    fn zayif_kanit_atlamaya_yetmez_dogrulamaya_yeter() {
        let kanitli = |ad: &str, kanit: &str| OgeDurum {
            metin: ad.into(),
            tam: ad.into(),
            isaretli: true,
            kanit: kanit.into(),
        };
        let liste = vec![
            oge("Hızlı", false),
            kanitli("Yüksek", "torun:aria-checked=true"),
        ];
        assert!(!zaten_secili(&liste, 1), "zayıf kanıt atlamaya yetmez");
        assert!(secim_dogrulandi(&liste, 1), "tek zayıf işaret hedefte");
    }

    /// İKİ EKSENLİ MENÜ (2026-10-24 incelemesinin blocker'ı): mod ekseni güçlü
    /// işaretli, düşünme düzeyi hedefi zayıf işaretli. Kural liste genelinde
    /// "tam bir işaret" arasaydı bu menüde seçim HİÇ doğrulanamaz, her istek
    /// düşerdi. Tıklanır ve hedefin kendi kanıtıyla doğrulanır.
    #[test]
    fn iki_eksenli_menude_hedef_dogrulanir() {
        let kanitli = |ad: &str, kanit: &str| OgeDurum {
            metin: ad.into(),
            tam: ad.into(),
            isaretli: true,
            kanit: kanit.into(),
        };
        let liste = vec![
            kanitli("Otomatik", "sinif:selected"),
            kanitli("Yüksek", "torun:aria-checked=true"),
        ];
        assert!(
            !zaten_secili(&liste, 1),
            "iki eksenli menüde atlama hükmü verilmez"
        );
        assert!(
            secim_dogrulandi(&liste, 1),
            "hedef tek zayıf işaretli öğeyse doğrulanır"
        );
    }

    /// Gemini'nin gerçek işareti `aria-label="Seçildi"` (güçlü), yanlış
    /// eşleşme ise çıplak `aria-checked` (zayıf). Güçlü kanıtlı hedef, başka
    /// satır işaretli olsa da doğrulanır; zayıf kanıtlı satır atlamaya yetmez.
    #[test]
    fn secildi_etiketi_cikplak_aria_checked_den_ustundur() {
        let kanitli = |ad: &str, kanit: &str| OgeDurum {
            metin: ad.into(),
            tam: ad.into(),
            isaretli: true,
            kanit: kanit.into(),
        };
        let liste = vec![
            kanitli("3.6 Flash", "torun:secildi"),
            kanitli("3.6 Düşünen", "torun:aria-checked=true"),
        ];
        assert!(
            secim_dogrulandi(&liste, 0),
            "güçlü kanıtlı öğe seçili sayılmalı"
        );
        assert!(!zaten_secili(&liste, 1), "zayıf kanıt atlamaya yetmez");
    }

    /// ÇOK İŞARETLİ MENÜ (2026-10-24 canlı ölçümü: "3.6 Flash* | 3.6 Düşünen*").
    ///
    /// Hedef "işaretli" görünse bile atlama hükmü verilmez (atlamak için güçlü
    /// ve TEK işaret gerekir); tıklanır ve hedefin kendi kanıtıyla doğrulanır.
    #[tokio::test]
    async fn cok_isaretli_menude_tiklama_atlanmaz() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
                panic!("OgeOku gelmeli")
            };
            // Bozuk okuma: iki öğe birden işaretli, hedef de aralarında.
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", true), oge("Yüksek", true)],
            })
            .await;
            let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
                panic!("çok işaretli menüde OgeBas gelmeli")
            };
            assert_eq!(indeks, 1, "hedef öğe (Yüksek) tıklanmalı");
            // Tıklama sonrası sağlıklı okuma: tek işaret, o da hedefte.
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", false), oge("Yüksek", true)],
            })
            .await;
            let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
                panic!("doğrulama sonrası Sor gelmeli")
            };
            h.mesaji_isle(GelenMesaj::Parca {
                id,
                delta: "tamam".into(),
                done: true,
            })
            .await;
        });

        let yanit = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(deneme_menusu()),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(yanit, "tamam");
        besleyici.await.unwrap();
    }

    /// İKİ EKSENLİ MENÜ UÇTAN UCA (2026-10-24 incelemesinin blocker'ı).
    ///
    /// Gerçek gemini menüsü: mod ekseni `sinif:selected` ile GÜÇLÜ işaretli,
    /// düşünme düzeyi hedefi (`Yüksek`) alt düğüm `aria-checked` ile ZAYIF
    /// işaretli. Eski kural liste genelinde "tam bir işaret" aradığı için bu
    /// menüde seçimi hiç doğrulayamaz, her isteği düşürürdü. Doğru davranış:
    /// tıkla, sonra hedefin kendi kanıtına bak (zayıf işaret tek zayıfsa yeter)
    /// ve isteği gönder — mod ekseninin güçlü işareti hedefi geçersiz kılmaz.
    #[tokio::test]
    async fn iki_eksenli_menude_istem_gonderilir() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let kanitli = |ad: &str, kanit: &str| OgeDurum {
            metin: ad.into(),
            tam: ad.into(),
            isaretli: true,
            kanit: kanit.into(),
        };
        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
                panic!("OgeOku gelmeli")
            };
            // Mod ekseni güçlü + hedef zayıf: tıklanmalı (atlanmamalı).
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![
                    kanitli("Otomatik", "sinif:selected"),
                    kanitli("Yüksek", "torun:aria-checked=true"),
                ],
            })
            .await;
            let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
                panic!("iki eksenli menüde hedefe tıklanmalı")
            };
            assert_eq!(indeks, 1, "hedef öğe (Yüksek) tıklanmalı");
            // Tıklama sonrası menü yine iki eksenli; hedef zayıf işaretli.
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![
                    kanitli("Otomatik", "sinif:selected"),
                    kanitli("Yüksek", "torun:aria-checked=true"),
                ],
            })
            .await;
            let Some(GidenMesaj::Sor { id, .. }) = rx.recv().await else {
                panic!("iki eksenli menüde doğrulama sonrası Sor gelmeli")
            };
            h.mesaji_isle(GelenMesaj::Parca {
                id,
                delta: "tamam".into(),
                done: true,
            })
            .await;
        });

        let yanit = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(deneme_menusu()),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(yanit, "tamam");
        besleyici.await.unwrap();
    }

    /// Seçim DOĞRULANAMAZSA istem hiç gönderilmez: sessizce yanlış modelle
    /// cevap vermek en pahalı hata türü.
    #[tokio::test]
    async fn dogrulanmayan_secim_istemi_dusurur() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
                panic!("OgeOku gelmeli")
            };
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", true), oge("Yüksek", false)],
            })
            .await;
            let Some(GidenMesaj::OgeBas { id, .. }) = rx.recv().await else {
                panic!("OgeBas gelmeli")
            };
            // Tıklama işe yaramadı: işaret yerinde değil.
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![oge("Hızlı", true), oge("Yüksek", false)],
            })
            .await;
            // Sonraki mesaj Sor OLSAYDI test kırılırdı; 1 sn bekleyip None döner.
            tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .ok()
                .flatten()
        });

        let menu = deneme_menusu();
        let hata = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(menu.clone()),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(5),
            )
            .await
            .expect_err("doğrulanmayan seçim hata vermeli");
        assert!(
            format!("{hata:#}").contains("dogrulanamadi"),
            "hata doğrulamayı söylemeli: {hata:#}"
        );
        assert!(
            besleyici.await.unwrap().is_none(),
            "istem gönderilmemeliydi"
        );
        assert!(hub.bekleyen_istekler.lock().await.is_empty());
    }

    /// Tur yanıtı gelmezse bekleyen kayıt DÜŞÜRÜLÜR; sızıntı süreç ömrü boyunca kalırdı.
    #[tokio::test]
    async fn zaman_asiminda_bekleyen_tur_silinir() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let h = hub.clone();
        tokio::spawn(async move {
            assert!(matches!(rx.recv().await, Some(GidenMesaj::OgeOku { .. })));
            // Yanıt yok: zaman aşımı bekleniyor.
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let _ = &h;
        });

        let menu = deneme_menusu();
        let hata = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(menu.clone()),
                    model_kodu: Some("yuksek".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_millis(300),
            )
            .await
            .expect_err("tur zaman aşımına uğramalı");
        assert!(
            format!("{hata:#}").contains("zaman asimina"),
            "hata zaman aşımını söylemeli: {hata:#}"
        );
        assert!(
            hub.bekleyen_istekler.lock().await.is_empty(),
            "zaman aşımından sonra bekleyen kayıt kalmamalı"
        );
    }

    /// Tanımsız model kodu seçim turuna ÇIKMADAN hata verir (menüde arama yok).
    #[tokio::test]
    async fn tanimsiz_kod_tura_cikmadan_hata_verir() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;
        let menu = deneme_menusu();
        let hata = hub
            .sor_secimli(
                "gemini",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: Some(menu.clone()),
                    model_kodu: Some("yok-boyle".into()),
                    anahtarlar: Vec::new(),
                },
                std::time::Duration::from_secs(2),
            )
            .await
            .expect_err("tanımsız kod hata vermeli");
        assert!(format!("{hata:#}").contains("tanimsiz model kodu"));
        assert!(
            rx.try_recv().is_err(),
            "eklentiye hiçbir mesaj gitmemeliydi"
        );
    }

    /// Aç/kapa doğrulaması "yokluk = kapalı" saymamalı.
    ///
    /// 2026-10-24 incelemesi: tıklamadan sonra öğe listeden kaybolduğunda
    /// `unwrap_or(false)` devreye giriyor, istenen durum `false` ise SAHTE
    /// BAŞARI üretiyor ve istem doğrulanmamış seçimle siteye gidiyordu.
    #[tokio::test]
    async fn kaybolan_anahtar_sahte_basari_vermez() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("deepseek".into(), "p".into(), tx).await;
        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            let Some(GidenMesaj::OgeOku { id, .. }) = rx.recv().await else {
                panic!("OgeOku bekleniyordu")
            };
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: vec![OgeDurum {
                    metin: "DeepThink".into(),
                    tam: "DeepThink".into(),
                    isaretli: true,
                    kanit: String::new(),
                }],
            })
            .await;
            let Some(GidenMesaj::OgeBas { id, indeks, .. }) = rx.recv().await else {
                panic!("OgeBas bekleniyordu")
            };
            assert_eq!(indeks, 0);
            // Tıklamadan sonra öğe sayfadan kayboldu: liste boş.
            h.mesaji_isle(GelenMesaj::Ogeler {
                id,
                ogeler: Vec::new(),
            })
            .await;
            // Buradan sonra istem GELMEMELİ.
            rx.try_recv()
        });

        let hata = hub
            .sor_secimli(
                "deepseek",
                "soru",
                Vec::new(),
                &EklentiSecimi {
                    menu: None,
                    model_kodu: None,
                    anahtarlar: vec![(
                        Anahtar {
                            kod: "derin".into(),
                            dugme: ".ds-toggle-button".into(),
                            etiket: "DeepThink".into(),
                        },
                        false,
                    )],
                },
                std::time::Duration::from_secs(2),
            )
            .await
            .expect_err("kaybolan öğe doğrulanamadı sayılmalı");
        assert!(
            format!("{hata:#}").contains("durum dogrulanamadi"),
            "beklenen hata: {hata:#}"
        );
        assert!(
            besleyici.await.unwrap().is_err(),
            "doğrulanmayan seçimde istem eklentiye gitmemeliydi"
        );
    }

    /// AYNI HESABA EŞZAMANLI İKİ İSTEK İÇ İÇE GEÇMEZ (2026-10-24 incelemesi).
    ///
    /// Seçim turları üç ayrı gidiş-dönüştür (oku → tıkla → doğrula). Kilit
    /// yokken A'nın doğrulaması bittikten sonra B'nin tıklaması sayfadaki
    /// modeli değiştiriyor, A'nın istemi **B'nin modeliyle** gidiyordu; üstelik
    /// seçim "doğrulanmış" göründüğü için arıza sessiz kalıyordu.
    ///
    /// Ölçüt: eklentiye giden mesajlarda bir isteğin kimliği bitmeden
    /// diğerinin kimliği başlamamalı (bitişiklik).
    #[tokio::test]
    async fn es_zamanli_istekler_ayni_hesapta_ic_ice_gecmez() {
        let hub = EklentiHub::new();
        let (tx, mut rx) = mpsc::channel(GIDEN_KUYRUK_KAPASITESI);
        hub.sekme_kaydet("gemini".into(), "p".into(), tx).await;

        let h = hub.clone();
        let besleyici = tokio::spawn(async move {
            // Sahte sayfa: seçili modelin indeksi. Tıklama bunu değiştirir.
            let mut secili = 0usize;
            let mut sira: Vec<(String, &'static str)> = Vec::new();
            let mut biten = 0usize;
            while let Some(mesaj) = rx.recv().await {
                match mesaj {
                    GidenMesaj::OgeOku { id, .. } => {
                        sira.push((id.clone(), "oku"));
                        h.mesaji_isle(GelenMesaj::Ogeler {
                            id,
                            ogeler: vec![oge("Hızlı", secili == 0), oge("Yüksek", secili == 1)],
                        })
                        .await;
                    }
                    GidenMesaj::OgeBas { id, indeks, .. } => {
                        sira.push((id.clone(), "bas"));
                        secili = indeks;
                        h.mesaji_isle(GelenMesaj::Ogeler {
                            id,
                            ogeler: vec![oge("Hızlı", secili == 0), oge("Yüksek", secili == 1)],
                        })
                        .await;
                    }
                    GidenMesaj::Sor { id, .. } => {
                        sira.push((id.clone(), "sor"));
                        // Yayın sürüyor: kilit olmasa diğer istek TAM bu boşlukta
                        // seçim turlarına girerdi.
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        h.mesaji_isle(GelenMesaj::Parca {
                            id: id.clone(),
                            delta: "ok".into(),
                            done: false,
                        })
                        .await;
                        sira.push((id.clone(), "bitti"));
                        h.mesaji_isle(GelenMesaj::Parca {
                            id,
                            delta: String::new(),
                            done: true,
                        })
                        .await;
                        biten += 1;
                        if biten == 2 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            sira
        });

        let menu = deneme_menusu();
        let istek = |k: &'static str| {
            let hub = hub.clone();
            let menu = menu.clone();
            tokio::spawn(async move {
                hub.sor_secimli(
                    "gemini",
                    k,
                    Vec::new(),
                    &EklentiSecimi {
                        menu: Some(menu),
                        model_kodu: Some("yuksek".into()),
                        anahtarlar: Vec::new(),
                    },
                    std::time::Duration::from_secs(5),
                )
                .await
            })
        };
        let (a, b) = (istek("bir"), istek("iki"));
        assert_eq!(a.await.unwrap().unwrap(), "ok");
        assert_eq!(b.await.unwrap().unwrap(), "ok");

        let sira = besleyici.await.unwrap();
        // Her istek "oku" ile başlar; bitişiklik ölçütü: bir isteğin grubu
        // "bitti" ile kapanmadan yeni bir "oku" gelmemeli.
        let mut gruplar: Vec<Vec<&str>> = Vec::new();
        for (_, tur) in &sira {
            if *tur == "oku" {
                gruplar.push(Vec::new());
            }
            gruplar.last_mut().expect("ilk mesaj oku olmalı").push(*tur);
        }
        assert_eq!(gruplar.len(), 2, "iki istek iki grup açmalı: {sira:?}");
        for (i, grup) in gruplar.iter().enumerate() {
            assert_eq!(grup.first(), Some(&"oku"), "{i}. grup: {sira:?}");
            assert_eq!(
                grup.last(),
                Some(&"bitti"),
                "{i}. grup kapanmadan diğer istek başladı (kilit tutmuyor): {sira:?}"
            );
            assert_eq!(grup.iter().filter(|t| **t == "sor").count(), 1);
        }
    }
}
