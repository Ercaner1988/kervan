//! Kervan GUI Model Tanımları, Medya Ayrıştırıcı ve Yardımcıları.
//!
//! Bu modül, `src/gui.rs` dosyasının 1.500 satır kuralına ("ekdenetle" kuralı)
//! kusursuz uyması ve model tanımlarının temizce yönetilmesi için ayrıştırılmıştır.

use crate::gui::{DusunmeSeviyesi, UretimModu};

/// Web arayüzü model tanımı
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTanimi {
    pub kod: &'static str,
    pub gorunen_ad: &'static str,
    pub aciklama: &'static str,
}

impl ModelTanimi {
    pub const fn new(kod: &'static str, gorunen_ad: &'static str, aciklama: &'static str) -> Self {
        Self {
            kod,
            gorunen_ad,
            aciklama,
        }
    }
}

pub static GEMINI_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "gemini-3.8-flash",
        "Gemini 3.8 Flash (Ultra Hızlı & Ajan Düşünme)",
        "En yeni nesil ultra hızlı, geniş bağlamlı amiral ajan modeli",
    ),
    ModelTanimi::new(
        "gemini-3.1-pro",
        "Gemini 3.1 Pro (Derin Akıl Yürütme & Kodlama)",
        "En güçlü karmaşık problem çözme ve çok modlu akıl yürütme",
    ),
    ModelTanimi::new(
        "gemini-3.0-pro",
        "Gemini 3.0 Pro (Gelişmiş Çok Modlu Amiral)",
        "Yüksek kapasiteli akıl yürütme, derin analiz ve araştırma",
    ),
    ModelTanimi::new(
        "gemini-3.0-flash",
        "Gemini 3.0 Flash (Hızlı Çok Modlu)",
        "Düşük gecikmeli, yüksek verimli multimodal işlem",
    ),
    ModelTanimi::new(
        "gemini-4-preview",
        "Gemini 4.0 Preview (Gelecek Nesil Öncü Model)",
        "Yeni nesil otonom akıl yürütme ve araştırma mimarisi",
    ),
    ModelTanimi::new(
        "gemini-2.5-pro",
        "Gemini 2.5 Pro (Gelişmiş Düşünme & Kodlama)",
        "Güçlü amiral model, karmaşık akıl yürütme",
    ),
    ModelTanimi::new(
        "gemini-2.5-flash",
        "Gemini 2.5 Flash (Ultra Hızlı & Çok Modlu)",
        "Düşük gecikme, yüksek kapasite",
    ),
    ModelTanimi::new(
        "gemini-2.0-flash-thinking",
        "Gemini 2.0 Flash Thinking (Derin Düşünme)",
        "Adım adım mantıksal çıkarım ve reasoning",
    ),
    ModelTanimi::new(
        "gemini-deep-research",
        "Gemini Deep Research (Derinlemesine Rapor)",
        "Kapsamlı web ve akademik araştırma",
    ),
    ModelTanimi::new(
        "imagen-3",
        "Imagen 3 (Fotogerçekçi Görsel Üretimi)",
        "Yüksek çözünürlüklü görsel ve sanat üretimi",
    ),
    ModelTanimi::new(
        "veo-2",
        "Veo 2 (Sinematik Video Hazırlama)",
        "Akıcı ve yüksek kaliteli video oluşturma",
    ),
    ModelTanimi::new(
        "gemini-1.5-pro",
        "Gemini 1.5 Pro (2M Token Geniş Bağlam)",
        "Devasa dosya, video ve kod bağlamı",
    ),
    ModelTanimi::new(
        "gemini-1.5-flash",
        "Gemini 1.5 Flash (Hafif & Hızlı)",
        "Gündelik hızlı işler",
    ),
];

pub static DEEPSEEK_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "deepseek-r1",
        "DeepSeek-R1 (Derin Düşünme / DeepThink)",
        "Sınırsız akıl yürütme, matematik ve kod",
    ),
    ModelTanimi::new(
        "deepseek-v3",
        "DeepSeek-V3 (Genel Sohbet & Çok Hızlı)",
        "671B MoE amiral sohbet modeli",
    ),
    ModelTanimi::new(
        "deepseek-r1-search",
        "DeepSeek-R1 + Web Arama",
        "Canlı internet verisiyle düşünme",
    ),
    ModelTanimi::new(
        "deepseek-coder",
        "DeepSeek-Coder-V2",
        "Uzman yazılım mimarisi",
    ),
];

