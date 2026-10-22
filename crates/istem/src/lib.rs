//! İstem kurulumu: OpenAI mesaj dizisinden tarayıcıya gidecek metni kurar.
//!
//! DURUM GERİLİMİ (bilinen sınırlama, sessizce geçiştirilmiyor):
//! OpenAI `/v1/chat/completions` DURUMSUZ — istemci her istekte tüm konuşma
//! geçmişini gönderir. Tarayıcı sohbeti ise DURUMLU — kendi dizisini kendi tutar.
//! 30 bin token'lik geçmişi sohbet kutusuna yazmak pratik değil, hem de anlamsız:
//! sayfa zaten hatırlıyor.
//!
//! v1 kararı: **yalnızca son kullanıcı mesajı** gönderilir, süreklilik tarayıcının
//! kendi dizisinden gelir. Dizi boşsa (hesap değişti, tarayıcı boşta kapandı) geçmiş
//! tohumlanır — [`tohum_istemi`]. Bu bir ADR konusu; "hesap / profil / oturum"
//! terim ayrımı tam da bunun için.

use serde::{Deserialize, Serialize};

/// OpenAI mesajı. `content` alanı araç çağrısı taşıyan asistan mesajında `null`
/// gelir; boş sayılır, çünkü modelin metni yok.
#[derive(Deserialize, Serialize, Clone, Default)]
pub struct Message {
    pub role: String,
    /// Araç çağrısı taşıyan asistan mesajında OpenAI `null` gönderir.
    #[serde(default, deserialize_with = "bos_olabilir")]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn bos_olabilir<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

/// Konuşmanın parmak izi: **ilk kullanıcı mesajının** özeti.
///
/// OpenAI API'sinde oturum kimliği yok, istemciden yeni bir alan istemeden
/// konuşmaları ayırt etmenin yolu içerikten tanımak. İlk kullanıcı mesajı her
/// turda gönderildiği için konuşma boyunca değişmez; farklı konuşmalar farklı iz
/// verir.
///
/// Neden yalnız o mesaj, sistem istemi de değil: sistem istemine tarih/saat gibi
/// her istekte değişen şeyler girebiliyor. Onu ize katmak izi her turda değiştirir,
/// yani sabitlemeyi sessizce işe yaramaz hale getirirdi.
///
/// İki bilinen sınır: (a) aynı cümleyle başlayan iki ayrı konuşma aynı ize düşer —
/// sonucu yalnız aynı hesaba yönlenmeleridir, yanlış yanıt değil; (b) istemci
/// geçmişi yeniden yazarsa (oturum özetlemede / compaction) iz değişir ve konuşma
/// başka hesaba taşınabilir.
pub fn konusma_izi(msgs: &[Message]) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let ilk = msgs.iter().find(|m| m.role == "user")?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ilk.content.hash(&mut h);
    Some(h.finish())
}

/// Yalnızca son `user` mesajı — gerekçesi dosya başındaki durum gerilimi notunda.
pub fn son_kullanici_mesaji(msgs: &[Message]) -> Option<String> {
    // Araç turu: istemci çağrıları çalıştırıp sonuçlarını `tool` mesajı olarak geri
    // yolladı. Modele giden, son kullanıcı sorusu değil bu sonuçlardır.
    let sonuclar: Vec<_> = msgs.iter().rev().take_while(|m| m.role == "tool").collect();
    if !sonuclar.is_empty() {
        use kervan_akis::translator::{AracCagrisi, AracCagrisiAyiklayici as A};
        let parcalar: Vec<String> = sonuclar
            .iter()
            .rev()
            .map(|m| {
                let ad = m.name.clone().unwrap_or_else(|| "arac".into());
                let cagri = AracCagrisi::GenelFonksiyon {
                    ad,
                    parametreler_json: String::new(),
                };
                A::sistem_geri_bildirimi_uret(&cagri, &m.content)
            })
            .collect();
        return Some(parcalar.join("\n\n"));
    }
    msgs.iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
}

/// Tohumlanmış geçmiş kaç karakteri aşmasın.
///
/// İstem kutusuna sığmayan bir metin gönderilemez; üstelik uzun tohum her turda
/// değil YALNIZCA dizi değiştiğinde gidiyor, yani bedeli seyrek.
pub const TOHUM_AZAMI: usize = 24_000;

/// Boş bir sohbet dizisine gönderilecek istem: geçmiş + asıl soru.
///
/// NEDEN: OpenAI API durumsuz, tarayıcı sohbeti durumlu. Normalde süreklilik
/// tarayıcının kendi dizisinden gelir ve yalnızca son mesajı göndermek yeter.
/// Ama dizi BOŞSA — hesap değişti, kota bitti, tarayıcı boşta kapanıp yeniden
/// açıldı — model önceki turları hiç görmemiş olur ve geçmişsiz cevaplar.
///
/// Özet çıkarmaya gerek yok: istemci geçmişin TAMAMINI zaten her istekte
/// gönderiyor, biz atıyorduk. Elde olanı kullanmak ek model çağrısından,
/// ek kotadan ve özetleme kaybından ucuz.
///
/// Sistem mesajı da giriyor: tarayıcı arayüzünde sistem istemi diye ayrı bir yer
/// yok, girmezse modelin yönergeleri kaybolur.
pub fn tohum_istemi(msgs: &[Message], azami: usize) -> String {
    let Some(son) = msgs.iter().rposition(|m| m.role == "user") else {
        return String::new();
    };
    let asil = &msgs[son].content;

    let etiket = |rol: &str| match rol {
        "system" => "Yonerge",
        "assistant" => "Sen",
        _ => "Ben",
    };

    // Sondan basa topla: kırpmak gerekirse EN ESKİ turlar düşsün, yakın bağlam kalsın.
    let mut parcalar: Vec<String> = Vec::new();
    let mut uzunluk = 0usize;
    let mut atlanan = 0usize;
    for m in msgs[..son].iter().rev() {
        if m.content.trim().is_empty() {
            continue;
        }
        let p = format!("{}: {}", etiket(&m.role), m.content.trim());
        if uzunluk + p.chars().count() > azami {
            atlanan += 1;
            continue;
        }
        uzunluk += p.chars().count();
        parcalar.push(p);
    }
    if parcalar.is_empty() {
        return asil.clone();
    }
    parcalar.reverse();

    let mut s = String::from(
        "Bu konusma baska bir oturumdan devrediliyor; asagisi onceki turlar. \
         Okuyup baglam olarak kullan, onlara ayrica cevap verme.\n\n--- onceki turlar ---\n",
    );
    // Sessiz kırpma yok: eksik varsa yazılı olsun.
    if atlanan > 0 {
        s.push_str(&format!(
            "(en eski {atlanan} mesaj uzunluk siniri nedeniyle atlandi)\n"
        ));
    }
    s.push_str(&parcalar.join("\n\n"));
    s.push_str("\n--- onceki turlar bitti ---\n\nSimdiki sorum:\n");
    s.push_str(asil);
    s
}
