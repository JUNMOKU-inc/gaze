# Gaze CLI — 実装設計書

> 作成日: 2026-03-29
> 更新日: 2026-03-31
> ステータス: 実装反映済み
> 前提: ベータリリースの必須機能として CLI を実装する

---

## 1. 背景と目的

Gaze の価値提案は「AIに画面を見せるツール」。GUI (人間→LLM) は実装済みだが、
LLM側から呼び出す手段 (CLI) がないと「AIの目」という位置づけが成立しない。

**ベータで検証したいこと:**
- Claude Code / Cursor / Aider などのターミナルエージェントから `gaze capture` で画面を取得できる
- パイプで他のコマンドと合成できる (`gaze capture | pbcopy`)
- JSON出力でプログラマティックに使える

**ベータでは不要なもの:**
- MCP Server (CLIがあれば Claude Code から直接呼べる)
- OCR (将来の Pro 機能)
- `brew install` (GUI アプリに CLI バイナリを同梱 or 手動 PATH 追加で十分)

---

## 2. アーキテクチャ決定

### 2.1 別バイナリクレート方式を採用

**Cap (Tauri v2 + Rust の先行 OSS) と同じパターン:**

```
Cargo.toml (workspace)
├── crates/
│   ├── snapforge-capture/    # 既存: キャプチャエンジン (CaptureEngine trait)
│   ├── snapforge-pipeline/   # 既存: LLM最適化パイプライン
│   ├── snapforge-core/       # 新規: 共通のキャプチャ処理/メタデータ/出力ユーティリティ
│   └── gaze-cli/             # 新規: CLI バイナリ
│       ├── Cargo.toml
│       ├── build.rs          # macOS Swift runtime の rpath を埋め込む
│       └── src/
│           ├── lib.rs        # clap + コマンド実行ロジック
│           └── main.rs       # 実環境バックエンド
└── src-tauri/                # 既存: Tauri GUI アプリ
```

**`tauri-plugin-cli` を不採用にした理由:**

| 観点 | tauri-plugin-cli | 別バイナリ (clap) |
|------|-----------------|------------------|
| stdout | Tauri イベントループ内で不安定 | 普通の stdout |
| 起動速度 | Tauri ランタイム全体を初期化 | 即起動 |
| CLI 設計 | tauri.conf.json で宣言 (derive 不可) | clap derive (型安全、補完対応) |
| テスト | Tauri コンテキスト必要 | 標準 Rust テスト |
| 終了 | イベントループを止める必要あり | main() return |

### 2.2 コード共有の構造

```
                    ┌──────────────┐
                    │ gaze-cli     │  clap + ランナー
                    │ (新規)        │
                    └──────┬───────┘
                           │
                           ▼
                  ┌────────────────┐
                  │ snapforge-core │  CaptureMetadata /
                  │ (新規)          │  process_* / 出力ヘルパ
                  └──────┬─────────┘
                         │
              ┌──────────┼──────────┐
              ▼                     ▼
    ┌─────────────────┐    ┌──────────────────┐
    │ snapforge-       │    │ snapforge-        │
    │ capture          │    │ pipeline          │
    │ (CaptureEngine)  │    │ (optimize_image)  │
    └─────────────────┘    └──────────────────┘
                         ▲
                         │
                  ┌──────┴───────┐
                  │ src-tauri    │  Tauri + preview + tray
                  │ (既存)       │
                  └──────────────┘
```

**共有される関数 (Tauri依存なし):**
- `snapforge_capture::create_engine()` → `CaptureEngine` trait object
- `snapforge_capture::has_permission()` / `request_permission()`
- `snapforge_capture::CaptureEngine::capture_fullscreen/window/list_displays/list_windows`
- `snapforge_pipeline::optimize_image()` → `OptimizeResult`
- `snapforge_pipeline::LlmProvider::estimate_tokens()`
- `snapforge_core::process_image_bytes()` / `process_rgba_capture()` → 共通の変換・最適化フロー
- `snapforge_core::CaptureMetadata` → GUI/CLI 共通のメタデータ構造
- `snapforge_core::detect_image_format()` / `build_capture_filename()` → 出力処理の共通化

