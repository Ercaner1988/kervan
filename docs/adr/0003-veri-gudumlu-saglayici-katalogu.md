# ADR 0003: Veri-Güdümlü Sağlayıcı Kataloğu ve Yürütücü İndirgemesi

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute'taki 233 yürütücü dosyasının (~360 sağlayıcı) kod yerine veri modellerine dönüştürülmesi.

## Bağlam
OmniRoute `open-sse/executors/` altında 233 dosya ve ~79 bin satır kod bulunmaktadır. Faz 0 envanter ölçümümüz, bu sınıfların %78'inin neredeyse tamamen aynı temel HTTP gönderme mantığını (`BaseExecutor`, `OpenAICompatibleExecutor`) kullandığını, sadece URL şablonu, yetkilendirme başlığı biçimi ve model haritalamasının değiştiğini ortaya koymuştur. Bunları 1:1 Rust sınıflarına çevirmek yüzlerce gereksiz dosya yaratacaktır.

## Karar
1. **Veri-Güdümlü Katalog (`kervan-katalog`):**
   - ~360 sağlayıcının meta verileri (taban URL, başlık formatı, kimlik doğrulama tipi, model takma adları, kota pencereleri) Bun betikleri tarafından TypeScript kaynaklarından derlenen durağan bir JSON/Rust veri tablosuna dönüştürülecektir.
2. **Birleşik Yürütücü Motorları (`kervan-yurutucu`):**
   - Bireysel 233 sınıf yerine 5 genel yürütücü uygulanacaktır:
     1. `StandartRestYurutucu`: OpenAI uyumlu tüm sağlayıcılar (DeepSeek, Groq, Mistral, Together, OpenRouter vb.).
     2. `ClaudeRestYurutucu`: Anthropic Messages API protokolü.
     3. `GeminiRestYurutucu`: Google Gemini REST/gRPC HTTP protokolü.
     4. `TarayiciOturumYurutucu`: Kervan'ın özgün Geckodriver/Zen Browser WebExtension köprüsü (Web UI oturumları).
     5. `WebSocketYurutucu`: Gerçek zamanlı iki yönlü akışlar.
3. Bu sayede kod tabanı %85 küçülecek, bakım kolaylaşacak ve Kervan'ın 1.500 satır sandık kuralları korunacaktır.
