# キャラバン (Kervan)

WebベースのAI利用を選択したインターフェースにルーティングするゲートウェイ基盤。Rust製、単一バイナリ。

> 多くの道を通って荷物を運び、それぞれの道がどれだけ耐えられるかを知っている。

[**Türkçe**](README.md#türkçe) | [**English**](README.en.md#english) | [**العربية**](README.ar.md#العربية) | [**日本語**](#日本語)

---

## 日本語

### 何をするか

APIキーのないAIアカウント（Gemini、Claude、PerplexityのWebインターフェース）を、単一のOpenAI互換エンドポイントに変換します。Cookieをコピーするのではなく、**実際のブラウザ**を駆動します：一度ログインすれば、セッションはプロファイルディレクトリに永続化されます。

既存のソリューションとの違いは3点：

- **アカウントごとの真の分離** — 各アカウントが独自のブラウザプロファイルを持つ。シークレットモードでは不十分：同じブラウザインスタンスのすべてのシークレットウィンドウは単一のプロファイルを共有する。
- **永続セッション** — Cookieの貼り付けなし、ローテーション時の更新なし。
- **人間のゲート** — 検証ウォールに遭遇しても黙って失敗しない；アカウントを一時停止し、ウィンドウを前面に表示し、ユーザーが解決できるようにする。ボット検出の回避は試みない。

### ステータス

フェーズ3（プールモード）が動作中：複数プロバイダー、**複数アカウント**、レート予算、人間ゲート。

| | |
|---|---|
| ブラウザ | Firefoxベース（ZenまたはFirefox）、geckodriver経由 |
| エンドポイント | `POST /v1/chat/completions`、`GET /v1/models`、`GET /health` |
| プール | `GET /accounts`、`POST /accounts/{i}/devam` |
| バインド | ローカルホストのみ（`127.0.0.1`） |

### プロバイダー

定義済み：`gemini`、`kimi`、`copilot`、`copilot-m365`、`perplexity`、`claude`、`deepseek`、`qwen`、`z.ai`。各アカウントは`kervan.toml`で1つ以上のプロバイダーにリンクされる。

#### プロバイダーステータス

| プロバイダー | ステータス |
|---|---|
| `gemini` | ✅ プール内 — 複数アカウント、約11秒 |
| `kimi` | ✅ プール内 — 複数アカウント、25–36秒 |
| `copilot` | ✅ プール内 — 個人Microsoftアカウント、約12秒 |
| `copilot-m365` | ✅ プール内 — 職場/学校アカウント、約14秒 |
| `perplexity` | ⚠️ 不安定 — 一部アカウントでCloudflare |
| `claude` | ❌ Cloudflare — プールで無効 |
| `deepseek` | ❌ Cloudflare — ほとんどのアカウントで |
| `qwen` | ❌ Cloudflare — すべてのアカウントで |
| `z.ai` | ❌ Cloudflare — 誤解を招く空の応答 |

### クォータ学習機能

Webプロバイダーは残りクォータを公開しない。Kervanは各リクエストの結果を`%LOCALAPPDATA%\kervan\kota.jsonl`に記録し、制限到達の瞬間をマークする。

```
curl http://127.0.0.1:20129/kota
```

- **捏造なし。** 観測された制限到達なしに推定値を生成しない。
- 制限メッセージは**応答として返らない** — クライアントは502を受け取る。

### 要件

- Rust 1.97+
- Firefoxベースのブラウザ（Zenサポートあり）
- [geckodriver](https://github.com/mozilla/geckodriver/releases) — `%LOCALAPPDATA%\kervan\bin\geckodriver.exe`またはPATH

### 使用方法

初回：可視ウィンドウでログイン（一度だけ）。

```bash
cargo run -- profile-name --giris
```

その後ゲートウェイとして実行：

```bash
cargo run -- --headless --serve
```

単一クエリモード：

```bash
cargo run -- profile-name --headless --sor "ミマール・シナンとは誰か？"
```

### 設定（`kervan.toml`）

```toml
idle_minutes = 15         # この時間未使用のブラウザを閉じる

[[accounts]]
name = "user@example.com"   # プロファイルディレクトリ = アカウント自体
providers = ["gemini", "perplexity"]

[pacing]
min_delay_ms = 3000       # 同じアカウントへのリクエスト間の最小遅延
jitter_ms = 2000          # その上のランダムジッター
hourly_cap = 30           # アカウントごとの毎時上限
```

### Hermes統合

`~/.hermes/config.yaml`で`custom`プロバイダーとして：

```yaml
  - name: kervan
    base_url: http://127.0.0.1:20129/v1
    api_key: kervan-local
    model: kervan/gemini
    models:
      - kervan/gemini
      - kervan/copilot
```

### セキュリティ

- プロファイルディレクトリはCookieとセッションを含み、`.gitignore`に記載。
- ゲートウェイは`127.0.0.1`にのみバインド。
- Kervan単体ではログインできない — 検証は常に人間を経由。

**行わないこと：** フィンガープリント偽装なし、CAPTCHA解決なし、プロキシ/IPローテーションなし、オートメーション署名隠蔽なし。

### ライセンス

MIT — [LICENSE](LICENSE)を参照。
