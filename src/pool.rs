//! Hesap havuzu ve yonlendirici.
//!
//! TASARIM: her hesap kendi tarayici oturumuna sahip ve oturumlar **tembel** acilir —
//! N hesap tanimlamak N tarayici baslatmak demek degil. Bir hesap ilk kez secildiginde
//! acilir, sonra acik kalir.
//!
//! Yonlendirme, kotayi degil **kota + hiz butcesini birlikte** gozetir: bir hesap
//! bosta olsa bile hiz penceresi dolmussa secilmez. Plandaki "7/24 calis ama insan gibi
//! gorun" gerilimi tek hesabi hizlandirarak degil, havuzu genisletip her hesabi hafif
//! kullanarak cozulur — burasi o cozumun yeri.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tokio::sync::{Mutex, Semaphore};

use crate::browser::{detect_login, BrowserBinary, LoginState, Provider, Session};
use crate::config::{Account, Pacing};

/// Bir hesabin calisma durumu. Oturumdan ayri tutuluyor ki durum sorgulamak icin
/// tarayici kilidini beklemek gerekmesin.
#[derive(Debug, Default)]
pub struct AccountState {
    /// Bu ana kadar yapilan istek sayisi (surec omru boyunca).
    pub total: u32,
    /// Son bir saatteki istek zamanlari — saatlik tavan icin.
    pub recent: Vec<Instant>,
    /// En erken ne zaman tekrar kullanilabilir (hiz butcesi + geri cekilme).
    pub not_before: Option<Instant>,
    /// Insan kapisi: dogrulama duvarina carpildi, insan cozene kadar kullanilmaz.
    pub paused_reason: Option<String>,
    /// Ust uste hata sayisi — ustel geri cekilme icin.
    pub consecutive_errors: u32,
}

impl AccountState {
    /// Saatlik pencereden dusenleri at.
    fn buda(&mut self, simdi: Instant) {
        self.recent.retain(|t| simdi.duration_since(*t) < Duration::from_secs(3600));
    }

    /// Hesap **ne zaman** hazir olur? `None` = insan kapisinda, suresi belirsiz.
    ///
    /// Neden bool degil de zaman: bekleme suresini yalniz `not_before`'dan hesaplamak,
    /// saatlik tavandan habersiz olmak demekti. Tavan dolup `not_before` gecmiste
    /// kalinca bekleme 0 cikiyor, dongu bir saat boyunca saniyede 20 tur atarak bosa
    /// doniyordu. Engelin ne oldugunu bilen tek yer burasi, o yuzden sureyi de burasi
    /// soylemeli.
    fn hazir_zamani(&mut self, simdi: Instant, pacing: &Pacing) -> Option<Instant> {
        if self.paused_reason.is_some() {
            return None;
        }
        self.buda(simdi);
        let mut ne_zaman = self.not_before.unwrap_or(simdi);
        if pacing.hourly_cap > 0 && self.recent.len() as u32 >= pacing.hourly_cap {
            // Tavan dolu: en eski istek pencereden dusunce yer acilir. `recent` sirali
            // eklendigi ve `buda` sirayi bozmadigi icin ilki en eskisidir.
            if let Some(&en_eski) = self.recent.first() {
                ne_zaman = ne_zaman.max(en_eski + Duration::from_secs(3600));
            }
        }
        Some(ne_zaman)
    }

    #[cfg(test)]
    fn hazir_mi(&mut self, simdi: Instant, pacing: &Pacing) -> bool {
        self.hazir_zamani(simdi, pacing).is_some_and(|t| t <= simdi)
    }
}

/// Bir saglayicinin sekmesi ve o sekmedeki sohbet dizisinin kimligi.
struct Sekme {
    tutamac: fantoccini::wd::WindowHandle,
    /// Bu sekmedeki sohbette hangi konusma yaziyor. None = bos/bilinmeyen dizi.
    ///
    /// NEDEN sabit tablosu yetmiyor: sabitler bellekte, tarayici dizisi ekranda.
    /// Bosta kapatma tarayiciyi kapatinca sekme yok oluyor ve yeniden acilista
    /// SIFIRDAN bir sohbet basliyor — ama sabit tablosunda kayit duruyor. O kayda
    /// bakip "devam ediyoruz" desek, modele gecmissiz bir dizide yalnizca son
    /// mesaji gonderirdik. Dogru soru "sabit var mi" degil, "BU SEKME o konusmayi
    /// tasiyor mu".
    konusma: Option<u64>,
}

/// Acik bir tarayici oturumu ve saglayici basina sekmeleri.
struct AcikOturum {
    session: Session,
    /// saglayici adi -> sekme.
    sekmeler: HashMap<String, Sekme>,
    /// En son ne zaman kiralandi — bosta kapatma icin.
    son_kullanim: Instant,
    /// Pencere insana gosterildi ve henuz birakilmadi.
    ///
    /// NEDEN: olculdu ve canli kirdi — kullanici bir profile elle giris yaparken
    /// (deepseek, qwen, kimi, z.ai sekmeleri) bosta supurucusu 15 dakika dolunca
    /// pencereyi ALTINDAN kapatti. Supurucu "istek gelmedi"yi "kimse kullanmiyor"
    /// saniyordu; insanin elindeki pencere de kullanimdadir.
    insan_elinde: bool,
}

/// Bir PROFIL = bir kimlik = bir tarayici. Ayni Google oturumu profildeki tum
/// saglayicilara yettigi icin (olculdu: Gemini oturumu acikken Perplexity'ye
/// "Continue with Google" ile sifresiz girildi) hesap basina ayri tarayici acmak
/// gereksiz. 3 kimlik x 3 saglayici artik 9 degil 3 tarayici.
pub struct Kimlik {
    pub ad: String,
    /// Tembel acilir; None = tarayici henuz baslatilmadi.
    oturum: Mutex<Option<AcikOturum>>,
}

