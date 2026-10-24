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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OgeDurum {
    /// Ogenin ADI: `.label` varsa o, yoksa ilk satir (site aciklamayi alt satira koyar).
    pub metin: String,
    /// Ogenin tum metni (ad + aciklama); eslestirmede son care.
    pub tam: String,
    pub isaretli: bool,
    /// Isaretin HANGI kuraldan geldigi (`sinif:selected`, `aria-checked=true`,
    /// `torun:aria-checked=true` gibi). Bos ise: oge isaretsiz ya da eski
    /// eklenti bu alani gondermiyor. Teshis icin: "iki oge birden secili"
    /// gibi bozuk okumada yanlis eslesen kural adi hata metninde gorunur.
    #[serde(default)]
    pub kanit: String,
}

/// İşaret kanıtı **zayıf** mı? Yalnız alt düğümdeki çıplak `aria-checked=true`
/// zayıftır: satırdaki herhangi bir alt denetimin durumu olabilir. 2026-10-24
/// canlı ölçümünde seçili OLMAYAN satırda eşleşti. Geri kalan her kanıt
/// (öğenin kendi sınıfı/özniteliği, alt düğümdeki `aria-label="Seçildi"`)
/// güçlüdür. Boş kanıt (eski eklenti ya da işaretsiz öğe) de güçlü sayılır.
///
/// DİKKAT: "güçlü" demek "seçili" demek DEĞİLDİR. Bu yalnız kanıtın niteliğini
/// söyler; "zaten seçili" hükmü için ayrıca menüde işaretli TEK öğe olması
/// gerekir (`zaten_secili`). Bu yüzden boş kanıtlı eski eklenti, çok işaretli
/// menüde artık `ZatenSecili` değil TIKLAMA alır — eski davranış birebir
/// korunmaz, bilerek sıkılaştırıldı.
pub fn guclu_kanit(kanit: &str) -> bool {
    !kanit.starts_with("torun:aria-checked")
}

/// "ZATEN SEÇİLİ": tıklamayı atlamaya yetecek kanıt.
///
/// İki şart birlikte: hedefin KENDİ işareti güçlü **ve** menüde işaretli tek
/// öğe o. Bu, tıklamayı atlamanın tek güvenli hâlidir; başka her durumda
/// tıklanır — model menüsü radyo olduğu için seçili öğeye tıklamak durumu
/// değiştirmez, seçili değilse doğru modeli kurar. Yani şüphede kalınca
/// **tıkla**, "zaten seçili" deme.
///
/// NEDEN SAYIM TEK BAŞINA YETMİYOR (2026-10-24 incelemesi): aynı menü iki
/// eksen taşıyabiliyor (gemini: mod ekseni + düşünme düzeyi; bkz. `model_sec`
/// belgesi ve `kervan.toml`). O hâlde liste genelinde İKİ işaret normaldir ve
/// "tam bir işaret" kuralı doğru seçilmiş modeli doğrulanamaz sayardı. Bu
/// yüzden sayım yalnız "atla" hükmünde ve yalnız güçlü kanıtla kullanılır.
pub fn zaten_secili(ogeler: &[OgeDurum], hedef: usize) -> bool {
    let Some(o) = ogeler.get(hedef) else {
        return false;
    };
    if !o.isaretli || !guclu_kanit(&o.kanit) {
        return false;
    }
    ogeler.iter().filter(|x| x.isaretli).count() == 1
}

