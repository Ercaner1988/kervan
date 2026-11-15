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
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc, oneshot};

/// Eklentiye giden komut kuyruğunun kapasitesi. Giden akış yalnız komut taşır
/// (token parçaları değil); yavaş bir sekme bu sınırı aşarsa yeni istek açık
/// hatayla reddedilir, bellek sınırsız şişmez.
pub const GIDEN_KUYRUK_KAPASITESI: usize = 64;

/// Eklentiden Kervan'a gelen mesaj tipleri.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GelenMesaj {
    /// Eklenti, tarayıcıdaki bir sağlayıcı sekmesini Kervan'a tanıtır.
    ///
    /// Kayıt iki yoldan gelir ve ikisi de aynı kapıdan geçer:
    /// (a) sekme açıldığında/yenilendiğinde içerik betiği `TAB_REGISTER` yollar,
    /// (b) `background.js` her 10 sn'lik tarama turunda açık her sekmeyi yeniden
    /// kaydeder (tazelik damgası). (b) olmadan, tarayıcı arka planı askıya
    /// aldığında sunucu ölü bir kaydı canlı sanıyordu; tazelik süzgeci
    /// `SEKME_TAZELIK_ESIGI` ile bu yüzden eklendi.
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
///
/// `son_gorulme` — kaydın son TAZELENDİĞİ an. Eklenti her 10 sn'lik tarama
/// turunda `kayit` mesajını yeniden gönderir (2026-11-15'ten itibaren; önceden
/// kenar-tetiklemeliydi ve yalnız eşleme değişince gönderiyordu). Yani bu alan
/// bir canlılık sinyalidir: tazelenmediği sürece kayıt yaşlanır ve düşer.
/// Yaş süzgeci olmadan `sekme_var_mi` ölü sekmeye "canlı" diyor ve istek ölü
/// sekmeye gidiyordu (2026-11-14 ölçümü; tazelik 2026-11-15'te eklendi).
pub struct BagliSekme {
    pub provider: String,
    pub profile: String,
    pub gonderici: mpsc::Sender<GidenMesaj>,
    /// Kaydın son tazelenme anı (`sekme_kaydet`).
    pub son_gorulme: Instant,
}