/// Kota ve hiz butcesinin birimi: (kimlik, saglayici).
///
/// Neden tarayiciyla ayni birim degil: limitler saglayici basina isliyor. Gemini'nin
/// tavani dolunca ayni profildeki Perplexity'nin de durmasi yanlis olurdu.
pub struct Slot {
    pub kimlik: usize,
    pub hesap_adi: String,
    pub provider: Arc<Provider>,
    pub state: Mutex<AccountState>,
}

/// Tarayici calistirma ortami.
///
/// Dordu birlikte tasindigi icin tek parca. Ayri parametre olduklarinda
/// `profiles_root` ile `geckodriver` yan yana iki `PathBuf`'tu — yer degistirseler
/// derleyici uyarmaz, Kervan sessizce yanlis dizinde profil arardi.
pub struct Ortam {
    pub profiles_root: PathBuf,
    pub browser: BrowserBinary,
    pub geckodriver: PathBuf,
    pub headless: bool,
}

pub struct Pool {
    pub kimlikler: Vec<Kimlik>,
    pub slots: Vec<Slot>,
    /// Saglayici adi -> ayni anda kac istek gecebilir. VARSAYILAN 1.
    ///
    /// NEDEN: olculdu ve zarar verdi. 8 hesaplik havuz claude'u denerken UC
    /// hesaba ayni anda gitti; dakikalar icinde ayni IP'den ayni Cloudflare
    /// sitesine yedi taze oturum acildi ve Cloudflare o an interaktif sinamaya
    /// gecti (cerezlerde `cf_chl_rc_ni` tam o dakikada belirdi). Oysa 11:25'te
    /// ayni WebDriver'li tarayici sorunsuz `cf_clearance` almisti — yani sorun
    /// otomasyon imzasi degil, ANI YOGUNLUKTU.
    ///
    /// Tek bir insan ayni siteye ayni anda yedi hesapla girmez. Bu yuzden kapi
    /// varsayilan olarak 1: kota bitince siradaki hesap uyanir, hepsi birden
    /// degil. Cok hesapli olmanin amaci es zamanlilik degil SUREKLILIKTI.
    ///
    /// Cloudflare arkasinda olmayan bir saglayicida (gemini, copilot) hiz
    /// isteniyorsa `[providers.gemini] eszamanli = 3` ile acilir.
    kapilar: HashMap<String, Arc<Semaphore>>,
    /// Sekmesi acikken tarayicinin bosta kapatilmayacagi saglayicilar.
    ///
    /// Cloudflare arkasindaki siteler. Bosta kapatma bellek icin dogru ama BU
    /// siteler icin ters calisiyor: her yeniden acilis taze bir oturum, yani yeni
    /// bir bot-risk degerlendirmesi. Ac-kapa dongusu sinamayi davet ediyor.
    kapanmaz: std::collections::HashSet<String>,
    pub pacing: Pacing,
    /// Konusma parmak izi -> hesap. Tarayici sohbeti durumlu oldugu icin bir konusmanin
    /// tum turlari AYNI hesaba gitmeli; baska hesabin gecmisi bos olur.
    /// Bellekte tutulur: yeniden baslatinca sabitler dusur, tarayici dizileri kalir.
    sabitler: Mutex<HashMap<u64, usize>>,
    ortam: Ortam,
}

/// Sabitlenmis hesap mesgulse ne kadar beklenir. Otesinde sureklilik yerine yanit
/// vermeyi seciyoruz — sessizce degil, gunluge yazarak.
const PIN_MAX_BEKLEME: Duration = Duration::from_secs(60);

/// Sabit tablosu sinirsiz buyumesin. 7/24 bir ayda bunun yanina bile yaklasilmaz;
/// asilirsa tablo bosaltilir — sureklilik kaybi olur, veri kaybi olmaz.
const SABIT_TAVANI: usize = 10_000;

/// Sabitlenmis hesap icin uc olasilik.
enum Sabit {
    Kullan,
    Bekle(Duration),
    /// Sabit birakiliyor; sebebi gunluge yazilir, sessizce hesap degistirilmez.
    Birak(&'static str),
}

/// Secilen hesabin oturumunu tutan kilit. Dusunce hesap serbest kalir.
pub struct Lease<'a> {
    pub index: usize,
    pub name: String,
    pub provider: Arc<Provider>,
    /// Bu sekmedeki sohbet dizisi bu konusma icin BOS. Cagiran taraf gecmisi
    /// tohumlamali, yoksa model onceki turlari hic gormemis olur.
    pub yeni_dizi: bool,
    /// Saglayici kapisindaki yer. Kiralama bitince otomatik birakilir.
    _kapi: tokio::sync::OwnedSemaphorePermit,
    guard: tokio::sync::MutexGuard<'a, Option<AcikOturum>>,
}

impl Lease<'_> {
    /// INSAN KAPISI: bu oturumu insanin eline birakir.
    ///
    /// NEDEN `goster` degil: `goster` kimlik kilidini kendisi alir, oysa `Lease`
    /// o kilidi zaten tutuyor — oradan cagirmak kilitlenme olurdu. Burada elde
    /// tutulan guard uzerinden isaretliyoruz.
    ///
    /// NEDEN GEREKLI: dogrulamayi cozmek icin acilan pencereyi bosta supurucu
    /// 15 dakika sonra kapatirdi. Ayni sinif hata daha once yasandi — Qwen'e
    /// giris yapilirken pencere altindan cekilmisti.
    pub fn insan_eline_birak(&mut self) {
        if let Some(o) = self.guard.as_mut() {
            o.insan_elinde = true;
        }
    }

    pub fn client(&self) -> &fantoccini::Client {
        // Kiralama ancak oturum acildiktan sonra verilir; burada None olamaz.
        &self.guard.as_ref().expect("oturum acik olmali").session.client
    }
}

