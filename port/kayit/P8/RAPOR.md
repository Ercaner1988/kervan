# Faz P8 Sonu Raporu: Gerçek SSE ve Araç Çağrısı Geçişi

- **Faz ID:** P8
- **Başlık:** Gerçek SSE ve Araç Çağrısı Geçişi (FR-6.3, FR-6.4)
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **`kervan-yukari` Gerçek Zamanlı SSE Akış Motoru (`akis.rs`) (FR-6.3, FR-6.4):**
   - `crates/yukari/src/akis.rs` modülü oluşturuldu ve `Yukari::gonder_akis` metodu eklendi.
   - `stream=true` isteklerinde OpenAI, Anthropic Claude ve Google Gemini yukarı akışlarına `text/event-stream` protokolüyle akışlı bağlantı kurulur.
   - Gecikmesiz Satır Satır Okuma & İletim:
     - Yukarı akıştan gelen `data: ...` satırları `BufReader::lines()` ile geldikleri an okunur ve `tokio::sync::mpsc::channel` üzerinden Axum yanıt gövdesine anında aktarılır.
     - İstemciye ilk parça yukarı akış tamamlanmadan (gecikmeli sahte sunucu testinde 100ms altında, toplam süre 150ms üzerinde iken) ulaşır (`t_ilk_parca < t_toplam`).
     - Akış sonlandığında `data: [DONE]\n\n` iletilir.
   - Kanıt: [p8_sse_ve_tool_test.rs:65](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L65), [akis.rs:18](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/src/akis.rs#L18).

2. **Claude ve Gemini Gerçek Zamanlı SSE Çevirisi (FR-6.3):**
   - Claude akışında: `event: message_start`, `content_block_delta`, `message_delta`, `message_stop` olayları anlık olarak `ClaudeOlay` ile ayrıştırılır, `claude_dan_openai_parcasi` ile OpenAI `chat.completion.chunk` biçimine dönüştürülüp istemciye basılır. `message_stop` geldiğinde akış `[DONE]` ile sonlandırılır.
   - Gemini akışında: `:streamGenerateContent?alt=sse` uç noktasından gelen `GeminiYanit` parçaları anlık olarak `gemini_den_openai_parcasi` ile OpenAI `chat.completion.chunk` biçimine dönüştürülüp iletilir.
   - Nonce parçalarının sırası ve bayt bütünlüğü korunur.
   - Kanıt: [p8_sse_ve_tool_test.rs:187](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L187), [p8_sse_ve_tool_test.rs:288](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L288).

3. **OpenAI Biçimli Araç Çağrısı Kayıpsız Geçişi (FR-6.4):**
   - Akışsız Mod: İstemciden gelen `tools` OpenAI sağlayıcısına aynen iletilir; yukarı akıştan dönen `choices[0].message.tool_calls` istemciye kayıpsız ulaşır.
   - Akışlı Mod (`stream=true`): OpenAI hedefinde `tools` iletilir; yukarı akışın akışlı parçalardaki `choices[0].delta.tool_calls` verisi ve nonce'lar istemciye kayıpsız akar.
   - Kanıt: [p8_sse_ve_tool_test.rs:388](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L388), [p8_sse_ve_tool_test.rs:498](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L498).

4. **Claude ve Gemini Hedeflerinde Akışta Araç Çağrısı Açık Hata (FR-5.3, FR-6.4):**
   - Claude veya Gemini hedefine `stream=true` iken `tools` gönderildiğinde yukarı akışa istek GİTMEZ (sayaç 0 kalır).
   - İstemciye açık hata döner (`501 Not Implemented`, "desteklenmiyor: tools").
   - Kanıt: [p8_sse_ve_tool_test.rs:604](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L604), [akis.rs:49](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/src/akis.rs#L49).

5. **Mutasyon Kapısı ve Yeni Mutant (FR-2.6):**
   - `sse-claude-arac-desteklenmiyor-denetimini-atla` mutantı [mutantlar.toml:40](file:///C:/Users/buzbe/Desktop/Github/kervan/port/mutantlar.toml#L40) kütüğüne eklendi.
   - `bun araclar/kapilar/mutasyon-kontrol.ts` çalıştırılarak toplam 6 mutantın tamamının öldürüldüğü kanıtlandı (6/6 mutant öldürüldü).
   - Kanıt: [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/mutasyon-kontrol.log).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|:---:|
| **P8-A1** | Gecikmeli sahte sunucuyla ilk parça yukarı akış sonundan önce gelir; nonce parçalarının sırası korunur; [DONE] ile biter | [p8_sse_ve_tool_test.rs:65](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L65) · [sse-ve-tool-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/sse-ve-tool-test.log) | ✅ GEÇTİ |
| **P8-A2** | Claude ve Gemini akış SSE çevirisi OpenAI chunk biçimine dönüştürülür ve parite korunur | [p8_sse_ve_tool_test.rs:187](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L187) · [p8_sse_ve_tool_test.rs:288](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L288) · [sse-ve-tool-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/sse-ve-tool-test.log) | ✅ GEÇTİ |
| **P8-A3** | OpenAI hedeflerde araç çağrısı (tools / tool_calls) akışsız ve akışlı kayıpsız iletilir | [p8_sse_ve_tool_test.rs:388](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L388) · [p8_sse_ve_tool_test.rs:498](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L498) · [sse-ve-tool-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/sse-ve-tool-test.log) | ✅ GEÇTİ |
| **P8-A4** | Claude ve Gemini hedeflerde stream=true iken tools gelirse yukarı akışa istek gitmeden açık hata (501) döner | [p8_sse_ve_tool_test.rs:604](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L604) · [sse-ve-tool-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/sse-ve-tool-test.log) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts) ile üretim kodunda hiçbir sabit sahte kalıp bulunmadığı [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/sabit-yanit.log) kütüğünde belgelenmiştir.
2. **Kendini doğrulayan kontrol:** Baktım: [p8_sse_ve_tool_test.rs:65](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p8_sse_ve_tool_test.rs#L65) gerçek gecikmeli TCP mock sunucusu ile ilk parçanın varış zamanını (`t_ilk < 100ms`) ölçerek toplam süreden (`t_toplam >= 140ms`) önce geldiğini doğrudan doğrulamıştır.
3. **Ulaşılamayan teslim:** Baktım: [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/ulasilabilirlik.log) ile tüm sandıkların kök kervan ikilisine bağlı olduğu belgelenmiştir.
4. **Kanıtı aşan rapor:** Baktım: Bu rapordaki her iddia doğrudan `p8_sse_ve_tool_test.rs` test satırlarına ve `port/kayit/P8/*.log` kütüklerine bağlanmıştır.
5. **Gevşetme:** Baktım: Hiçbir kapı kuralı gevşetilmemiştir; [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/muhur-kontrol.log) mühürlü kapıların korunduğunu doğrulamıştır.
6. **Başarısız olamayan test:** Baktım: `sse-claude-arac-desteklenmiyor-denetimini-atla` mutantı uygulandığında `test_p8_claude_akista_arac_acik_hata` testinin başarısız olarak mutantı öldürdüğü [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/mutasyon-kontrol.log) kütüğüyle kanıtlanmıştır.
