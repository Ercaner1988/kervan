# Kervan

A gateway infrastructure that routes AI usage from web interfaces to your preferred endpoints. Rust, single binary.

> Carrying loads across many paths, knowing how much each path can bear.

[**Türkçe**](README.md#türkçe) | [**English**](#english) | [**العربية**](README.ar.md) | [**日本語**](README.ja.md)

---

## English

### What it does

Transforms AI accounts without API keys (Gemini, Claude, Perplexity web interfaces) into a single OpenAI-compatible endpoint. Instead of copying cookies, it drives a **real browser**: log in once, and the session persists in a profile directory.

Key differences from existing solutions:

- **True per-account isolation** — each account in its own browser profile. Incognito mode is insufficient: all incognito windows in the same browser instance share a single profile.
- **Persistent session** — no cookie pasting, no refreshing when cookies rotate.
- **Human gate** — doesn't silently fail on verification walls; pauses the account, brings the window to the foreground, lets you resolve it. Doesn't attempt to bypass bot detection.

### Status

Phase 3 (pool mode) is working: multiple providers, **multiple accounts**, rate budget, human gate.

| | |
|---|---|
| Browser | Firefox-based (Zen or Firefox), via geckodriver |
| Endpoint | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Pool | `GET /accounts`, `POST /accounts/{i}/devam` |
| Binding | localhost only (`127.0.0.1`) |

### Providers

Defined: `gemini`, `kimi`, `copilot`, `copilot-m365`, `perplexity`, `claude`, `deepseek`, `qwen`, `z.ai`. Each account is linked to one or more providers in `kervan.toml`.

#### Provider Status

| Provider | Status |
|---|---|
| `gemini` | ✅ In pool — multiple accounts, ~11s |
| `kimi` | ✅ In pool — multiple accounts, 25–36s |
| `copilot` | ✅ In pool — personal Microsoft account, ~12s |
| `copilot-m365` | ✅ In pool — work/school account, ~14s |
| `perplexity` | ⚠️ Unstable — Cloudflare on some accounts |
| `claude` | ❌ Cloudflare — disabled in pool |
| `deepseek` | ❌ Cloudflare — on most accounts |
| `qwen` | ❌ Cloudflare — on all accounts |
| `z.ai` | ❌ Cloudflare — misleading empty response |

### Quota Learner

Web providers don't expose remaining quota. Kervan logs each request result to `%LOCALAPPDATA%\kervan\kota.jsonl`, marking limit-hit moments.

```
curl http://127.0.0.1:20129/kota
```

- **No fabrication.** Won't produce estimates without an observed limit hit.
- Limit message **doesn't return as response** — client gets 502.

### Requirements

- Rust 1.97+
- Firefox-based browser (Zen is supported)
- [geckodriver](https://github.com/mozilla/geckodriver/releases) — at `%LOCALAPPDATA%\kervan\bin\geckodriver.exe` or in PATH

### Usage

First time: log in through a visible window (one-time).

```bash
cargo run -- profile-name --giris
```

Then run as a gateway:

```bash
cargo run -- --headless --serve
```

Single query mode:

```bash
cargo run -- profile-name --headless --sor "Who is Mimar Sinan?"
```

### Configuration (`kervan.toml`)

```toml
idle_minutes = 15         # close browser unused for this long

[[accounts]]
name = "user@example.com"   # profile directory = account itself
providers = ["gemini", "perplexity"]

[pacing]
min_delay_ms = 3000       # minimum delay between requests to same account
jitter_ms = 2000          # random jitter on top
hourly_cap = 30           # per-account hourly limit
```

### Hermes Integration

In `~/.hermes/config.yaml` as a `custom` provider:

```yaml
  - name: kervan
    base_url: http://127.0.0.1:20129/v1
    api_key: kervan-local
    model: kervan/gemini
    models:
      - kervan/gemini
      - kervan/copilot
```

### Security

- Profile directories contain cookies and sessions, listed in `.gitignore`.
- Gateway binds only to `127.0.0.1`.
- Kervan cannot log in on its own — verification always goes through a human.

**What it doesn't do:** no fingerprint spoofing, no CAPTCHA solving, no proxy/IP rotation, no automation signature hiding.

### License

MIT — see [LICENSE](LICENSE).
