**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# Kervan

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)

A gateway infrastructure that routes AI usage from web interfaces to your preferred endpoints. Rust, single binary.

> Carrying loads across many paths, knowing how much each path can bear.

---

## 📦 Prerequisites

| Requirement | Version / Note |
|---|---|
| Rust | 1.88+ — the workspace uses `edition = "2024"` and let-chains; older toolchains cannot build it |
| Firefox | Zen or standard Firefox |
| geckodriver | Looked up at server start: `KERVAN_GECKODRIVER` or `%LOCALAPPDATA%\kervan\bin\geckodriver.exe` (including extension-bridge-only setups) |
| OS | Windows, Linux, macOS |

## 🛠 Features & Functionality

Transforms AI accounts without API keys (Gemini, Claude, Perplexity web interfaces) into a single OpenAI-compatible endpoint. Instead of copying cookies, it drives a **real browser**: log in once, and the session persists in a profile directory.

Key differences from existing solutions:

- **True per-account isolation** — each account in its own browser profile. Incognito mode is insufficient: all incognito windows in the same browser instance share a single profile.
- **Persistent session** — no cookie pasting, no refreshing when cookies rotate.
- **Human gate** — doesn't silently fail on verification walls; pauses the account, brings the window to the foreground, lets you resolve it. Doesn't attempt to bypass bot detection.

## ⚙ Installation

### Installation Steps

1. Clone the repository:

```bash
git clone https://github.com/Ercaner1988/kervan.git
cd kervan
```

2. Build:

```bash
cargo build --release
```

3. Create the config file:

```bash
cp kervan.toml.example kervan.toml
```

4. Define your accounts in `kervan.toml`.

### Usage

```bash
./target/release/kervan --serve
```

Server runs on `127.0.0.1:20129`. OpenAI-compatible endpoint:

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gemini", "messages": [{"role": "user", "content": "Hello"}]}'
```

Run modes:

| Mode | What it does |
|---|---|
| `--serve` | Starts the server (endpoint + extension bridge). `--port <n>` changes the port. |
| `--giris <account>` | Opens the account's browser window for a one-time manual login. |
| `--tara` | Re-scans the session state of the defined accounts. |
| (no flag) | Single-session mode: opens the profile and waits for Enter — does not start the server. |

Account definitions and every field: `kervan.toml.example`.

### Extension bridge (WebSocket)

The browser extension lives under `eklenti/` and connects to Kervan without
WebDriver: `ws://127.0.0.1:20129/ws/eklenti` (the server still looks for
geckodriver at startup — see Prerequisites). The extension registers each tab's
session under a `provider:profile` key; the profile is Firefox's
`cookieStoreId`, or `varsayilan` if absent.

**First-time install (once):**

1. Firefox → `about:debugging#/runtime/this-firefox` → **Load Temporary Add-on…**
   → pick `kervan/eklenti/manifest.json`.
2. Kervan must be running with `--serve`; while the server is down the extension
   retries every 3 seconds.
3. Open a provider page (e.g. `gemini.google.com`) and sign in — the tab
   registers with the bridge automatically.

Temporary loading is dropped when the browser closes; persistent use requires a
signed XPI package (with `xpinstall.signatures.required` for Firefox ESR).
After a reload, old registrations drop and the new connection re-registers.

**Multiple accounts:** a second account means opening the provider in a
**second Firefox container** (Multi-Account Containers extension). A second tab
in the same container is **not** a separate account — the most recent one
replaces the previous registration; a distinct `cookieStoreId` is required.
Selection is least-used at the account level.

The bridge now requires identity:

- `Host` may only be `127.0.0.1`, `localhost` or `[::1]` (against DNS rebinding).
- `Origin` is accepted when absent (local client) or when it is an extension
  scheme (`chrome-extension://`, `moz-extension://`); `http(s)://` pages are rejected.
- A shared token is mandatory. Kervan prints its path at startup:
  `%LOCALAPPDATA%\kervan\eklenti.token` (on Linux/macOS
  `~/.local/state/kervan/eklenti.token`; if `HOME` is also absent, `kervan/eklenti.token`
  in the working directory). The extension fetches the token from
  `GET /eklenti/token`, which only answers extension origins, and caches it in
  `storage.local` — no user action needed.

An older extension build cannot connect (it sends no token); reload `eklenti/`
following the steps above.

