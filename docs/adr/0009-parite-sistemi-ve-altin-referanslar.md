# ADR 0009: Parite Sistemi, Altın Referanslar ve Mutasyon Kontrolü

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute davranışlarının Kervan kodu yazılmadan önce altın dosyalara kaydedilmesi ve yeniden oynatılarak doğrulanması.

## Bağlam
`altin-kapi` felsefesi uyarınca referans, ölçülenden önce kaydedilmelidir (§7.3). Kervan'ın aktarım başarısı kişisel beyanlara değil, OmniRoute'un gerçekte ürettiği HTTP/SSE yanıtları ile Kervan'ın ürettiği yanıtların karşılaştırılmasına (parite farkı = 0) dayanmalıdır.

## Karar
1. **Parite Kaydedici (`araclar/parite/kayit.ts`):**
   - Referans OmniRoute örneğini ve sentetik bir yukarı akış sunucusunu ayağa kaldırır.
   - Belirlenen senaryoları (Sohbet, Akış, Hata, Araç Çağrısı, Devre Kesici, MCP) çalıştırır.
   - Uçucu alanları (tarih, uuid, request_id) normalize ederek `port/altin/<senaryo>.json` dosyalarına altın kayıt olarak kaydeder.
2. **Parite Koşucusu (`araclar/parite/kos.ts`):**
   - Aynı senaryoları Kervan üzerinde yeniden oynatır.
   - Yanıt gövdesi, durum kodu ve SSE akış deltalarını altın dosyayla kıyaslar.
   - Farklılıkları `port/defter/sapmalar.jsonl` dosyasına teknik gerekçesiyle kaydeder.
3. **Mutasyon Kontrolü:** Altın dosyadaki bir alan bilerek bozulduğunda parite testinin kırmızıya düştüğü (başarısız olduğu) kanıtlanacaktır ("Düşemeyen kapı kapı değildir").