/// TIKLAMA SONRASI DOĞRULAMA: hedefin işareti yerinde mi?
///
/// 1. Hedef güçlü kanıtla işaretli → doğrulandı. Başka eksenin de işaretli
///    olması bunu geçersiz kılmaz (iki eksenli menü).
/// 2. Hedef yalnız zayıf kanıtla işaretli → listede zayıf kanıtlı TEK öğe ise
///    doğrulandı. Gerekçe: radyo ekseninde işaret tek yerdedir; zayıf kanıt
///    başka bir satırda da varsa hangisinin gerçek seçim olduğu bilinmiyor.
///
/// KALAN RİSK (açıkça): tıklama yutulur ve hedefte tıklamadan ÖNCE de duran
/// bir zayıf işaret (satır içi bir alt denetimin `aria-checked`ı) varsa 2.
/// madde yanlış pozitif verebilir. Bunu elemek için "işaret hedefe taşındı"
/// farkı gerekir; ama zayıf kanıtla işaretlenen eksende seçim zaten hedefteyse
/// tıklama hiçbir değişiklik üretmez ve fark ölçülemez — o hâlde 2. maddeyi
/// sıkılaştırmak, doğru seçilmiş isteği her turda düşürürdü (2026-10-24
/// incelemesi: "işlev yok"). Tercih: isteği düşürmemek, riski belgelemek.
pub fn secim_dogrulandi(sonra: &[OgeDurum], hedef: usize) -> bool {
    let Some(o) = sonra.get(hedef) else {
        return false;
    };
    if !o.isaretli {
        return false;
    }
    if guclu_kanit(&o.kanit) {
        return true;
    }
    sonra
        .iter()
        .filter(|x| x.isaretli && !guclu_kanit(&x.kanit))
        .count()
        == 1
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
/// BU BETİK `eklenti/content_common.js`teki `kervanOgeOkuHam` İLE AYNI KURALI
/// UYGULAMAK ZORUNDA. İki kopya ayrıştığında bir yolun "seçili" dediğine öbürü
/// "seçili değil" diyor ve aynı arıza yalnız bir yolda kapanıyordu (2026-10-24
/// incelemesi). `eklenti/test/oge_listesi.test.js` iki uygulamayı aynı DOM
/// üzerinde karşılaştırır; kural değişecekse İKİSİ BİRDEN değişmeli.
const OGE_LISTESI: &str = r#"
const d = (s) => (s || '').replace(/[ \t\r]+/g, ' ').trim();
// SVG ogesinde className nesnedir (SVGAnimatedString); dogrudan regex'e vermek
// sinif isaretini sessizce kaciriyordu (2026-10-24).
const sinif = (e) => (typeof e.className === 'string' ? e.className : (e.className && e.className.baseVal) || '');
const isaret = (e) => /(^|[\s-])(selected|checked|active|on)(\s|$)/.test(sinif(e)) || ['data-selected','data-state','aria-checked','aria-selected','aria-pressed'].some((a) => ['true','checked'].includes(e.getAttribute(a)))
  || !!e.querySelector('[aria-label="Seçildi"],[aria-label="Selected"],[aria-checked="true"]');
// KANIT: hangi kural esledi? "torun:aria-checked=true" ZAYIF kanittir (satirdaki
// herhangi bir alt denetimin durumu olabilir); gerisi guclu. Bkz. zaten_secili.
const isaretKaniti = (e) => {
  const m = /(^|[\s-])(selected|checked|active|on)(\s|$)/.exec(sinif(e));
  if (m) return 'sinif:' + m[2];
  for (const a of ['data-selected','data-state','aria-checked','aria-selected','aria-pressed']) {
    const v = e.getAttribute(a);
    if (['true','checked'].includes(v)) return a + '=' + v;
  }
  if (e.querySelector('[aria-label="Seçildi"],[aria-label="Selected"]')) return 'torun:secildi';
  if (e.querySelector('[aria-checked="true"]')) return 'torun:aria-checked=true';
  return '';
};
const ad = (e) => {
  const l = d((e.querySelector('.label') || {}).innerText);
  if (l) return l;
  // IKON ATLAMA: AI Studio kartlari ikon adini ILK satira koyuyor ("spark",
  // "audio_magic_eraser"), model adi ikinci satirda. Kopuk (detached) kopyada
  // innerText satir sonlarini kaybettigi icin kopya YETMIYOR (olculdu: tum kart
  // tek satira dondu) — bu yuzden canli ogenin satirlari okunur, ikon
  // dugumlerinin metniyle BIREBIR eslesen satirlar atlanir.
  const ikonlar = new Set([...e.querySelectorAll('mat-icon,svg,[class*="icon"],[class*="symbol"],[class*="ligature"]')]
    .map((x) => d(x.innerText || x.textContent)).filter(Boolean));
  const satirlar = (e.innerText || '').split('\n').map(d).filter(Boolean);
  return satirlar.find((s) => !ikonlar.has(s)) || satirlar[0] || '';
};
return [...document.querySelectorAll(arguments[0])].map((e) => {
  return { metin: ad(e), tam: d((e.innerText || e.textContent || '').replace(/\s+/g, ' ')), isaretli: isaret(e), kanit: isaretKaniti(e) };
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
    //    Once dugmenin CIZILMESINI bekle: `ask` bu asamada istem kutusunu henuz
    //    beklememis oluyor (AI Studio'da dugme gec ciziliyor).
    if !dugme_bekle(c, &menu.dugme, std::time::Duration::from_secs(15)).await {
        bail!(
            "model menusu dugmesi sayfada cizilmedi ('{}') — sayfa yuklenmedi ya da secici kirilmis",
            menu.dugme
        );
    }
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
    if zaten_secili(&ogeler, i) {
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
    // TEŞHİS: işaretli satırların HEPSİ, hangi kanıtla işaretlendiğiyle birlikte.
    // Yalnız ilk işaretliyi metin olarak basmak, zayıf (güvenilmez) bir satırı
    // "işaretli" diye adlandırıp teşhisi yanıltıyordu (2026-10-24 incelemesi).
    let isaretli = sonra
        .iter()
        .filter(|o| o.isaretli)
        .map(|o| format!("{}[{}]", o.metin, o.kanit))
        .collect::<Vec<_>>()
        .join(", ");
    let hedef_isaretli = hedef_indeks(&sonra, etiket).is_some_and(|j| secim_dogrulandi(&sonra, j));
    // IKINCI TANIK: dugmenin kendi metni. AI Studio'da (olculdu 2026-10-23) model
    // kartlarinda HICBIR isaret yok — ne sinif ne aria; secili model yalnizca acma
    // dugmesinin metninde gorunuyor ("Gemini 3.8 Flash"). Yalniz isarete bakmak bu
    // sitede her secimi "uygulanmadi" diye dusururdu. Iki taniktan BIRI yeter:
    // yanlis pozitif icin dugmenin zaten hedef adi gostermesi gerekir, o da zaten
    // istenen durumdur.
    let dugme_metni = if hedef_isaretli {
        String::new()
    } else {
        dugme_metni(c, &menu.dugme).await
    };
    let dogru = hedef_isaretli || metin_iceriyor_mu(&dugme_metni, etiket);
    if menu_acik_mi(c, &menu.oge).await? {
        dugmeye_bas(c, &menu.dugme).await?;
    }
    // SON KAPATMA GARANTISI: bazi sitelerde (AI Studio) acma dugmesine tiklamak
    // menuyu kapatmiyor ve acik kalan katman istem kutusunu kapatiyor. Istek
    // gonderilmeden once menunun KAPALI oldugu dogrulanir.
    if menu_acik_mi(c, &menu.oge).await? {
        escape_bas(c).await?;
        bekle(500).await;
        if menu_acik_mi(c, &menu.oge).await? {
            bail!("model menusu kapanmadi ('{}'); ESCAPE de yetmedi", menu.oge);
        }
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
                kanit: e
                    .get("kanit")
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

/// Model menusunu acan dugme sayfada cizildi mi?
async fn dugme_var_mi(c: &Client, dugme: &str) -> bool {
    const VAR: &str = r#"
return !!document.querySelector(arguments[0]);
"#;
    matches!(
        c.execute(VAR, vec![serde_json::json!(dugme)]).await,
        Ok(v) if v.as_bool() == Some(true)
    )
}

/// Dugme cizilene kadar bekle.
///
/// NEDEN: `ask` model secimini istem kutusunu beklemeden ONCE yapiyor (site model
/// degistirirken istemi yutabiliyor). AI Studio olcumunde (2026-10-23) sayfa ilk
/// acildiginda dugme henuz DOM'da olmadigi icin "model dugmesi bulunamadi" hatasi
/// cikiyordu; oysa birkac saniye sonra oradaydi. Beklemek, gercek arizayi
/// (secici kirilmis) yavasliktan ayirir.
async fn dugme_bekle(c: &Client, dugme: &str, zaman_asimi: std::time::Duration) -> bool {
    let basla = std::time::Instant::now();
    while basla.elapsed() < zaman_asimi {
        if dugme_var_mi(c, dugme).await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    false
}

/// Menuyu ESCAPE ile kapat (dugmeye tiklamak yetmediginde).
///
/// OLCULDU 2026-10-23 (AI Studio): model secildikten sonra dogrulama icin menu
/// yeniden aciliyor; acma dugmesine tiklamak bu menuyu KAPATMIYOR. Menü acik
/// kalinca `cdk-overlay-backdrop` istem kutusunu kapatiyor ve istek "isten
/// kutusunun ustunu bir katman kapatiyor" hatasina dusuyordu. ESCAPE'i body'ye
/// gonderiyoruz: metin alanina odakliyken gondermek kutuya kacis karakteri
/// yazabilirdi.
async fn escape_bas(c: &Client) -> Result<()> {
    let govde = c
        .find(fantoccini::Locator::Css("body"))
        .await
        .context("ESCAPE icin body bulunamadi")?;
    govde
        .send_keys("\u{E00C}")
        .await
        .context("ESCAPE gonderilemedi")?;
    Ok(())
}

/// Acma dugmesinin o anki metni (secili modelin ikinci tanigi).
async fn dugme_metni(c: &Client, dugme: &str) -> String {
    const METIN: &str = r#"
const d = document.querySelector(arguments[0]);
return d ? (d.innerText || d.textContent || '') : '';
"#;
    match c.execute(METIN, vec![serde_json::json!(dugme)]).await {
        Ok(v) => v.as_str().unwrap_or_default().to_string(),
        // Okunamayan tanik "yanlis" sayilmaz: karar isarete duser, hata mesaji
        // zaten menudeki isaretli ogeyi yazar.
        Err(_) => String::new(),
    }
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
            kanit: String::new(),
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

    /// İşaret kuralı: "zaten seçili" (atla) ile "doğrulandı" (tıklama sonrası)
    /// AYRI hükümlerdir; ikisi de bu dosyada TEK YERDE yaşar, eklenti yolu da
    /// buradan çağırır.
    ///
    /// 2026-10-24 canlı ölçümü iki eksenli menüyü gösterdi (bir satır
    /// `sinif:selected`, başka satır `torun:aria-checked=true`). Bu yüzden
    /// doğrulama liste genelinde "tam bir işaret" şartına BAĞLANMAZ; şart
    /// hedefin kendi kanıtıdır.
    #[test]
    fn zaten_secili_yalniz_guclu_ve_tek_isarette_atar() {
        let isaretli = |ad: &str, kanit: &str| OgeDurum {
            metin: ad.to_string(),
            tam: ad.to_string(),
            isaretli: true,
            kanit: kanit.to_string(),
        };
        let isaretsiz = |ad: &str| OgeDurum {
            metin: ad.to_string(),
            tam: ad.to_string(),
            isaretli: false,
            kanit: String::new(),
        };

        // Tek güçlü işaret, o da hedefte → tıklama atlanır.
        let tek = vec![
            isaretli("3.6 Flash", "sinif:selected"),
            isaretsiz("3.1 Pro"),
        ];
        assert!(zaten_secili(&tek, 0));
        assert!(!zaten_secili(&tek, 1));

        // İki eksen: mod ekseni güçlü + düşünme düzeyi zayıf. Sayım iki olduğu
        // için "zaten seçili" DENMEZ; tıklanır (radyoda zararsız) ve tıklama
        // sonrası hedefin kendi kanıtıyla doğrulanır.
        let iki_eksen = vec![
            isaretli("Otomatik", "sinif:selected"),
            isaretli("Yüksek", "torun:aria-checked=true"),
        ];
        assert!(!zaten_secili(&iki_eksen, 0), "iki işaret = atlama yok");
        assert!(!zaten_secili(&iki_eksen, 1), "zayıf kanıt atlamaya yetmez");
        assert!(
            secim_dogrulandi(&iki_eksen, 1),
            "tıklama sonrası hedef zayıf ve tek zayıf işaret onda → doğrulandı"
        );

        // Zayıf kanıt başka satırda da varsa hangisinin gerçek seçim olduğu
        // bilinmez: doğrulama reddedilir.
        let iki_zayif = vec![
            isaretli("Düşük", "torun:aria-checked=true"),
            isaretli("Yüksek", "torun:aria-checked=true"),
        ];
        assert!(!secim_dogrulandi(&iki_zayif, 1));

        // İki güçlü işaret: iki eksen de olabilir bozuk okuma da. Atlama yok;
        // doğrulama hedefin kendi güçlü kanıtına bakar (iki eksenli menü).
        let iki_guclu = vec![
            isaretli("Otomatik", "sinif:selected"),
            isaretli("Yüksek", "sinif:selected"),
        ];
        assert!(
            !zaten_secili(&iki_guclu, 1),
            "iki güçlü işaret = atlama yok"
        );
        assert!(
            secim_dogrulandi(&iki_guclu, 1),
            "hedefin kendi güçlü kanıtı"
        );

        // Hedef hiç işaretsizse doğrulanmaz (upsell / uygulanmayan tıklama).
        let isaretsiz_hedef = vec![isaretli("Otomatik", "sinif:selected"), isaretsiz("3.1 Pro")];
        assert!(!secim_dogrulandi(&isaretsiz_hedef, 1));

        // Boş kanıt (eski eklenti) güçlü sayılır; ama atlama için yine TEK
        // işaret şartı aranır — eski davranış birebir korunmaz.
        let eski = vec![isaretli("3.6 Flash", ""), isaretsiz("3.1 Pro")];
        assert!(zaten_secili(&eski, 0));
        let eski_cok = vec![isaretli("3.6 Flash", ""), isaretli("3.6 Düşünen", "")];
        assert!(
            !zaten_secili(&eski_cok, 1),
            "çok işaretli menüde atlama yok"
        );
        assert!(secim_dogrulandi(&eski_cok, 1));
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