**GUI専用 (CLI では使わない):**
- `src-tauri/src/capture_flow.rs` — `run_screencapture()` (macOS `screencapture` コマンド経由)
- `src-tauri/src/preview_window.rs` — プレビュー表示
- `src-tauri/src/tray.rs` — トレイメニュー
- `src-tauri/src/clipboard.rs` — ここは CLI でも再利用したい (後述)

### 2.3 クリップボードコードの共有

`clipboard.rs` は現在 `src-tauri/` 内にあるが、Tauri に依存していない (arboard のみ)。

**選択肢:**
1. **`crates/snapforge-clipboard/` を新設** — 過剰な分割
2. **`gaze-cli` から直接 `arboard` を使う** — 2関数だけなので重複許容
3. **`clipboard.rs` を `snapforge-pipeline` に移動** — pipeline の責務を超える

→ **選択肢 2 を採用。** CLI 側で `arboard` を直接使う。関数2つ (copy_rgba / copy_encoded) のコピーは不要で、`arboard::Clipboard::new()` + `set_image()` の3行で済む。

---

## 3. CLI インターフェース設計

### 3.1 コマンド体系

```
gaze <COMMAND>

Commands:
  capture     画面をキャプチャして出力
  list        キャプチャ対象を一覧表示
  optimize    既存の画像ファイルをLLM向けに最適化
  version     バージョン情報
```

### 3.2 `gaze capture` (メインコマンド)

```
gaze capture [OPTIONS]

Options:
  -m, --mode <MODE>          キャプチャモード [default: full]
                             full    = フルスクリーン
                             area    = エリア選択 (インタラクティブ)
                             window  = ウィンドウ選択 (インタラクティブ)

  -d, --display <ID>         対象ディスプレイID (fullモード時)
  -w, --window <ID>          対象ウィンドウID (直接指定、インタラクティブ不要)

  -p, --provider <PROVIDER>  LLMプロバイダ [default: claude]
                             claude  = 1568px, WebP
                             gpt     = 2048px, PNG
                             gemini  = 3072px, WebP

  -o, --output <PATH>        ファイル出力先 (省略時はstdoutにJSON)
      --copy                 クリップボードにもコピー
      --raw                  最適化せず生画像を出力
      --pin <X,Y[:NOTE]>     絶対ピクセル指定で Pin を追加（複数指定可）
      --rect <X,Y,W,H[:NOTE]> 絶対ピクセル指定で Rectangle を追加（複数指定可）
  -f, --format <FORMAT>      出力フォーマット [default: json]
                             json    = メタデータ + base64画像
                             path    = ファイルパスのみ (--output と併用)
                             base64  = base64エンコード画像のみ
```

### 3.3 使用例

```bash
# 基本: フルスクリーンキャプチャ → JSON (メタデータ + base64)
gaze capture

# Claude Code から呼ばれる典型パターン
gaze capture --provider claude -o /tmp/screenshot.webp

# ウィンドウ一覧 → 特定ウィンドウをキャプチャ
gaze list windows
gaze capture --window 12345 --provider claude

# エリア選択 (インタラクティブ)
gaze capture --mode area --provider gpt

# パイプ: base64 出力を直接利用
gaze capture --format base64 | ...

# 既存画像をLLM向けに最適化
gaze optimize screenshot.png --provider claude -o optimized.webp

# クリップボードにもコピー (GUI と同じ動作)
gaze capture --copy

# 注目点を指定して promptHint 付き JSON を得る
gaze optimize screenshot.png \
  --pin 640,320:broken button \
  --rect 120,90,420,180:spacing issue
```

### 3.4 JSON 出力フォーマット

```json
{
  "originalWidth": 2560,
  "originalHeight": 1440,
  "optimizedWidth": 1568,
  "optimizedHeight": 882,
  "fileSize": 45230,
  "tokenEstimate": 1842,
  "provider": "Claude",
  "timestamp": "2026-03-29T21:30:00+09:00",
  "imageBase64": "UklGR...",
  "outputPath": null,
  "promptHint": "Focus on the marked regions in the attached screenshot:\n- Pin 1 ...",
  "annotations": [
    { "id": "1", "kind": { "type": "pin", "x": 0.5, "y": 0.4 }, "note": "broken button" }
  ]
}
```

