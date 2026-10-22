//! Model menusu: site arayuzunden GERCEK model secimi.
//!
//! Neden ayri dosya: `browser.rs` iskeleti (oturum, istem, yanit okuma) ile model
//! menusu ayri sorumluluklar. Menu kodu yalniz OLCULMUS secicilerle calisir ve
//! secimi DOGRULAMADAN devam etmez — sessiz yanlis model, en pahali hata turu.
//!
//! ESLESTIRME TEK YERDE: menu metni ile istenen etiket Rust tarafinda, ortak
//! `katla` sandigiyla karsilastirilir. Once JS icinde de katlama yapiliyordu; iki
//! katlama ayrisinca var olan secenek "menude yok" sayildi (olculdu 2026-10-22).
//! Simdi JS yalniz HAM metni ve isaret durumunu dondurur, karar Rust'ta verilir.

use super::*;

/// Model menusundeki tek secenek: kimligi Kervan verir, etiketi site yazar.
///
/// `kod` model kimliginde gecer (`kervan/gemini/yuksek`), `etiket` sitedeki
/// gorunen metindir. Ikisini ayirmak sart: site metni degisken ("Yüksek",
/// "YUKSEK"), kimlik ise istemciyle yapilan sozlesme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSecenegi {
    pub kod: String,
    pub etiket: String,
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
    /// Koda karsilik gelen site etiketi. Tanimsiz kod `None`.
    pub fn etiket(&self, kod: &str) -> Option<&str> {
        self.modeller
            .iter()
            .find(|m| m.kod.eq_ignore_ascii_case(kod))
            .map(|m| m.etiket.as_str())
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

/// Menudeki bir ogenin ham metni ve secim isareti.
///
/// `metin` HAM gelir (buyuk harf, satir sonu, bosluk ne varsa); katlama ve
/// karsilastirma Rust tarafinda yapilir.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OgeDurum {
    metin: String,
    isaretli: bool,
}

/// Menudeki ogeleri sira ile dondurur (metin + isaret). Menu ACIK olmali.
const OGE_LISTESI: &str = r#"
const hamMetin = (e) => {
  const l = e.querySelector('.label');
  return ((l ? l.innerText : e.innerText) || '').replace(/\s+/g, ' ').trim();
};
const isaretli = (e) => e.classList.contains('selected')
  || !!e.querySelector('[aria-label="Seçildi"],[aria-label="Selected"],[aria-checked="true"]')
  || e.getAttribute('aria-checked') === 'true'
  || e.getAttribute('aria-selected') === 'true';
return [...document.querySelectorAll(arguments[0])].map(
  (e) => ({ metin: hamMetin(e), isaretli: isaretli(e) }));
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
pub(crate) async fn model_sec(
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

/// Hedef etiketi iceren ogenin sirasi. Yoksa `None`.
///
/// Ilk eslesen alinir; menu metinleri arasinda ic ice gecen ad varsa (ornegin
/// "Flash" ve "Flash-Lite") yapilandirmadaki etiketin tam yazimi secilmelidir.
fn hedef_indeks(ogeler: &[OgeDurum], etiket: &str) -> Option<usize> {
    ogeler
        .iter()
        .position(|o| metin_iceriyor_mu(&o.metin, etiket))
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
pub(crate) fn metin_iceriyor_mu(metin: &str, aranan: &str) -> bool {
    let norm = |s: &str| {
        katla::katla(s)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    norm(metin).contains(&norm(aranan))
}