impl Pool {
    /// `defter` hesabin `provider` adini cozmek icin. Bilinmeyen ad SESSIZCE
    /// varsayilana dusmez — yanlis hesabi yanlis siteye baglamak en kotu hata olurdu.
    pub fn new(
        accounts: Vec<Account>,
        pacing: Pacing,
        defter: &std::collections::HashMap<String, Arc<Provider>>,
        eszamanli: &std::collections::HashMap<String, u32>,
        ortam: Ortam,
    ) -> Result<Self> {
        let mut kimlikler = Vec::new();
        let mut slots = Vec::new();
        let mut kapilar: HashMap<String, Arc<Semaphore>> = HashMap::new();
        let mut kapanmaz = std::collections::HashSet::new();
        for a in accounts {
            let k = kimlikler.len();
            kimlikler.push(Kimlik { ad: a.name.clone(), oturum: Mutex::new(None) });
            for sad in a.saglayicilar() {
                let provider = crate::saglayicilar::bul(defter, &sad)
                    .with_context(|| format!("'{}' hesabi icin", a.name))?;
                kapilar.entry(provider.ad.clone()).or_insert_with(|| {
                    let n = eszamanli.get(&provider.ad).copied().unwrap_or(1).max(1);
                    Arc::new(Semaphore::new(n as usize))
                });
                if provider.cloudflare {
                    kapanmaz.insert(provider.ad.clone());
                }
                slots.push(Slot {
                    kimlik: k,
                    hesap_adi: a.name.clone(),
                    provider,
                    state: Mutex::new(AccountState::default()),
                });
            }
        }
        Ok(Self {
            kimlikler,
            slots,
            kapilar,
            kapanmaz,
            pacing,
            sabitler: Mutex::new(HashMap::new()),
            ortam,
        })
    }

    /// Bir kimligin tarayici penceresini one getirir (giris yapmak, model secmek,
    /// dogrulama cozmek icin) ve **insanin elinde** olarak isaretler.
    ///
    /// Oturum kapaliysa ACAR: kullanici bir hesaba giris yapmak istediginde
    /// "once bir istek gonder ki tarayici acilsin" demek anlamsizdi.
    ///
    /// `sekmeler` verilirse o saglayicilarin sayfalari sekme olarak acilir; zaten
    /// acik olanlar tekrar acilmaz.
    pub async fn goster(&self, kimlik: usize, sekmeler: &[Arc<Provider>]) -> Result<()> {
        let Some(k) = self.kimlikler.get(kimlik) else {
            bail!("{kimlik} numarali kimlik yok");
        };
        let mut g = k.oturum.lock().await;
        if g.is_none() {
            let s = Session::open(
                &self.ortam.profiles_root,
                &k.ad,
                &self.ortam.browser,
                &self.ortam.geckodriver,
                self.ortam.headless,
            )
            .await?;
            *g = Some(AcikOturum {
                session: s,
                sekmeler: HashMap::new(),
                son_kullanim: Instant::now(),
                insan_elinde: false,
            });
        }
        let o = g.as_mut().expect("oturum acik");
        // Istenen saglayicilarin sekmelerini ac. Elle giris turunda 8 kimlik x 4
        // site = 32 kez adres yazmak yerine sekmeler hazir geliyor.
        for sag in sekmeler {
            if o.sekmeler.contains_key(&sag.ad) {
                continue;
            }
            let h = if o.sekmeler.is_empty() {
                o.session.goto(&sag.url).await?;
                o.session.gecerli_sekme().await?
            } else {
                o.session.yeni_sekme(&sag.url).await?
            };
            // Insan icin acilan sekme: hangi konusmayi tasidigi bilinmiyor. None
            // birakmak, sonraki istekte gecmisin tohumlanmasini saglar — yanlis
            // varsayimla gecmissiz cevap almaktansa fazladan baglam gondermek iyi.
            o.sekmeler.insert(sag.ad.clone(), Sekme { tutamac: h, konusma: None });
        }
        o.session.goster().await?;
        // Birakilana kadar supurucu bu pencereye dokunmaz.
        o.insan_elinde = true;
        o.son_kullanim = Instant::now();
        Ok(())
    }

