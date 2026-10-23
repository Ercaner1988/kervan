//! Secim katmani: site arayuzundeki model menusu ve ac/kapa ozellikler.
//!
//! Neden ayri sandik: secim mantigi (menu okuma, eslestirme, dogrulama) tarayici
//! iskeletinden bagimsiz bir sorumluluk; `kervan-tarayici` sandigi 1500 satir
//! sinirina dayandigi icin bu is buraya tasindi. Kod yalniz OLCULMUS secicilerle
//! calisir ve secimi DOGRULAMADAN devam etmez — sessiz yanlis model, en pahali
//! hata turu.
//!
//! ESLESTIRME TEK YERDE: menu metni ile istenen etiket Rust tarafinda, ortak
//! `katla` sandigiyla karsilastirilir. Once JS icinde de katlama yapiliyordu; iki
//! katlama ayrisinca var olan secenek "menude yok" sayildi (olculdu 2026-10-22).
//! Simdi JS yalniz HAM metni ve isaret durumunu dondurur, karar Rust'ta verilir.

use anyhow::{Context, Result, bail};
use fantoccini::Client;

/// Model menusundeki tek secenek: kimligi Kervan verir, etiketi site yazar.
///
/// `kod` model kimliginde gecer (`kervan/gemini/yuksek`), `etiket` sitedeki
/// gorunen metindir. Ikisini ayirmak sart: site metni degisken ("Yüksek",
/// "YUKSEK"), kimlik ise istemciyle yapilan sozlesme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSecenegi {
    pub kod: String,
    pub etiket: String,
    /// Menude GORUNUYOR ama bu hesapta SECILEMIYOR (ucretli plan gerekiyor).
    ///
    /// NEDEN TASINIYOR: kullanici bir hesabin potansiyelini gormeli, ama
    /// secilemeyen bir secenegi "secilebilir" gostermek sessiz yanlis model
    /// demektir. Bu yuzden kilit AYRI bir alandir: kimlik listelenir, panelde
    /// tiklanamaz, istek gelirse acik hata doner.
    pub kilitli: bool,
    /// Kilit nedeninin insan okunur hali ("ucretli plan gerekiyor").
    pub kilit_nedeni: Option<String>,
}

/// Bir saglayicinin model menusu: nasil acilir, ogeleri nasil bulunur.
///
/// OLCULEREK DOLDURULUR. Gemini icin 2026-10-22 olcumu: dugme
/// `button[aria-label*='Mod seçici']`, ogeler `[role=menuitem]`; secenegi
/// `aria-label="Seçildi"` tasiyan cocuk ya da `.selected` sinifi isaretliyor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelMenusu {
    /// Model menusunu acan dugme.
    pub dugme: String,
    /// Menudeki oge secicisi.
    pub oge: String,
    pub modeller: Vec<ModelSecenegi>,
}

impl ModelMenusu {
    /// Koda karsilik gelen secenek. Tanimsiz kod `None`.
    pub fn secenek(&self, kod: &str) -> Option<&ModelSecenegi> {
        self.modeller
            .iter()
            .find(|m| m.kod.eq_ignore_ascii_case(kod))
    }

    /// Koda karsilik gelen site etiketi. Tanimsiz kod `None`.
    pub fn etiket(&self, kod: &str) -> Option<&str> {
        self.secenek(kod).map(|m| m.etiket.as_str())
    }

    /// Kullanilabilir kodlar (hata mesajlarinda listelemek icin).
    pub fn kodlar(&self) -> Vec<&str> {
        self.modeller.iter().map(|m| m.kod.as_str()).collect()
    }
}

/// Model seciminin sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecimSonucu {
    /// Tiklandi ve dogrulandi.
    Degisti,
    /// Istenen model zaten seciliydi; tiklama yapilmadi.
    ZatenSecili,
}

