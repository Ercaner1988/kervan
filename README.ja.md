**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# Kervan（ケルヴァン）

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)
[![Altın Kapı](https://img.shields.io/badge/alt%C4%B1n--kap%C4%B1-16%E6%AC%A1%E5%85%83-brightgreen)](#黄金の門とコード監査規律)

複数のAIプロバイダーを単一のインターフェースに統合する、高性能かつゼロコストのユニバーサルAIゲートウェイおよびルーティングエンジン。純粋なRust、モジュラーワークスペース（20クレート）、ネイティブeguiデスクトップGUI、およびBunツールキットを採用。

> 多様な道から荷を運び、それぞれの道がどれだけの負荷に耐えられるかを熟知する。

---

## 📦 機能とアーキテクチャ

Kervanは、APIキー不要のWebセッション（Gemini、Claude、Perplexityなど）と、276以上の公式・オープンソースAIプロバイダー（OpenAI、Anthropic、Google、DeepSeek、Qwen、Mistral、Ollamaなど）をOpenAI互換のエンドポイントに統合します。

* **276プロバイダーと5つの統一エグゼキュータ:** 標準REST、Claude Messages、Gemini Content、ブラウザセッション、およびOAuth2トークン更新エンジン。
* **19の標準ルーティング戦略:** 優先度、ラウンドロビン、重み付け、最小使用頻度、コスト最適化、コンテキスト最適化、セマンティックキャッシュ最適化、クォータ共有など。
* **3層レジリエンス:** サーキットブレーカー、指数バックオフクールダウン追跡、一時的/永続的モデルロック。
* **双方向フォーマット変換:** OpenAI ↔ Claude ↔ Geminiの双方向フォーマット変換とSSEストリーミングチャンク変換（サブマイクロ秒の遅延）。
* **暗号化ストレージと193のSQLマイグレーション:** AES-256-GCM + scrypt暗号化ローカルSQLite、134テーブルのゴールドスキーマ、`VACUUM INTO`ホットバックアップ対応。
* **A2Aプロトコル（v0.3）:** JSON-RPC 2.0タスク管理（`tasks.create`, `tasks.get`, `tasks.cancel`）およびエージェントカード発見（`/.well-known/agent.json`）。
* **ハードルール#20保護シールド:** オプトインPIIマスキング（デフォルト無効）、認証情報秘匿（`[REDACTED_API_KEY]`）、ヒューリスティックなプロンプトインジェクション防御。
* **モジュラーeguiデスクトップパネル:** ダッシュボード、モデル管理（276プロバイダー検索・フィルタ）、セキュリティパネル。
* **CLI運用ツール:** マイグレーション（ドライラン標準、`--uygula`で本番適用）、ホットバックアップ（`kervan yedekle`）、整合性検査（`kervan dogrula`）。

---

## 🚀 ベンチマーク測定（CodSpeed / Divan）

Kervanのコア処理はサブマイクロ秒からマイクロ秒単位で完了します：

| 処理 | 測定された中央値 | 概要 |
|---|---|---|
| **OpenAI ➔ Claude 変換** | `~384 ns` | リクエストスキーマ正規化およびシステムプロンプト分離 |
| **OpenAI ➔ Gemini 変換** | `~315 ns` | リクエストおよびマルチモーダルコンテンツ変換 |
| **19コンボルーティングソート** | `~799 ns` | ターゲット間のコストおよびコンテキスト優先度ソート |
| **AES-256-GCM + scrypt 暗号化** | `~1.39 µs` | 機密キーおよびトークンの暗号化・復号 |

---

## ⚙ インストールと実行

### 必要環境
* Rust 1.80+（2024 edition）
* Windows, Linux, または macOS

### ビルド

```bash
git clone https://github.com/Ercaner1988/kervan.git
cd kervan
cargo build --release
cargo test --workspace --all-features
```

### 実行

```bash
# バックグラウンドサーバー起動 (標準: 127.0.0.1:20129)
./target/release/kervan

# デスクトップGUIで起動:
cargo run --features gui
```

---

## 🛡 黄金の門とコード監査規律

本プロジェクトは厳格なエンジニアリング規範に準拠しています：
1. **クレート行数上限:** `crates/` 内の全20クレートは例外なく**1,500コード行以下**。
2. **ゼロパニック:** プロダクションコードにおける `unwrap` および `expect` の完全排除。
3. **ループバック保護:** `127.0.0.1` のみでリッスンし、外部ネットワークに露出しない。
4. **エラー情報保護:** 生のデータベースエラーやスタックトレースをクライアントに漏洩させない。

---

## 📄 ライセンス

MIT License © 2026 Ercan ER