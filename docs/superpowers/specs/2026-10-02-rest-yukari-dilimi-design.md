# REST yukarı akış dilimi — tasarım

Tarih: 2026-10-02 · Durum: **taslak, onay bekliyor** · Kapsam: OmniRoute → Kervan aktarımının ilk dürüst dikey dilimi

## 1. Amaç

Kervan'ın çalışan `/v1/chat/completions` ucuna, tarayıcı oturumunun yanında **API anahtarlı REST sağlayıcı** türü eklemek: istek gerçekten bir sağlayıcıya HTTP ile gider, yanıt gerçekten ondan döner, çeviri OmniRoute'un kendi çıktısıyla karşılaştırılır.

Kullanıcının söyledikleri:
- Antigravity/Flash teslimi büyük ölçüde cepheydi (sabit metin dönen geçit, kendini doğrulayan parite, ilerlemeyen defter). Dal `omniroute-aktarim` `main`'e girmez.
- Seçenek 1 seçildi: dikey dilim. HTTP istemcisi: `ureq`.

Varsayımlar (itiraz edilebilir):
- "Çalışıyor" ölçütü: gerçek ikili/işleyici + gerçek HTTP + gerçek yukarı akış + gerçek altın. Aşağıdakilerden biri eksikse yeşil sayılmaz.
- İlk dilim akışsız (non-stream) çalışır. Gerçek SSE ikinci dilim.
- `main.rs`'in tarayıcı yolu davranışı değişmez.

## 2. Mevcut durum (kod okunarak doğrulandı)

| Bulgu | Yer |
|---|---|
| Çalışan geçit `chat` işleyicisi: sağlayıcı seç → eklenti → havuz (tarayıcı). REST yok. | `crates/sunucu/src/server.rs:452` |
| `ChatRequest` yalnız `model, messages, stream, tools` alır; `temperature`, `max_tokens`, `tool_call_id` vb. atılır. | `server.rs:352` |
| `kervan-v1` bağlı değil; `/v1/chat/completions` sabit metin döner, `/v1/models` OpenAI model adlarını (gpt-4o…) uydurur. | `crates/v1/src/sohbet.rs:135`, `modeller.rs` |
| `kervan-yurutucu` ağa çıkmaz ve konuşmayı **tek `istem` dizisine** düzleştirir (çok turlu konuşma, system, araçlar kaybolur). | `crates/yurutucu/src/modeller.rs`, `rest.rs` |
| `kervan-ceviri` tam OpenAI ↔ Claude/Gemini istek/yanıt çevirisini yazılı `OpenAiIstek` üzerinden yapar; `tool_calls` hep `None`, içerik yalnız `String`. | `crates/ceviri/src/{claude,gemini}.rs` |
| `kervan-direnc`: `DevreKesici` (`&mut self`), `SogumaTakipcisi`, `ModelKilidi`. | `crates/direnc/src/` |
| `Cargo.lock`'ta HTTP istemcisi yok (yalnız `hyper`/`hyper-util`). `ureq` 3.4.2: varsayılan `rustls`(+`ring`) + `gzip`. | `Cargo.lock` |
| `sunucu` ≈1236 kod satırı (sınır 1500). | sayım |
| `AppState` testte kurulabilir: `Pool::new` boş hesap listesiyle, `Ortam` yalnız `PathBuf`, `Defter::ac(yol)`, `Pacing::default()`; ayrıca yalnız `src/main.rs:133` kurar. | `pool.rs:230`, `kota.rs:85` |
| `Config`'te `deny_unknown_fields` yok: yeni `[rest.*]` tablosu geriye uyumlu. | `tarayici/src/config.rs` |
| OmniRoute çeviricisi `genel\OmniRoute` (translator dizini `dbe703a`/v3.8.52 ile aynı) içinde; ama `node_modules` 116/116 sembolik bağı **kırık** (09-29 taşıması, eski mutlak yol). `.bun` deposu yerinde. | `genel\OmniRoute\node_modules` |

## 3. Kapsam

**Dahil**
1. `kervan.toml` içinde `[rest.<ad>]` sağlayıcı tablosu.
2. Yeni crate `kervan-yukari`: gerçek HTTP çağrısı + çeviri + dayanıklılık.
3. `chat` içinde REST dalı (en fazla ~40 net satır).
4. `/v1/models`'e REST sağlayıcıları eklenir.
5. `kervan-v1`'in sahte işleyicileri (sohbet, yerleştirmeler, modeller) silinir.
6. Testler: uçtan uca (gerçek HTTP, sahte yukarı akış), altın parite, mutasyon.
7. `port/DURUM.md` gerçek duruma göre yeniden yazılır.