/// Istek cozumleme hatasi: HTTP durum kodu + insan okunur mesaj.
///
/// Durum kodlari bilerek ayri: tanimsiz kod (400) ile "sitede boyle bir sey yok"
/// (501) ile "var ama bu hesapta secilemiyor" (402) farkli sorunlardir ve
/// istemciye ayni cevabi vermek teshisi imkansizlastirir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IstekHatasi {
    pub durum: u16,
    pub mesaj: String,
}

impl IstekHatasi {
    fn yeni(durum: u16, mesaj: String) -> Self {
        Self { durum, mesaj }
    }
}

/// Cozulmus istek: siteye uygulanacak model kodu (yoksa site varsayilani) ve
/// tanimlariyla eslesmis ac/kapa anahtarlari.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CozulmusIstek {
    pub model_kodu: Option<String>,
    pub anahtarlar: Vec<(Anahtar, bool)>,
}

/// Istegin model kodunu ve ac/kapa anahtarlarini TANIMA cozer.
///
/// Siteye hicbir sey yapilmadan once cagrilir: tanimsiz/kilitli bir secim
/// yuzunden istek gonderilip varsayilanla cevap almak, bu projenin yasakladigi
/// sessiz yanlis durumdur. `menu` o saglayicinin olculmus model menusudur
/// (`None` = model secimi hic tanimli degil).
pub fn coz_istek(
    saglayici: &str,
    menu: Option<&ModelMenusu>,
    kod: Option<&str>,
    tanimli_anahtarlar: &[Anahtar],
    istenen_anahtarlar: &[(String, bool)],
) -> Result<CozulmusIstek, IstekHatasi> {
    let mut model_kodu = None;
    if let Some(k) = kod {
        let Some(m) = menu else {
            return Err(IstekHatasi::yeni(
                501,
                format!(
                    "'{saglayici}' icin model secimi tanimli degil: sitede olculmus bir model \
                     menusu yok. Yalniz 'kervan/{saglayici}' kullanilabilir."
                ),
            ));
        };
        let Some(secenek) = m.secenek(k) else {
            let kodlar: Vec<String> = m
                .modeller
                .iter()
                .map(|s| format!("{} => {}", s.kod, s.etiket))
                .collect();
            return Err(IstekHatasi::yeni(
                400,
                format!(
                    "'{saglayici}' icin tanimsiz model kodu '{k}'. Tanimli: {}",
                    kodlar.join(", ")
                ),
            ));
        };
        // Kilitli model: menude GORUNUYOR ama bu hesapta secilemiyor.
        // Sessizce site varsayilanina dusmek yasak; nedeni acikca soylenir.
        if secenek.kilitli {
            let secilebilir: Vec<&str> = m
                .modeller
                .iter()
                .filter(|s| !s.kilitli)
                .map(|s| s.kod.as_str())
                .collect();
            return Err(IstekHatasi::yeni(
                402,
                format!(
                    "'{saglayici}' icin '{}' bu hesapta secilemiyor: {}. Secilebilir kodlar: {}",
                    secenek.etiket,
                    secenek
                        .kilit_nedeni
                        .as_deref()
                        .unwrap_or("sitede ucretli plan gerektiriyor"),
                    secilebilir.join(", ")
                ),
            ));
        }
        model_kodu = Some(k.to_string());
    }

    let mut cozulmus: Vec<(Anahtar, bool)> = Vec::new();
    if !istenen_anahtarlar.is_empty() {
        if tanimli_anahtarlar.is_empty() {
            return Err(IstekHatasi::yeni(
                501,
                format!(
                    "'{saglayici}' icin ac/kapa ozelligi tanimli degil: sitede olculmus bir \
                     dugme yok. Istegi anahtarsiz tekrarla."
                ),
            ));
        }
        for (kod, istenen) in istenen_anahtarlar {
            match tanimli_anahtarlar
                .iter()
                .find(|a| a.kod.eq_ignore_ascii_case(kod))
            {
                Some(a) => cozulmus.push((a.clone(), *istenen)),
                None => {
                    let kodlar: Vec<String> = tanimli_anahtarlar
                        .iter()
                        .map(|a| format!("{} => {}", a.kod, a.etiket))
                        .collect();
                    return Err(IstekHatasi::yeni(
                        400,
                        format!(
                            "'{saglayici}' icin tanimsiz anahtar kodu '{kod}'. Tanimli: {}",
                            kodlar.join(", ")
                        ),
                    ));
                }
            }
        }
    }
    Ok(CozulmusIstek {
        model_kodu,
        anahtarlar: cozulmus,
    })
}