### Direct REST providers

A provider that should hit its own API without a browser tab is defined with
`[rest.<ad>]` (`bicim`: `openai`, `claude`, `gemini`). The key is read from an
environment variable; it is never written to the file, and the target appears in
`/v1/models` as `kervan/<ad>`. Details and name-collision rules are in
`kervan.toml.example`.

Known limitation: when the client disconnects, the browser path stops producing,
but an in-flight REST call cannot be cancelled (blocking HTTP client); it runs
until `zaman_asimi` and its result is discarded. The permanent fix is an async
client; deliberately deferred.

### Routing

`model: "auto"` races three sources: connected extension tabs, the browser pool
and `[rest.*]` targets. The strategy is selected via `[yonlendirme]`, read and
changed at runtime with `GET`/`POST /yonlendirme`. 20 names are accepted; only 7
actually reorder in production (`weighted`, `round-robin`, `p2c`, `random`,
`strict-random`, `least-used`, `lkgp`). Of the other 13: 8 are not yet ported
(warning at startup), 5 are silent no-ops (`cost-optimized`,
`context-optimized` — price/context tables are empty; `priority`, `fill-first`,
`context-relay` — the core does not touch order, candidates already arrive
alphabetically). List and rationale: `kervan.toml.example`. For requests that
carry `tools`, browser candidates are dropped whenever a REST candidate exists.

## 🗺 Status & Roadmap

| Status | Topic |
|---|---|
| ✅ Done | Multi-provider, multi-account, rate budget, human gate |
| 🔬 Experimental | Quota learning (observed quota limits) |
| 📅 Future | `kervan.toml` templates, per-provider quota learning |

### Technical Details

| | |
|---|---|
| Browser | Firefox-based (Zen or Firefox), via geckodriver; or the `eklenti/` WebSocket bridge |
| Endpoint | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Pool | `GET /accounts`, `POST /accounts/{i}/devam`, `POST /accounts/{i}/duraklat` |
| Routing | `GET /yonlendirme`, `POST /yonlendirme` |
| Extension | `GET /ws/eklenti` (token + Origin), `GET /eklenti/token`, `GET /eklenti/sekmeler` |
| Binding | localhost only (`127.0.0.1`) |

Full endpoint registry: `port/uclar.toml` (including `/telemetri`, `/bakiye`,
`/kimlikler`, `/kota` and account-visibility endpoints).

### Providers

Defined: `gemini`, `kimi`, `copilot`, `copilot-m365`, `perplexity`, `claude`, `deepseek`, `qwen`, `z.ai`. Each account is linked to one or more providers in `kervan.toml`.

#### Provider Status

<!-- DURUM:BASLA -->
> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json

- Faz: P3 (2026-10-22T12:26:54.082Z)
- Tüm kapılar yeşil mi: hayır (9/11)
- Fazlar: yok
- Defter: planlandi 5, dogrulandi 1; route_ts 730
<!-- DURUM:BITIS -->

##### Provider notes (hand-written; not verified by a gate)

| Provider | Status |
|---|---|
| `gemini` | ✅ In pool — multiple accounts, ~11s |
| `kimi` | ✅ In pool — multiple accounts, 25–36s |
| `copilot` | ✅ In pool — personal Microsoft account, ~12s |
| `copilot-m365` | ✅ In pool — work/school account, ~14s |
| `perplexity` | ⚠️ Unstable — some accounts hit Cloudflare |
| `claude` | ❌ Cloudflare — disabled in pool |
| `deepseek` | ❌ Cloudflare — most accounts |
| `qwen` | ❌ Cloudflare — all accounts |
| `z.ai` | ❌ Cloudflare — misleading empty responses |

## ⚠️ Known Issues / Experimental Features

- **Quota learning:** Web providers don't expose quota status. Kervan logs every request's outcome to `%LOCALAPPDATA%\kervan\kota.jsonl` and marks quota exhaustion.
- **Cloudflare:** Claude, Deepseek, Qwen, Z.ai hit Cloudflare walls on most accounts.
- **Human gate:** Pauses the account and brings the window to the foreground on verification walls — no automated resolution.

## 🤝 Contributors

- [Ercan ER](https://github.com/Ercaner1988) — design & development

## 📜 License

MIT — see [LICENSE](LICENSE).