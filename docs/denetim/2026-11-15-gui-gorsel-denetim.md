# GUI Görsel Denetimi — 2026-11-15

**Kapsam:** `crates/gui` + `crates/pano-ekran` (6 ekran), `cargo run --features gui -- --serve --gui`
ile gerçekten koşan pencere üzerinde gözle muayene.

**Yöntem:** Pano §3'teki "Masaüstü GUI (egui) ağırlık 8, kısmi %40 — Eksik: bugün koşmuyor,
görsel doğruluk ölçülmedi" iddiasını sınamak için GUI başlatıldı, altı ekran tek tek tıklandı ve
`ffmpeg -f gdigrab` ile yakalandı. Bulgular ekran görüntüsü + kaynak satırı ile eşleştirildi.

**Kapılar (koşuldu):** `cargo build --features gui` yeşil; pencere açıldı (hwnd 23397818,
1600x1000 istemci, `renderer=Wgpu`); 8 hesap yüklendi; `akilli-rota: strateji = [lkgp]`.

---

## Özet

| # | Bulgu | Ağırlık | Durum |
|---|-------|---------|-------|
| G1 | Rota ekranındaki "Gönder ve Test Et" **sahte yanıt** yazıyor | **YÜKSEK** | Açık |
| G2 | Oturum ekranında egui `ScrollArea` kimlik çakışması — kırmızı uyarı metni ekranda | **ORTA** | Açık |
| G3 | "Çalışma Süresi: 0 sn" hiç güncellenmiyor (alan varsayılanda donuk) | ORTA | Açık |
| G4 | İzleme "Başarı Oranı %100.0" — istek 0 iken bile 100 gösteriyor | ORTA | Açık |
| G5 | Ayarlar "Ortam Değişkeninde Tanımlı" — `kimlikler` = hesap adları, anahtar değil | **YÜKSEK** (yanlış iddia) | Açık |
| G6 | Üst şeritteki "⚙" ve "🔍 Tanı al" düğmeleri ölü (`let _ =`) | DÜŞÜK | Açık |
| G7 | "+ Hesap Ekle", "👁 Aç", "↻ Yenile", tanı eylem düğmesi ölü | DÜŞÜK | Açık |
| G8 | Rota ekranı başlığı "Yönlendirme Stratejileri (21)" listelemiyor — yalnız sayı | DÜŞÜK | Açık |

Doğru bulunanlar: 6 sekme gezinmesi, Canlı ekranın hesap/model/araç üç sütunu ve gerçek POST
gönderimi (`crates/gui/src/canli.rs:107-137` — `ureq::post` ile `/v1/chat/completions`, 180 sn
timeout), sol nav seçim durumu, alt şerit, tema seçici, gizli anahtar kuralı (değer yazılmıyor).

---

## G1 — Rota ekranı sahte yanıt üretiyor (YÜKSEK)

`crates/pano-ekran/src/ekran2_rota.rs:92-94`:

```rust
if ui.button("▶ Gönder ve Test Et").clicked() {
    *test_yaniti = "Kervan yanıtı: İstek başarıyla iletildi (200 OK, latency: 1120ms).".to_string();
}
```

Düğme hiçbir ağ isteği atmıyor; sabit bir metin basıyor. Üstelik metin **uydurma ölçüm** içeriyor
("200 OK, latency: 1120ms") — panonun kendi kuralına ("ölçüm değil, değerlendirme" şeridi) aykırı
bir yanlış beyan. Canlı ekranda (`ekran6_canli.rs` + `crates/gui/src/canli.rs:107`) doğru uygulama
zaten var; Rota ekranı ona bağlanmalı ya da düğme kaldırılmalı.

**Düzeltme yönü:** `render_ana_ekran` dönüşü `bool` → tek bir `gonder` kanalı var; Rota ekranını
aynı kanala bağlayıp `test_yaniti` yerine `GonderimDurumu` okutmak. Ya da düğmeyi "henüz yok"
rozetine çevirmek.

## G2 — ScrollArea kimlik çakışması (ORTA)

Ekran görüntüsünde görünen kırmızı metin: `First use of ScrollArea ID 38FB` / `Second use of
ScrollArea ID 38FB` — egui'nin kendi teşhis uyarısı.

`crates/pano-ekran/src/ekran1_oturum.rs` üç `ScrollArea::vertical()` çağırıyor (`:65`, `:99`,
`:168`) ve hiçbirine `id_salt` vermiyor. İki sütun düzeninde (`:31-60`) sol ve sağ sütun aynı karede
çizildiği için egui aynı otomatik kimliği üretiyor.

**Düzeltme:** her birine `.id_salt("oturum_liste")`, `.id_salt("oturum_detay")`, `.id_salt("dar_mod")`
gibi ayrı tuz verilmeli. `ekran6_canli.rs:349` bunu doğru yapıyor (`id_salt("canli_yanit")`) — örnek alınacak yer orası.

## G3 — "Çalışma Süresi: 0 sn" donuk (ORTA)

`crates/pano-ekran/src/ekran5_ayarlar.rs:38` `ozet.saglik.calisma_suresi_sn` yazdırıyor.
Bu alan `crates/pano-veri/src/lib.rs:73` içinde `0` olarak varsayılan; atayan tek bir üretim kodu
YOK (`rg calisma_suresi_sn` → yalnız tanım, varsayılan ve ekran). `crates/gui/src/canli.rs` ozeti
kurarken `saglik` alanına hiç dokunmuyor (`:290-302`, `..Default::default()`).