**Hariç (sonraki dilimler)**
- Gerçek SSE akışı (dilim 2), yönlendirme stratejilerinin REST'e bağlanması ve 20 strateji düzeltmesi (`fusion`/`pipeline` dahil; `random≡strict-random`, `reset-aware≡reset-window` kopyaları) (dilim 3), OAuth/belirteç yenileme, `auto` adaylarına REST katılması, embeddings, Turso, panel, README düzeltmesi (silinen "Sağlayıcı Durumu"/"Bilinen Sorunlar" bölümlerinin geri getirilmesi ayrı iş).
- Claude/Gemini için araç çağrısı ve çok parçalı (görsel) içerik çevirisi: `ceviri` desteklemiyor; **sessizce düşürülmez, açık hata döner** (bkz. 4.4).

## 4. Tasarım

### 4.1 Yapılandırma

```toml
[rest.deepseek]
bicim = "openai"                     # openai | claude | gemini
taban_url = "https://api.deepseek.com"
anahtar_env = "DEEPSEEK_API_KEY"     # anahtar dosyaya YAZILMAZ
model = "deepseek-chat"              # istemci model vermezse yukarı akışa giden ad
# yol = "/v1/chat/completions"       # isteğe bağlı; biçime göre varsayılan
```

- İstemci model kimliği: `kervan/deepseek` (yapılandırmadaki `model`) ya da `kervan/deepseek/<yukarı-akış-modeli>`.
- `RestAyar` tanımı `tarayici/src/config.rs`'e (≈25 satır); `kervan-yukari` bu tipe bağımlı değildir, düz değer alır.
- Varsayılan yollar: openai `/v1/chat/completions`, claude `/v1/messages` (`x-api-key`, `anthropic-version: 2023-06-01`), gemini `/v1beta/models/{model}:generateContent` (`x-goog-api-key`), openai `Authorization: Bearer`.

### 4.2 `kervan-yukari`

```text
Hedef { bicim, taban_url, yol, anahtar_env, model, zaman_asimi }
Yukari { ajan: ureq::Agent, kesiciler: Mutex<HashMap<ad, DevreKesici>>, soguma: Mutex<SogumaTakipcisi> }
async fn Yukari::gonder(ad, &Hedef, govde: &Value) -> Result<Value, YukariHatasi>
```

- **Girdi**: istemcinin OpenAI-biçimli gövdesi (`serde_json::Value`, hiçbir alan atılmadan). **Çıktı**: OpenAI-biçimli yanıt.
- `openai`: gövde olduğu gibi geçer, yalnız `model` yukarı akış adıyla değişir, `stream=false` zorlanır.
- `claude`/`gemini`: `ceviri::openai_den_*_istegi` → POST → `*_dan_openai_yaniti`. `ceviri` bir alanı taşıyamıyorsa (araç, çok parçalı içerik) `YukariHatasi::Desteklenmiyor(alan)`.
- ureq bloklayıcıdır: çağrı `tokio::task::spawn_blocking` içinde; tek paylaşılan `Agent` (bağlantı havuzu), bağlanma 10 sn, toplam `AppState.timeout`, `http_status_as_error(false)`, gövde üst sınırı 8 MB.
- **Anahtar**: ortam değişkeninden çağrı anında okunur; günlüğe/hata metnine asla girmez. Yoksa `YukariHatasi::AnahtarYok(env_adı)` (değer değil ad).
- **Dayanıklılık (gerçek bağlama)**: çağrıdan önce `DevreKesici::istek_izni_al`; ağ hatası/zaman aşımı/5xx → `hata_kaydet`, 2xx → `basari_kaydet`; 4xx (429 hariç) kesiciyi etkilemez. 429 → `SogumaTakipcisi` (Retry-After varsa o); soğuma süresince istek yukarı akışa **gitmeden** 429 + `Retry-After` döner.
- **Hata temizliği** (OmniRoute Hard Rule 12): istemciye yukarı akış durumu + kısaltılmış gövde gider; anahtar, başlık ve iç yol sızmaz.

### 4.3 `chat` kancası (`server.rs`)

- `chat`, `Json<ChatRequest>` yerine `Json<serde_json::Value>` alır; `ChatRequest` `from_value` ile üretilir (tarayıcı yolu aynı), ham gövde REST yoluna gider. Kayıp alan sorunu böylece kökten kapanır. Davranış farkı: bozuk gövde hata metni değişir (400 kalır).
- Model çözümlemesinde önce `st.rest` bakılır; eşleşirse `Yukari::gonder`, sonuç OpenAI-biçimli yanıt olarak döner. Eşleşmezse mevcut akış aynen sürer.
- `stream=true`: yukarı akış yine akışsız çağrılır, yanıt mevcut `yanit_olustur` ile tek parça SSE'ye sarılır (tarayıcı yolundaki mevcut taviz). Yanıtta `tool_calls` varsa sessiz bozulma yerine `501` + açık metin.
- `AppState`'e tek alan: `rest: Yukari` (+ `rest_hedefleri`). Tek kurucu `main.rs`.
- Sandık: `sunucu` ≤1500 kod satırı korunur; sığmazsa bölünür, büyütülmez.

### 4.4 Sahte kodun temizliği

