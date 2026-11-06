//! Sohbet yolu: OpenAI uyumlu istek/yanit bicimleri ve saglayici deneme donguleri.
//!
//! NEDEN AYRI DOSYA (2026-11-06): sandik siniri dosya basina 1500 fiziksel satirdir;
//! `server.rs` hesap duzeyi yonlendirme eklenince 2094'e cikmisti. Buradaki her sey
//! yalniz `/v1/chat/completions` yoluna hizmet eder; uclar ve durum `server.rs`te kalir.
//! `hata` / `simdi` yardimcilari baska modullerden de cagrildigi icin orada durur.

use std::sync::Arc;
use std::time::Duration;

use crate::browser::ask;
use crate::kota::{self, Sonuc};
use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::rest::rest_denemesi;
use crate::server::{
    AppState, Message, TOHUM_AZAMI, hata, konusma_izi, kota_anlik_goruntuleri, model_kimligi,
    simdi, son_kullanici_mesaji, tohum_istemi, yonlendirme_hedefleri,
};

// ─── OpenAI bicimleri ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub model: String,
    pub messages: Vec<Message>,
    /// Çoğu ajan istemcisi VARSAYILAN OLARAK akis ister. Bunu desteklemeyen
    /// bir uc, istemci tarafinda "empty stream" olarak gorunur ve sessizce yedege duser.
    #[serde(default)]
    pub stream: bool,
    /// OpenAI araç tanımları. Tarayıcı modeli bunları bilmez: istemine biçim
    /// yönergesi eklenir, yanıttaki ```json bloğu `tool_calls`'a çevrilir.
    #[serde(default)]
    pub tools: Vec<serde_json::Value>,
    /// Kervan'a özgü alanlar (site aç/kapa özellikleri). OpenAI istemcileri bunu
    /// bilmez ve göndermez; panel gönderir. Sessiz yok sayılmaz: tanımsız anahtar
    /// ya da anahtar desteklemeyen yol AÇIK hataya düşer.
    #[serde(default)]
    pub kervan: Option<KervanIstegi>,
}

/// `kervan` alanı: sitedeki aç/kapa özelliklerinin istenen durumu.
///
/// Örnek: `{"anahtarlar": {"derin": true, "arama": false}}`.
#[derive(Debug, Default, Deserialize)]
pub struct KervanIstegi {
    #[serde(default)]
    pub anahtarlar: std::collections::BTreeMap<String, bool>,
    /// Panelden SEÇİLEN hesap: `provider:profile` (ör. `gemini:ercaner1988@gmail.com`).
    ///
    /// NEDEN: sağlayıcıda yedi hesap bağlıyken sunucu en-az-kullanılanı seçiyordu;
    /// kullanıcı panelde A'yı seçip B'den yanıt alabiliyordu. Verilirse yalnız O
    /// hesap kullanılır; bağlı değilse açık hata (sessiz hesap değişimi yasak).
    #[serde(default)]
    pub hesap: Option<String>,
}

#[derive(Serialize)]
struct ChatResponse {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Serialize)]
struct Choice {
    index: u32,
    message: Message,
    finish_reason: &'static str,
}

/// Tarayici arayuzu token saymiyor. Uydurma sayi vermek, kota hesabini bozar ve
/// istemciyi yaniltir — sifir doner, bilinmedigi acikca belli olur.
/// Asama 3'teki kota ogrenici istek SAYISINDAN ogrenecek, token'dan degil.
#[derive(Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

