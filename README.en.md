**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# Kervan

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)
[![Altın Kapı](https://img.shields.io/badge/alt%C4%B1n--kap%C4%B1-16%20dimensions-brightgreen)](#altin-kapi-and-code-audit-discipline)

High-performance, zero-cost universal AI gateway and routing engine unifying multiple AI providers into a single interface. Pure Rust, modular workspace (20 crates), native egui desktop UI, and Bun tooling.

> Carrying cargo across multiple roads, knowing exactly how much each road can bear.

---

## 📦 Features and Architecture

Kervan combines both keyless web sessions (Gemini, Claude, Perplexity, etc.) and over 276 official and open-source AI providers (OpenAI, Anthropic, Google, DeepSeek, Qwen, Mistral, Ollama, Groq, vLLM...) behind a single unified OpenAI-compatible endpoint.

* **276 Providers & 5 Unified Executors:** Standard REST, Claude Messages, Gemini Content, Browser Session, and OAuth2 token refresh executors.
* **19 Canonical Routing Strategies:** Priority, Round-Robin, Weighted, Least-Used, Cost-Optimized, Context-Optimized, Semantic Cache-Optimized, Quota-Share, etc.
* **3-Layer Resilience:** Circuit Breaker, Exponential Backoff Cooldown Tracker, and Temporary/Permanent Model Lock.
* **Bidirectional Format Translation:** Zero-loss OpenAI ↔ Claude ↔ Gemini format and SSE streaming chunk transformation with sub-microsecond latency.
* **Encrypted Storage & 193 SQL Migrations:** AES-256-GCM + scrypt encrypted local SQLite database, 134-table gold schema, and `VACUUM INTO` hot backup.
* **A2A Protocol (v0.3):** JSON-RPC 2.0 `tasks.create`, `tasks.get`, `tasks.cancel` and `/.well-known/agent.json` agent card discovery.
* **Hard Rule #20 Protection Shield:** Opt-in PII masking (default OFF), sensitive credential redaction (`[REDACTED_API_KEY]`), and heuristic prompt injection defense.
* **Modular egui Desktop Panels:** Dashboard metrics, Model Management (276 providers search/filter), and Security/PII panel.
* **CLI Operational Tools:** `kervan goc` (dry-run by default; `--uygula` applies changes), `kervan yedekle`, `kervan dogrula`.

---

## 🚀 Performance Benchmarks (CodSpeed / Divan)

Core operations in Kervan execute in sub-microsecond to microsecond durations:

| Operation | Median Latency | Details |
|---|---|---|
| **OpenAI ➔ Claude Translation** | `~384 ns` | Request schema normalization & system prompt extraction |
| **OpenAI ➔ Gemini Translation** | `~315 ns` | Multimodal and text request transformation |
| **19-Strategy Routing Sort** | `~799 ns` | Cost & context-aware ranking across multiple targets |
| **AES-256-GCM + scrypt Crypto** | `~1.39 µs` | Key & token encryption/decryption |

---

## ⚙ Installation and Usage

### Prerequisites
* Rust 1.80+ (2024 edition)
* Windows, Linux, or macOS
* Optional: Firefox or Zen Browser for browser-driven sessions

### Build

```bash
git clone https://github.com/Ercaner1988/kervan.git
cd kervan
cargo build --release
cargo test --workspace --all-features
```

### Run

```bash
# Start background server (default 127.0.0.1:20129)
./target/release/kervan

# Launch with native desktop GUI:
cargo run --features gui
```

### Example Request

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-7-sonnet",
    "messages": [{"role": "user", "content": "Hello Kervan!"}]
  }'
```

---

## 🛡 Altın Kapı and Code Audit Discipline

Kervan strictly adheres to strict engineering constraints:
1. **Crate Size Ceiling:** Every single one of the 20 crates in `crates/` is strictly **≤ 1,500 lines of code**.
2. **Zero Panic:** No `unwrap` or `expect` calls in production code.
3. **Loopback Guard:** Binds strictly to `127.0.0.1`; never exposed directly to external networks.
4. **Sanitized Error Discipline:** Raw database errors or internal stack traces are never leaked to clients.

---

## 📄 License

MIT License © 2026 Ercan ER