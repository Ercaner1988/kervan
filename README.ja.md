**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# キャラバン (Kervan)

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)

WebベースのAI利用を選択したインターフェースにルーティングするゲートウェイ基盤。Rust製、単一バイナリ。

> 多くの道を通り、各道がどれだけの荷を運べるかを知る。

---

## 📦 前提条件

| 要件 | バージョン / 備考 |
|---|---|
| Rust | 1.88+ — ワークスペースは `edition = "2024"` と let-chains を使用。古いツールチェーンではビルドできない |
| Firefox | Zenまたは標準Firefox |
| geckodriver | サーバー起動時に探索：`KERVAN_GECKODRIVER` か `%LOCALAPPDATA%\kervan\bin\geckodriver.exe`（拡張ブリッジのみの構成でも必要） |
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
./target/release/kervan --serve
```

サーバーは`127.0.0.1:20129`で動作する。OpenAI互換エンドポイント：

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gemini", "messages": [{"role": "user", "content": "こんにちは"}]}'
```

実行モード：

| モード | 動作 |
|---|---|
| `--serve` | サーバーを起動（エンドポイント＋拡張ブリッジ）。`--port <n>`でポート変更。 |
| `--giris <アカウント>` | そのアカウントのブラウザウィンドウを開く（初回の手動ログイン用）。 |
| `--tara` | 定義済みアカウントのセッション状態を再スキャン。 |
| （フラグなし） | 単一セッションモード：プロファイルを開きEnterを待つ。サーバーは起動しない。 |

アカウント定義と全フィールド：`kervan.toml.example`。

### 拡張ブリッジ（WebSocket）

ブラウザ拡張は`eklenti/`にあり、WebDriverなしでKervanに接続する：
`ws://127.0.0.1:20129/ws/eklenti`（サーバー起動時には geckodriver の探索を
行う — 前提条件を参照）。拡張は各タブのセッションを`provider:profile`キーで
登録する。プロファイルはFirefoxの`cookieStoreId`、なければ`varsayilan`。

**初回インストール（一度だけ）：**

1. Firefox → `about:debugging#/runtime/this-firefox` → **一時的なアドオンを読み込む…**
   → `kervan/eklenti/manifest.json`を選ぶ。
2. Kervanは`--serve`で起動している必要がある。サーバー停止中は拡張が3秒ごとに
   再接続を試みる。
3. プロバイダーのページ（例：`gemini.google.com`）を開いてログインすると、
   タブが自動的にブリッジへ登録される。

一時読み込みはブラウザ終了時に失われる。常設利用には署名済みXPIパッケージが
必要（Firefox ESRでは`xpinstall.signatures.required`設定と併用）。再読み込み
後は古い登録が外れ、新しい接続が再登録する。

**複数アカウント：** 2つ目のアカウントは、プロバイダーを**2つ目のFirefox
コンテナ**で開くこと（Multi-Account Containers拡張）。同じコンテナ内の2つ目の
タブは別アカウント**ではない** — 後から開いた方が前の登録を置き換える。別
アカウントには別の`cookieStoreId`が必要。選択はアカウント単位の
最小使用量方式で行われる。

ブリッジは識別情報を要求する：

- `Host`は`127.0.0.1`、`localhost`、`[::1]`のみ許可（DNSリバインディング対策）。
- `Origin`は、存在しない場合（ローカルクライアント）または拡張スキーム
  （`chrome-extension://`、`moz-extension://`）なら許可；`http(s)://`の
  ページは拒否。
- 共有トークンは必須。Kervanは起動時にパスを表示する：
  `%LOCALAPPDATA%\kervan\eklenti.token`（Linux/macOSでは
  `~/.local/state/kervan/eklenti.token`、`HOME`も無ければ作業ディレクトリの
  `kervan/eklenti.token`）。拡張は拡張オリジンのみに開放された
  `GET /eklenti/token`からトークンを取得し`storage.local`に保存する —
  ユーザーの追加作業はない。

旧バージョンの拡張はトークンを送らないため接続できない。上記の手順で
`eklenti/`を読み込み直すこと。

### 直接RESTプロバイダー

ブラウザタブを介さず自前のAPIへ接続するプロバイダーは`[rest.<ad>]`で定義する
（`bicim`：`openai`、`claude`、`gemini`）。キーは環境変数から読み、ファイルには
決して書かない。対象は`/v1/models`に`kervan/<ad>`として現れる。詳細と名前衝突の
規則は`kervan.toml.example`にある。

既知の制限：クライアントが切断するとブラウザ経路は生成を止めるが、進行中の
REST呼び出しは取り消せない（ブロッキングHTTPクライアント）。`zaman_asimi`まで
走り続け、結果は破棄される。恒久対策は非同期クライアントへの移行で、意図的に
先送りしている。

### ルーティング

`model: "auto"`は3つの供給源を競わせる：接続中の拡張タブ、ブラウザプール、
`[rest.*]`ターゲット。戦略は`[yonlendirme]`で選び、実行中に
`GET`/`POST /yonlendirme`で読み書きできる。20の名前を受け付けるが、本番で
実際に順序を変えるのは7つ（`weighted`、`round-robin`、`p2c`、`random`、
`strict-random`、`least-used`、`lkgp`）。残り13のうち8は未移植（起動時に警告）、
5は警告なしで無効（`cost-optimized`、`context-optimized` — 価格・文脈の表が
空；`priority`、`fill-first`、`context-relay` — コアが順序に触れず、候補は
もともとアルファベット順で届く）。一覧と根拠は`kervan.toml.example`。
`tools`を含むリクエストでは、REST候補があればブラウザ候補はすべて除外される。

## 🗺 ステータスとロードマップ

| ステータス | 項目 |
|---|---|
| ✅ 完了 | マルチプロバイダー、マルチアカウント、レート予算、人間のゲート |
| 🔬 実験的 | クォータ学習（観測されたクォータ制限） |
| 📅 将来 | `kervan.toml`テンプレート、プロバイダーごとのクォータ学習 |

### 技術詳細

| | |
|---|---|
| ブラウザ | Firefoxベース（ZenまたはFirefox）、geckodriver経由；または`eklenti/`WebSocketブリッジ |
| エンドポイント | `POST /v1/chat/completions`、`GET /v1/models`、`GET /health` |
| プール | `GET /accounts`、`POST /accounts/{i}/devam`、`POST /accounts/{i}/duraklat` |
| ルーティング | `GET /yonlendirme`、`POST /yonlendirme` |
| 拡張 | `GET /ws/eklenti`（トークン＋Origin）、`GET /eklenti/token`、`GET /eklenti/sekmeler` |
| バインディング | ローカルホストのみ（`127.0.0.1`） |

エンドポイントの全登録：`port/uclar.toml`（`/telemetri`、`/bakiye`、`/kimlikler`、
`/kota`、アカウント可視性の各エンドポイントを含む）。

### プロバイダー

定義済み：`gemini`、`kimi`、`copilot`、`copilot-m365`、`perplexity`、`claude`、`deepseek`、`qwen`、`z.ai`。各アカウントは`kervan.toml`で1つ以上のプロバイダーにリンクされる。

#### プロバイダーの状態

<!-- DURUM:BASLA -->
> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json

- Faz: P3 (2026-10-21T10:08:04.363Z)
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