pub(crate) async fn chat(
    State(st): State<Arc<AppState>>,
    Json(raw_val): Json<serde_json::Value>,
) -> Response {
    let req: ChatRequest = match serde_json::from_value(raw_val.clone()) {
        Ok(r) => r,
        Err(e) => {
            return hata(
                StatusCode::BAD_REQUEST,
                &format!("Geçersiz ChatRequest gövdesi: {}", e),
            );
        }
    };

    let model_adi = req.model.trim();

    // 1. REST yukarı akış kancası: model [rest.*] yapılandırmasına çözülürse yukari'ya devret
    if let Some((hedef, alt_model)) = st.rest_hedefi_coz(model_adi) {
        return match rest_denemesi(&st, &hedef, &raw_val, alt_model.as_deref(), req.stream).await {
            Ok(yanit) => yanit,
            Err((yanit, _)) => yanit,
        };
    }

    // Hesap kimliğe gömülü gelir: `kervan/gemini/3-1-pro@eposta`. OpenCode
    // istekte yalnız `model` alanını gönderdiği için hesap ancak böyle taşınır.
    let (model_temel, model_hesap) = crate::olcum::hesap_eki(model_adi);
    let model_temel = model_temel.as_str();

    let Some(istem) = son_kullanici_mesaji(&req.messages) else {
        return hata(StatusCode::BAD_REQUEST, "messages icinde user rolu yok");
    };

    let otomatik = model_adi.is_empty() || model_adi == "auto";

    // Model kimliginden saglayiciyi sec. 'auto' verilirse SmartRouter adayları sıralar.
    // Model KODU ayrica cozulur: `kervan/gemini/yuksek`. Bos ise site varsayilani.
    let mut model_kodu: Option<String> = None;
    // İsteğin çözüldüğü menü: seçim aşamasına AYNEN taşınır (bkz. SecimIstegi::menu).
    let mut secim_menusu: Option<kervan_secim::ModelMenusu> = None;

    // Ac/kapa ozellikleri: "auto" yolunda hangi saglayicinin secilecegi onceden
    // bilinmedigi icin uygulanamaz — sessizce yok saymak yerine acik hata.
    let istenen_anahtarlar: Vec<(String, bool)> = req
        .kervan
        .as_ref()
        .map(|k| k.anahtarlar.iter().map(|(a, b)| (a.clone(), *b)).collect())
        .unwrap_or_default();
    if otomatik && !istenen_anahtarlar.is_empty() {
        return hata(
            StatusCode::BAD_REQUEST,
            "ac/kapa anahtarlari icin saglayiciyi acikca belirt ('auto' ile uygulanamaz)",
        );
    }
    // Cozulmus istek: (tanim, istenen durum). Cozulmemis kod istek GONDERILMEDEN hata.
    let mut anahtar_istegi: Vec<(crate::browser::Anahtar, bool)> = Vec::new();
    let (adaylar, yedeklenebilir) = if otomatik {
        // 'auto' adayları ÜÇ kaynaktan gelir: bağlı eklenti sekmeleri, havuz
        // hesapları ve REST hedefleri. Böylece yerel (anahtarlı) ve web
        // (oturumlu) sağlayıcılar tek yönlendirme kararında yarışır.
        //
        // Karar HESAP düzeyinde verilir: aynı sağlayıcının iki hesabı ayrı
        // adaydır, böylece kota/least-used stratejileri hesap seçebilir.
        let hedefler = yonlendirme_hedefleri(&st).await;
        if hedefler.is_empty() {
            (
                vec![crate::router::Hedef::saglayici(&st.varsayilan_saglayici)],
                false,
            )
        } else {
            // Kota sinyali sıralamadan ÖNCE beslenir: `headroom`/`reset-aware`/
            // `reset-window`/`quota-weighted` ancak bu harita doluysa ayırt edici.
            st.router
                .kota_sinyalleri_besle(kota_anlik_goruntuleri(&st).await)
                .await;
            let sirali = st.router.hedef_sirasi(&hedefler).await;
            (sirali, true)
        }
    } else {
        // Hesap eki yukarıda kimlikten çözüldü; burada yalnız sağlayıcı+kod
        // ayrıştırılır. Ek yoksa davranış eskisi gibi (havuz/site varsayılanı).
        let (saglayici, kod) = match crate::saglayicilar::model_coz(model_temel, &st.saglayicilar) {
            Some(v) => v,
            None => {
                let sade = model_temel.strip_prefix("kervan/").unwrap_or(model_temel);
                if st.eklenti_hub.sekme_var_mi(sade).await {
                    (sade.to_string(), None)
                } else {
                    let mut mevcut: Vec<String> = st
                        .pool
                        .slots
                        .iter()
                        .map(|s| model_kimligi(&s.provider.ad))
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    for ext_s in st.eklenti_hub.bagli_sekmeler().await {
                        mevcut.push(model_kimligi(&ext_s));
                    }
                    mevcut.push("auto".into());
                    mevcut.sort();
                    mevcut.dedup();
                    return hata(
                        StatusCode::BAD_REQUEST,
                        &format!(
                            "'{}' hangi saglayiciya ait bilinmiyor. Kullanilabilir: {}",
                            req.model,
                            mevcut.join(", ")
                        ),
                    );
                }
            }
        };
        // Ölçüm varsa O hesabın menüsü otoritedir (kilit kararı tek yerde:
        // `coz_istek`). Ölçüm yoksa sağlayıcı geneli yapılandırma kullanılır.
        let yapilandirma = st.saglayicilar.get(&saglayici);
        let olculen_menu = match (model_hesap.as_deref(), yapilandirma) {
            (Some(profil), Some(y)) => y.model.as_ref().and_then(|m| {
                crate::olcum::menu_olarak(
                    &crate::olcum::yukle(),
                    &saglayici,
                    profil,
                    m.dugme.clone(),
                    m.oge.clone(),
                )
            }),
            _ => None,
        };
        let menu = olculen_menu
            .as_ref()
            .or_else(|| yapilandirma.and_then(|s| s.model.as_ref()));
        let tanimli_anahtarlar = yapilandirma
            .map(|s| s.anahtarlar.as_slice())
            .unwrap_or_default();
        let cozulen = match kervan_secim::coz_istek(
            &saglayici,
            menu,
            kod.as_deref(),
            tanimli_anahtarlar,
            &istenen_anahtarlar,
        ) {
            Ok(v) => v,
            Err(e) => {
                let durum = StatusCode::from_u16(e.durum).unwrap_or(StatusCode::BAD_REQUEST);
                return hata(durum, &e.mesaj);
            }
        };
        model_kodu = cozulen.model_kodu;
        anahtar_istegi = cozulen.anahtarlar;
        secim_menusu = menu.cloned();
        (vec![crate::router::Hedef::saglayici(&saglayici)], false)
    };

    // Panelden hesap sabitleme (`kervan.hesap`): "saglayici:profil" ya da yalniz
    // "profil". Sabitlenen hesap bagli degilse SESSIZCE baska hesaba dusmek
    // yasak — istek acik hatayla doner.
    //
    // İki kaynak çelişirse (kimlikte `@A`, `kervan.hesap`=`B`) sessiz seçim
    // yapılmaz: hangisinin kastedildiği istemciden gelmeli, sunucu tahmin etmez.
    let panel_hesabi: Option<String> = match req.kervan.as_ref().and_then(|k| k.hesap.as_deref()) {
        None => None,
        Some(ham) if ham.trim().is_empty() => None,
        Some(ham) => {
            if otomatik {
                return hata(
                    StatusCode::BAD_REQUEST,
                    "'auto' ile hesap sabitlenemez; saglayiciyi acikca belirt",
                );
            }
            let ham = ham.trim();
            let (onek, profil) = match ham.split_once(':') {
                Some((s, p)) => (Some(s.trim()), p.trim()),
                None => (None, ham),
            };
            let saglayici_adi = adaylar
                .first()
                .map(|h| h.provider.as_str())
                .unwrap_or_default();
            if let Some(onek) = onek
                && onek != saglayici_adi
            {
                return hata(
                    StatusCode::BAD_REQUEST,
                    &format!("'{ham}' secilen saglayiciya ('{saglayici_adi}') ait degil"),
                );
            }
            if profil.is_empty() {
                return hata(StatusCode::BAD_REQUEST, "kervan.hesap icinde profil bos");
            }
            if !st.eklenti_hub.sekme_var_mi(saglayici_adi).await {
                return hata(
                    StatusCode::BAD_REQUEST,
                    &format!("'{saglayici_adi}' icin bagli eklenti hesabi yok; hesap sabitlenemez"),
                );
            }
            Some(profil.to_string())
        }
    };
    if let (Some(a), Some(b)) = (panel_hesabi.as_deref(), model_hesap.as_deref())
        && a != b
    {
        return hata(
            StatusCode::BAD_REQUEST,
            &format!(
                "hesap celiskisi: model kimligi '{b}' diyor, kervan.hesap '{a}' diyor. \
                 Tek kaynak birak: ya model kimligindeki eki kullan ya kervan.hesap'i kaldir"
            ),
        );
    }
    let hedef_profil: Option<String> = panel_hesabi.or_else(|| model_hesap.clone());

    // Hesap GERÇEKTEN bağlı mı? Bağlı değilse sessizce başka hesaba düşmek
    // yasak: istek, hangi sağlayıcıda hangi hesapların bağlı olduğunu söyleyen
    // açık 400 ile döner. (2026-10-25: bu denetim yoktu, `kervan/gemini@olmayan`
    // eklentiye gidip 500 üretiyordu — kullanıcı sebebi göremiyordu.)
    if let Some(profil) = hedef_profil.as_deref() {
        let saglayici_adi = adaylar
            .first()
            .map(|h| h.provider.as_str())
            .unwrap_or_default();
        let bagli = st.eklenti_hub.bagli_hesaplar().await;
        let aranan = format!("{saglayici_adi}:{profil}");
        if !bagli.iter().any(|k| k == &aranan) {
            return hata(
                StatusCode::BAD_REQUEST,
                &format!(
                    "'{profil}' hesabi '{saglayici_adi}' icin bagli degil. Bagli hesaplar: {}",
                    bagli
                        .iter()
                        .filter(|k| k.starts_with(&format!("{saglayici_adi}:")))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        }
    }

    // TOKEN SIKIŞTIRMA (RTK + Caveman): Gelen istemi göndermeden önce gereksiz boşluk ve kalıplardan arındır
    let sikistirma = st.compressor.rtk_sikistir(&istem);
    let mut gonderilecek_istem = if sikistirma.tasarruf_yuzdesi > 3.0 {
        println!(
            "sıkıştırma   : %{:.1} token tasarrufu sağlandı ({} -> {} karakter)",
            sikistirma.tasarruf_yuzdesi, sikistirma.orijinal_uzunluk, sikistirma.yeni_uzunluk
        );
        sikistirma.metin.clone()
    } else {
        istem.clone()
    };
    // Yönerge sıkıştırmadan SONRA eklenir: biçim örneği bozulmamalı.
    let yonerge = arac_yonergesi(&req.tools);
    gonderilecek_istem.push_str(&yonerge);

    // Araç isteğinde gerçek işlev çağrısını yalnız REST hedefleri yapabilir;
    // tarayıcı yolu araçları yönergeyle taklit eder. REST adayı varsa diğerleri
    // elenir. Yoksa istek tamamen düşmesin diye tarayıcı yoluyla denenir ve bu
    // açıkça uyarılır.
    let mut adaylar = adaylar;
    if otomatik && !req.tools.is_empty() {
        if adaylar
            .iter()
            .any(|h| st.rest_hedefleri.contains_key(&h.provider))
        {
            adaylar.retain(|h| st.rest_hedefleri.contains_key(&h.provider));
        } else {
            println!(
                "akilli-rota   : uyari: araç isteği var ama REST adayı yok; tarayıcı yolu yalnız yönergeyle denenecek"
            );
        }
    }

    // Yedek deneme: yalnız 'auto' isteğinde ve akış/yanıt başlamadan önceki
    // hatada sıradaki adaya geçilir. `azami_deneme` kaç aday deneneceğini sınırlar.
    let azami = if yedeklenebilir && st.yedek_deneme {
        st.azami_deneme.max(1) as usize
    } else {
        1
    };

    // Deneme listesi SAĞLAYICI ÇEŞİTLİLİĞİ ile kurulur. Neden: aday listesi
    // hesap düzeyinde ve çok hesaplı bir sağlayıcının TÜM hesapları strateji
    // sırasında art arda gelebiliyor; `azami_deneme = 3` ile ilk üç deneme tek
    // sağlayıcıya gidip diğer sağlayıcıları dışarıda bırakıyordu. İlk tur her
    // sağlayıcıdan bir aday alır, ikinci tur kalanları strateji sırasıyla doldurur.
    //
    // Bilinçli sapma: tek sağlayıcı + çok hesap kurulumunda ikinci tur devreye
    // girer, yani hesap yedeği kaybolmaz. Stratejinin 2. tercihi, farklı bir
    // sağlayıcı varsa 3. sıraya düşebilir — "üç deneme aynı sağlayıcıya gitmesin"
    // kuralı bunu bilerek seçer (bkz. kervan.toml.example).
    let mut denenecek: Vec<&crate::router::Hedef> = Vec::new();
    if azami < adaylar.len() {
        let mut gorulen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for h in &adaylar {
            if denenecek.len() == azami {
                break;
            }
            if gorulen.insert(h.provider.as_str()) {
                denenecek.push(h);
            }
        }
        for h in &adaylar {
            if denenecek.len() == azami {
                break;
            }
            if !denenecek.iter().any(|d| d.execution_key == h.execution_key) {
                denenecek.push(h);
            }
        }
    } else {
        denenecek.extend(adaylar.iter());
    }

    let mut son_hata: Option<Response> = None;
    // Secimler tek yapida toplanir: deneme dongusu icinde ayni sekilde kullanilir.
    // `tercih` her adayda değiştiği için döngü içinde kopyalanır.
    let secim_temel = SecimIstegi {
        model_kodu: model_kodu.clone(),
        anahtarlar: anahtar_istegi.clone(),
        hesap: hedef_profil,
        menu: secim_menusu,
        tercih: None,
    };
    for hedef in denenecek.iter().copied() {
        let ad = hedef.provider.as_str();
        let secim = SecimIstegi {
            tercih: hedef.connection_id.clone(),
            ..secim_temel.clone()
        };
        if otomatik {
            println!(
                "akilli-rota   : 'auto' adayı deneniyor -> [{}{}]",
                ad,
                hedef
                    .connection_id
                    .as_deref()
                    .map(|h| format!("@{h}"))
                    .unwrap_or_default()
            );
        }

        let sonuc = if let Some(hedef) = st.rest_hedefleri.get(ad).cloned() {
            rest_denemesi(&st, &hedef, &raw_val, None, req.stream).await
        } else if st.eklenti_hub.sekme_var_mi(ad).await {
            eklenti_denemesi(&st, ad, &gonderilecek_istem, &req, &secim).await
        } else {
            havuz_denemesi(&st, ad, &req, &istem, &yonerge, &gonderilecek_istem, &secim).await
        };

        match sonuc {
            Ok(yanit) => return yanit,
            Err((yanit, yeniden)) => {
                son_hata = Some(yanit);
                if !yeniden {
                    break;
                }
            }
        }
    }

    son_hata.unwrap_or_else(|| {
        hata(
            StatusCode::SERVICE_UNAVAILABLE,
            "kullanilabilir saglayici bulunamadi",
        )
    })
}

/// Eklenti sekmesi üzerinden tek deneme. Yanıt kotaysa bu model yanıtı değildir:
/// 429 döner, sağlayıcı sağlıksız işaretlenir ve yedek deneme sıradaki adaya geçer.
#[allow(
    clippy::result_large_err,
    reason = "hata yaniti istemciye aynen doner; Boxing cagri yerlerini karmasiklastirirdi"
)]
async fn eklenti_denemesi(
    st: &Arc<AppState>,
    saglayici: &str,
    gonderilecek_istem: &str,
    req: &ChatRequest,
    secim: &SecimIstegi,
) -> Result<Response, (Response, bool)> {
    println!(
        "[{}] istek Kervan Eklentisi uzerinden iletiliyor ({} karakter)",
        saglayici,
        gonderilecek_istem.len()
    );
    let t = std::time::Instant::now();
    // Model menusu cozulurken dogrulanmisti; burada yalniz etiket eslestirmesi
    // icin gerekli. Secim turlari ISTEMDEN ONCE, ayni sekmede uygulanir.
    // Ölçüm varsa ÖLÇÜLEN menü, yoksa yapılandırma. Karar aşamasıyla AYNI
    // kaynak kullanılmalı; aksi hâlde ölçülen kod ("otomatik") burada tanımsız
    // görünüp 500 üretiyordu (2026-10-25).
    let menu = secim
        .menu
        .clone()
        .or_else(|| st.saglayicilar.get(saglayici).and_then(|s| s.model.clone()));
    let es = crate::eklenti::EklentiSecimi {
        menu,
        model_kodu: secim.model_kodu.clone(),
        anahtarlar: secim.anahtarlar.clone(),
    };
    let sonuc = match st
        .eklenti_hub
        .sor_secimli_hesap(
            saglayici,
            secim.hesap.as_deref(),
            gonderilecek_istem,
            Vec::new(),
            &es,
            st.timeout,
        )
        .await
    {
        Err(e) => match e.downcast::<crate::eklenti::YarimYanit>() {
            Ok(crate::eklenti::YarimYanit(yarim)) => kurtar(st, saglayici, yarim).await,
            Err(e) => Err(e),
        },
        tamam => tamam,
    };
    match sonuc {
        Ok(yanit) => {
            if kota::kota_sinyali(&yanit, &st.kota_ayar.isaretler) {
                st.router.record_failure(saglayici).await;
                println!("[{}] eklenti KOTA bildirdi", saglayici);
                return Err((
                    hata(
                        StatusCode::TOO_MANY_REQUESTS,
                        "saglayici kotasini bildirdi (eklenti)",
                    ),
                    true,
                ));
            }
            let latency = t.elapsed().as_millis() as u64;
            st.router.record_success(saglayici, latency).await;
            println!(
                "[{}] eklenti yaniti alindi ({:.1} sn)",
                saglayici,
                t.elapsed().as_secs_f32()
            );
            Ok(yanit_olustur(
                st,
                req,
                yanit,
                yanit_model_adi(&req.model, saglayici),
            ))
        }
        Err(e) => {
            st.router.record_failure(saglayici).await;
            println!("[{}] eklenti hatasi: {e:#}", saglayici);
            Err((
                hata(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Eklenti hatası: {e:#}"),
                ),
                true,
            ))
        }
    }
}

/// Tek bir denemede site arayuzune uygulanacak secimler (model + ac/kapa).
///
/// NEDEN TEK YAPI: ikisi de ayni yerde cozulur, ayni `ask` cagrisina gider ve
/// ikisi de "uygulanamazsa acik hata" kuralina tabidir. Ayri parametreler
/// `havuz_denemesi` imzasini okunamaz hale getiriyordu.
#[derive(Debug, Default, Clone)]
struct SecimIstegi {
    model_kodu: Option<String>,
    anahtarlar: Vec<(crate::browser::Anahtar, bool)>,
    /// Panelden sabitlenen eklenti profili (hesap). `None` = en-az-kullanılan.
    hesap: Option<String>,
    /// İsteğin çözüldüğü ANDAKİ menü (ölçüm varsa ÖLÇÜLEN, yoksa yapılandırma).
    ///
    /// NEDEN TAŞINIYOR: seçim aşaması menüyü yeniden `kervan.toml`'dan okuduğu
    /// için ölçülen menüde olup yapılandırmada olmayan kodlar ("otomatik")
    /// burada "tanımsız model kodu" hatasına düşüyordu (2026-10-25 ölçüldü:
    /// `kervan/gemini/otomatik@hesap` → 500). Karar ile uygulama AYNI menüyü
    /// görmek zorunda.
    menu: Option<kervan_secim::ModelMenusu>,
    /// Yönlendiricinin seçtiği hesap (YUMUŞAK tercih).
    ///
    /// `hesap` ile karıştırılmamalı: `hesap` insanın sabitlediği profildir ve
    /// bağlı değilse istek 400 döner. Buradaki `tercih` yalnız sıralama
    /// sonucudur; hesap hazır değilse sessizce en-az-kullanılana düşülür —
    /// kota dolmuş bir hesapta beklemek isteği kilitlerdi.
    tercih: Option<String>,
}

/// WebDriver havuzundan tek deneme. Hesap kiralanamazsa ya da model yanıtı
/// gelmezse hata yedeklenebilir sayılır (akış henüz başlamadı).
#[allow(
    clippy::result_large_err,
    reason = "hata yaniti istemciye aynen doner; Boxing cagri yerlerini karmasiklastirirdi"
)]
async fn havuz_denemesi(
    st: &Arc<AppState>,
    saglayici: &str,
    req: &ChatRequest,
    istem: &str,
    yonerge: &str,
    gonderilecek_istem: &str,
    secim: &SecimIstegi,
) -> Result<Response, (Response, bool)> {
    // Model kodu cozulurken dogrulanmisti; burada yalniz eslestirme yapilir.
    // Yine de savunma amacli: tanimsiz kod istek gonderilmeden acik hataya duser.
    let model_istegi: Option<(crate::browser::ModelMenusu, String)> =
        match secim.model_kodu.as_deref() {
            None => None,
            Some(kod) => {
                let menu = st.saglayicilar.get(saglayici).and_then(|s| s.model.clone());
                let etiket = menu
                    .as_ref()
                    .and_then(|m| m.etiket(kod))
                    .map(str::to_string);
                match (menu, etiket) {
                    (Some(m), Some(e)) => Some((m, e)),
                    _ => {
                        return Err((
                            hata(
                                StatusCode::BAD_REQUEST,
                                &format!("'{saglayici}' icin tanimsiz model kodu '{kod}'"),
                            ),
                            false,
                        ));
                    }
                }
            }
        };
    // Havuzdan hesap kirala: konusma kendi hesabina sabitlenir, hiz butcesi burada
    // uygulanir, gerekirse beklenir. `secim.tercih` yönlendiricinin seçtiği
    // hesaptır; hazır değilse en-az-kullanılan yoluna düşülür (kilit değil).
    let mut lease = match st
        .pool
        .acquire_for_hesap(
            konusma_izi(&req.messages),
            saglayici,
            secim.tercih.as_deref(),
        )
        .await
    {
        Ok(l) => l,
        Err(e) => {
            return Err((
                hata(StatusCode::SERVICE_UNAVAILABLE, &format!("{e:#}")),
                true,
            ));
        }
    };
    // Dizi bossa gecmisi TOHUMLA. Devam eden dizide tohumlamak gecmisi ikinci kez
    // yazmak olurdu; bos dizide tohumlamamak ise modele gecmissiz cevap verdirir.
    let gonderilecek = if lease.yeni_dizi {
        tohum_istemi(&req.messages, TOHUM_AZAMI) + yonerge
    } else {
        gonderilecek_istem.to_string()
    };
    println!(
        "[{}/{}] istek ({} karakter{})",
        saglayici,
        lease.name,
        gonderilecek.len(),
        if lease.yeni_dizi && gonderilecek.len() > istem.len() {
            ", yeni dizi — gecmis tohumlandi"
        } else {
            ""
        }
    );
    let secililer = lease.provider.selectors.clone();
    let t_wd = std::time::Instant::now();
    let model_cagrisi = model_istegi.as_ref().map(|(m, e)| (m, e.as_str()));
    let mut sonuc = ask(
        lease.client(),
        &secililer,
        &gonderilecek,
        st.timeout,
        model_cagrisi,
        &secim.anahtarlar,
    )
    .await;

    // Saglayici "limite ulastin" dediyse bu bir YANIT DEGIL. Oldugu gibi dondurmek
    // istemciye modelin cevabi diye limit mesajini vermek olurdu; ustelik kota
    // ogrenici de olayi hic gormezdi.
    let kota_carpmasi = match &sonuc {
        Ok(m) if kota::kota_sinyali(m, &st.kota_ayar.isaretler) => Some(m.clone()),
        _ => None,
    };
    if let Some(m) = &kota_carpmasi {
        sonuc = Err(anyhow::anyhow!("saglayici kota bildirdi: {m}"));
    }
    if let Err(e) = &sonuc {
        println!("[{}] HATA: {e:#}", lease.name);
        // INSAN KAPISI: dogrulama katmani cikmissa pencere one getirildi ve insan
        // cozecek. Oturumu insanin eline birakiyoruz ki bosta supurucu 15 dakika
        // sonra pencereyi kapatmasin — cozum yarida kalirdi.
        if format!("{e:#}").contains(crate::browser::INSAN_KAPISI) {
            lease.insan_eline_birak();
            println!(
                "[{}] oturum insanin elinde: dogrulamayi coz, sonra istegi tekrarla.",
                lease.name
            );
        }
    }

    // Sonucu havuza bildir: basari sayaci sifirlar, hata ustel geri cekilme uygular
    // ve dogrulama duvariysa hesabi insan kapisinda duraklatir.
    match (&kota_carpmasi, &sonuc) {
        (Some(metin), _) => {
            st.router
                .record_failure_hesap(&lease.provider.ad, Some(&lease.name))
                .await;
            st.defter
                .yaz(
                    &lease.name,
                    &lease.provider.ad,
                    Sonuc::Kota,
                    Some(kirp(metin)),
                )
                .await;
            // Pencere ogrenildiyse onu, ogrenilmediyse muhafazakar varsayilani bekle.
            let ozet = st.defter.ozet(&lease.name, &lease.provider.ad).await;
            // Ogrenilmis pencere varsa onu bekle; yoksa muhafazakar varsayilan.
            // `max(1)`: ogrenilen pencere sifir dakika cikarsa beklemeden yeniden
            // denemek limite carpip durmak olurdu.
            let dk = ozet
                .tahmin
                .pencere_dk
                .map(|d| d.max(1))
                .unwrap_or(st.kota_ayar.bilinmeyen_pencere_dk);
            println!("[{}] KOTA doldu, {dk} dk bekletiliyor", lease.name);
            st.pool
                .kota_doldu(lease.index, Duration::from_secs(dk * 60))
                .await;
        }
        (None, Ok(_)) => {
            let latency = t_wd.elapsed().as_millis() as u64;
            st.router
                .record_success_hesap(&lease.provider.ad, Some(&lease.name), latency)
                .await;
            st.defter
                .yaz(&lease.name, &lease.provider.ad, Sonuc::Basari, None)
                .await;
            st.pool.basarili(lease.index).await;
        }
        (None, Err(e)) => {
            st.router
                .record_failure_hesap(&lease.provider.ad, Some(&lease.name))
                .await;
            st.defter
                .yaz(&lease.name, &lease.provider.ad, Sonuc::Hata, None)
                .await;
            st.pool.basarisiz(lease.index, &format!("{e:#}")).await;
        }
    }

    let model_adi = if req.model.is_empty() {
        model_kimligi(saglayici)
    } else {
        req.model.clone()
    };

    match sonuc {
        Ok(metin) => Ok(yanit_olustur(st, req, metin, model_adi)),
        // Ham neden zinciri ({e:#}) istemciye gider. Hatayi maskelemek, OmniRoute'un
        // gemini-web'de yaptigi ve teshisi imkansizlastiran seydi.
        Err(e) => Err((hata(StatusCode::BAD_GATEWAY, &format!("{e:#}")), true)),
    }
}