Sonuç: etiket "Çalışma Zamanı (/health)" diyor ama `/health`'ten hiçbir şey okunmuyor; değer her
zaman 0. **Ya gerçek uptime bağlanmalı ya da satır kaldırılmalı.**

## G4 — "Başarı Oranı %100.0" istek yokken (ORTA)

`crates/pano-ekran/src/ekran3_izleme.rs:31-35`:

```rust
let oran = if ozet.telemetri.toplam_istek > 0 {
    (basarili / toplam) * 100.0
} else {
    100.0
};
```

İstek yokken "%100.0" göstermek yanıltıcı — ekranda "Toplam İstek: 0" ile yan yana duruyor.
`"—"` veya `"%0"` değil, **veri yok** demek doğru olan. Ayrıca `basarili_istek` ve
`ortalama_gecikme_ms` hiçbir yerde atanmıyor (`crates/gui/src/canli.rs:295-299` yalnız
`toplam_istek` ve `aktif_yuvalar` dolduruyor) → "0 başarılı" ve "0 ms" de donuk.

## G5 — Ayarlar anahtar listesi yanlış iddia (YÜKSEK)

`crates/pano-ekran/src/ekran5_ayarlar.rs:55-67` "REST Sağlayıcı Anahtarları ([rest.*])" başlığı
altında `ozet.kimlikler` üzerinde dönüp her birine yeşil "● Ortam Değişkeninde Tanımlı" yazıyor.

Ama `crates/gui/src/canli.rs:300`:

```rust
kimlikler: state.pool.kimlikler.iter().map(|k| k.ad.clone()).collect(),
```

`pool.kimlikler` = **tarayıcı profili/hesap adları** (`crates/havuz/src/pool.rs:250-259`,
`Kimlik { ad: a.name.clone(), oturum: Mutex::new(None) }`), REST anahtarı değil. Yani ekranda
"erercan1988@gmail.com" satırının yanında "Ortam Değişkeninde Tanımlı" yazıyor — anahtarın varlığı
hakkında hiçbir bilgi yok, üstelik "anahtar değerleri asla görüntülenmez" güvenlik notuyla birlikte
okunduğunda anahtarın doğrulandığı izlenimi veriyor.

**Düzeltme:** `kimlikler` alanını REST anahtar adlarıyla doldurmak (`kervan.toml [rest.*]` /
`[providers.*] anahtarlar`), ya da bu kartı hesap listesine çevirip "Ortam Değişkeninde Tanımlı"
etiketi yerine gerçek durumu yazmak.

## G6/G7 — Ölü düğmeler (DÜŞÜK)

- `crates/pano-ekran/src/lib.rs:90` `let _ = ui.button("⚙").clicked();`
- `crates/pano-ekran/src/lib.rs:92` `let _ = ui.button("🔍 Tanı al").clicked();`
- `crates/pano-ekran/src/ekran1_oturum.rs:92-93` "+ Hesap Ekle" → gövde yalnız yorum
  (`// Hesap ekleme diyaloğu`)
- `crates/pano-ekran/src/ekran1_oturum.rs:148-149` `let _ = ui.small_button("👁 Aç").clicked();` /
  `let _ = ui.small_button("↻ Yenile").clicked();`
- `crates/pano-ekran/src/ekran1_oturum.rs:244` `if ui.button(format!("[{}]", tani.oneri.eylem)).clicked() {}` — boş gövde

Kullanıcıya basılabilir görünen ama hiçbir şey yapmayan düğme, "çalışmıyor" izlenimi veriyor.
Ya bağlanmalı ya `add_enabled(false, ...)` ile devre dışı gösterilmeli.

## G8 — Strateji listesi eksik (DÜŞÜK)

`crates/pano-ekran/src/ekran2_rota.rs:28` `*st == ozet.aktif_strateji` karşılaştırması var ama
listeyi ekranda **sayı** olarak gösteriyor: "Yönlendirme Stratejileri (21)" + "OmniRoute
ROUTING_STRATEGY_VALUES ile küme eşittir (20 kamu + 1 dahili)." — 21 maddenin adı çizilmiyor.
Seçim yapılabilir görünen bir liste beklenirken yalnız başlık var.

---

## Pano §3 için önerilen güncelleme

Önce: "ağırlık 8, kısmi %40 — bugün koşmuyor, görsel doğruluk ölçülmedi"
Sonra: **"ağırlık 8, %65 — derleniyor ve açılıyor (ölçüldü 2026-11-15); 6 ekran çiziliyor;
3 ekran gerçek veriye bağlı (Canlı/Oturum/İzleme), Rota ve Ayarlar'da 2 yanlış beyan (G1, G5)"**

Gerekçe: "koşmuyor" iddiası artık yanlış — pencere açıldı, GPGPU çizdi, canlı POST gönderiyor.
Eksilen şey koşma değil, **beyan doğruluğu**: G1 uydurma ölçüm basıyor, G5 anahtar varlığını yanlış
bildiriyor. Bu ikisi kapanmadan %65'in üstüne çıkmamalı; ikisi kapandığında %85 savunulabilir.