    /// Pencereyi ekran disina alir ve insanin elinden birakir.
    pub async fn gizle(&self, kimlik: usize) -> Result<bool> {
        let Some(k) = self.kimlikler.get(kimlik) else {
            bail!("{kimlik} numarali kimlik yok");
        };
        let mut g = k.oturum.lock().await;
        match g.as_mut() {
            Some(o) => {
                o.session.gizle().await;
                o.insan_elinde = false;
                o.son_kullanim = Instant::now();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// `esik` suresince dokunulmamis tarayicilari kapatir; kac tane kapandigini doner.
    ///
    /// NEDEN: hesap sayisinin gercek tavani kod degil BELLEK. Olculdu: 7 kimlik = 21
    /// surec, 2.4 GB (kimlik basina ~344 MB). Havuz buyudukce hepsini acik tutmak
    /// makineyi yer; oysa 7/24 kosuda hesaplarin cogu her an bosta. Oturum diske
    /// yazili oldugu icin kapatmak veri kaybetmez, yalniz bir sonraki istekte
    /// yeniden acilma bedeli oder.
    ///
    /// Mesgul kimlige DOKUNMAZ: kilit alinamiyorsa o kimlik su an istek isliyordur.
    pub async fn bosta_kapat(&self, esik: Duration) -> usize {
        let mut kapanan = 0;
        for k in &self.kimlikler {
            let Ok(mut g) = k.oturum.try_lock() else { continue };
            // Insanin elindeki pencereye ASLA dokunma: kullanici o sirada giris
            // yapiyor ya da dogrulama coziyor olabilir. Sure dolmasi "kimse
            // kullanmiyor" demek degil.
            if g.as_ref()
                .is_none_or(|o| o.insan_elinde || o.son_kullanim.elapsed() < esik)
            {
                continue;
            }
            // Cloudflare arkasindaki bir sekme acikken kapatma. Her yeniden acilis
            // o site icin TAZE bir oturum, yani yeni bir bot-risk degerlendirmesi;
            // bellek icin dogru olan sey burada sinamayi davet ediyor.
            if let Some(o) = g.as_ref()
                && let Some(sag) = o.sekmeler.keys().find(|s| self.kapanmaz.contains(*s))
            {
                println!(
                    "[{}] bosta ama kapatilmiyor: '{sag}' sekmesi acik ve o site \
                     Cloudflare arkasinda (yeniden acilis sinama davet eder)",
                    k.ad
                );
                continue;
            }
            if let Some(o) = g.take() {
                if let Err(e) = o.session.close().await {
                    eprintln!("[{}] bosta oturum kapatilamadi: {e:#}", k.ad);
                } else {
                    println!("[{}] bosta kaldi, tarayici kapatildi", k.ad);
                    kapanan += 1;
                }
            }
        }
        kapanan
    }

    /// Kac konusma sabitlenmis (gorunurluk icin).
    pub async fn sabit_sayisi(&self) -> usize {
        self.sabitler.lock().await.len()
    }

    /// Kullanilabilir hesaplardan **en az kullanilani** sec, oturumunu (gerekirse acarak)
    /// kilitle ve hiz gecikmesini uygula.
    ///
    /// Hicbiri hazir degilse en erken hazir olacagi bekler. Sessizce hata dondurmek
    /// yerine beklemek dogru: hiz butcesi bir arıza degil, tasarim geregi.
    /// `oturum` verilirse o konusma **kendi hesabina sabitlenir**; None ise yalniz
    /// en-az-kullanilan secimi isler.
    ///
    /// NEDEN: tarayici sohbeti durumlu. Ayni konusmanin ikinci turu baska hesaba
    /// duserse o hesabin dizisinde birinci tur hic yoktur — model gecmissiz cevaplar.
    /// Sabit, havuzun yuk dagitmasiyla sohbetin surekliligi arasindaki gerilimi
    /// surekliligi ustun tutarak cozer; ama hesap insan kapisindaysa ya da tavani
    /// dolmussa yanit vermeyi sureklilige tercih eder ve bunu gunluge yazar.
    ///
    /// `saglayici` verilirse yalniz o saglayicinin hesaplari degerlendirilir: bir
    /// claude istegi gemini hesabina DUSMEMELI, dusseydi baska bir modelin cevabini
    /// istenen modelin cevabi diye dondururduk.
    pub async fn acquire_for(&self, oturum: Option<u64>, saglayici: &str) -> Result<Lease<'_>> {
        let uygun: Vec<usize> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.provider.ad == saglayici)
            .map(|(i, _)| i)
            .collect();
        if uygun.is_empty() {
            bail!("'{saglayici}' saglayicisi icin tanimli hesap yok (kervan.toml)");
        }

        // Saglayici kapisi. Hesap SECILMEDEN once alinir: amac "ayni hesaba iki
        // istek" degil, "ayni SITEYE ayni anda birden fazla oturum" olmasin.
        // Bekleme hesap secimini de kapsiyor, cunku hizli ard arda acilan taze
        // oturumlarin kendisi risk uretiyor.
        let kapi = self.kapilar.get(saglayici).cloned().with_context(|| {
            format!("'{saglayici}' icin kapi yok — havuz kurulumunda atlanmis")
        })?;
        // Option: dongu icinde tasinabilmesi icin. Yalnizca basarida `take` edilir.
        let mut izin = Some(kapi.acquire_owned().await.expect("kapi kapatilmiyor"));

        loop {
            let simdi = Instant::now();

            // 1) Sabitlenmis hesap varsa onceligi onun — ama yalniz ayni saglayiciysa.
            if let Some(anahtar) = oturum
                && let Some(i) = self.sabitler.lock().await.get(&anahtar).copied()
                && uygun.contains(&i)
            {
                let karar = sabit_karari(
                    &mut *self.slots[i].state.lock().await,
                    simdi,
                    &self.pacing,
                );
                match karar {
                    Sabit::Kullan => match self.kilitle(i, oturum).await {
                        Ok((guard, yeni_dizi)) => {
                            return Ok(Lease {
                                index: i,
                                name: self.slots[i].hesap_adi.clone(),
                                provider: self.slots[i].provider.clone(),
                                yeni_dizi,
                                _kapi: izin.take().expect("izin bir kez kullanilir"),
                                guard,
                            });
                        }
                        Err(e) => {
                            let mesaj = format!("{e:#}");
                            eprintln!("[{}] sabit hesap acilamadi: {mesaj}", self.slots[i].hesap_adi);
                            self.basarisiz(i, &mesaj).await;
                            continue;
                        }
                    },
                    Sabit::Bekle(sure) => {
                        tokio::time::sleep(sure + Duration::from_millis(50)).await;
                        continue;
                    }
                    Sabit::Birak(neden) => {
                        self.sabitler.lock().await.remove(&anahtar);
                        eprintln!(
                            "[{}] konusma sabiti birakildi ({neden}); yeni hesapta sohbet \
                             gecmissiz baslayacak",
                            self.slots[i].hesap_adi
                        );
                    }
                }
            }
            let mut aday: Option<(usize, u32)> = None;
            let mut en_erken: Option<Instant> = None;

            for &i in &uygun {
                let slot = &self.slots[i];
                let mut st = slot.state.lock().await;
                match st.hazir_zamani(simdi, &self.pacing) {
                    // en az kullanilan kazanir
                    Some(t) if t <= simdi => {
                        if aday.is_none_or(|(_, n)| st.total < n) {
                            aday = Some((i, st.total));
                        }
                    }
                    Some(t) if en_erken.is_none_or(|e| t < e) => en_erken = Some(t),
                    Some(_) => {}
                    // Insan kapisinda: ne secilir ne beklenir.
                    None => {}
                }
            }

            if let Some((i, _)) = aday {
                match self.kilitle(i, oturum).await {
                    Ok((guard, yeni_dizi)) => {
                        if let Some(anahtar) = oturum {
                            self.sabitle(anahtar, i).await;
                        }
                        return Ok(Lease {
                            index: i,
                            name: self.slots[i].hesap_adi.clone(),
                            provider: self.slots[i].provider.clone(),
                            yeni_dizi,
                            _kapi: izin.take().expect("izin bir kez kullanilir"),
                            guard,
                        });
                    }
                    // Hesap acilamadi (tarayici baslamadi, oturum dusmus...). Durumu
                    // `basarisiz` isliyor: geri cekilme uygular, dogrulama duvariysa
                    // insan kapisinda duraklatir. Boylece dongu ya baska hesaba gecer
                    // ya da hepsi elenince asagida acik hatayla biter — sonsuz donmez.
                    Err(e) => {
                        let mesaj = format!("{e:#}");
                        eprintln!("[{}] hesap acilamadi: {mesaj}", self.slots[i].hesap_adi);
                        self.basarisiz(i, &mesaj).await;
                        continue;
                    }
                }
            }

            let Some(nb) = en_erken else {
                bail!(
                    "'{saglayici}' icin kullanilabilir hesap yok: hepsi duraklatilmis ya da \
                     saatlik tavani dolmus. Insan kapisinda bekleyen var mi diye bak."
                );
            };
            tokio::time::sleep(nb.saturating_duration_since(Instant::now()) + Duration::from_millis(50)).await;
        }
    }

    async fn sabitle(&self, anahtar: u64, i: usize) {
        let mut s = self.sabitler.lock().await;
        if s.len() >= SABIT_TAVANI {
            eprintln!("sabit tablosu {SABIT_TAVANI} girdiyi asti, bosaltiliyor");
            s.clear();
        }
        s.insert(anahtar, i);
    }

    /// Oturumu (gerekirse acarak) kilitler ve saglayicinin sekmesine gecer.
    ///
    /// `Lease` DEGIL guard donuyor: kiralama basarisiz olunca saglayici kapisindaki
    /// yeri kaybetmeyelim diye. Kapi izni `acquire_for`'da tutuluyor ve `Lease`
    /// yalnizca basarida kuruluyor.
    /// `konusma` bu istegin konusma izi. Donen `bool`, sekmedeki sohbet dizisinin
    /// o konusma icin **BOS** oldugunu soyler — yani cagiran tarafin gecmisi
    /// tohumlamasi gerekir.
    async fn kilitle(
        &self,
        i: usize,
        konusma: Option<u64>,
    ) -> Result<(tokio::sync::MutexGuard<'_, Option<AcikOturum>>, bool)> {
        let slot = &self.slots[i];
        let sad = slot.provider.ad.clone();
        let mut guard = self.kimlikler[slot.kimlik].oturum.lock().await;

        if guard.is_none() {
            let s = Session::open(
                &self.ortam.profiles_root,
                &slot.hesap_adi,
                &self.ortam.browser,
                &self.ortam.geckodriver,
                self.ortam.headless,
            )
            .await?;
            *guard = Some(AcikOturum {
                session: s,
                sekmeler: HashMap::new(),
                son_kullanim: Instant::now(),
                insan_elinde: false,
            });
        }
        // Kiralama anini isaretle. Istek bitisini degil baslangicini yazmak yeterli:
        // surerken zaten kilit bizde, supurucu o kimligi eline alamaz.
        guard.as_mut().expect("oturum acik").son_kullanim = Instant::now();

        // Saglayicinin sekmesine gec; yoksa ac. Ilk saglayici zaten acik olan
        // pencereyi kullanir, sonrakiler yeni sekme alir.
        let yeni_dizi;
        {
            let o = guard.as_mut().expect("oturum acik");
            match o.sekmeler.get(&sad).map(|s| (s.tutamac.clone(), s.konusma)) {
                Some((h, tasidigi)) => {
                    o.session.sekmeye_gec(h).await?;
                    // Ayni konusma ise dizide gecmis zaten var; degilse bu sekmeye
                    // BASKA bir konusma giriyor demektir ve gecmisi tohumlanmali.
                    yeni_dizi = tasidigi.is_none() || tasidigi != konusma;
                    o.sekmeler.get_mut(&sad).expect("sekme var").konusma = konusma;
                }
                None => {
                    let h = if o.sekmeler.is_empty() {
                        o.session.goto(&slot.provider.url).await?;
                        o.session.gecerli_sekme().await?
                    } else {
                        o.session.yeni_sekme(&slot.provider.url).await?
                    };
                    // Taze sekme = bos sohbet dizisi.
                    yeni_dizi = true;
                    o.sekmeler.insert(sad.clone(), Sekme { tutamac: h, konusma });

                    // Yeni acilan sekmede oturum durumunu OLC. Tahmin yok.
                    let durum =
                        detect_login(&o.session.client, &slot.provider.selectors).await?;
                    let neden = match durum {
                        LoginState::Ready => None,
                        LoginState::NeedsLogin => Some(format!(
                            "'{sad}' oturumu kapali: bu profilde elle giris gerekiyor"
                        )),
                        LoginState::Unknown => Some(format!(
                            "'{sad}' sayfasinda ne istem kutusu ne giris baglantisi var — \
                             dogrulama duvari ya da kirilmis secici"
                        )),
                    };
                    if let Some(neden) = neden {
                        slot.state.lock().await.paused_reason = Some(neden.clone());
                        bail!(
                            "'{}': {neden}. Pencereyi one getir: POST /accounts/{i}/goster \
                             — cozunce: POST /accounts/{i}/devam",
                            slot.hesap_adi
                        );
                    }
                }
            }
        }
        {
            let mut st = slot.state.lock().await;
            let simdi = Instant::now();
            st.total += 1;
            st.recent.push(simdi);
            // Taban: istek hic raporlanmadan dusse bile hesap paceless kalmasin.
            // Asil bekleme `basarili`/`basarisiz` icinde, istek BITTIGINDE kurulur.
            st.not_before = Some(simdi + self.gecikme());
        }
        Ok((guard, yeni_dizi))
    }

    fn gecikme(&self) -> Duration {
        let j = if self.pacing.jitter_ms == 0 {
            0
        } else {
            rand::random_range(0..self.pacing.jitter_ms)
        };
        Duration::from_millis(self.pacing.min_delay_ms + j)
    }

    /// Basarili istek: hata sayaci sifirlanir ve **bitisten itibaren** hiz butcesi kurulur.
    ///
    /// Bekleme neden bitiste? Istek BASINDA kurulunca 14 saniyelik bir yanit 3 saniyelik
    /// butceyi zaten yiyor, sonraki istek sifir arayla gidiyordu — olculdu. Insan da
    /// cevabi okuyup sonra yaziyor; aradaki bosluk bitiste baslar.
    pub async fn basarili(&self, i: usize) {
        let bekle = self.gecikme();
        let mut st = self.slots[i].state.lock().await;
        st.consecutive_errors = 0;
        st.not_before = Some(Instant::now() + bekle);
    }

    /// Basarisiz istek: ustel geri cekilme. Dogrulama duvari suphesi varsa hesabi
    /// duraklatir — insan kapisi. Sessizce yeniden denemek hesabi yakar.
    pub async fn basarisiz(&self, i: usize, hata: &str) {
        let mut st = self.slots[i].state.lock().await;
        st.consecutive_errors += 1;
        let ust = 6u32.min(st.consecutive_errors);
        let bekle = Duration::from_secs(2u64.pow(ust)); // 2,4,8,...,64 sn
        st.not_before = Some(Instant::now() + bekle);

        // Insan kapisi iki yoldan acilir:
        // (a) hata metni acik bir dogrulama duvari,
        // (b) sebep ne olursa olsun ust uste N basarisizlik. 7/24 kosuda bu ikincisi
        //     sart: olu bir oturuma 64 saniyede bir sonsuza kadar istek atmak hesabi
        //     bosuna yipratir, kimse de fark etmez.
        if insan_kapisi_gerekir(hata) || st.consecutive_errors >= UST_USTE_SINIR {
            st.paused_reason = Some(hata.chars().take(200).collect());
        }
    }

    /// Saglayici kotayi bildirdi: hesabi pencere kadar beklet.
    ///
    /// Duraklatma DEGIL — insan cozecek bir sey yok, sure gecince kendi acilir.
    /// Hata sayaci da artirilmaz: kota bir ariza degil, beklenen bir durum.
    pub async fn kota_doldu(&self, i: usize, bekleme: Duration) {
        let mut st = self.slots[i].state.lock().await;
        st.consecutive_errors = 0;
        st.not_before = Some(Instant::now() + bekleme);
    }

    /// Insan kapisini ac: hesabi tekrar kullanima sok.
    pub async fn devam_ettir(&self, i: usize) {
        let mut st = self.slots[i].state.lock().await;
        st.paused_reason = None;
        st.consecutive_errors = 0;
        st.not_before = None;
    }
}

/// Sabitlenmis hesap su an kullanilabilir mi, beklenmeli mi, birakilmali mi?
///
/// Ayri fonksiyon cunku uc yolun ucu de yanlis olabilir ve ucu de tarayicisiz sinanabilir:
/// yanlis "Bekle" istegi bir saat asar, yanlis "Birak" sohbet gecmisini sessizce kaybeder.
fn sabit_karari(st: &mut AccountState, simdi: Instant, pacing: &Pacing) -> Sabit {
    let Some(ne_zaman) = st.hazir_zamani(simdi, pacing) else {
        return Sabit::Birak("hesap insan kapisinda");
    };
    if ne_zaman <= simdi {
        return Sabit::Kullan;
    }
    // Kisa bekleme (hiz butcesi) surekliligi korumaya deger; uzun bekleme (saatlik
    // tavan) degmez — 7/24 kosuda istegi bir saat askida birakmak yanit vermemektir.
    let bekleme = ne_zaman.duration_since(simdi);
    if bekleme <= PIN_MAX_BEKLEME {
        Sabit::Bekle(bekleme)
    } else {
        Sabit::Birak("bekleme cok uzun — saatlik tavan dolmus olabilir")
    }
}

/// Kac ust uste hatadan sonra insan cagrilir.
const UST_USTE_SINIR: u32 = 5;

/// Dogrulama duvari mi, gecici ariza mi? Duvarsa insan cozmeli; otomatik yeniden deneme
/// hem ise yaramaz hem hesabi riske atar.
///
/// Liste bilerek DAR: yalniz sitenin kendi soyledigi kesin ifadeler. "dogrulama" gibi
/// genis bir sozcuk, bizim kendi *tahmin* iceren hata metinlerimize de uyuyordu
/// ("dogrulama duvari cikmis olabilir") — bir yavas yanit saglam bir hesabi
/// duraklatirdi. Belirsiz haller `UST_USTE_SINIR` ile zaten yakalaniyor.
pub fn insan_kapisi_gerekir(hata: &str) -> bool {
    let h = hata.to_lowercase();
    ["captcha", "unusual traffic", "sign in", "are you a robot", "verify it's you"]
        .iter()
        .any(|k| h.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pacing(cap: u32) -> Pacing {
        Pacing { min_delay_ms: 0, jitter_ms: 0, hourly_cap: cap }
    }

    #[test]
    fn saatlik_tavan_dolunca_hesap_secilmez() {
        let mut st = AccountState::default();
        let p = pacing(2);
        let simdi = Instant::now();
        assert!(st.hazir_mi(simdi, &p));
        st.recent.push(simdi);
        st.recent.push(simdi);
        assert!(!st.hazir_mi(simdi, &p), "tavan dolmusken hazir gorunmemeli");
    }

    #[test]
    fn duraklatilmis_hesap_secilmez() {
        let mut st = AccountState { paused_reason: Some("captcha".into()), ..Default::default() };
        assert!(!st.hazir_mi(Instant::now(), &pacing(0)));
    }

    /// Hiz penceresi dolmadan ayni hesap tekrar secilmemeli.
    #[test]
    fn hiz_penceresi_beklenir() {
        let mut st = AccountState {
            not_before: Some(Instant::now() + Duration::from_secs(30)),
            ..Default::default()
        };
        assert!(!st.hazir_mi(Instant::now(), &pacing(0)));
    }

    /// Dogrulama duvari ile gecici ag hatasi ayrilmali; ilkinde insan cagrilir,
    /// ikincisinde otomatik yeniden denenir.
    #[test]
    fn insan_kapisi_yalniz_dogrulama_icin() {
        assert!(insan_kapisi_gerekir("Please complete the CAPTCHA"));
        assert!(insan_kapisi_gerekir("Sign in to continue"));
        assert!(!insan_kapisi_gerekir("connection reset by peer"));
        assert!(!insan_kapisi_gerekir("yanit 120 sn icinde tamamlanmadi"));
    }

    fn havuz(pacing: Pacing) -> Pool {
        havuz_n(pacing, 1, &HashMap::new())
    }

    /// `n` hesapli havuz; hepsi ayni saglayiciya bagli.
    fn havuz_n(pacing: Pacing, n: usize, eszamanli: &HashMap<String, u32>) -> Pool {
        // Tarayici baslatilmiyor: bu testler yalniz durum mantigina dokunuyor.
        let defter = crate::saglayicilar::kayit_defteri(&HashMap::new());
        Pool::new(
            (0..n)
                .map(|i| Account {
                    name: format!("hesap{i}"),
                    provider: None,
                    providers: vec!["gemini".into()],
                })
                .collect(),
            pacing,
            &defter,
            eszamanli,
            Ortam {
                profiles_root: PathBuf::from("."),
                browser: BrowserBinary(PathBuf::from(".")),
                geckodriver: PathBuf::from("."),
                headless: true,
            },
        )
        .expect("gemini taninmali")
    }

    /// Kapi varsayilan olarak 1 olmali: hesap sayisi kac olursa olsun ayni siteye
    /// ayni anda tek istek. Olculdu — yedi profille ayni anda gitmek Cloudflare'i
    /// interaktif sinamaya gecirdi.
    #[test]
    fn kapi_varsayilani_siralidir() {
        let p = havuz_n(pacing(0), 7, &HashMap::new());
        assert_eq!(p.slots.len(), 7, "yedi hesap kurulmali");
        assert_eq!(
            p.kapilar["gemini"].available_permits(),
            1,
            "yedi hesap olsa da kapi tek gecis vermeli"
        );
    }

    /// Cloudflare arkasinda olmayan bir saglayicida hiz istenirse acilabilmeli.
    #[test]
    fn kapi_yapilandirmayla_genisler() {
        let mut e = HashMap::new();
        e.insert("gemini".to_string(), 3);
        let p = havuz_n(pacing(0), 7, &e);
        assert_eq!(p.kapilar["gemini"].available_permits(), 3);
    }

    /// 0 verilirse havuz KILITLENIRDI: sifir izinli bir kapidan hicbir istek gecmez
    /// ve `acquire_for` sonsuza kadar bekler. Yazim hatasi butun saglayiciyi
    /// sessizce olduremesin.
    #[test]
    fn kapi_sifira_dusurulemez() {
        let mut e = HashMap::new();
        e.insert("gemini".to_string(), 0);
        let p = havuz_n(pacing(0), 2, &e);
        assert!(p.kapilar["gemini"].available_permits() >= 1);
    }

    /// Havuz, Cloudflare arkasindaki saglayicilari "kapanmaz" diye tanimali.
    /// Bu kume bos kalirsa supurucu o tarayicilari kapatir ve her yeniden acilis
    /// yeni bir bot-risk degerlendirmesi olur — bugun tam bunu yasadik.
    #[test]
    fn cloudflare_saglayicisi_kapanmaz_sayilir() {
        let defter = crate::saglayicilar::kayit_defteri(&HashMap::new());
        let p = Pool::new(
            vec![Account {
                name: "a".into(),
                provider: None,
                providers: vec!["gemini".into(), "perplexity".into()],
            }],
            pacing(0),
            &defter,
            &HashMap::new(),
            Ortam {
                profiles_root: PathBuf::from("."),
                browser: BrowserBinary(PathBuf::from(".")),
                geckodriver: PathBuf::from("."),
                headless: true,
            },
        )
        .expect("iki saglayici da taninmali");
        assert!(p.kapanmaz.contains("perplexity"), "perplexity Cloudflare arkasinda");
        assert!(!p.kapanmaz.contains("gemini"), "gemini takilmiyor, kapatilabilir");
    }

    /// Bilinmeyen saglayici adi havuz kurulurken PATLAMALI: sessizce varsayilana
    /// dusmek, hesabi yanlis siteye baglamak demek olurdu.
    #[test]
    fn bilinmeyen_saglayicili_hesap_havuzu_kurmaz() {
        let defter = crate::saglayicilar::kayit_defteri(&HashMap::new());
        let r = Pool::new(
            vec![Account { name: "a".into(), provider: None, providers: vec!["yokboyle".into()] }],
            pacing(0),
            &defter,
            &HashMap::new(),
            Ortam {
                profiles_root: PathBuf::from("."),
                browser: BrowserBinary(PathBuf::from(".")),
                geckodriver: PathBuf::from("."),
                headless: true,
            },
        );
        assert!(r.is_err());
    }

    /// Supurucu MESGUL kimlige dokunmamali. `try_lock` yerine `lock` yazilsaydi bu
    /// cagri, istek isleyen bir hesabin kilidinde sonsuza kadar asili kalir ve
    /// supurucu gorevi bir daha hic tik atmazdi.
    #[tokio::test]
    async fn supurucu_mesgul_kimlikte_asili_kalmaz() {
        let p = havuz(Pacing { min_delay_ms: 0, jitter_ms: 0, hourly_cap: 0 });
        let _mesgul = p.kimlikler[0].oturum.lock().await;
        let kapanan = tokio::time::timeout(
            Duration::from_secs(2),
            p.bosta_kapat(Duration::from_secs(0)),
        )
        .await
        .expect("mesgul kimlik supurucuyu bloklamamali");
        assert_eq!(kapanan, 0);
    }

    /// Hiz butcesi istek BITTIGINDE kurulmali. Bastan kurulunca uzun suren bir yanit
    /// butceyi yiyip sonraki istegi sifir arayla gecirtiyordu — canli olcumde gorulmustu.
    #[tokio::test]
    async fn hiz_butcesi_bitisten_itibaren_sayilir() {
        let p = havuz(Pacing { min_delay_ms: 3000, jitter_ms: 0, hourly_cap: 0 });
        p.basarili(0).await;
        let mut st = p.slots[0].state.lock().await;
        assert!(!st.hazir_mi(Instant::now(), &p.pacing), "bitisten hemen sonra hazir olmamali");
        let nb = st.not_before.expect("bitiste not_before kurulmali");
        assert!(nb.duration_since(Instant::now()) > Duration::from_millis(2500));
    }

    /// Sebebi ne olursa olsun ust uste N hata insan kapisini acmali; olu bir oturuma
    /// sonsuza kadar 64 saniyede bir istek atmak hesabi bosuna yipratir.
    #[tokio::test]
    async fn ust_uste_hata_insan_kapisini_acar() {
        let p = havuz(Pacing { min_delay_ms: 0, jitter_ms: 0, hourly_cap: 0 });
        for _ in 0..UST_USTE_SINIR - 1 {
            p.basarisiz(0, "connection reset by peer").await;
        }
        assert!(p.slots[0].state.lock().await.paused_reason.is_none(), "sinira gelmeden duraklamamali");
        p.basarisiz(0, "connection reset by peer").await;
        assert!(p.slots[0].state.lock().await.paused_reason.is_some(), "sinirda insan cagrilmali");
    }

    /// Tavan dolunca bekleme suresi SIFIR degil, pencerenin bosalmasina kalan sure
    /// olmali. Sifir cikarsa dongu bir saat boyunca bosa doner (canli kosuda islemciyi
    /// yakan, hicbir yere hata da yazmayan bir hata).
    #[test]
    fn tavan_dolunca_bekleme_pencere_sonuna_kadar() {
        let simdi = Instant::now();
        let mut st = AccountState {
            recent: vec![simdi],
            // hiz butcesi coktan gecmis: tek engel tavan
            not_before: Some(simdi - Duration::from_secs(10)),
            ..Default::default()
        };
        let t = st.hazir_zamani(simdi, &pacing(1)).expect("duraklatilmis degil");
        let kalan = t.duration_since(simdi);
        assert!(kalan > Duration::from_secs(3500), "pencere sonuna kadar beklemeli, kalan: {kalan:?}");
    }

    /// Sabitlenmis hesap kisa bir hiz butcesi beklemesi icin BIRAKILMAMALI — birakilirsa
    /// konusma baska hesaba gecer ve tarayicidaki sohbet gecmisi bosa cikar.
    #[test]
    fn kisa_bekleme_icin_sabit_birakilmaz() {
        let p = pacing(0);
        let mut st = AccountState {
            not_before: Some(Instant::now() + Duration::from_secs(4)),
            ..Default::default()
        };
        assert!(matches!(sabit_karari(&mut st, Instant::now(), &p), Sabit::Bekle(_)));
    }

    /// Saatlik tavan dolduysa beklemek istegi bir saat askida birakirdi: sureklilik
    /// yerine yanit vermeyi seciyoruz.
    #[test]
    fn tavan_dolunca_sabit_birakilir() {
        let p = pacing(1);
        let simdi = Instant::now();
        // not_before gecmiste: eski kodda bekleme 0 cikip Bekle(0) donuyordu, yani
        // sabit hic birakilmiyor ve dongu bir saat bosa doniyordu.
        let mut st = AccountState {
            recent: vec![simdi],
            not_before: Some(simdi - Duration::from_secs(10)),
            ..Default::default()
        };
        assert!(matches!(sabit_karari(&mut st, simdi, &p), Sabit::Birak(_)));
    }

    /// Insan kapisindaki hesap beklenmez, birakilir.
    #[test]
    fn duraklatilmis_sabit_birakilir() {
        let mut st = AccountState { paused_reason: Some("captcha".into()), ..Default::default() };
        assert!(matches!(sabit_karari(&mut st, Instant::now(), &pacing(0)), Sabit::Birak(_)));
    }

    /// Bos hesap dogrudan kullanilmali.
    #[test]
    fn hazir_sabit_kullanilir() {
        let mut st = AccountState::default();
        assert!(matches!(sabit_karari(&mut st, Instant::now(), &pacing(0)), Sabit::Kullan));
    }

    /// Sabit tablosu tavani asinca bosalmali; sinirsiz buyumemeli.
    #[tokio::test]
    async fn sabit_tablosu_tavanda_bosalir() {
        let p = havuz(Pacing { min_delay_ms: 0, jitter_ms: 0, hourly_cap: 0 });
        for k in 0..SABIT_TAVANI as u64 {
            p.sabitle(k, 0).await;
        }
        assert_eq!(p.sabit_sayisi().await, SABIT_TAVANI);
        p.sabitle(u64::MAX, 0).await;
        assert_eq!(p.sabit_sayisi().await, 1, "tavan asilinca tablo bosalip yeniden baslamali");
    }

    /// Kendi TAHMIN iceren hata metnimiz hesabi duraklatmamali; yoksa tek bir yavas
    /// yanit saglam bir hesabi insan kapisinda kilitler.
    #[test]
    fn kendi_tahminimiz_hesabi_duraklatmaz() {
        let m = "yanit gelmedi (120 sn). Dogrulama duvari cikmis olabilir — \
                 gorunur kipte calistirip bak.";
        assert!(!insan_kapisi_gerekir(m));
        let m2 = "istem kutusu bulunamadi — oturum dusmus ya da secici kirilmis olabilir";
        assert!(!insan_kapisi_gerekir(m2));
    }
}