/// Eklenti zaman aşımında kesilen yanıtı bir kez devam istemiyle sürdürür ve diker.
/// Devam da gelmezse yarım metni yanıt diye vermek yerine hata döner.
async fn kurtar(st: &AppState, saglayici: &str, yarim: String) -> Result<String> {
    use kervan_akis::stream_recovery::DikisBirlestirici as Dikis;
    println!(
        "[{saglayici}] yanit yarida kesildi ({} karakter), devam isteniyor",
        yarim.len()
    );
    let devam = st
        .eklenti_hub
        .sor(saglayici, &Dikis::devam_istemi_olustur(&yarim), st.timeout)
        .await
        .map_err(|e| anyhow::anyhow!("yanit yarida kesildi, devami da alinamadi: {e:#}"))?;
    st.sayaclar
        .kurtarilan_yanit
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(Dikis::dikis_ekle(&yarim, &devam))
}

/// Yanıtta gösterilecek model adı: istemcinin İSTEDİĞİ kimlik.
///
/// NEDEN: `kervan/gemini/otomatik@hesap` istendiğinde yanıt `kervan/gemini`
/// diyordu; istemci hangi hesabın/modeline cevap verdiğini göremiyordu
/// (2026-10-25 ölçüldü). Kimlik istemciden geldiyse aynen yankılanır; istemci
/// model belirtmediyse (`auto`/boş) sağlayıcı kimliği yazılır.
fn yanit_model_adi(istenen: &str, saglayici: &str) -> String {
    let t = istenen.trim();
    if t.is_empty() || t == "auto" {
        model_kimligi(saglayici)
    } else {
        t.to_string()
    }
}