pub static CHATGPT_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "o3-mini-high",
        "o3-mini-high (Derin Akıl Yürütme)",
        "STEM ve karmaşık mantık, yüksek düşünme",
    ),
    ModelTanimi::new(
        "o3-mini",
        "o3-mini (Akıl Yürütme / STEM & Kod)",
        "Hızlı ve hassas reasoning",
    ),
    ModelTanimi::new(
        "o1",
        "o1 (Tam Akıl Yürütme / Full Reasoning)",
        "OpenAI derin akıl yürütme amiral modeli",
    ),
    ModelTanimi::new(
        "o1-pro",
        "o1-pro (Kapsamlı Problem Çözme)",
        "Yoğun hesaplamalı ileri seviye reasoning",
    ),
    ModelTanimi::new(
        "gpt-4o",
        "GPT-4o (En Yetenekli Amiral Model)",
        "Çok modlu metin, görsel ve ses",
    ),
    ModelTanimi::new(
        "gpt-4o-canvas",
        "GPT-4o Canvas (Kod & Yazı Çalışma Alanı)",
        "İnteraktif düzenleme",
    ),
    ModelTanimi::new(
        "searchgpt",
        "SearchGPT (Canlı Web Arama)",
        "Gerçek zamanlı internet araması",
    ),
    ModelTanimi::new(
        "dall-e-3",
        "DALL·E 3 (Görsel Üretimi)",
        "Hassas detaylı görsel çizimi",
    ),
    ModelTanimi::new(
        "sora",
        "Sora (Video Üretimi)",
        "Video oluşturma motoru",
    ),
];

pub static QWEN_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "qwen-2.5-max",
        "Qwen 2.5 Max (En Güçlü Amiral Model)",
        "Devasa parametreli amiral gemisi",
    ),
    ModelTanimi::new(
        "qwen-2.5-plus",
        "Qwen 2.5 Plus (Dengeli Yüksek Performans)",
        "Hızlı ve kapsamlı",
    ),
    ModelTanimi::new(
        "qwen-2.5-coder-32b",
        "Qwen 2.5 Coder 32B (Uzman Kodlama)",
        "Derin yazılım geliştirme",
    ),
    ModelTanimi::new(
        "qwq-32b-preview",
        "QwQ-32B Preview (Akıl Yürütme / o1 Seviyesi)",
        "Düşünce zinciri (CoT)",
    ),
    ModelTanimi::new(
        "qwen-deepsearch",
        "Qwen DeepSearch (Derin Web Araştırması)",
        "Kapsamlı araştırma raporu",
    ),
    ModelTanimi::new(
        "wanx-2.1",
        "Wan 2.1 (Görsel & Video Üretimi)",
        "Multimedya üretim motoru",
    ),
];

pub static KIMI_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "kimi-k1.5-thinking",
        "Kimi k1.5 Long Thinking (Derin Akıl Yürütme)",
        "Adım adım derinlemesine düşünme",
    ),
    ModelTanimi::new(
        "kimi-k1.5",
        "Kimi k1.5 (Gelişmiş Çok Modlu & Uzun Bağlam)",
        "Metin, görsel, belge",
    ),
    ModelTanimi::new(
        "kimi-explore",
        "Kimi Explore (Derin Araştırma)",
        "Ayrıntılı web araması ve sentez",
    ),
    ModelTanimi::new(
        "moonshot-v1-128k",
        "Kimi Moonshot V1 (128k-2M Token)",
        "Uzun döküman analizi",
    ),
];

pub static COPILOT_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "copilot-think-deeper",
        "Copilot Think Deeper (Derin Akıl Yürütme)",
        "Derinlemesine adım adım düşünme ve analiz",
    ),
    ModelTanimi::new(
        "copilot-pro-creative",
        "Copilot Pro (Yaratıcı / GPT-4o)",
        "Yaratıcı metin, fikir ve beyin fırtınası",
    ),
    ModelTanimi::new(
        "copilot-pro-precise",
        "Copilot Pro (Hassas Kod & Bilim)",
        "Hassas yanıtlar, kodlama ve matematik",
    ),
    ModelTanimi::new(
        "designer-dalle3",
        "Designer (DALL-E 3 Görsel)",
        "Görsel tasarım ve resim",
    ),
];

