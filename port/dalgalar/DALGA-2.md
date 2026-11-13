# PRD: Dalga 2: Veritabanı Kalıcılığı ve SQLite/Drizzle Modelleri

## 1. Problem Statement & Scope
OmniRoute dbe703a anlık görüntüsünden: 193 db_modul_sayisi, 252 goc_dosyasi_sayisi; pano eşlemesinde 10 bölüm D2 (DALGA-2) olarak işaretli; defterde (tüm dalgalar ortak) açık pencere 8 kayıtta 2.092 satır.
Bu taslak, defterdeki açık pencereler kapatılmadan başlamaz; kapsam ve sayılar her koşuda
port/envanter.json, crates/pano-ekran/esleme.toml ve port/defter/*.jsonl dosyalarından yeniden türetilir.

## 2. Goals and Non-Goals
### 2.1 Goals
- G1: 193 db_modul_sayisi ve 252 goc_dosyasi_sayisi kaleminin Kervan karşılığına bağlanması
- G2: 10 pano bölümünün D2 rozetinden gerçek kaynağa geçirilmesi
- G3: Açık defter pencerelerinin (8 kayıt) bu dalga içinde kapatılması

### 2.2 Non-Goals
- Kapsam dışı bırakılmış 7 defter kaydının bu dalgaya dahil edilmesi
- İnsan kararı bekleyen 3 defter kaydının bu dalgada karara bağlanması

## 3. Users and Personas
- **Ercan**: Dalga kapsamının ve varsayımların onayı
- **Builder**: Aktarımı yapan ajan
- **Hakem**: Kapanan pencerelerin kanıtını denetleyen bağımsız değerlendirici

## 4. Functional Requirements
### FR-2.1: Şema Göç Motoru
OmniRoute dbe703a anlık görüntüsünden: 193 db_modul_sayisi, 252 goc_dosyasi_sayisi; pano eşlemesinde 10 bölüm D2 (DALGA-2) olarak işaretli; defterde (tüm dalgalar ortak) açık pencere 8 kayıtta 2.092 satır.
*Acceptance:* Envanterdeki 193 db_modul_sayisi ve 252 goc_dosyasi_sayisi kalemi için karşılaştırma testi yazılır; test, düzeltme geri alındığında kırmızıya döner.

## 5. Türetim İzi
Girdiler: port/dalgalar/dalga-tanimi.json (kimlik) · port/envanter.json (kaynak_sha dbe703a, tarih 2026-10-03) · crates/pano-ekran/esleme.toml (D2: 10 bölüm) · port/defter/dosyalar.jsonl (19 kayıt) · port/defter/davranislar.jsonl (7 kayıt) · port/durum.json (faz P3).

Açık defter pencereleri:
- DOS-1 · ref/omniroute-dbe703a/open-sse/translator/request/openai-to-claude.ts [1,300] → crates/ceviri/src/claude.rs (aktarildi)
- DOS-6 · ref/omniroute-dbe703a/open-sse/translator/request/openai-to-claude.ts [301,600] → crates/ceviri/src/claude.rs (planlandi)
- DOS-6-2 · ref/omniroute-dbe703a/open-sse/translator/request/openai-to-claude.ts [601,844] → crates/ceviri/src/claude.rs (planlandi)
- DOS-2 · ref/omniroute-dbe703a/open-sse/translator/request/openai-to-gemini.ts [1,300] → crates/ceviri/src/gemini.rs (aktarildi)
- DOS-7 · ref/omniroute-dbe703a/open-sse/translator/request/openai-to-gemini.ts [301,600] → crates/ceviri/src/gemini.rs (planlandi)
- DOS-7-2 · ref/omniroute-dbe703a/open-sse/translator/request/openai-to-gemini.ts [601,900] → crates/ceviri/src/gemini.rs (planlandi)
- DOS-7-3 · ref/omniroute-dbe703a/open-sse/translator/request/openai-to-gemini.ts [901,948] → crates/ceviri/src/gemini.rs (planlandi)
- DOS-9 · ref/omniroute-dbe703a/open-sse/translator/request/claude-to-openai.ts [1,300] → crates/ceviri/src/claude.rs (aktarildi)