`CaptureMetadata` と同じ構造。`--output` 指定時は `outputPath` にパスが入り、
`imageBase64` は省略 (ファイルに書き込み済みのため、stdout を軽く保つ)。
注釈を付けた場合は `promptHint` と `annotations` が追加される。

### 3.5 終了コード

| コード | 意味 |
|--------|------|
| 0 | 成功 |
| 1 | キャプチャ失敗 (権限なし、対象なし等) |
| 2 | 引数エラー (clap が自動処理) |
| 3 | ユーザーキャンセル (area/window 選択で Escape) |

---

## 4. 実装計画

### 4.1 ファイル構成

```
crates/snapforge-core/
├── Cargo.toml
└── src/
    ├── capture.rs
    ├── output.rs
    └── lib.rs

crates/gaze-cli/
├── Cargo.toml
├── build.rs
└── src/
    ├── lib.rs         # clap 定義 + コマンドハンドラ + テスト対象
    └── main.rs        # 実環境バックエンド
```

`main.rs` 1ファイル案は採らず、テスト性のために `lib.rs` と `main.rs` を分離する。
CLI の大半のロジックは `lib.rs` に置き、モック可能な `CaptureBackend` trait 経由で検証する。

### 4.2 Cargo.toml

```toml
[package]
name = "gaze-cli"
version.workspace = true
edition.workspace = true
description = "Gaze CLI - LLM-optimized screen capture"

[[bin]]
name = "gaze"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
snapforge-capture = { path = "../snapforge-capture" }
snapforge-pipeline = { path = "../snapforge-pipeline" }
serde.workspace = true
serde_json.workspace = true
image.workspace = true
arboard.workspace = true
base64.workspace = true
chrono.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
```

### 4.3 workspace 変更

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "src-tauri",
    "crates/snapforge-capture",
    "crates/snapforge-pipeline",
    "crates/snapforge-core",
    "crates/gaze-cli",           # 追加
]
```

### 4.4 キャプチャフロー (CLI)

```
CLI引数パース (clap)
  │
  ├── gaze capture
  │     │
  │     ├─ 権限チェック: snapforge_capture::has_permission()
  │     │   └─ false → エラーメッセージ + exit(1)
  │     │
  │     ├─ エンジン生成: snapforge_capture::create_engine()
  │     │
  │     ├─ キャプチャ実行 (mode に応じて分岐):
  │     │   ├─ full:   engine.capture_fullscreen(display_id)
  │     │   ├─ area:   macOS screencapture -i -s -x (既存パターン流用)
  │     │   └─ window: engine.capture_window(window_id)
  │     │              or macOS screencapture -i -w -x (インタラクティブ)
  │     │
  │     ├─ 最適化: snapforge_core::process_*()
  │     │
  │     ├─ 出力:
  │     │   ├─ --output: ファイル書き込み
  │     │   ├─ --copy:  arboard でクリップボード
  │     │   └─ stdout:  JSON / base64 / path
  │     │
  │     └─ exit(0)
  │
  ├── gaze list displays
  │     └─ engine.list_displays() → JSON 出力
  │
  ├── gaze list windows
  │     └─ engine.list_windows() → JSON 出力
  │
  └── gaze optimize <file>
        ├─ ファイル読み込み
        ├─ optimize_image()
        └─ ファイル出力 or stdout