pub static PERPLEXITY_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "sonar-deep-research",
        "Sonar Deep Research (Kapsamlı Rapor)",
        "Çok adımlı derin web ve literatür taraması",
    ),
    ModelTanimi::new(
        "sonar-reasoning-pro",
        "Sonar Reasoning Pro (CoT Mantık & Analiz)",
        "Gelişmiş akıl yürütme ve problem çözme",
    ),
    ModelTanimi::new(
        "sonar-pro",
        "Sonar Pro (En Güçlü Web Arama Modeli)",
        "Gelişmiş internet araması ve sentez",
    ),
    ModelTanimi::new(
        "claude-3-5-sonnet",
        "Claude 3.5 Sonnet (Perplexity Entegre)",
        "Üstün yazım ve kodlama kalitesi",
    ),
];

pub static MISTRAL_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "mistral-large-2",
        "Mistral Large 2 (Amiral Model)",
        "Çok dilli en güçlü amiral model, karmaşık akıl yürütme",
    ),
    ModelTanimi::new(
        "mistral-canvas",
        "Mistral Canvas (Çalışma Alanı & Ajan)",
        "İnteraktif doküman, kod ve araştırma çalışma alanı",
    ),
    ModelTanimi::new(
        "pixtral-large",
        "Pixtral Large (Çok Modlu Görsel & Belge OCR)",
        "Görsel, arşiv ve doküman derin anlama",
    ),
    ModelTanimi::new(
        "voxtral",
        "Voxtral (Ses Transkripsiyon & Seslendirme STT/TTS)",
        "Çift yönlü yüksek doğruluklu ses motoru",
    ),
    ModelTanimi::new(
        "codestral",
        "Codestral (Yazılım & Kod Geliştirme)",
        "80+ dilde uzman kodlama",
    ),
    ModelTanimi::new(
        "mistral-research",
        "Mistral Web Research",
        "İnternet araması ve derin akademik araştırma",
    ),
];

pub static ZAI_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "glm-4-plus",
        "GLM-4-Plus (En Güçlü Amiral Model)",
        "En güçlü genel amiral modeli",
    ),
    ModelTanimi::new(
        "glm-4-long",
        "GLM-4-Long (1M Token Geniş Bağlam)",
        "Büyük dosya ve veri analizi",
    ),
    ModelTanimi::new(
        "glm-4-alltools",
        "GLM-4-AllTools (Kod Yorumlayıcı + Web)",
        "Araç kullanımı ve veri işleme",
    ),
    ModelTanimi::new(
        "cogvideox",
        "CogVideoX (Video Üretimi)",
        "Video üretim modeli",
    ),
];

pub static AUTO_MODELLERI: &[ModelTanimi] = &[
    ModelTanimi::new(
        "otomatik",
        "Otomatik (Akıllı Rota)",
        "İsteminize göre en uygun sağlayıcı ve modeli otomatik seçer",
    ),
];

/// Sağlayıcı adına göre model listesini döndürür
pub fn saglayici_modelleri(saglayici: &str) -> &'static [ModelTanimi] {
    match saglayici {
        "gemini" => GEMINI_MODELLERI,
        "deepseek" => DEEPSEEK_MODELLERI,
        "chatgpt" => CHATGPT_MODELLERI,
        "qwen" => QWEN_MODELLERI,
        "kimi" => KIMI_MODELLERI,
        "copilot" => COPILOT_MODELLERI,
        "perplexity" => PERPLEXITY_MODELLERI,
        "mistral" => MISTRAL_MODELLERI,
        "z.ai" => ZAI_MODELLERI,
        "auto" => AUTO_MODELLERI,
        _ => &[],
    }
}

