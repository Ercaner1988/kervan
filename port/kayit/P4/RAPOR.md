# Faz P4 Sonu Raporu: REST Yukarı Akış A — Yapılandırma, crates/yukari ve chat Kancası

- **Faz ID:** P4
- **Başlık:** REST Yukarı Akış A — [rest.*] Yapılandırması, crates/yukari ve chat Kancası
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **`kervan.toml` `[rest.<ad>]` Yapılandırması (FR-4.1):**
   - `crates/tarayici/src/config.rs` içine `RestAyar` struct'ı ve `Config.rest` haritası eklendi.
   - Desteklenen biçimler (`openai`, `claude`, `gemini`) doğrulandı; geçersiz biçim açıkça hata verir (`dogrula`).
   - Ortam değişkeni yoksa çağrı anında tespit edilir, değer asla sızdırılmaz.
   - Kanıt: [config.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/tarayici/src/config.rs), [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/kabul-haritasi.log).

2. **`crates/yukari` Crate'i ve OpenAI-Biçimli Geçiş (FR-4.2):**
   - `ureq` 3.4.2 (+`rustls`, +`gzip`) ile `crates/yukari` crate'i oluşturuldu.
   - İstemciden gelen OpenAI-biçimli gövde (`messages`, `temperature`, `max_tokens`, `tools`, `stream`) model adı haricinde derin eşit olarak ve `Authorization: Bearer <key>` başlığıyla yukarı akışa iletildi.
   - Kanıt: [lib.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/src/lib.rs), [p4_test.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p4_test.rs).

3. **`crates/sunucu/src/server.rs` `chat` Kancası (FR-4.3):**
   - `chat` işleyicisi ham `serde_json::Value` alacak şekilde güncellendi.
   - `st.rest_hedefi_coz` ile model `[rest.*]` hedefine çözülürse `yukari.gonder` çağrılır, aksi halde mevcut tarayıcı/eklenti yolu aynen sürer.
   - Mevcut birim testlerin tamamı (16 test) [server.rs:1048](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/src/server.rs#L1048) satırında değişmeden ve başarıyla geçti.
   - Kanıt: [server.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/src/server.rs), [uc-kaydi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/uc-kaydi.log).

4. **Mutasyon Kapısı ve Yeni Mutant (FR-2.6):**
   - `port/mutantlar.toml` dosyasına `rest-kancasini-kaldir` mutantı append-only kuralına uygun olarak eklendi.
   - 3 mutantın tamamı başarıyla öldürüldü.
   - Kanıt: [mutantlar.toml](file:///C:/Users/buzbe/Desktop/Github/kervan/port/mutantlar.toml), [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/mutasyon-kontrol.log).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|:---:|
| **P4-A1** | `kervan.toml` içinde [rest.<ad>] tablosu okunur; geçersiz biçim hata verir, eksik ortam değişkeninde 503 döner, örnek dosya geriye uyumludur | [p4_test.rs:11](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p4_test.rs#L11) (`p4_a1`) · [config.rs:259](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/tarayici/src/config.rs#L259) (`gecersiz_rest_bicimi_hata_verir`) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/kabul-haritasi.log) | ✅ GEÇTİ |
| **P4-A2** | openai biçimli sağlayıcıya istek kayıpsız gider; model hariç istemci gövdesiyle derin eşittir ve Authorization Bearer başlığı doğrudur | [p4_test.rs:49](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p4_test.rs#L49) (`p4_a2`) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/kabul-haritasi.log) | ✅ GEÇTİ |
| **P4-A3** | chat kancası model [rest.*] hedefine çözülürse yukari akışa devreder; REST olmayan model tarayıcı yoluna düşer | [p4_test.rs:188](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p4_test.rs#L188) (`p4_a3`) · [server.rs:1220](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/src/server.rs#L1220) (`rest_hedefi_cozumleme_ve_yonlendirme`) · [uc-kaydi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/uc-kaydi.log) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts) ile yeni eklenen `crates/yukari` dahil 103 dosya taranmış, [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/sabit-yanit.log) içinde hiçbir sahte kalıp bulunmadığı kanıtlanmıştır.
2. **Kendini doğrulayan kontrol:** Baktım: [p4_test.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p4_test.rs) sahte bir HTTP TCP soketi açarak gelen ham isteği, başlıkları ve gövdeyi ayrıştırmış ve derin eşitliği doğrulamıştır.
3. **Ulaşılamayan teslim:** Baktım: [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/ulasilabilirlik.log) ile `kervan-yukari` dahil 10 workspace crate'inin tamamının kök kervan ikilisine bağlı olduğu belgelenmiştir.
4. **Kanıtı aşan rapor:** Baktım: Bu rapordaki her iddia doğrudan `p4_test.rs`, `config.rs`, `server.rs` satırlarına ve `port/kayit/P4/*.log` kütüklerine bağlanmıştır.
5. **Gevşetme:** Baktım: Hiçbir kapı kuralı gevşetilmemiştir; [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P4/muhur-kontrol.log) mühürlü 18 dosyanın değişmediğini ve mutantlar.toml'un yalnız-ekleme kuralını sağladığını doğrulamıştır.
6. **Başarısız olamayan test:** Baktım: P4-A1 (eksik ortam değişkeninde 503 ve geçersiz biçimde hata), P4-A2 (başlık/gövde uyuşmazlığında başarısızlık) ve P4-A3 (REST olmayan modelde None dönme) negatif senaryolarla sınanmıştır.

---

## 4. Sandık Kod Satırı ve Sınır Denetimi

- **`crates/yukari`**: 185 satır (PRD sınırı ≤ 1500) — UYGUN.
- **`crates/sunucu`**: 1280 satır (PRD sınırı ≤ 1500) — UYGUN.
