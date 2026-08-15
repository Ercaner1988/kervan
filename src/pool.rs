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
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use tokio::sync::Mutex;

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

pub struct Slot {
    pub account: Account,
    /// Tembel acilir; None = tarayici henuz baslatilmadi.
    session: Mutex<Option<Session>>,
    pub state: Mutex<AccountState>,
}

pub struct Pool {
    pub slots: Vec<Slot>,
    pub pacing: Pacing,
    /// ponytail: tek saglayici. Hesap basina saglayici gerekince `Slot`'a tasinir.
    pub provider: Provider,
    /// Konusma parmak izi -> hesap. Tarayici sohbeti durumlu oldugu icin bir konusmanin
    /// tum turlari AYNI hesaba gitmeli; baska hesabin gecmisi bos olur.
    /// Bellekte tutulur: yeniden baslatinca sabitler dusur, tarayici dizileri kalir.
    sabitler: Mutex<HashMap<u64, usize>>,
    profiles_root: PathBuf,
    browser: BrowserBinary,
    geckodriver: PathBuf,
    headless: bool,
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
    guard: tokio::sync::MutexGuard<'a, Option<Session>>,
}

impl Lease<'_> {
    pub fn client(&self) -> &fantoccini::Client {
        // Kiralama ancak oturum acildiktan sonra verilir; burada None olamaz.
        &self.guard.as_ref().expect("oturum acik olmali").client
    }
}

impl Pool {
    pub fn new(
        accounts: Vec<Account>,
        pacing: Pacing,
        provider: Provider,
        profiles_root: PathBuf,
        browser: BrowserBinary,
        geckodriver: PathBuf,
        headless: bool,
    ) -> Self {
        let slots = accounts
            .into_iter()
            .map(|a| Slot {
                account: a,
                session: Mutex::new(None),
                state: Mutex::new(AccountState::default()),
            })
            .collect();
        Self {
            slots,
            pacing,
            provider,
            sabitler: Mutex::new(HashMap::new()),
            profiles_root,
            browser,
            geckodriver,
            headless,
        }
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
    pub async fn acquire_for(&self, oturum: Option<u64>) -> Result<Lease<'_>> {
        loop {
            let simdi = Instant::now();

            // 1) Sabitlenmis hesap varsa onceligi onun.
            if let Some(anahtar) = oturum
                && let Some(i) = self.sabitler.lock().await.get(&anahtar).copied()
            {
                let karar = sabit_karari(
                    &mut *self.slots[i].state.lock().await,
                    simdi,
                    &self.pacing,
                );
                match karar {
                    Sabit::Kullan => match self.kilitle(i).await {
                        Ok(l) => return Ok(l),
                        Err(e) => {
                            let mesaj = format!("{e:#}");
                            eprintln!("[{}] sabit hesap acilamadi: {mesaj}", self.slots[i].account.name);
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
                            self.slots[i].account.name
                        );
                    }
                }
            }
            let mut aday: Option<(usize, u32)> = None;
            let mut en_erken: Option<Instant> = None;

            for (i, slot) in self.slots.iter().enumerate() {
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
                match self.kilitle(i).await {
                    Ok(l) => {
                        if let Some(anahtar) = oturum {
                            self.sabitle(anahtar, i).await;
                        }
                        return Ok(l);
                    }
                    // Hesap acilamadi (tarayici baslamadi, oturum dusmus...). Durumu
                    // `basarisiz` isliyor: geri cekilme uygular, dogrulama duvariysa
                    // insan kapisinda duraklatir. Boylece dongu ya baska hesaba gecer
                    // ya da hepsi elenince asagida acik hatayla biter — sonsuz donmez.
                    Err(e) => {
                        let mesaj = format!("{e:#}");
                        eprintln!("[{}] hesap acilamadi: {mesaj}", self.slots[i].account.name);
                        self.basarisiz(i, &mesaj).await;
                        continue;
                    }
                }
            }

            let Some(nb) = en_erken else {
                bail!(
                    "kullanilabilir hesap yok: hepsi duraklatilmis ya da saatlik tavani dolmus. \
                     Insan kapisinda bekleyen var mi diye bak."
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

    async fn kilitle(&self, i: usize) -> Result<Lease<'_>> {
        let slot = &self.slots[i];
        let mut guard = slot.session.lock().await;
        if guard.is_none() {
            let s = Session::open(
                &self.profiles_root,
                &slot.account.name,
                &self.browser,
                &self.geckodriver,
                self.headless,
            )
            .await?;
            // Tarayici bos sayfayla acilir; saglayiciya gitmeden istem kutusu yoktur.
            s.goto(self.provider.url).await?;
            let durum = detect_login(&s.client, &self.provider.selectors).await?;
            // Pencereyi KAPATMIYORUZ: gorunur kipte insan tam bu pencerede giris yapar,
            // sonra `POST /accounts/{i}/devam` ile hesap kaldigi yerden devam eder.
            *guard = Some(s);
            // Burada TAHMIN yok, olcum var: giris durumunu dogrudan gorduk. O yuzden
            // metin sezgisine birakmadan hesabi kendimiz duraklatiyoruz.
            let neden = match durum {
                LoginState::Ready => None,
                LoginState::NeedsLogin => {
                    Some("oturum kapali: acilan pencerede elle giris gerekiyor".to_string())
                }
                LoginState::Unknown => Some(
                    "ne istem kutusu ne giris baglantisi bulundu — dogrulama duvari ya da \
                     kirilmis secici. Pencereye bak."
                        .to_string(),
                ),
            };
            if let Some(neden) = neden {
                slot.state.lock().await.paused_reason = Some(neden.clone());
                bail!("'{}': {neden} Cozunce: POST /accounts/{i}/devam", slot.account.name);
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
        Ok(Lease { index: i, name: slot.account.name.clone(), guard })
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
        // Tarayici baslatilmiyor: bu testler yalniz durum mantigina dokunuyor.
        Pool::new(
            vec![Account { name: "a".into(), provider: "gemini".into() }],
            pacing,
            Provider {
                url: "http://ornek",
                selectors: crate::browser::Selectors {
                    prompt_input: "#p",
                    login_hint: "#l",
                    response: "#r",
                },
            },
            PathBuf::from("."),
            BrowserBinary(PathBuf::from(".")),
            PathBuf::from("."),
            true,
        )
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
