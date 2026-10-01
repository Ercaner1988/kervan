# Sert Kurallar Eşleme Tablosu (OmniRoute 23 Sert Kural ↔ Kervan Rust Karşılığı)

OmniRoute `AGENTS.md` (release/v3.8.52, dbe703a) belgesindeki 23 Sert Kuralın Kervan mimarisine birebir aktarımı ve denetim mekanizması:

| No | OmniRoute Sert Kuralı | Kervan / Rust Karşılığı | Denetim & Test Yolu |
|---|---|---|---|
| **1** | Asla kimlik bilgisi veya sır commit'leme. | `.gitignore` (`kervan.toml`, `.env`, `profiles/`, `*.sqlite`, `*.log`), testlerde yalnız sentetik anahtar. | `kapi` (güvenlik) + CI git denetimi. |
| **2** | `localDb.ts` üzerinden toplu import yasak. | `kervan-depo-*` içinde alan bazlı modüller (`auth`, `quota`, `logs`, `config`). | Modüler crate yapısı ve sınır denetimi. |
| **3** | `eval()` / `new Function()` asla kullanma. | Saf Rust tip sistemi; çalışma zamanı dinamik eval/kod çalıştırma kodu içermez. | `kapi` + Rust derleyici garantisi. |
| **4** | Doğrudan `main` dalına commit yasak. | `omniroute-aktarim` ve konu dalları; yalnız doğrulama sonrası birleştirme. | Git hook ve branch koruması. |
| **5** | Rotalarda doğrudan ham SQL yazma. | `Depo` trait soyutlaması üzerinden tipli sorgu ve metotlar. | `kervan-v1-*` rotalarında SQL import denetimi. |
| **6** | SSE akışlarında hataları sessizce yutma. | `kervan-akis` ve `stream_recovery`: Hatalar SSE hata olayına çevrilir, istemciye iletilir. | `cargo test -p kervan-akis`. |
| **7** | Girdileri daima şema ile doğrula (Zod). | `serde` ve tipli DTO'lar; her HTTP gövdesi için `Deserialize` ve sınır doğrulaması. | API gövde doğrulama testleri. |
| **8** | Üretim kodu değiştiğinde daima test ekle. | Her crate için `%100` birim test ve parite testi zorunluluğu. | `cargo test --workspace --all-features`. |
| **9** | Test kapsamı gerileyemez (asgari %60 taban). | altin-kapi test gerçekliği boyutu ve CodSpeed kıyaslamaları. | `kapi` taban denetimi. |
| **10** | Kanca atlatma (`--no-verify`) yasak. | `kapi` pre-push ve commit denetimleri zorunlu; `--no-verify` kullanılmaz. | Kervan `.githooks/pre-push`. |
| **11** | Halka açık OAuth anahtarlarını düz metin gömme. | `resolve_public_cred` yardımcı fonksiyonu / ortam değişkeni / çalışma zamanı çözümleme. | Kimlik tarama testleri. |
| **12** | İstemciye ham `err.stack` / `err.message` dönme. | `hata_temizle` (`sanitize_error_message`): İç hata ayrıntıları gizlenir, güvenli mesaj döner. | Hata gövdesi temizleme birim testi. |
| **13** | Komut argümanlarını shell string olarak birleştirme. | `std::process::Command::new(prog).args([a1, a2])` kullanımı; asla `cmd /c "string"` yok. | `kervan-tarayici` ve süreç yöneticisi. |
| **14** | Güvenlik uyarılarını gerekçesiz kapatma. | Her güvenlik bulgusu teknik gerekçesiyle ADR'ye veya sapma defterine işlenir. | `port/defter/sapmalar.jsonl`. |
| **15** | Çocuk süreç başlatan rotaları ağa açma. | `non_loopback_guard`: Sadece `127.0.0.1` loopback bağlantılarına izin verilir. | `tests/loopback_guard_test.rs`. |
| **16** | Commit/PR mesajlarına yapay zeka botu atfetme. | Tüm commit'ler doğrudan Ercan kimliğiyle; bot/AI trailer veya footer eklenmez. | `git log` denetimi. |
| **17** | `/api/services/` rotalarını ağa açma. | Servis ve süreç yönetimi rotaları loopback-only olarak kilitlenir. | Loopback rota koruma testi. |
| **18** | Hata düzeltmelerinde TDD veya kanıtlı test şart. | Önce başarısız test -> düzeltme -> yeşil test zinciri. | Git commit geçmişi ve test dosyası. |
| **19** | Paylaşımlı checkout üzerinde doğrudan geliştirme yapma. | `.w/<ajan>` worktree'leri ve ayrık geliştirme dalları. | `.gitignore`'da `/.w` koruması. |
| **20** | PII maskeleme varsayılan olarak KAPALI (opt-in). | `pii_maskeleme_aktif: false`; kullanıcı açıkça açmadıkça veri değiştirilmez. | `test_pii_varsayilan_kapali`. |
| **21** | Sürüm dondurma (release-freeze) protokolüne uy. | Sürüm etiketleri ve dal dondurma kurallarına riayet edilir. | `port/defter/SABIT.md`. |
| **22** | Asla `git stash` / `git stash pop` yapma. | Birden fazla ajanın çalıştığı depoda stash yasaktır; `git show`/`git diff` kullanılır. | Süreç protokolü. |
| **23** | Çalışma artefaktlarını depoya bulaştırma. | Test ve çalışma çıktıları `port/kanit/` ve `port/defter/` altına düzenli kaydedilir. | Defter ve git durum kontrolü. |
