//! OLCUM araclari: CLI'dan site arayuzunu incelemek icin.
//!
//! Bunlar istek yolunun parcasi DEGIL. `--tikla`, `--tikla-metin`, `--metin`,
//! `--js`, `--incele` bayraklari buradan gecer. Uretim yolu yalniz `ask` ve
//! `model_sec`'tir; buradaki her sey "sitede ne var" sorusunu sormak icin.
//!
//! Neden ayri dosya: `browser.rs` iskeleti 1500 satir sinirina yaklasti ve olcum
//! yardimcilari urun kodunun arasina karisinca "bu cagrilir mi?" sorusu her
//! seferinde yeniden doguyordu.

use super::*;

/// Olcum icin: seciciye uyan ILK ogeye tikla ve kisa sure bekle.
///
/// NEDEN VAR: model secimi gibi ozellikler bir MENU acmayi gerektiriyor. Menunun
/// icindekiler ancak tiklandiktan sonra DOM'da olusuyor; tiklamadan `--incele`
/// bos liste donuyor ve "secici yok" gibi yanlis bir sonuca vardiriyordu.
pub async fn tikla_ilk(c: &Client, secici: &str) -> Result<()> {
    let ogeler = c.find_all(Locator::Css(secici)).await?;
    let ilk = ogeler
        .into_iter()
        .next()
        .with_context(|| format!("'{secici}' hicbir oge eslemedi"))?;
    ilk.click()
        .await
        .with_context(|| format!("'{secici}' ogesine tiklanamadi"))?;
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    Ok(())
}

/// Olcum icin: fareyi seciciye uyan ILK ogenin uzerine gotur ve bekle.
///
/// NEDEN VAR: bazi arayuzler alt menuyu YALNIZ fare girdisiyle aciyor; tiklama
/// metodu alt menuyu acmak yerine ana menuyu kapatiyor (olculdu 2026-10-22,
/// perplexity: `[role=menuitem][aria-haspopup=menu]` tiklaninca menu kapaniyor).
/// Tiklama degil gercek isaretci hareketi gonderilir.
pub async fn uzerine_gel(c: &Client, secici: &str) -> Result<()> {
    let ogeler = c.find_all(Locator::Css(secici)).await?;
    let ilk = ogeler
        .into_iter()
        .next()
        .with_context(|| format!("'{secici}' hicbir oge eslemedi"))?;
    fareyi_gotur(c, ilk).await
}

/// Seciciye uyan ogeler arasindan metni `metin` iceren ILK ogenin uzerine gel.
///
/// Perplexity'de alt menuyu acmak icin hedef ogenin KENDISINE hover gerekiyor;
/// ilk ogeye hover yapmak yanlis alt menuyu acar.
pub async fn metinle_uzerine_gel(c: &Client, secici: &str, metin: &str) -> Result<()> {
    let ogeler = c.find_all(Locator::Css(secici)).await?;
    for oge in ogeler {
        let t = oge.text().await.unwrap_or_default();
        if katla::katla(&t).contains(&katla::katla(metin)) {
            return fareyi_gotur(c, oge).await;
        }
    }
    bail!("'{secici}' icinde '{metin}' metnini tasiyan oge yok");
}

/// Fareyi bir ogenin merkezine goturup kisa sure bekle.
async fn fareyi_gotur(c: &Client, oge: Element) -> Result<()> {
    use fantoccini::actions::{InputSource, MouseActions, PointerAction};
    let fare = MouseActions::new("kervan-fare".to_string())
        .then(PointerAction::MoveToElement {
            element: oge,
            duration: Some(std::time::Duration::from_millis(300)),
            x: 0.0,
            y: 0.0,
        })
        .pause(std::time::Duration::from_millis(400));
    c.perform_actions(fare)
        .await
        .context("fare hareketi uygulanamadi")?;
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    Ok(())
}

