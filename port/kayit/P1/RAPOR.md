# Faz P1 Sonu Raporu: Kapı Altyapısı

- **Faz ID:** P1
- **Başlık:** Kapı Altyapısı
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **`araclar/kapilar/hepsi.ts` (FR-1.1):**
   - Kayıtlı kapıları sıralı koşturan, süreç zaman aşımlarını (`timeoutMs`) yakalayan, ham çıktıları `port/kayit/<faz>/<kapi>.log` kütüğüne yazan ve `port/durum.json` durum dosyasını üreten ana orkestratör.
   - Kanıt: [hepsi.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/hepsi.ts), [makine-sakin.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P1/makine-sakin.log), [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P1/kabul-haritasi.log).

2. **`araclar/kapilar/temiz-kopya.ts` (FR-1.2):**
   - İlgili commit'i geçici bir `git worktree` altına (`port/kayit/_gecici/wt_*`) çıkarıp komutları temiz yalıtılmış ortamda çalıştıran, çalışma ağacını (`git status --porcelain`) koruyan ve geçici dizini temizleyen yalıtım denetleyicisi.
   - Kanıt: [temiz-kopya.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/temiz-kopya.ts).

3. **`araclar/kapilar/kabul-haritasi.ts` (FR-1.3):**
   - `port/kabul/<faz>.json` dosyasındaki kabul maddelerinin `cargo test --list` ve `bun test` gerçek test kimlikleri ile örtüştüğünü denetleyen, sahte veya tanımsız test kimliği girildiğinde çıkış kodu `1` veren kapı.
   - Kanıt: [kabul-haritasi.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/kabul-haritasi.ts), [P1.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kabul/P1.json).

4. **`araclar/kapilar/makine-sakin.ts` (FR-1.4):**
   - Sistem boş RAM miktarını ve CPU yükünü ölçerek ağır derleme/test adımları öncesinde kaynak uygunluğunu doğrulayan, eşik aşımlarında çıkış kodu `1` vererek park eden denetleyici.
   - Kanıt: [makine-sakin.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/makine-sakin.ts).

5. **`araclar/kapilar/p1.test.ts` (Birim ve Kabul Test Paketi):**
   - 4 kabul kriterinin tamamını (P1-A1, P1-A2, P1-A3, P1-A4) negatif ve pozitif senaryolarla sınayan 4 test vakası (13 onay).
   - Kanıt: [p1.test.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p1.test.ts).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Komut | Sonuç |
|---|---|---|:---:|
| **P1-A1** | Kasıtlı kırık ve zaman aşan örnek kapılar `hepsi.ts` ile çıkış != 0 verir; `port/kayit` altında ham log oluşur | [p1.test.ts:11](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p1.test.ts#L11) (`p1::a1`) · [makine-sakin.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P1/makine-sakin.log) | ✅ GEÇTİ |
| **P1-A2** | İzlenmeyen dosyaya bağımlı test çalışma dizininde yeşil, temiz kopyada kırmızıdır; çalışma dizinine dokunulmaz | [p1.test.ts:46](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p1.test.ts#L46) (`p1::a2`) · [temiz-kopya.ts:18](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/temiz-kopya.ts#L18) | ✅ GEÇTİ |
| **P1-A3** | Kabul haritasında var olmayan veya boş test kimliği kapıyı kırmızı yapar | [p1.test.ts:66](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p1.test.ts#L66) (`p1::a3`) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P1/kabul-haritasi.log) | ✅ GEÇTİ |
| **P1-A4** | `makine-sakin` eşik dışında çıkış != 0, içinde 0 verir | [p1.test.ts:93](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p1.test.ts#L93) (`p1::a4`) · [makine-sakin.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P1/makine-sakin.log) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [hepsi.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/hepsi.ts) gerçek `child_process.spawn` süreçlerini koşturur; [makine-sakin.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/makine-sakin.ts) gerçek `os.freemem()` ve CPU sayaçlarını okur; sabit simüle değer dönülmez.
2. **Kendini doğrulayan kontrol:** Baktım: P1-A2 testi gerçek bir izlenmeyen dosya yazarak `git worktree add` ile açılan temiz kopyanın bu dosyayı görmediğini ve `calismaDiziniDegistiMi` durumunun `false` kaldığını doğrulamıştır.
3. **Ulaşılamayan teslim:** Baktım: Bu fazda Rust crate'i yazılmamış olup, yazılan 4 Bun betiği doğrudan `bun araclar/kapilar/hepsi.ts` üzerinden yürütülmektedir.
4. **Kanıtı aşan rapor:** Baktım: Rapordaki tüm süreler ve çıkış kodları doğrudan `port/durum.json` ve `p1.test.ts` çalışma çıktılarından alınmıştır.
5. **Gevşetme:** Baktım: Mühür henüz P3'te yazılacaktır; hiçbir kapı gevşetilmemiştir.
6. **Başarısız olamayan test:** Baktım: P1-A1 (çıkış 42 ve zaman aşımı 124), P1-A2 (temiz kopyada çıkış 1), P1-A3 (sahte ve boş kimlikte red) ve P1-A4 (9999 GB RAM eşiğinde red) negatif testleriyle başarısız olma kabiliyeti kanıtlanmıştır.

---

## 4. Ertelenenler (Deferrals)

- P2 fazı kapıları: `ulasilabilirlik.ts`, `sabit-yanit.ts`, `uc-kaydi.ts`, `muhur-kontrol.ts`, `rapor-kontrol.ts` (Plan gereği P2'de yazılacaktır).
- Mühürleme dosyası (`port/muhur.toml`): Plan gereği P3'te Ercan'ın kapısında üretilecektir.