/// Başarılı yanıtı istemcinin istediği biçimde (akış ya da tek parça) kurar; araç
/// tanımı geldiyse yanıttaki çağrıları `tool_calls`'a çevirir.
fn yanit_olustur(st: &AppState, req: &ChatRequest, metin: String, model: String) -> Response {
    let (icerik, cagrilar) = arac_cagrilarini_ayikla(&req.tools, metin);
    st.sayaclar
        .arac_cagrisi
        .fetch_add(cagrilar.len() as u64, std::sync::atomic::Ordering::Relaxed);
    let bitis = if cagrilar.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    if req.stream {
        return sse_yanit(&icerik, &cagrilar, bitis, &model);
    }
    Json(ChatResponse {
        id: format!("chatcmpl-kervan-{}", simdi()),
        object: "chat.completion",
        created: simdi(),
        model,
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: "assistant".into(),
                content: icerik,
                tool_calls: (!cagrilar.is_empty()).then_some(serde_json::Value::Array(cagrilar)),
                ..Default::default()
            },
            finish_reason: bitis,
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    })
    .into_response()
}

fn arac_adlari(tools: &[serde_json::Value]) -> Vec<&str> {
    tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect()
}

/// Araç tanımlarını tarayıcı modelinin anlayacağı düz yönergeye çevirir.
pub fn arac_yonergesi(tools: &[serde_json::Value]) -> String {
    if arac_adlari(tools).is_empty() {
        return String::new();
    }
    let mut y = String::from(
        "\n\n[ARAÇLAR] Bir aracı çağırman gerekirse yanıtına YALNIZCA şu biçimde bir blok koy, \
         sonucu bir sonraki mesajda alacaksın:\n```json\n{\"name\": \"ARAÇ_ADI\", \"arguments\": {}}\n```\n\
         Kullanılabilir araçlar:\n",
    );
    for t in tools {
        let f = &t["function"];
        if let Some(ad) = f["name"].as_str() {
            let aciklama = f["description"].as_str().unwrap_or("");
            y.push_str(&format!(
                "- {ad}: {aciklama} (parametreler: {})\n",
                f["parameters"]
            ));
        }
    }
    y
}