- `kervan-v1`: `sohbet.rs`, `yerlestirmeler.rs`, `modeller.rs` işleyicileri ve onları "doğrulayan" testler silinir; rota kalmaz. Geri kalan (`guvenlik`, `durum`) bağlanmamıştır; `port/DURUM.md` bunu açıkça "bağlı değil" diye yazar. Crate'in çalışma alanından çıkarılması ayrı karar.
- `Kervan yanıtı: [` sabit metni depoda bulunmamalı (doğrulama: `git grep`).

## 5. Test ve kanıt planı

**T1 — birim (`kervan-yukari`)**: gövde/yanıt çevirisi, anahtar yok, desteklenmeyen alan, hata temizliği.

**T2 — uçtan uca (gerçek HTTP)**
- Sahte yukarı akış: `tokio::net::TcpListener`, elle HTTP/1.1 yanıtı, aldığı isteği kaydeder. Ek bağımlılık yok.
- `AppState` boş havuzla kurulur; `server::router` kısa ömürlü bir yuvada `axum::serve` ile açılır; istemci ureq ile bağlanır.
- **Sahte-kapı engeli**: her koşuda rastgele üretilmiş bir `nonce`, sahte sunucunun yanıt içeriği olur; istemciye dönen yanıt onu içermek zorundadır (sabit metinle geçilemez).
- Doğrulananlar: yukarı akışın aldığı gövde `messages`, `temperature`, `max_tokens`'u kayıpsız taşır; doğru başlık (Bearer / x-api-key / x-goog-api-key); 500 → 502 + kesici sayacı; 429 → soğuma, ikinci istek yukarı akışa gitmez; anahtar yok → 503; zaman aşımı.
- Üç biçim (openai/claude/gemini) için ayrı sahte yanıt.

**T3 — altın parite**
- `araclar/parite/altin-uret.ts` (Bun): OmniRoute'un `openaiToClaudeRequest`, `openaiToGeminiRequest` ve yanıt çevirilerini 6 sabit girdiyle çalıştırıp `port/altin/*.json` yazar (kaynak SHA ve dosya yolu üst bilgide).
- Rust testi aynı girdileri `ceviri` ile çevirir, JSON eşitliğini sınar. Tek tek kayıtlı sapmalar `port/altin/sapmalar.toml`'da gerekçeyle durur; kayıtsız fark = hata.
- **Ön koşul**: `genel\OmniRoute` içinde `bun install` (116 kırık bağ). Canlı OmniRoute'a ve `~\.omniroute`'a dokunulmaz; betik geçici `DATA_DIR` ile koşar.
- **Mutasyon**: `ceviri`de bir alan eşlemesi bozulur → T3 kırmızı olmalı; `server.rs` REST kancası çıkarılır → T2 kırmızı olmalı. İkisinin kırmızı çıktısı `port/DURUM.md`'ye eklenir.

**T4 — canlı (yalnız kullanıcı onayıyla)**: gerçek bir REST sağlayıcıya tek istek; çıktı ekrana yazılır. Anahtarı kullanıcı ortam değişkenine koyar, ben dosyaya/komuta yazmam.

## 6. Başarı ölçütleri

1. `cargo test -p kervan-yukari -p kervan-sunucu` yeşil; T2 nonce testi dahil.
2. Mutasyonlar kırmızı çıktı (T2 ve T3), çıktı kayıtlı.
3. `git grep "Kervan yanıtı: \["` boş.
4. `kapi` (fmt + clippy `-D warnings` + test + baseline'lar) gerçek çıktısıyla yeşil.
5. `port/DURUM.md` doğru: hangi dilim yapıldı, neyin kanıtı var, ne bağlı değil.
6. (İsteğe bağlı) T4 canlı çıktısı.

## 7. Riskler

- **R1 — parite kaynağı**: çeviricinin içe aktarma zinciri (`src/lib/db/*` dahil) Bun'da yalıtılmış koşmayabilir. Yedek: aynı sahte yukarı akışa bağlı, ayrı portlu, geçici `DATA_DIR`'li kaynak-OmniRoute örneğinden kayıt (canlı 20128'e dokunmadan). İlk adım bu yolun spike'ı.
- **R2 — derleme maliyeti**: `rustls`+`ring` C derlemesi ekler; makine yük altında. Tek ağır süreç, `CARGO_BUILD_JOBS=4`, makine sakin değilse beklenir.
- **R3 — bloklayıcı istemci**: akış dilimi geldiğinde ureq okumasını kanala bağlamak gerekir; ilk dilimde sorun değil.
- **R4 — sandık**: `sunucu` 1236 → hedef <1300; aşarsa bölünür.

## 8. Dal ve teslim

- Önerilen dal: `dilim-rest-yukari` (`omniroute-aktarim` ucundan; `ceviri`/`direnc`/`katalog` oradan gelir). `omniroute-aktarim` olduğu gibi `main`'e girmez; bu dilimden `main`'e yalnız gerçekten kanıtlanan parçalar alınır.
- Commit/itme yalnız kullanıcı isteyince.