/// Bir secicinin ILK ogesinin metnini oku (model dugmesindeki secili model gibi).
pub async fn oge_metni(c: &Client, secici: &str) -> Result<String> {
    let ogeler = c.find_all(Locator::Css(secici)).await?;
    let ilk = ogeler
        .into_iter()
        .next()
        .with_context(|| format!("'{secici}' hicbir oge eslemedi"))?;
    Ok(ilk.text().await.unwrap_or_default())
}

/// Seciciye uyan ogeler arasindan metni `metin` iceren ILK ogeye tikla.
///
/// NEDEN METIN: model menusundeki ogelerin kimligi yok; kararli olan tek sey
/// gorunen ad ("3.1 Pro", "Gemini 2.5 Flash"). Bu yuzden eslesme metin uzerinden.
/// Metin bulunamazsa hata — "tikladim sayalim" yok.
pub async fn metinle_tikla(c: &Client, secici: &str, metin: &str) -> Result<()> {
    let ogeler = c.find_all(Locator::Css(secici)).await?;
    for oge in ogeler {
        let t = oge.text().await.unwrap_or_default();
        if katla::katla(&t).contains(&katla::katla(metin)) {
            oge.click()
                .await
                .with_context(|| format!("'{metin}' ogesine tiklanamadi"))?;
            // Secim sunucu tarafinda isleniyor ve dugme etiketi ancak sonra
            // guncelleniyor; beklemeden okumak "secim yapilmadi" gibi yanlis
            // sonuc veriyordu (olculdu: 3.1 Pro tiklandi, dugme hala Otomatik).
            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
            return Ok(());
        }
    }
    bail!("'{secici}' icinde '{metin}' metnini tasiyan oge yok");
}

/// Olcum icin: sayfada JS calistir, sonucu JSON olarak dondur.
///
/// NEDEN VAR: secici metinlerinin yaninda `aria-checked`, `.selected` gibi
/// DURUM bilgisi de gerekiyor ("menu acildi mi, hangi oge secili"). WebDriver'in
/// oge API'si bu alanlari tek tek sorgulatir; tek turda butun tabloyu almak
/// teshisi saatlerden saniyelere indiriyor.
///
/// Betik ACIK `return` ile baslamali: W3C `execute` govdeyi fonksiyon olarak
/// sarar, aksi halde sonuc `null` doner.
pub async fn js_calistir(c: &Client, kod: &str) -> Result<String> {
    let v = c.execute(kod, vec![]).await.context("JS calistirilamadi")?;
    Ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()))
}

/// Bir secicinin esledigi ogeleri TEK TEK doker: sinif, data-testid ve metin basi.
///
/// Sayim yetmedigi yerde gerekiyor. Ornegin bir "tur satiri" secicisi hem
/// kullanici hem model turunu esliyorsa, sonuncusunu okumak istemi cevap sanmak
/// demek olur — hangi ogenin kime ait oldugunu ancak boyle gorursun.
pub async fn oge_dokumu(c: &Client, secici: &str) -> Result<String> {
    let ogeler = c.find_all(Locator::Css(secici)).await.unwrap_or_default();
    let mut s = format!("inceleme '{secici}': {} oge\n", ogeler.len());
    for (i, e) in ogeler.iter().enumerate() {
        let al = |v: Option<String>, n: usize| -> String {
            v.unwrap_or_default().chars().take(n).collect()
        };
        let tid = al(e.attr("data-testid").await.ok().flatten(), 40);
        let sinif = al(e.attr("class").await.ok().flatten(), 70);
        // aria-label ve href, hesap kimligini tasiyan yerler: Google'in hesap
        // dugmesi eposta adresini aria-label icinde veriyor.
        let aria = al(e.attr("aria-label").await.ok().flatten(), 90);
        let href = al(e.attr("href").await.ok().flatten(), 70);
        let metin = al(Some(e.text().await.unwrap_or_default()), 60);
        s.push_str(&format!(
            "  [{i}] testid={tid:?} aria={aria:?}\n      href={href:?}\n      class={sinif:?}\n      text={metin:?}\n"
        ));
    }
    Ok(s)
}