/// Yanıttaki ```json çağrılarından YALNIZ istemcinin tanımladığı araçları `tool_calls`
/// biçimine çevirir. Eşleşme yoksa metin olduğu gibi kalır.
pub fn arac_cagrilarini_ayikla(
    tools: &[serde_json::Value],
    metin: String,
) -> (String, Vec<serde_json::Value>) {
    use kervan_akis::translator::{AracCagrisi, AracCagrisiAyiklayici};
    let adlar = arac_adlari(tools);
    if adlar.is_empty() {
        return (metin, Vec::new());
    }
    let sonuc = AracCagrisiAyiklayici::ayikla(&metin);
    let cagrilar: Vec<_> = sonuc
        .cagrilar
        .into_iter()
        .filter_map(|c| match c {
            AracCagrisi::GenelFonksiyon {
                ad,
                parametreler_json,
            } if adlar.contains(&ad.as_str()) => Some(serde_json::json!({
                "id": format!("call_{:08x}", rand::random::<u32>()),
                "type": "function",
                "function": { "name": ad, "arguments": parametreler_json },
            })),
            _ => None,
        })
        .collect();
    if cagrilar.is_empty() {
        (metin, cagrilar)
    } else {
        (sonuc.temiz_metin, cagrilar)
    }
}

/// Kota mesajindan deftere yazilacak kisa kesit. Amac gercek ifadeyi ogrenmek,
/// sohbet biriktirmek degil.
fn kirp(m: &str) -> String {
    m.chars().take(200).collect()
}

