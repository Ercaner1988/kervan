**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# キャラバン (Kervan)

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)

WebベースのAI利用を選択したインターフェースにルーティングするゲートウェイ基盤。Rust製、単一バイナリ。

> 多くの道を通り、各道がどれだけの荷を運べるかを知る。

---

## 📦 前提条件

| 要件 | バージョン / 備考 |
|---|---|
| Rust | 1.70+（ビルド用） |
| Firefox | Zenまたは標準Firefox（geckodriver付き） |
| OS | Windows、Linux、macOS |

## 🛠 機能と特徴

APIキーのないAIアカウント（Gemini、Claude、PerplexityのWebインターフェース）を、単一のOpenAI互換エンドポイントに変換する。Cookieのコピーではなく、**実際のブラウザ**を駆動する：一度ログインすれば、セッションはプロファイルディレクトリに保持される。

既存のソリューションとの違いは3点：

- **アカウントごとの真の分離** — 各アカウントは独自のブラウザプロファイルを持つ。シークレットモードでは不十分：同じブラウザインスタンス内のすべてのシークレットウィンドウは1つのプロファイルを共有する。
- **永続的なセッション** — Cookieの貼り付けなし、Cookieがローテーションしても更新なし。
- **人間のゲート** — 検証壁に当たっても静かに失敗しない；アカウントを一時停止し、ウィンドウを前面に持ってきて、自分で解決できるようにする。ボット検出を回避しようとはしない。

## ⚙ セットアップ

### セットアップ手順

1. リポジトリをクローンする：

```bash
git clone https://github.com/Ercaner1988/kervan.git
cd kervan
```

2. ビルドする：

```bash
cargo build --release
```

3. 設定ファイルを作成する：

```bash
cp kervan.toml.example kervan.toml
```

4. `kervan.toml`でアカウントを定義する。

### 使用方法

```bash
./target/release/kervan
```

サーバーは`127.0.0.1:20129`で動作する。OpenAI互換エンドポイント：

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gemini", "messages": [{"role": "user", "content": "こんにちは"}]}'
```

## 🗺 ステータスとロードマップ

| ステータス | 項目 |
|---|---|
| ✅ 完了 | マルチプロバイダー、マルチアカウント、レート予算、人間のゲート |
| 🔬 実験的 | クォータ学習（観測されたクォータ制限） |
| 📅 将来 | `kervan.toml`テンプレート、プロバイダーごとのクォータ学習 |

### 技術詳細

| | |
|---|---|
| ブラウザ | Firefoxベース（ZenまたはFirefox）、geckodriver経由 |
| エンドポイント | `POST /v1/chat/completions`、`GET /v1/models`、`GET /health` |
| プール | `GET /accounts`、`POST /accounts/{i}/devam` |
| バインディング | ローカルホストのみ（`127.0.0.1`） |

### プロバイダー

定義済み：`gemini`、`kimi`、`copilot`、`copilot-m365`、`perplexity`、`claude`、`deepseek`、`qwen`、`z.ai`。各アカウントは`kervan.toml`で1つ以上のプロバイダーにリンクされる。

#### プロバイダーの状態

<!-- DURUM:BASLA -->
> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json

- Faz: P3 (2026-10-14T05:51:04.955Z)
- Tüm kapılar yeşil mi: hayır (9/11)
- Fazlar: yok
- Defter: planlandi 5, dogrulandi 1; route_ts 730
<!-- DURUM:BITIS -->

##### プロバイダーのメモ (手書き。ゲートは検証しません)

| プロバイダー | 状態 |
|---|---|
| `gemini` | ✅ プール内 — 複数アカウント、~11秒 |
| `kimi` | ✅ プール内 — 複数アカウント、25–36秒 |
| `copilot` | ✅ プール内 — 個人Microsoftアカウント、~12秒 |
| `copilot-m365` | ✅ プール内 — 仕事/学校アカウント、~14秒 |
| `perplexity` | ⚠️ 不安定 — 一部アカウントでCloudflare |
| `claude` | ❌ Cloudflare — プール内で無効 |
| `deepseek` | ❌ Cloudflare — ほとんどのアカウント |
| `qwen` | ❌ Cloudflare — すべてのアカウント |
| `z.ai` | ❌ Cloudflare — 誤解を招く空の応答 |

## ⚠️ 既知の問題 / 実験的機能

- **クォータ学習**：Webプロバイダーはクォータ状態を公開しない。Kervanはすべてのリクエストの結果を`%LOCALAPPDATA%\kervan\kota.jsonl`に記録し、クォータの枯渇をマークする。
- **Cloudflare**：Claude、Deepseek、Qwen、Z.aiはほとんどのアカウントでCloudflareの壁に当たる。
- **人間のゲート**：検証壁に当たるとアカウントを一時停止し、ウィンドウを前面に持ってくる — 自動解決はない。

## 🤝 貢献者

- [Ercan ER](https://github.com/Ercaner1988) — 設計と開発

## 📜 ライセンス

MIT — 詳細は [LICENSE](LICENSE) を参照。