/// Sağlayıcının varsayılan modelini döndürür
pub fn varsayilan_model(saglayici: &str) -> &'static str {
    match saglayici {
        "gemini" => "gemini-2.5-pro",
        "deepseek" => "deepseek-r1",
        "chatgpt" => "o3-mini-high",
        "qwen" => "qwen-2.5-max",
        "kimi" => "kimi-k1.5-thinking",
        "copilot" => "copilot-think-deeper",
        "perplexity" => "sonar-deep-research",
        "mistral" => "mistral-large-2",
        "z.ai" => "glm-4-plus",
        "auto" => "otomatik",
        _ => "varsayilan",
    }
}

/// Sağlayıcı ve üretim moduna göre en güncel amiral model kodunu belirler.
pub fn amiral_model(saglayici: &str, mod_: UretimModu) -> Option<&'static str> {
    match (saglayici, mod_) {
        ("gemini", UretimModu::GorselUretimi) => Some("imagen-3"),
        ("gemini", UretimModu::VideoHazirlama) => Some("veo-2"),
        ("gemini", UretimModu::Dusunme) => Some("gemini-3.1-pro"),
        ("gemini", UretimModu::DerinArastirma) => Some("gemini-deep-research"),
        ("gemini", UretimModu::Sohbet) => Some("gemini-3.8-flash"),

        ("deepseek", UretimModu::Dusunme) => Some("deepseek-r1"),
        ("deepseek", UretimModu::DerinArastirma) => Some("deepseek-r1-search"),
        ("deepseek", UretimModu::Sohbet) => Some("deepseek-v3"),

        ("chatgpt", UretimModu::GorselUretimi) => Some("dall-e-3"),
        ("chatgpt", UretimModu::VideoHazirlama) => Some("sora"),
        ("chatgpt", UretimModu::Dusunme) => Some("o3-mini-high"),
        ("chatgpt", UretimModu::DerinArastirma) => Some("searchgpt"),
        ("chatgpt", UretimModu::Sohbet) => Some("gpt-4o"),

        ("qwen", UretimModu::GorselUretimi) => Some("wanx-2.1"),
        ("qwen", UretimModu::VideoHazirlama) => Some("wanx-2.1"),
        ("qwen", UretimModu::Dusunme) => Some("qwq-32b-preview"),
        ("qwen", UretimModu::DerinArastirma) => Some("qwen-deepsearch"),
        ("qwen", UretimModu::Sohbet) => Some("qwen-2.5-max"),

        ("kimi", UretimModu::Dusunme) => Some("kimi-k1.5-thinking"),
        ("kimi", UretimModu::DerinArastirma) => Some("kimi-explore"),
        ("kimi", UretimModu::Sohbet) => Some("kimi-k1.5"),

        ("copilot", UretimModu::GorselUretimi) => Some("designer-dalle3"),
        ("copilot", UretimModu::Dusunme) => Some("copilot-think-deeper"),
        ("copilot", UretimModu::DerinArastirma) => Some("copilot-pro-precise"),
        ("copilot", UretimModu::Sohbet) => Some("copilot-pro-creative"),

        ("perplexity", UretimModu::Dusunme) => Some("sonar-reasoning-pro"),
        ("perplexity", UretimModu::DerinArastirma) => Some("sonar-deep-research"),
        ("perplexity", UretimModu::Sohbet) => Some("sonar-pro"),

        ("mistral", UretimModu::GorselUretimi) => Some("pixtral-large"),
        ("mistral", UretimModu::Dusunme) => Some("mistral-large-2"),
        ("mistral", UretimModu::DerinArastirma) => Some("mistral-research"),
        ("mistral", UretimModu::Sohbet) => Some("mistral-large-2"),

        ("z.ai", UretimModu::VideoHazirlama) => Some("cogvideox"),
        ("z.ai", UretimModu::Dusunme) => Some("glm-4-plus"),
        ("z.ai", UretimModu::DerinArastirma) => Some("glm-4-alltools"),
        ("z.ai", UretimModu::Sohbet) => Some("glm-4-plus"),

        ("auto", UretimModu::GorselUretimi) => Some("imagen-3"),
        ("auto", UretimModu::VideoHazirlama) => Some("veo-2"),
        ("auto", UretimModu::Dusunme) => Some("gemini-3.1-pro"),
        ("auto", UretimModu::DerinArastirma) => Some("gemini-deep-research"),
        ("auto", UretimModu::Sohbet) => Some("otomatik"),

        _ => None,
    }
}

