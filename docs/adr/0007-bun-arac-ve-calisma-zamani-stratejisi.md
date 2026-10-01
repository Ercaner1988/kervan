# ADR 0007: Bun Araç Katmanı ve İsteğe Bağlı Dış Süreç Stratejisi

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute'un Node/Next.js bağımlılıklarının temizlenmesi, Bun'ın rolü ve ürün çekirdeğinin saf Rust tutulması.

## Bağlam
OmniRoute dev sunucusu Bun altında ilk derlemede ~15 dakika sürmüş, 3–4,7 GB bellek tüketmiş ve geliştirme oturumlarını dondurmuştur (§1). Ercan'ın Rust'a geçişteki temel amacı bu devasa kaynak israfını sonlandırmaktır.

## Karar
1. **Ürün Çekirdeğinde Node ve Next.js Sıfır:** Kervan'ın derlenmiş ikilisi hiçbir şekilde Node.js, Next.js veya npm çalışma zamanı barındırmayacaktır.
2. **Bun'ın Rolü (§5-9):**
   - (a) Geliştirme araçları (`kervan/araclar/`): Envanter üretimi, defter kontrolü, parite kayıt ve yeniden oynatma testleri.
   - (b) Eklenti testleri: `eklenti/` altındaki MV3 tarayıcı eklentisi içerik betiklerinin testleri (`bun test`).
   - (c) İsteğe bağlı dış süreç: Kullanıcı OmniRoute JS eklentilerini veya harici JS sandbox becerilerini çalıştırmak isterse, Bun ayrı bir izole alt süreç olarak çağrılacak; ana ikiliye asla gömülmeyecektir.
