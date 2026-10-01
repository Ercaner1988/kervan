# ADR 0001: Crate Topolojisi ve 1.500 Satır Sandık Sınırı

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute'un devasa modüler yapısının (1,1 milyon satır) Kervan'a aktarılırken `altin-kapi` 1.500 kod satırı kuralına uygun bölünmesi.

## Bağlam
OmniRoute; 612 servis, 233 yürütücü, 194 veritabanı modülü, 798 API rotası ve 877 pano dosyasından oluşan devasa bir TypeScript monoreposudur. Bu mimarinin doğrudan devasa tek bir Rust sandığına aktarılması hem derleme sürelerini uzatacak, hem bellek tüketimini artıracak hem de Kervan'ın bağlı olduğu `altin-kapi` anayasasındaki "crate başına en fazla 1.500 kod satırı" kuralını ihlal edecektir.

## Karar
1. **Modüler Crate Topolojisi:** Kervan, sorumlulukları açıkça ayrılmış, her biri en fazla 1.500 kod satırından (`src/` altındaki aktif kod; testler ve yorumlar hariç) oluşan mikro-crate'ler halinde organize edilecektir.
2. **Crate Adlandırma ve Hiyerarşi:**
   - `kervan` (Kök ikili ve CLI komutları)
   - `kervan-cekirdek` / `kervan-v1-*` (HTTP / SSE uçları ve axum servisleri)
   - `kervan-ceviri` (OpenAI ↔ Claude ↔ Gemini ↔ Responses format çeviricileri)
   - `kervan-katalog` (Veri güdümlü sağlayıcı tablosu ve modeller)
   - `kervan-yurutucu-*` (HTTP, WebSocket, Tarayıcı ve Tersine Mühendislik yürütücüleri)
   - `kervan-yonlendirme` (19 Combo stratejisi, akıllı yönlendirici)
   - `kervan-direnc` (3 katmanlı dayanıklılık: Devre kesici, hesap soğuma, model kilidi)
   - `kervan-depo` (`Depo` trait'i, SQLite göçleri, Turso ve rusqlite sürücüleri)
   - `kervan-mcp` / `kervan-a2a` (Model Context Protocol ve A2A protokol katmanı)
   - `kervan-gui` / `kervan-gui-*` (egui 0.36 + WGPU panel kayıt sistemi)
   - `kervan-akis` (Kopan akış kurtarma, oturum havuzu, medya yöneticisi)
3. **Sandık Sınırı Denetimi:** Her fazda `kapi` aracı çalıştırılarak satır limitleri makine tarafından doğrulanacaktır.