```

### 4.5 area/window インタラクティブ選択の扱い

現在の GUI は macOS の `screencapture -i -s/-w` コマンドに委譲している。
CLI でも同じアプローチを取る:

```rust
// area モードの場合
fn capture_area_interactive(provider: LlmProvider) -> Result<CaptureResult> {
    let tmp = temp_capture_path();
    let status = Command::new("screencapture")
        .args(["-i", "-s", "-x", tmp.to_str().unwrap()])
        .status()?;
    if !status.success() || !tmp.exists() {
        return Err(anyhow!("Capture cancelled"));  // exit code 3
    }
    let bytes = std::fs::read(&tmp)?;
    std::fs::remove_file(&tmp)?;
    // bytes を decode して CaptureResult 相当にする
    Ok(...)
}
```

`--window <ID>` が直接指定された場合は `engine.capture_window(id)` を使い、
インタラクティブ選択を回避する。これが LLM エージェントにとって重要。

### 4.6 `capture_flow.rs` のコード再利用

設計初稿では CLI 側で `process_image_bytes()` 相当を複製する方針だったが、
実装ではそれを採らない。

**採用した方針:**
- `CaptureMetadata`
- `process_image_bytes()` / `process_rgba_capture()`
- `temp_capture_path()`
- `detect_image_format()` / `build_capture_filename()`

を `crates/snapforge-core/` に抽出し、GUI と CLI の両方から再利用する。

この変更で、
- GUI/CLI で最適化・base64・メタデータ生成の挙動が一致する
- CLI のテストを Tauri 非依存で書ける
- 将来 `raw` モードや JSON スキーマを変更しても差分が 1 箇所で済む

### 4.7 macOS ランタイム対策

`snapforge-capture` は ScreenCaptureKit 経由で Swift runtime に依存する。
CLI バイナリでは起動時に `libswift_Concurrency.dylib` 解決が必要になるため、
`crates/gaze-cli/build.rs` で `/usr/lib/swift` の `rpath` を埋め込む。

これにより `cargo run -p gaze-cli -- --help` でも追加の環境変数なしで起動できる。

---

## 5. テスト戦略

### 5.1 ユニットテスト (CI で実行可能)

| テスト対象 | 内容 |
|-----------|------|
| `snapforge-core` | 画像処理、base64、JSON、フォーマット判定、temp path |
| `gaze-cli` | clap 引数パース、出力分岐、終了コード、モック経由の command runner |
| optimize コマンド | 画像ファイル → 最適化後ファイルの E2E |
| capture コマンド | full/area/window の分岐、`--copy` / `--raw` / `--output` |

### 5.2 統合テスト (ローカルのみ、画面録画権限が必要)

```bash
# フルスクリーンキャプチャ → JSON 出力
gaze capture | jq .tokenEstimate

# ファイル出力
gaze capture -o /tmp/test.webp && file /tmp/test.webp

# ウィンドウ一覧
gaze list windows | jq '.[0].title'
```

---

## 6. 配布方法

### 6.1 Homebrew (推奨)

開発者の標準インストール方法。ベータ初日から提供する。

```bash
brew install gaze
```

**Homebrew Tap を使用** (公式 homebrew-core への登録はユーザー数が増えてから):

```bash
# Tap 経由でインストール
brew tap anthropics/gaze https://github.com/anthropics/homebrew-gaze
brew install gaze
```

**Formula の構成:**

```ruby
class Gaze < Formula
  desc "LLM-optimized screen capture CLI"
  homepage "https://gaze.dev"
  url "https://github.com/anthropics/gaze/releases/download/v0.1.0/gaze-v0.1.0-aarch64-apple-darwin.tar.gz"
  sha256 "..."

  depends_on :macos

  def install
    bin.install "gaze"
  end

  test do
    assert_match "gaze", shell_output("#{bin}/gaze --version")
  end
end
```

**リリースフロー:**
1. GitHub Actions で `aarch64-apple-darwin` / `x86_64-apple-darwin` のバイナリをビルド
2. GitHub Releases にアップロード (universal binary or 個別)
3. Formula の URL と sha256 を更新

### 6.2 シェルインストーラ

Homebrew を使わないユーザー向け。curl ワンライナーで完結。

```bash
curl -fsSL https://gaze.dev/install.sh | sh
```

**install.sh の動作:**
1. OS / アーキテクチャ検出 (macOS aarch64 / x86_64)
2. GitHub Releases から最新バイナリをダウンロード
3. `/usr/local/bin/gaze` に配置
4. 実行権限付与

```bash
#!/bin/sh
set -e