/// OpenAI uyumlu SSE akisi.
///
/// ponytail: tarayici bize metni parca parca degil, tamamlandiginda veriyor
/// (bitisi kararlilikla algiliyoruz), o yuzden tek parca + [DONE] gonderiyoruz.
/// Protokol acisindan gecerli; istemci akis bekledigi icin bu sart.
/// Yukseltme yolu: `ask` icindeki yoklama dongusu zaten metnin BUYUDUGUNU goruyor —
/// oradan bir kanal acilirsa gercek artimli akis neredeyse bedava gelir. Ihtiyac
/// olunca yap; su an tek parca yeterli ve yanlis degil.
fn sse_yanit(metin: &str, cagrilar: &[serde_json::Value], neden: &str, model: &str) -> Response {
    let id = format!("chatcmpl-kervan-{}", simdi());
    let olustu = simdi();
    let mut delta = serde_json::json!({ "role": "assistant", "content": metin });
    if !cagrilar.is_empty() {
        // Akışta her çağrı sırasını `index` ile taşır.
        let sirali: Vec<_> = cagrilar
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut c = c.clone();
                c["index"] = i.into();
                c
            })
            .collect();
        delta["tool_calls"] = sirali.into();
    }
    let parca = serde_json::json!({
        "id": id, "object": "chat.completion.chunk", "created": olustu, "model": model,
        "choices": [{ "index": 0, "delta": delta }]
    });
    let bitis = serde_json::json!({
        "id": id, "object": "chat.completion.chunk", "created": olustu, "model": model,
        "choices": [{ "index": 0, "delta": {}, "finish_reason": neden }]
    });
    let govde = format!("data: {parca}\n\ndata: {bitis}\n\ndata: [DONE]\n\n");
    (
        StatusCode::OK,
        [
            ("content-type", "text/event-stream"),
            ("cache-control", "no-cache"),
        ],
        govde,
    )
        .into_response()
}