/// Bir eklenti kaydının "canlı" sayılma süresi.
///
/// Eklenti 10 sn'de bir tarama yapıyor ve (2026-11-15'ten itibaren) her turda
/// `kayit` gönderiyor; yine de sunucu taraflı yaş süzgeci bu aralıktan belirgin
/// biçimde uzun olmalı, yoksa kısa bir gecikme (tarayıcı arka plan sayfasını
/// kısması, tek bir turun kaçması) çalışan bir sekmeyi bayat sayar.
/// 90 sn = 9 tarama turu: kısa bir uyku/duraklama kaydı düşürmez, kapanmış
/// tarayıcı ise en geç 90 sn içinde düşer.
pub const SEKME_TAZELIK_ESIGI: Duration = Duration::from_secs(90);

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
    ///
    /// TODO(A5): bu harita ŞU AN YALNIZ YAZILIYOR, HİÇ OKUNMUYOR. `Durum`
    /// mesajı gelince güncelleniyor (`:1161`, `:1218`) ama `YanitKanali::Akan`
    /// hiçbir yerde kurulmadığı için akış yolu yok; dolayısıyla "insan kapısı
    /// uyarısı kullanıcıya ulaşıyor" iddiası bugün için geçersiz. Erteleme
    /// bilinçli (2026-11-15 denetimi, A5) — ama kodda iz bırakmadığı için
    /// okuyan çalışıyor sanıyordu. Akış kanalı kurulduğunda bu notu sil.
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
                    son_gorulme: Instant::now(),
                },
            )
            .is_none()
        {
            println!("eklenti      : '{anahtar}' sekmesi bağlandı");
        }
    }

    /// Bayat kayıtları (son tazelenmesi `SEKME_TAZELIK_ESIGI`'nı aşanları)
    /// haritadan düşürür ve düşen anahtarları döner.
    ///
    /// Okuma yollarının HEPSİ bunu çağırır: eklenti sessizce düştüğünde sunucu
    /// `ayril` mesajını hiç alamaz, tek çare yaş süzgecidir.
    ///
    /// Dönüş değeri BOŞA DEĞİL: düşen anahtarların `kullanim` sayacı da
    /// silinmeli, yoksa yeniden bağlanan hesap eski sayaçla başlar ve yük
    /// dengelemesi yanlış hesabı seçer (denetim, 2026-11-15). Çağıran, bunu
    /// `sekmeler` kilidini BIRAKTIKTAN sonra `kullanim_temizle` ile yapar.
    fn bayatlari_buda(map: &mut HashMap<String, BagliSekme>) -> Vec<String> {
        let esik = SEKME_TAZELIK_ESIGI;
        let bayat: Vec<String> = map
            .iter()
            .filter(|(_, s)| s.son_gorulme.elapsed() > esik)
            .map(|(k, _)| k.clone())
            .collect();
        for anahtar in &bayat {
            map.remove(anahtar);
            println!("eklenti      : '{anahtar}' kaydı bayatladı (tazelenmedi), düşürüldü");
        }
        bayat
    }

    /// Bayatlamış kayıtların kullanım sayaçlarını siler.
    ///
    /// NEDEN AYRI: `kullanim` kilidi `sekmeler` kilidinden SONRA alınmalı
    /// (kilit sırası); `bayatlari_buda` ise haritayı ödünç alırken çalışır.
    ///
    /// KİLİT SIRASI (tam zincir, 2026-11-15 denetimiyle doğrulandı):
    /// `hesap_kilidi` → `bekleyen_istekler` → `biriken_metinler`, ve ayrıca
    /// `sekmeler` → `kullanim`. Bu iki zincir birbirine karışmaz; ters yönde
    /// kilit alan bir yol YOKTUR, bu yüzden deadlock yok. Yeni kod eklerken
    /// bu sırayı bozmayın: kilitliyken başka bir kilidi almadan önce iki
    /// kilidin bu listede hangi sırada olduğunu kontrol edin.
    async fn kullanim_temizle(&self, dusen: &[String]) {
        if dusen.is_empty() {
            return;
        }
        let mut kullanim = self.kullanim.lock().await;
        for anahtar in dusen {
            kullanim.remove(anahtar);
        }
    }

    /// Kaydı düşürür — ancak YALNIZ kayıt hâlâ `gonderici`ye aitse.
    ///
    /// Eski bir soket kapanırken araya giren yeni bağlantının kaydını
    /// silmemek için kimlik kapısı şart. Bu yüzden göndericisiz bir
    /// "sekme ayrıldı" yolu YOKTUR: kimliksiz silme, aynı hesabın ikinci
    /// sekmesi kapanırken canlı kaydı düşürürdü.
    ///
    /// `kullanim` sayacı ANAHTAR düzeyinde silinir ve silme kararı
    /// `sekmeler` haritasındaki gerçek silmeye bağlanır — sağlayıcının
    /// başka profili canlı olsa bile bu anahtarın sayacı sızmaz.
    pub async fn sekme_ayril_eger(
        &self,
        provider: &str,
        profile: &str,
        gonderici: &mpsc::Sender<GidenMesaj>,
    ) {
        let anahtar = hesap_anahtari(provider, profile);
        // KİLİT SIRASI: `sekmeler` → `kullanim`. `kullanim` yalnız harita
        // kilidi bırakıldıktan sonra alınır (`kullanim_temizle` ile aynı
        // sıra); ters yön hiçbir yerde yok.
        let silindi = {
            let mut map = self.sekmeler.lock().await;
            let eslesiyor = map
                .get(&anahtar)
                .is_some_and(|s| s.gonderici.same_channel(gonderici));
            if eslesiyor {
                map.remove(&anahtar);
                println!("eklenti      : '{anahtar}' sekmesi ayrıldı");
            }
            eslesiyor
        };
        if silindi {
            self.kullanim.lock().await.remove(&anahtar);
        }
    }

    /// Bir sağlayıcının eklenti sekmesi (herhangi bir profili) canlı mı?
    ///
    /// "Canlı" = kaydın son 90 sn içinde tazelenmiş olması. Bayat kayıt
    /// düşürülür: çağıran (`chat.rs` taşıyıcı ayrımı) yanlışlıkla ölü sekmeye
    /// istek göndermesin.
    pub async fn sekme_var_mi(&self, provider: &str) -> bool {
        let onek = format!("{provider}:");
        let (canli, dusen) = {
            let mut map = self.sekmeler.lock().await;
            let dusen = Self::bayatlari_buda(&mut map);
            (map.keys().any(|k| k.starts_with(&onek)), dusen)
        };
        self.kullanim_temizle(&dusen).await;
        canli
    }

    /// Sağlayıcı için en az kullanılan profili seçer ve sayacını artırır.
    ///
    /// `istenen` verilirse O profil kullanılır (panelden hesap seçimi). Profil
    /// bağlı değilse sessizce başka hesaba düşmek YASAK: kullanıcı A hesabını
    /// seçtiğini sanırken istek B'ye giderdi. Açık hata döner.
    ///
    /// KİLİT SIRASI: `sekmeler` → `kullanim`. Adaylar sahiplenilmiş
    /// `Vec<(anahtar, gonderici, profile)>` olarak toplanır ve harita kilidi
    /// seçim sırasında TUTULMAZ; `kullanim` ancak harita kilidi bırakıldıktan
    /// sonra (`kullanim_temizle` içinde) alınır.
    async fn hesap_sec(
        &self,
        provider: &str,
        istenen: Option<&str>,
    ) -> Result<(mpsc::Sender<GidenMesaj>, String)> {
        let onek = format!("{provider}:");
        // KİLİT SIRASI: `sekmeler` bırakılmadan `kullanim` alınmaz. Adaylar
        // sahiplenilerek toplanır ki harita kilidi seçim sırasında tutulmasın.
        let (adaylar, dusen) = {
            let mut map = self.sekmeler.lock().await;
            let dusen = Self::bayatlari_buda(&mut map);
            let adaylar: Vec<(String, mpsc::Sender<GidenMesaj>, String)> = map
                .iter()
                .filter(|(k, _)| k.starts_with(&onek))
                .map(|(k, s)| (k.clone(), s.gonderici.clone(), s.profile.clone()))
                .collect();
            (adaylar, dusen)
        };
        self.kullanim_temizle(&dusen).await;
        if adaylar.is_empty() {
            bail!(
                "'{provider}' için bağlı bir eklenti sekmesi bulunamadı. Tarayıcıda ilgili sayfa açık mı?"
            );
        }
        if let Some(istenen) = istenen.filter(|p| !p.trim().is_empty()) {
            let aranan = hesap_anahtari(provider, istenen.trim());
            let Some((_, gonderici, profile)) = adaylar.iter().find(|(k, _, _)| **k == aranan)
            else {
                let mut mevcut: Vec<String> = adaylar
                    .iter()
                    .map(|(k, _, _)| k.trim_start_matches(&onek).to_string())
                    .collect();
                mevcut.sort();
                bail!(
                    "'{aranan}' bağlı değil. Bağlı hesaplar: {}",
                    mevcut.join(", ")
                );
            };
            let mut kullanim = self.kullanim.lock().await;
            *kullanim.entry(aranan).or_insert(0) += 1;
            return Ok((gonderici.clone(), profile.clone()));
        }
        // Eşitlikte anahtar sırası kararlı olsun: deterministik seçim.
        let mut adaylar = adaylar;
        adaylar.sort_by(|a, b| a.0.cmp(&b.0));
        let mut kullanim = self.kullanim.lock().await;
        let Some((anahtar, gonderici, profile)) = adaylar
            .into_iter()
            .min_by_key(|(k, _, _)| kullanim.get(k).copied().unwrap_or(0))
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
        let (gonderici, dusen) = {
            let mut map = self.sekmeler.lock().await;
            let dusen = Self::bayatlari_buda(&mut map);
            let gonderici = match map.get(&anahtar) {
                Some(s) => s.gonderici.clone(),
                None => bail!("'{anahtar}' bağlı değil"),
            };
            (gonderici, dusen)
        };
        self.kullanim_temizle(&dusen).await;
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
        let (gonderici, dusen) = {
            let mut map = self.sekmeler.lock().await;
            let dusen = Self::bayatlari_buda(&mut map);
            let gonderici = match map.get(&anahtar) {
                Some(s) => s.gonderici.clone(),
                None => bail!("'{anahtar}' bağlı değil"),
            };
            (gonderici, dusen)
        };
        self.kullanim_temizle(&dusen).await;
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
    ///
    /// GÖNDERİCİ KİMLİĞİ YOK. Soket yolundan gelen mesajlar için
    /// [`Self::mesaji_isle_gonderici`] kullanılmalı; bu biçim yalnız
    /// kimlik gerektirmeyen yollar (testler, iç çağrılar) içindir.
    /// `Ayril` bu yoldan gelirse YOK SAYILIR: kimliksiz silme, aynı
    /// hesabın ikinci sekmesi kapanırken canlı kaydı düşürürdü.
    pub async fn mesaji_isle(&self, mesaj: GelenMesaj) {
        self.mesaji_isle_ic(mesaj, None).await;
    }

    /// Soket yolundan gelen mesajı, gönderenin kimliğiyle birlikte işler.
    ///
    /// `Ayril` yalnız kayıt gerçekten bu göndericiye aitse düşürülür
    /// (`sekme_ayril_eger`): aynı hesabın ikinci sekmesi kapandığında
    /// canlı kaydı silmemek için.
    pub async fn mesaji_isle_gonderici(
        &self,
        mesaj: GelenMesaj,
        gonderici: &mpsc::Sender<GidenMesaj>,
    ) {
        self.mesaji_isle_ic(mesaj, Some(gonderici)).await;
    }

    async fn mesaji_isle_ic(
        &self,
        mesaj: GelenMesaj,
        gonderici: Option<&mpsc::Sender<GidenMesaj>>,
    ) {
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
                let profile = profile.unwrap_or_else(|| "varsayilan".into());
                match gonderici {
                    // Kimlik kapısı: yalnız bu soketin kaydettiği anahtar düşer.
                    Some(gonderici) => {
                        self.sekme_ayril_eger(&provider, &profile, gonderici).await;
                    }
                    // Kimliksiz yol: kimi düşüreceğimiz bilinmiyor, silme yok.
                    None => {
                        println!(
                            "eklenti      : '{provider}:{profile}' ayrılma isteği kimliksiz geldi, düşürülmedi"
                        );
                    }
                }
            }
            GelenMesaj::Pong => {}
        }
    }

    /// Bağlı tüm sekmelerin sağlayıcı adlarını (tekilleştirilmiş) döner.
    /// Yönlendirme adayları sağlayıcı düzeyindedir; profiller `bagli_hesaplar`
    /// ile ayrıca görünür.
    pub async fn bagli_sekmeler(&self) -> Vec<String> {
        let (adlar, dusen) = {
            let mut map = self.sekmeler.lock().await;
            let dusen = Self::bayatlari_buda(&mut map);
            let mut adlar: Vec<String> = map.values().map(|s| s.provider.clone()).collect();
            adlar.sort();
            adlar.dedup();
            (adlar, dusen)
        };
        self.kullanim_temizle(&dusen).await;
        adlar
    }

    /// Bağlı hesapların `provider:profile` anahtarlarını döner.
    pub async fn bagli_hesaplar(&self) -> Vec<String> {
        let (anahtarlar, dusen) = {
            let mut map = self.sekmeler.lock().await;
            let dusen = Self::bayatlari_buda(&mut map);
            let mut anahtarlar: Vec<String> = map.keys().cloned().collect();
            anahtarlar.sort();
            (anahtarlar, dusen)
        };
        self.kullanim_temizle(&dusen).await;
        anahtarlar
    }
}

#[cfg(test)]
mod tests;