REPO="anthropics/gaze"
INSTALL_DIR="/usr/local/bin"

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
  x86_64)        TARGET="x86_64-apple-darwin" ;;
  *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Fetch latest release
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
URL="https://github.com/$REPO/releases/download/$LATEST/gaze-$LATEST-$TARGET.tar.gz"

echo "Installing gaze $LATEST for $TARGET..."
curl -fsSL "$URL" | tar xz -C "$INSTALL_DIR" gaze
chmod +x "$INSTALL_DIR/gaze"
echo "Installed gaze to $INSTALL_DIR/gaze"
gaze --version
```

### 6.3 GitHub Releases (直接ダウンロード)

上記2つの裏側。CI でビルドしたバイナリを GitHub Releases に置く。

```
gaze-v0.1.0-aarch64-apple-darwin.tar.gz
gaze-v0.1.0-x86_64-apple-darwin.tar.gz
```

### 6.4 CI ビルドパイプライン

```yaml
# .github/workflows/release-cli.yml (概要)
on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        target:
          - aarch64-apple-darwin
          - x86_64-apple-darwin
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - run: rustup target add ${{ matrix.target }}
      - run: cargo build --release --package gaze-cli --target ${{ matrix.target }}
      - run: tar czf gaze-${{ github.ref_name }}-${{ matrix.target }}.tar.gz -C target/${{ matrix.target }}/release gaze
      - uses: softprops/action-gh-release@v2
        with:
          files: "*.tar.gz"
```

---

## 7. 実装タスク (見積もり)

| # | タスク | 工数目安 | 依存 |
|---|--------|---------|------|
| 1 | `crates/gaze-cli/` クレート作成 + workspace 追加 | 小 | — |
| 2 | clap 定義 (Cli, Commands, CaptureArgs 等) | 小 | #1 |
| 3 | `gaze capture --mode full` 実装 | 中 | #2 |
| 4 | `gaze capture --mode area/window` (インタラクティブ) | 小 | #3 |
| 5 | `gaze capture --window <ID>` (直接指定) | 小 | #3 |
| 6 | `--output` / `--copy` / `--format` 出力分岐 | 中 | #3 |
| 7 | `gaze list displays/windows` 実装 | 小 | #2 |
| 8 | `gaze optimize` 実装 | 小 | #2 |
| 9 | ユニットテスト | 中 | #3-#8 |
| 10 | 権限チェック + エラーメッセージ | 小 | #3 |
| 11 | README / `--help` テキスト整備 | 小 | #2 |

**クリティカルパス:** #1 → #2 → #3 → #6 → #9

---

## 8. 非目標 (ベータスコープ外)

- MCP Server (`gaze mcp-server`) — CLI があれば不要
- OCR (`gaze ocr`) — 将来の Pro 機能
- `brew install` via homebrew-core (Tap で初期提供)
- Windows CLI — Mac ベータのみ
- シェル補完 (`gaze completion`) — Nice to have
- GUI との連携 (CLI から GUI を起動、等) — 将来
- 設定ファイル (`~/.config/gaze/config.toml`) — デフォルト値で十分

---

## 9. 設計判断ログ

| 判断 | 採用案 | 却下案 | 理由 |
|------|--------|--------|------|
| バイナリ構成 | 別クレート | tauri-plugin-cli | stdout/起動速度/テスタビリティ |
| 引数パーサ | clap derive | tauri.conf.json 宣言型 | 型安全、補完、テスト容易 |
| クリップボード共有 | CLI で arboard 直接使用 | 共通クレート作成 | 関数2つのためにクレートは過剰 |
| capture_flow 共有 | CLI 側に再実装 (30行) | snapforge-core 抽出 | ベータでは重複許容 |
| 配布 | Homebrew Tap + curl installer | homebrew-core / cargo install | Tap は即日公開可、core は審査待ち |
| area/window 選択 | screencapture コマンド委譲 | CaptureEngine 直接 | 既存 GUI と同じパターン、実績あり |
