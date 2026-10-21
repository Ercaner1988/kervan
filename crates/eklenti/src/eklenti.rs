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
    /// Nabız sorusu.
    Ping,
}

/// Devam eden bir istemin yanıt kanalı.
pub enum YanitKanali {
    /// Tek seferde tam yanıt bekleyen akış.
    Tekil(oneshot::Sender<Result<String>>),
    /// Canlı akış (streaming) bekleyen akış.
    Akan(mpsc::Sender<Result<String>>),
}

/// Bağlı bir eklenti oturumu. Birden çok hesap (profil) aynı sağlayıcıda
/// olabildiği için kimlik `provider:profile` anahtarıdır.
pub struct BagliSekme {
    pub provider: String,
    pub profile: String,
    pub gonderici: mpsc::Sender<GidenMesaj>,
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
        println!("eklenti      : '{anahtar}' sekmesi bağlandı");
        map.insert(
            anahtar,
            BagliSekme {
                provider,
                profile,
                gonderici,
            },
        );
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
    /// Profil yoksa `Ok(None)` (eski eklenti davranışı: provider ile ara).
    async fn hesap_sec(&self, provider: &str) -> Result<(mpsc::Sender<GidenMesaj>, String)> {
        let onek = format!("{provider}:");
        let map = self.sekmeler.lock().await;
        let mut adaylar: Vec<(&String, &BagliSekme)> =
            map.iter().filter(|(k, _)| k.starts_with(&onek)).collect();
        if adaylar.is_empty() {
            bail!(
                "'{provider}' için bağlı bir eklenti sekmesi bulunamadı. Tarayıcıda ilgili sayfa açık mı?"
            );
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

    /// Eklentiye ek dosyalarla birlikte soru sorar ve yanıtın tamamlanmasını bekler.
    /// Sağlayıcıda birden çok profil bağlıysa en-az-kullanılan seçilir.
    pub async fn sor_ekli(
        &self,
        provider: &str,
        istem: &str,
        dosyalar: Vec<DosyaEki>,
        timeout: std::time::Duration,
    ) -> Result<String> {
        let (tx_giden, profile) = self.hesap_sec(provider).await?;
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
                if let Some(YanitKanali::Tekil(tx)) = bekleyen.remove(&id) {
                    // Alıcı düşmüşse hata kaybolur; bekleyen istek yok.
                    drop(tx.send(Err(anyhow::anyhow!("Eklenti hatası: {mesaj}"))));
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
            let (_tx, profile) = hub.hesap_sec("gemini").await.unwrap();
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
}