/// Seçilen moda göre İstem Çerçeveleme (Prompt Framing)
pub fn istem_cercevele(ham_istem: &str, mod_: UretimModu, seviye: DusunmeSeviyesi) -> String {
    let ham = ham_istem.trim();
    match mod_ {
        UretimModu::Sohbet => ham.to_string(),
        UretimModu::Dusunme => format!(
            "[Düşünme Seviyesi: {}] Lütfen soruyu adım adım derinlemesine düşünerek ve akıl yürüterek yanıtla: {}",
            seviye.etiket(),
            ham
        ),
        UretimModu::DerinArastirma => format!(
            "[Derin Araştırma Modu] Lütfen konuyu tüm boyutlarıyla, teknik detaylar ve alt başlıklarla derinlemesine araştırıp raporla: {}",
            ham
        ),
        UretimModu::GorselUretimi => format!(
            "Lütfen şu sahneyi yüksek kalitede, fotogerçekçi ve ayrıntılı bir görsel olarak oluştur (Generate Image): {}",
            ham
        ),
        UretimModu::VideoHazirlama => format!(
            "Lütfen şu sahne için sinematik kalitede akıcı bir video oluştur (Generate Video): {}",
            ham
        ),
    }
}

/// Ayrıştırılan Medya Öğesi Tipi
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MedyaTipi {
    Gorsel,
    Video,
}

/// Ayrıştırılan Medya Öğesi
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedyaOgesi {
    pub tip: MedyaTipi,
    pub baslik: String,
    pub url: String,
}

/// Gelen yanıttaki `![Üretilen Görsel](URL)` ve `[Üretilen Video](URL)` bağlantılarını ayıklayan hafif parser
pub fn medya_ayikla(metin: &str) -> Vec<MedyaOgesi> {
    let mut ogeler = Vec::new();
    let mut kalan = metin;

    while let Some(pos) = kalan.find("[Üretilen ") {
        let oncesi = &kalan[..pos];
        let is_gorsel = oncesi.ends_with('!');
        let sonrasi = &kalan[pos..];

        if let Some(parantez_basi) = sonrasi.find("](") {
            let baslik = &sonrasi[1..parantez_basi];
            let url_basi = &sonrasi[parantez_basi + 2..];
            if let Some(url_sonu) = url_basi.find(')') {
                let url = url_basi[..url_sonu].trim();
                if url.starts_with("http://") || url.starts_with("https://") {
                    let tip = if is_gorsel {
                        MedyaTipi::Gorsel
                    } else if baslik.to_lowercase().contains("video") {
                        MedyaTipi::Video
                    } else {
                        MedyaTipi::Gorsel
                    };
                    ogeler.push(MedyaOgesi {
                        tip,
                        baslik: baslik.to_string(),
                        url: url.to_string(),
                    });
                }
                kalan = &url_basi[url_sonu + 1..];
                continue;
            }
        }
        kalan = &sonrasi[1..];
    }

    ogeler
}

pub use medya_ayikla as medyalari_ayikla;

/// Kullanıcının arayüzden seçtiği veya sürükleyip bıraktığı ek dosya
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YuklenenDosya {
    pub yol: String,
    pub ad: String,
    pub boyut: u64,
    pub mime_tipi: String,
}

/// Dosya uzantısına göre MIME tipini belirler.
pub fn dosya_mime_belirle(yol: &str) -> &'static str {
    let uzanti = yol
        .rsplit('.')
        .next()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match uzanti.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "json" => "application/json",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Harici bağımlılık gerektirmeyen hafif ve saf Base64 kodlayıcı.
pub fn base64_encode(data: &[u8]) -> String {
    const TABLO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut sonuc = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        let u = (b0 << 16) | (b1 << 8) | b2;

        sonuc.push(TABLO[(u >> 18) & 0x3F] as char);
        sonuc.push(TABLO[(u >> 12) & 0x3F] as char);

        if chunk.len() > 1 {
            sonuc.push(TABLO[(u >> 6) & 0x3F] as char);
        } else {
            sonuc.push('=');
        }

        if chunk.len() > 2 {
            sonuc.push(TABLO[u & 0x3F] as char);
        } else {
            sonuc.push('=');
        }
    }

    sonuc
}