/// Panelin secim listesi icin tek saglayicinin kaydi; secenegi yoksa `None`.
///
/// Hicbir secenegi olmayan saglayiciyi listeye koymuyoruz: panelde bos bir
/// baslik gormek "secenek var ama yuklenmedi" izlenimi veriyordu.
pub fn secenek_kaydi(
    menu: Option<&ModelMenusu>,
    anahtarlar: &[Anahtar],
) -> Option<serde_json::Value> {
    let modeller: Vec<serde_json::Value> = menu
        .map(|m| {
            m.modeller
                .iter()
                .map(|x| {
                    serde_json::json!({
                        "kod": x.kod,
                        "etiket": x.etiket,
                        // Panel kilitli secenegi GORUR ama seciciden tiklanamaz
                        // yapar; nedeni yaninda yazar.
                        "kilitli": x.kilitli,
                        "kilit_nedeni": x.kilit_nedeni,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let anahtarlar: Vec<serde_json::Value> = anahtarlar
        .iter()
        .map(|a| serde_json::json!({ "kod": a.kod, "etiket": a.etiket }))
        .collect();
    if modeller.is_empty() && anahtarlar.is_empty() {
        return None;
    }
    Some(serde_json::json!({ "modeller": modeller, "anahtarlar": anahtarlar }))
}

/// Site arayuzundeki tek bir AC/KAPA ozelligi (deepseek "DeepThink"/"Search" gibi).
///
/// Model menusunden farki: o tek secimli (radio), bu iki durumlu (checkbox). Ortak
/// olan sey ayni: oge metinle bulunur, DURUMU siteden okunur ve uygulama sonrasi
/// yeniden okunarak DOGRULANIR. Bu yuzden ayni eslestirme (`hedef_indeks`) ve ayni
/// isaret okuma kullanilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anahtar {
    /// Kervan kimligi (`derin`), istemciden bu adla istenir.
    pub kod: String,
    /// Dugmeleri tasiyan kapsayici secici (olculur). Ornek: `.ds-toggle-button`.
    pub dugme: String,
    /// Sitedeki gorunen ad; metinle eslesir ("DeepThink").
    pub etiket: String,
}

/// Bir ac/kapa ozelliginin uygulama sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnahtarSonucu {
    /// Durum degistirildi ve dogrulandi.
    Degisti,
    /// Ozellik zaten istenen durumdaydi; tiklanmadi.
    ZatenIstendigi,
}

/// Menudeki bir ogenin ham metni ve secim isareti.
///
/// `metin` HAM gelir (buyuk harf, satir sonu, bosluk ne varsa); katlama ve
/// karsilastirma Rust tarafinda yapilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OgeDurum {
    /// Ogenin ADI: `.label` varsa o, yoksa ilk satir (site aciklamayi alt satira koyar).
    pub metin: String,
    /// Ogenin tum metni (ad + aciklama); eslestirmede son care.
    pub tam: String,
    pub isaretli: bool,
}

/// Menudeki ogeleri sira ile dondurur (ad + tam metin + isaret). Menu ACIK olmali.
///
/// ISARET OLCUMU (2026-10-22, uc site olculdu): gemini `.selected` /
/// `aria-label="Seçildi"`, kimi oge uzerinde `checked` sinifi, z.ai
/// `data-selected="true"`. Tek bir isaret bicimine guvenmek "hedef isaretlenmedi"
/// yanlis alarmina yol aciyordu.
///
/// AD TAHMINI: z.ai gibi siteler adi ve aciklamayi ayri satira koyar. Tam metinle
/// icerme aramasi yapilirsa "GLM-5.3" etiketi "GLM-5.3-Flash" ogesine de uyar ve
/// yanlis model secilirdi; bu yuzden ad ilk satirdan okunur, tam metin son caredir.
const OGE_LISTESI: &str = r#"
const d = (s) => (s || '').replace(/[ \t\r]+/g, ' ').trim();
const isaret = (e) => /(^|[\s-])(selected|checked|active|on)(\s|$)/.test(e.className || '') || ['data-selected','data-state','aria-checked','aria-selected','aria-pressed'].some((a) => ['true','checked'].includes(e.getAttribute(a)))
  || !!e.querySelector('[aria-label="Seçildi"],[aria-label="Selected"],[aria-checked="true"]');
return [...document.querySelectorAll(arguments[0])].map((e) => {
  const ad = d((e.querySelector('.label') || {}).innerText) || (e.innerText || '').split('\n').map(d).filter(Boolean)[0] || '';
  return { metin: ad, tam: d((e.innerText || e.textContent || '').replace(/\s+/g, ' ')), isaretli: isaret(e) };
});
"#;

/// Siradaki ogeye tikla. Secim DOGRULANMAZ; dogrulama cagiranda.
const OGE_TIKLA: &str = r#"
const ogeler = document.querySelectorAll(arguments[0]);
if (!ogeler[arguments[1]]) { return 'yok'; }
ogeler[arguments[1]].click();
return 'tamam';
"#;

/// Verilen seciciye uyan dugmeye DOM uzerinden tikla.
const DUGME_TIKLA: &str = r#"
const d = document.querySelector(arguments[0]);
if (!d) { return 'dugme-yok'; }
d.click();
return 'tamam';
"#;

/// Bir model secenegini sec ve secildigini DOGRULA.
///
/// Akis: menuyu ac -> hedef oge menude var mi / zaten isaretli mi -> degilse
/// tikla -> menuyu yeniden acip HEDEF ogenin isaretini dogrula -> kapat.
///
/// NEDEN ISARETCI TIKLAMASI DEGIL: olculdu 2026-10-22 (gemini) — WebDriver'in
/// isaretci tiklamasi menuyu kapatiyor ama secimi UYGULAMIYOR; DOM `.click()`
/// uyguluyor ("Yüksek" secildi, yeniden acilista isaretli gorundu). Ayni olcumde
/// "3.1 Pro" tiklamasi da uygulanmadi cunku o oge `upsell_` kimlikli, yani
/// ucretli plan gerektiriyor; bunu ancak DOGRULAMA yakaliyor.
///
/// NEDEN "menu genelinde isaretli oge" DEGIL: hesaplarin bir kisminda menu hem
/// MODEL ekseni ("3.5 Flash-Lite") hem DUSUNME DUZEYI ekseni tasiyor; "ilk
/// isaretli oge" okumak dogru secim yapilmisken yanlis sonuc veriyordu. Olcut
/// HEDEF ogenin kendi isareti.
///
/// Dugme metnine guvenilmiyor: gemini'de dusunme duzeyi degisince dugme metni
/// degismiyor ("Gemini / Otomatik" kaliyor), o yuzden olcut menudeki isaret.
pub async fn model_sec(
    c: &Client,
    menu: &ModelMenusu,
    etiket: &str,
    zaman_asimi: std::time::Duration,
) -> Result<SecimSonucu> {
    let bekle = |ms: u64| tokio::time::sleep(std::time::Duration::from_millis(ms));

    // Bilinmeyen etiket istemci hatasidir: sitede olmayan bir seyi "secer gibi"
    // yapip varsayilanla cevap vermek sessiz yanlis model olurdu.
    if !menu
        .modeller
        .iter()
        .any(|m| metin_iceriyor_mu(&m.etiket, etiket))
    {
        bail!(
            "sitede tanimli olmayan model etiketi '{etiket}'. Tanimli: {}",
            menu.modeller
                .iter()
                .map(|m| format!("{}=>{}", m.kod, m.etiket))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Kilitli secenek sitede TIKLANINCA UYGULANMIYOR (ucretli plan). Denemek
    // zaman kaybi ve yaniltici "(Uygulanmadi)" hatasi uretir; nedeni acikca soyle.
    if let Some(m) = menu
        .modeller
        .iter()
        .find(|m| metin_iceriyor_mu(&m.etiket, etiket))
        && m.kilitli
    {
        bail!(
            "'{}' bu hesapta secilemiyor: {}",
            m.etiket,
            m.kilit_nedeni
                .as_deref()
                .unwrap_or("sitede ucretli plan gerektiriyor")
        );
    }

    // 0) Menu acik kalmis olabilir (onceki hata): kapat.
    if menu_acik_mi(c, &menu.oge).await? {
        dugmeye_bas(c, &menu.dugme).await?;
    }
    // 1) Ac ve ogelerin cizilmesini bekle.
    dugmeye_bas(c, &menu.dugme).await?;
    if !menu_bekle(c, &menu.oge, std::time::Duration::from_secs(15)).await? {
        bail!(
            "model menusu acilmadi ('{}'). Dugme secicisi site arayuzune uymuyor olabilir.",
            menu.dugme
        );
    }

    // 2) Hedef oge menude var mi, zaten isaretli mi?
    let ogeler = oge_listesi(c, &menu.oge).await?;
    let Some(i) = hedef_indeks(&ogeler, etiket) else {
        let liste = ogeler
            .iter()
            .map(|o| o.metin.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        dugmeye_bas(c, &menu.dugme).await?;
        bail!("model menusunde '{etiket}' yok. Menudekiler: {liste}");
    };
    if ogeler[i].isaretli {
        dugmeye_bas(c, &menu.dugme).await?;
        return Ok(SecimSonucu::ZatenSecili);
    }

    // 3) Tikla.
    let r = c
        .execute(
            OGE_TIKLA,
            vec![serde_json::json!(menu.oge), serde_json::json!(i)],
        )
        .await
        .context("model ogesine tiklama betigi calistirilamadi")?;
    if r.as_str() != Some("tamam") {
        dugmeye_bas(c, &menu.dugme).await?;
        bail!("model menusu yenilendi ve {i}. oge artik yok");
    }
    bekle(1500).await;

    // 4) Dogrula: menuyu yeniden ac, hedefi YENIDEN bul, isaretini oku, kapat.
    if menu_acik_mi(c, &menu.oge).await? {
        dugmeye_bas(c, &menu.dugme).await?;
    }
    dugmeye_bas(c, &menu.dugme).await?;
    if !menu_bekle(c, &menu.oge, zaman_asimi).await? {
        bail!("model menusu dogrulama icin yeniden acilmadi");
    }
    let sonra = oge_listesi(c, &menu.oge).await?;
    let isaretli = sonra
        .iter()
        .find(|o| o.isaretli)
        .map(|o| o.metin.clone())
        .unwrap_or_default();
    let dogru = hedef_indeks(&sonra, etiket).is_some_and(|j| sonra[j].isaretli);
    if menu_acik_mi(c, &menu.oge).await? {
        dugmeye_bas(c, &menu.dugme).await?;
    }
    if dogru {
        Ok(SecimSonucu::Degisti)
    } else {
        bail!(
            "'{etiket}' tiklandi ama hedef oge isaretlenmedi (menude isaretli: '{}') — \
             secim uygulanmadi (ucretli plan gerektiren 'upsell' secenegi olabilir). \
             Sessizce yanlis modelle devam etmemek icin istek dusuruldu.",
            if isaretli.is_empty() {
                "bilinmiyor"
            } else {
                &isaretli
            }
        )
    }
}

/// Tek bir AC/KAPA ozelligini istenen duruma getir ve DOGRULA.
///
/// Akis: dugmeleri oku -> metinle hedefi bul -> durum zaten istenen mi -> degilse
/// DOM `.click()` -> yeniden oku -> durum istenen mi.
///
/// OLCULDU 2026-10-22 (deepseek): iki dugme `.ds-toggle-button`, durum
/// `aria-pressed="true|false"` VE `ds-toggle-button--selected` sinifi. Isaretci
/// tiklamasi da DOM tiklamasi da uyguluyor; dogrulama yine de sitedan okunur.
///
/// NEDEN DOGRULAMA: bu dugmeler site tarafinda plan/kota ile kapatilabilir. Tiklama
/// "gecti" gorunup durum degismezse istek sessizce YANLIS ozellikle gonderilirdi.
pub async fn anahtar_ayarla(
    c: &Client,
    anahtar: &Anahtar,
    istenen: bool,
    zaman_asimi: std::time::Duration,
) -> Result<AnahtarSonucu> {
    let basla = std::time::Instant::now();
    let mut ogeler = oge_listesi(c, &anahtar.dugme).await?;
    while ogeler.is_empty() && basla.elapsed() < zaman_asimi {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        ogeler = oge_listesi(c, &anahtar.dugme).await?;
    }
    let liste = || {
        ogeler
            .iter()
            .map(|o| o.metin.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let Some(i) = hedef_indeks(&ogeler, &anahtar.etiket) else {
        bail!(
            "'{}' ac/kapa ozelligi sayfada bulunamadi ('{}'). Sayfadakiler: {}",
            anahtar.kod,
            anahtar.dugme,
            if ogeler.is_empty() {
                "hic oge yok".to_string()
            } else {
                liste()
            }
        );
    };
    if ogeler[i].isaretli == istenen {
        return Ok(AnahtarSonucu::ZatenIstendigi);
    }

    let r = c
        .execute(
            OGE_TIKLA,
            vec![serde_json::json!(anahtar.dugme), serde_json::json!(i)],
        )
        .await
        .context("ac/kapa ogesine tiklama betigi calistirilamadi")?;
    if r.as_str() != Some("tamam") {
        bail!(
            "'{}' ac/kapa ogesi tiklanamadi (liste yenilendi)",
            anahtar.kod
        );
    }
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;

    let sonra = oge_listesi(c, &anahtar.dugme).await?;
    let simdi = hedef_indeks(&sonra, &anahtar.etiket)
        .map(|j| sonra[j].isaretli)
        .unwrap_or(false);
    if simdi == istenen {
        Ok(AnahtarSonucu::Degisti)
    } else {
        bail!(
            "'{}' tiklandi ama durum degismedi (istenen: {}, simdi: {}). Ozellik plan/kota \
             nedeniyle kilitli olabilir; sessizce yanlis ayarla devam etmemek icin istek \
             dusuruldu.",
            anahtar.kod,
            if istenen { "acik" } else { "kapali" },
            if simdi { "acik" } else { "kapali" }
        )
    }
}

/// Hedef etiketi iceren ogenin sirasi. Yoksa `None`.
///
/// SIRA ONEMLI (olculdu 2026-10-22, z.ai): menude hem "GLM-5.3-Flash" hem
/// "GLM-5.3" var. Tek gecisli icerme aramasi "GLM-5.3" etiketini ILK ogeye
/// (Flash) bagliyordu — yani yanlis model. Bu yuzden once ad TAM esitligi, sonra
/// ad icermesi, en son tam metin (ad + aciklama) icermesi denenir.
pub fn hedef_indeks(ogeler: &[OgeDurum], etiket: &str) -> Option<usize> {
    let aranan = katlanmis(etiket);
    ogeler
        .iter()
        .position(|o| katlanmis(&o.metin) == aranan)
        .or_else(|| {
            ogeler
                .iter()
                .position(|o| metin_iceriyor_mu(&o.metin, etiket))
        })
        .or_else(|| {
            ogeler
                .iter()
                .position(|o| metin_iceriyor_mu(&o.tam, etiket))
        })
}

/// Katlanmis ve bosluklari tekillestirilmis metin — karsilastirmanin tek bicimi.
fn katlanmis(s: &str) -> String {
    katla::katla(s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Menude kac oge var? (menu acik mi sorusunun olculebilir karsiligi)
async fn menu_acik_mi(c: &Client, oge: &str) -> Result<bool> {
    Ok(!oge_listesi(c, oge).await?.is_empty())
}

/// Menunun ogeleri cizilene kadar bekle.
async fn menu_bekle(c: &Client, oge: &str, zaman_asimi: std::time::Duration) -> Result<bool> {
    let basla = std::time::Instant::now();
    while basla.elapsed() < zaman_asimi {
        if menu_acik_mi(c, oge).await? {
            return Ok(true);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Ok(false)
}

/// Menudeki ogeleri oku: metin + isaret. Menu ACIK olmali.
async fn oge_listesi(c: &Client, oge: &str) -> Result<Vec<OgeDurum>> {
    let v = c
        .execute(OGE_LISTESI, vec![serde_json::json!(oge)])
        .await
        .context("model menusu okunamadi")?;
    let mut liste = Vec::new();
    if let Some(dizi) = v.as_array() {
        for e in dizi {
            liste.push(OgeDurum {
                metin: e
                    .get("metin")
                    .and_then(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string(),
                isaretli: e.get("isaretli").and_then(|m| m.as_bool()).unwrap_or(false),
                tam: e
                    .get("tam")
                    .and_then(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
    }
    Ok(liste)
}

/// Menu dugmesine DOM uzerinden bas ve kisa sure bekle.
async fn dugmeye_bas(c: &Client, dugme: &str) -> Result<()> {
    let r = c
        .execute(DUGME_TIKLA, vec![serde_json::json!(dugme)])
        .await
        .context("model dugmesine basilamadi")?;
    if r.as_str() != Some("tamam") {
        bail!("model dugmesi bulunamadi: '{dugme}'");
    }
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    Ok(())
}

/// Katlanmis metin, katlanmis aranan'i iceriyor mu?
///
/// Katlama ortak `katla` sandigindan: `I`/`İ`/`ı` → `i`, aksanlar atilir. Menu
/// metni "3.1 PRO", " Yüksek ", "Gemini\nOtomatik" gibi gelebiliyor; ham
/// `contains` bunlarin cogunu kacirir ve var olan secenek "menude yok" sayilirdi.
pub fn metin_iceriyor_mu(metin: &str, aranan: &str) -> bool {
    katlanmis(metin).contains(&katlanmis(aranan))
}

#[cfg(test)]
mod testler {
    use super::*;

    /// Iki gecisli eslestirme: "GLM-5.3" etiketi "GLM-5.3-Flash" ogesine baglanmamali.
    ///
    /// Olculdu 2026-10-22 (z.ai): menu sirasi Flash, 5.3, 5.2. Tek gecisli icerme
    /// aramasi 5.3 istegini Flash'a bagliyordu; secim uygulanmis gibi gorunurken
    /// yanlis modelle yanit verilirdi. Once AD tam esitligi aranir.
    #[test]
    fn etiket_once_tam_ada_gore_eslesir() {
        let oge = |ad: &str, tam: &str| OgeDurum {
            metin: ad.to_string(),
            tam: tam.to_string(),
            isaretli: false,
        };
        let menudekiler = vec![
            oge(
                "GLM-5.3-Flash",
                "GLM-5.3-Flash NEW Lightweight flagship model, premium quality",
            ),
            oge(
                "GLM-5.3",
                "GLM-5.3 Flagship model, excels at coding and long-horizon tasks",
            ),
            oge("GLM-5.2", "GLM-5.2 Previous flagship model"),
        ];
        assert_eq!(hedef_indeks(&menudekiler, "GLM-5.3"), Some(1));
        assert_eq!(hedef_indeks(&menudekiler, "GLM-5.3-Flash"), Some(0));
        assert_eq!(hedef_indeks(&menudekiler, "GLM-5.2"), Some(2));
        // Tam ad esitligi yoksa ad icermesi, o da yoksa tam metin denenir. Tam metin
        // son caredir: aciklamada gecen kelime de eslesir ("flagship" → Flash).
        assert_eq!(hedef_indeks(&menudekiler, "flagship"), Some(0));
        assert_eq!(hedef_indeks(&menudekiler, "premium quality"), Some(0));
        assert_eq!(hedef_indeks(&menudekiler, "yok-boyle-bir-sey"), None);
    }

    fn menu() -> ModelMenusu {
        ModelMenusu {
            dugme: "b".into(),
            oge: "o".into(),
            modeller: vec![
                ModelSecenegi {
                    kod: "orta".into(),
                    etiket: "Orta".into(),
                    kilitli: false,
                    kilit_nedeni: None,
                },
                ModelSecenegi {
                    kod: "pro".into(),
                    etiket: "3.1 Pro".into(),
                    kilitli: true,
                    kilit_nedeni: Some("ucretli plan gerekiyor".into()),
                },
            ],
        }
    }

    fn anahtar(kod: &str) -> Anahtar {
        Anahtar {
            kod: kod.into(),
            dugme: ".ds-toggle-button".into(),
            etiket: "Search".into(),
        }
    }

    /// Kilitli model: sessizce varsayilana dusmek yerine 402 + neden. Serbest model
    /// gecer; tanimsiz kod 400, menusu olmayan saglayici 501.
    #[test]
    fn kilitli_model_402_serbest_model_gecer() {
        let m = menu();
        let serbest = coz_istek("gemini", Some(&m), Some("orta"), &[], &[]).expect("serbest");
        assert_eq!(serbest.model_kodu.as_deref(), Some("orta"));

        let h = coz_istek("gemini", Some(&m), Some("pro"), &[], &[]).unwrap_err();
        assert_eq!(h.durum, 402);
        assert!(h.mesaj.contains("ücretli plan") || h.mesaj.contains("ucretli plan"));
        assert!(
            h.mesaj.contains("orta"),
            "secilebilirleri saymali: {}",
            h.mesaj
        );

        let h = coz_istek("gemini", Some(&m), Some("yok"), &[], &[]).unwrap_err();
        assert_eq!(h.durum, 400);

        let h = coz_istek("perplexity", None, Some("x"), &[], &[]).unwrap_err();
        assert_eq!(h.durum, 501);
    }

    /// Anahtarlar da ayni sekilde: tanimsiz kod 400, hic anahtar tanimli degilse 501;
    /// tanimli anahtar istenen durumuyla birlikte cozulur.
    #[test]
    fn anahtar_cozumu_400_501_ve_basari() {
        let a = anahtar("arama");
        let ok = coz_istek(
            "deepseek",
            None,
            None,
            std::slice::from_ref(&a),
            &[("arama".to_string(), false)],
        )
        .expect("tanimli anahtar");
        assert_eq!(ok.anahtarlar.len(), 1);
        assert!(!ok.anahtarlar[0].1);

        let h = coz_istek(
            "deepseek",
            None,
            None,
            std::slice::from_ref(&a),
            &[("yok".to_string(), true)],
        )
        .unwrap_err();
        assert_eq!(h.durum, 400);

        let h = coz_istek("kimi", None, None, &[], &[("arama".to_string(), true)]).unwrap_err();
        assert_eq!(h.durum, 501);
    }
}
