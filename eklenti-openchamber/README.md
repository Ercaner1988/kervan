# Kervan — OpenChamber kontrol paneli

Kervan yerel ağ geçidini (`127.0.0.1:20129`) OpenChamber içinden yöneten
uzantı. OpenCode'un sağlayıcı yolu değişmez: OpenCode yine doğrudan
`http://127.0.0.1:20129/v1` konuşur. Bu uzantı yalnız **kontrol yüzeyi**.

## Ne yapar

| Panel bölümü | Kervan ucu |
|---|---|
| Durum / ölçümler | `/accounts`, `/eklenti/sekmeler`, `/telemetri` |
| Yönlendirme stratejisi | `GET/POST /yonlendirme` |
| Havuz hesabı duraklat / devam | `POST /accounts/{i}/duraklat`, `/devam` |
| Bağlı web sekmeleri | `/eklenti/sekmeler` |
| Kota | `/kota` |

Kervan kapalıyken panel bunu kırmızı noktayla gösterir ve **Başlat** düğmesi
yerel servise Kervan'ı başlattırır; `/health` yeşile dönünce panel kendini
tazeler.

## Kurulum

1. OpenChamber → Settings → Extensions → **Add path** ve bu klasörü seç:
   `...\kervan\eklenti-openchamber`
2. Uzantıya **service** iznini ver (yerel Node shim'i çalıştırmak için).
3. Yan panelde **Kervan** sekmesi görünür.

Kurulum `~/.config/openchamber/extensions.json` içine `paths` + `sources:
path` olarak yazılır; `capabilityGrants.kervan = ["service"]` ve eşleşen
`capabilityScopes` olmadan servis başlamaz.

## İkili (binary) bulma sırası

`KERVAN_BIN` ortam değişkeni → depodaki `target/{release,debug}/kervan(.exe)`
içinde **en son derlenen** → `PATH`'teki `kervan`. Depo kökü uzantı klasörünün
bir üstüdür; `kervan.toml` varsa çocuk sürece `KERVAN_CONFIG` olarak verilir.

> Bayat `release` ikilisi taze `debug`'ın önüne geçmesin diye seçim dosya
> değiştirme zamanına göredir. Üretimde `KERVAN_BIN` ile sabitleyin.

## Yerel servis sözleşmesi

OpenChamber servisi `service/main.js` dosyasını kendi Node'u ile başlatır ve
iki ortam değişkeni verir:

- `OPENCHAMBER_SERVICE_PORT` — yalnız `127.0.0.1` üzerinde dinlenir
- `OPENCHAMBER_SERVICE_TOKEN` — her istekte `Authorization: Bearer …` zorunlu

Uçlar (hepsi token ister):

| Yol | İş |
|---|---|
| `GET /health` | `{ok:true}` — OpenChamber hazırlık denetimi |
| `GET /durum` | Kervan uçlarının birleşik özeti |
| `POST /yonlendirme` | `{strateji}` → Kervan'a proxy |
| `POST /accounts/duraklat` | `{index}` |
| `POST /accounts/devam` | `{index}` |
| `POST /baslat` | Kervan'ı başlatır, `/health` yeşile dönene kadar bekler |

Kervan adresi `KERVAN_URL` ile değiştirilebilir (varsayılan
`http://127.0.0.1:20129`).

## Panel köprüsü

Sandbox iframe ES modülü yükleyemez. Bu yüzden panel, `@openchamber/sdk`
protokolünün yalnız üç mesajını (`hello`, `ready`, `result`) kendi içinde
konuşur: `{channel:"openchamber.sdk", v:1, type, id, payload}`. Yeni bir SDK
sürümü zarfı değiştirirse `panel/main.js` içindeki `KANAL`/`SURUM` ve
`service-request` gövdesi güncellenmelidir.

## Sınırlar

- Uzantı **sağlayıcı kaydedemez**; OpenCode tarafında Kervan, Custom/Other
  OpenAI-uyumlu sağlayıcı olarak tanımlı kalır.
- Panel domain mantığı taşımaz: seçim, kota, duraklatma kuralları Kervan'da.
- Servis yalnız loopback'e bağlanır ve token olmadan yanıt vermez.
