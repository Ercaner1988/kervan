# ADR 0008: egui Masaüstü Mimarisi, Panel Kayıt Sistemi ve WGPU Render Kuralı

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute'un 122 sayfalık Next.js web panosunun egui (Rust) masaüstü arayüzüne dönüştürülmesi ve Windows render güvenliği.

## Bağlam
OmniRoute; model yönetimi, sağlayıcı ayarları, MCP araçları, kullanım grafikleri, loglar, oturumlar ve güvenlik kuralları için 122 Next.js sayfası sunmaktadır.
Ayrıca Windows platformunda egui kullanılırken OpenGL/glow motorunun AMD/Intel kartlarda kara ekran vermesi ve WGPU (DirectX 12) motorunun zorunlu olması kuralı (§6-4) mevcuttur.

## Karar
1. **Panel Kayıt Sistemi (`PanelKayit`):**
   - 122 sayfa, modüler egui panellerine dönüştürülecektir.
   - Her panel `KervanPaneli` trait'ini uygulayacak ve merkezi navigasyona dinamik kaydolacaktır.
   - Sayfalar HTTP üzerinden değil, doğrudan Kervan'ın yerel servis katmanına iç çağrıyla (`in-memory`) bağlanacaktır.
2. **Windows Render Kuralları (§6-4):**
   - Renderer: `eframe::Renderer::Wgpu` (DirectX 12).
   - `transparent = false`, `multisampling = 0`, `depth_buffer = 0`, `vsync = true`.
   - OpenGL/glow Windows'ta kesinlikle kullanılmayacaktır.
3. **Tema ve Taban:** Ortak tema olarak `kilim-tema`, pencere tabanı olarak `pencere-egui` (tepsi ve dock desenleri) entegre edilecektir.
4. **Görsel Kanıt:** Her panel ekran görüntüsü kanıtıyla (`port/kanit/ui/<panel>.png`) doğrulanacaktır.
