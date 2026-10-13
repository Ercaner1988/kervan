**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# Kervan

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)

A gateway infrastructure that routes AI usage from web interfaces to your preferred endpoints. Rust, single binary.

> Carrying loads across many paths, knowing how much each path can bear.

---

## 📦 Prerequisites

| Requirement | Version / Note |
|---|---|
| Rust | 1.70+ (for building) |
| Firefox | Zen or standard Firefox (with geckodriver) |
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
./target/release/kervan
```

Server runs on `127.0.0.1:20129`. OpenAI-compatible endpoint:

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gemini", "messages": [{"role": "user", "content": "Hello"}]}'
```

## 🗺 Status & Roadmap

| Status | Topic |
|---|---|
| ✅ Done | Multi-provider, multi-account, rate budget, human gate |
| 🔬 Experimental | Quota learning (observed quota limits) |
| 📅 Future | `kervan.toml` templates, per-provider quota learning |

### Technical Details

| | |
|---|---|
| Browser | Firefox-based (Zen or Firefox), via geckodriver |
| Endpoint | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Pool | `GET /accounts`, `POST /accounts/{i}/devam` |
| Binding | localhost only (`127.0.0.1`) |

### Providers

Defined: `gemini`, `kimi`, `copilot`, `copilot-m365`, `perplexity`, `claude`, `deepseek`, `qwen`, `z.ai`. Each account is linked to one or more providers in `kervan.toml`.

#### Provider Status

<!-- DURUM:BASLA -->
> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json

- Faz: P3 (2026-10-13T22:36:05.287Z)
- Tüm kapılar yeşil mi: hayır (8/11)
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