# CLAUDE.md — Gaze

## Project Overview

**Gaze** — LLMインプット特化のスクリーンキャプチャツール（Mac/Windows）

「スクリーンショットを撮る → LLMに最適な形で即座に渡す」を1キーストロークで実現するデスクトップアプリ。

### Tech Stack

- **Framework**: Tauri v2
- **Backend**: Rust
- **Frontend**: 自己完結型HTML (`public/preview.html`) — Reactは撤去済み
- **Screen Capture**: `scap` crate (macOS: ScreenCaptureKit, Windows: Windows.Graphics.Capture)
- **Image Processing**: `image` crate, `gifski`, `webp`
- **OCR**: `uni-ocr` (macOS: Vision framework, Windows: Windows.Media.OCR)
- **Clipboard**: `arboard`

### Key Reference

- **Cap** (https://github.com/CapSoftware/Cap) — Tauri v2 + Rust で4K@60fpsスクリーン録画を実現したOSS。アーキテクチャの最重要参考。

## Documentation

戦略・設計ドキュメントは `docs/` 配下に格納:

| File | Content |
|------|---------|
| `docs/00_research_and_strategy.md` | 市場調査、コンセプト、事業戦略、ロードマップ |
| `docs/01_expanded_ideas.md` | 拡張アイデア（MCP, GIF, Cloud, Mobile, AI機能） |
| `docs/02_development_order_and_tech_selection.md` | 開発順序、技術選定、アーキテクチャ詳細 |
| `docs/03_rust_coding_guidelines.md` | Rustコーディングガイドライン（エラー処理、テスト、プラットフォーム抽象化） |
| `docs/03_frontend_coding_guidelines.md` | **[ARCHIVED]** フロントエンドガイドライン（React撤去済み、再導入時の参考） |
| `docs/04_ux_design_specification.md` | UX設計仕様（全フロー、プレビュー、アノテーション、エッジケース） |
| `docs/learnings/tauri-v2-gotchas.md` | Tauri v2 プラットフォーム制約集（実戦で踏んだ地雷） |
| `docs/learnings/adr-001-preview-window-architecture.md` | ADR: プレビューウィンドウのアーキテクチャ変更 |
| `docs/learnings/adr-002-preview-window-ipc.md` | ADR: IPC bridge保全と自己完結型HTML移行（**必読**） |
| `docs/learnings/debug-preview-click.md` | デバッグ実験ログ（ADR-002の裏付け） |
| `docs/learnings/performance-tips.md` | パフォーマンス計測結果と最適化パターン |

実装前に必ず関連ドキュメントを確認すること。
**コードレビュー時は `docs/03_rust_coding_guidelines.md` のチェックリストを基準として使用すること。**

## Development

### Prerequisites

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Tauri CLI
cargo install tauri-cli
```

### macOS Screen Recording Permission (開発時)

`cargo tauri dev` でスクリーンキャプチャを動作させるには、**ターミナルアプリ**にScreen Recording権限を付与する必要がある（Capプロジェクトと同じ方式）:

1. システム設定 → プライバシーとセキュリティ → 画面収録とシステムオーディオ録音
2. 「+」→ `/Applications/Utilities/ターミナル.app` を追加 → ON
3. ターミナルを `⌘Q` で完全終了 → 再起動

開発ビルド（bare binary）はmacOS設定画面に直接表示されない。ターミナルの権限を子プロセスが継承する仕組み。

**注意**: `tccutil reset ScreenCapture` は全アプリの権限をリセットするので絶対に使わない。個別リセットは `tccutil reset ScreenCapture <bundle_id>` を使用。

### Commands

```bash
cargo tauri dev       # Development mode
cargo tauri build     # Production build
cargo clippy          # Rust linter
cargo test            # Rust tests
cargo fmt             # Rust formatter
```

### Quality Gates

PRマージ前に必ず全て通すこと:

```bash
cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test --workspace --exclude snapforge-capture
```

## Architecture

```
gaze/
├── src-tauri/                 # Rust (Tauri backend)
│   ├── src/
│   │   ├── main.rs
│   │   ├── capture/           # Screen capture engine
│   │   ├── pipeline/          # Image processing & LLM optimization
│   │   ├── clipboard.rs       # Clipboard management
│   │   ├── mcp/               # MCP Server for AI agents
│   │   └── cli.rs             # CLI interface
│   └── Cargo.toml
├── public/                    # Static frontend assets
│   └── preview.html           # Self-contained preview popup
└── CLAUDE.md
```

## Git Workflow

- Main branch: `main`
- Feature branches: `feat/*`
- Commit format: `type(scope): description`
  - Types: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`
  - Scope: `capture`, `pipeline`, `annotation`, `mcp`, `cli`, `ui`, `cloud`

## Work Style

### Agent Team

複雑なタスクや並行作業が可能な場合、積極的にエージェントチームを活用すること:

- **リサーチ**: 技術調査、ライブラリ比較、ベストプラクティス調査は専用エージェントに委任
- **並行実装**: 独立したモジュール（例: キャプチャエンジンとアノテーションUI）は並行してエージェントを起動
- **テスト**: 実装完了後、テストエージェントを起動してテスト作成・実行
- **Explore**: コードベース探索やファイル検索はExploreエージェントを活用

単一のファイル編集や単純な修正にはエージェントは不要。判断基準: 「3つ以上の独立したサブタスクがあるか？」

### Code Review with Codex CLI

実装が一区切りついたタイミングで、`codex review` を使ってコードレビューを実施する:

```bash
# 現在の変更をレビュー
codex review

# 特定のコミット範囲をレビュー
codex review --diff "HEAD~3..HEAD"
```

**レビュータイミング**:
- PR作成前
- 大きな機能実装の完了時
- リファクタリング後
- 「これで大丈夫か？」と不安な時はいつでも

Claude Codeからも直接 `codex review` をBashツールで実行可能。積極的に活用すること。

## LLM Provider Specs (Quick Reference)

画像最適化パイプライン実装時の参考:

| Provider | Max Resolution | Token Formula | Optimal |
|----------|---------------|---------------|---------|
| Claude | 8000x8000 | `(w*h)/750` | 1568px long edge |
| GPT-4o | 2048x2048 | 170/tile(512px) + 85 | 2048px, low=85tok flat |
| Gemini | unlimited | 258/tile(768px) | 768px tiles |

詳細は `docs/00_research_and_strategy.md` Appendix A を参照。

## Learnings (実戦で得た制約集)

詳細は `docs/learnings/` を参照。以下はコード生成時に**必ず守る**ルール。

### Tauri v2 必守ルール

- **ウィンドウ作成は `app.run_on_main_thread()` 経由で行う** — バックグラウンドスレッドからの `WebviewWindowBuilder::build()` はデッドロックする（エラーもパニックも出ない）
- **close() 後に同じラベルでウィンドウを作らない** — ラベルは即座に解放されない。毎回ユニークラベル（`format!("name-{seq}")`）を使う
- **hidden webview にイベントを送って表示制御しない** — macOS WebKit は hidden webview の JS を停止する。データはHTML/initialization_scriptに埋め込む
- **ephemeral UI（プレビュー、通知）に React を使わない** — 自己完結型の静的 HTML で十分。制御権を分散させない
- **plain HTMLページでは `withGlobalTauri: true` を必ず設定する** — `@tauri-apps/api` を使わないページでは `window.__TAURI__` が注入されない。ADR-002参照
- **`document.write()` / `document.open()` は絶対に使わない** — WKWebView上でTauri IPC bridge (`window.__TAURI__`) を完全破壊する。`document.documentElement.innerHTML` も同様にNG。ADR-002参照
- **追加ウィンドウは自己完結型HTMLで `initialization_script` 経由のデータ注入 + ポーリングboot** — ページ内の `<script>` で `__TAURI__` と データの両方を待ってから初期化する。`eval()` でのJS注入は不要
- **追加ウィンドウのラベルパターンを capabilities の `windows` 配列に含める** — 例: `"preview-*"`。含めないと `invoke()` がブロックされる

### macOS 必守ルール

- **`Cmd+Shift+3/4` はシステム予約** — アプリのショートカットには `Alt+Shift+数字` 等を使う
- **グローバルショートカットは必ず 500ms デバウンスする** — 1回のキー押下で複数回発火する
- **ウィンドウ透明化は NSWindow + WKWebView の両方に設定する** — 片方だけだと白い矩形が残る
- **スペース追随には `NSWindowCollectionBehavior::CanJoinAllSpaces` を設定する**

### パフォーマンス必守ルール

- **`image` crate は dev profile で `opt-level = 3` にする** — debug build では Lanczos3 リサイズが 50倍遅い
- **クリップボードコピーで画像を二重デコードしない** — optimize 時に RGBA も返して直接渡す

### 設計原則（このプロジェクト固有）

- **3回パッチで直せなかったらアーキテクチャを疑う** — ADR を書いて代替案を評価する
- **2つのランタイムでウィンドウ表示を共同管理しない** — 必ずどちらか1つが全権を持つ
- **ボトルネック調査は計測から始める** — tracing span で各ステップの `time.busy` を記録してから改善する

## Key Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-03-25 | Tauri v2 + Rust + React/TS | Cap (OSS) で本番品質が証明済み。Electron比10倍軽量 |
| 2026-03-25 | MCP/CLI統合をクラウドより先に | 最大の差別化ポイント。開発コストも低い |
| 2026-03-25 | GIF + MP4のデュアルフォーマット | GitHub READMEはGIFのみインライン。用途別に自動選択 |
| 2026-03-25 | $19一括 + $4.99/mo Pro+ | CleanShot X ($29) より低くエントリー障壁を下げる |
| 2026-03-26 | opt-level=3 for image crates in dev | debug buildで画像処理が50倍遅い問題の根本解決 |
| 2026-03-27 | Preview popup: React → Rust-driven HTML | hidden webview問題を構造的に解消。ADR-001参照 |
| 2026-03-27 | Preview popup: eval()+document.write() → 自己完結型HTML | document.write()がTauri IPC bridgeを破壊。withGlobalTauri+initialization_script+ポーリングbootに移行。ADR-002参照 |
