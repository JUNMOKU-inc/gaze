# Gaze — 開発順序 & 技術選定

> 作成日: 2026-03-25
> ステータス: 技術選定フェーズ

---

## 1. 開発順序の戦略

### 提案する順序

```
Phase 1: Mac版（MVP → ローンチ）          Month 1-4
Phase 2: Windows版                        Month 5-7
Phase 3: Developer Integration (MCP/CLI)  Month 7-9  ← 順序変更を提案
Phase 4: クラウド共有                      Month 9-11
Phase 5: スマホ連携（iOS → Android）       Month 12+
```

### 元の案からの変更点と理由

**クラウドよりMCP/CLI統合を先に（Phase 3 ↔ 4 入替え）**:

| 理由 | 詳細 |
|------|------|
| **差別化が最大** | クラウド共有はCleanShot Xが既にやっている。MCP統合は**誰もやっていない** |
| **開発コストが低い** | MCP Server = ローカルstdio通信。クラウド = インフラ構築 + 認証 + 課金 |
| **バイラル効果** | Claude Code / Cursorユーザーが `.mcp.json` を共有 → 自然拡散 |
| **ターゲットとの親和性** | Primary userはAI開発者。クラウドより先にMCPが欲しいはず |
| **収益への影響なし** | MCP/CLIはPro機能に含める。クラウドはPro+の収益ドライバーなので後でOK |

**スマホ連携は最後**:
- iOS版は別アプリとして開発が必要（工数大）
- Mac + Windows + MCP + Cloudだけで十分戦える
- スマホ連携はユーザーからの需要を確認してからでも遅くない

---

## 2. 技術選定 — 結論

### 推奨: Tauri v2 + Rust + React/TypeScript

**最大の発見**: [**Cap**](https://github.com/CapSoftware/Cap) というオープンソースのLoom代替ツールが、**Tauri v2 + Rustで4K@60fpsスクリーン録画**をmacOS/Windowsの両方で実現済み。本番品質で動作することが証明されている。

```
┌──────────────────────────────────────────────────┐
│              Gaze Architecture              │
├──────────────────────────────────────────────────┤
│  Frontend (React + TypeScript)                   │
│  ├── Annotation Editor (Canvas API / Konva.js)   │
│  ├── Settings UI                                 │
│  ├── Capture Preview / Quick Overlay             │
│  └── Token Cost Calculator                       │
├──────────────────────────────────────────────────┤
│  Tauri Shell                                     │
│  ├── System Tray / Menu Bar                      │
│  ├── Global Hotkeys                              │
│  ├── Window Management (Overlay)                 │
│  └── Auto Updater                                │
├──────────────────────────────────────────────────┤
│  Rust Backend (Tauri Commands)                   │
│  ├── Capture Engine                              │
│  │   ├── macOS: screencapturekit crate           │
│  │   └── Windows: windows-capture crate          │
│  │   └── (or scap crate: 統一API)               │
│  ├── Image Pipeline                              │
│  │   ├── image crate (リサイズ、フォーマット変換) │
│  │   ├── gifski (高品質GIF生成)                  │
│  │   └── LLM Optimizer (プロバイダ別最適化)      │
│  ├── OCR Engine                                  │
│  │   ├── macOS: Vision framework (via objc2)     │
│  │   └── Windows: Windows.Media.OCR              │
│  ├── Clipboard Manager                           │
│  ├── MCP Server (stdio)                          │
│  └── CLI Interface                               │
└──────────────────────────────────────────────────┘
```

### なぜTauri v2か — 全選択肢の比較

| 基準 | Swift/SwiftUI | **Tauri v2** | Electron | Flutter | KMP/Compose |
|------|-------------|-------------|----------|---------|-------------|
| **アプリサイズ** | 5-15MB | **10-30MB** | 150-300MB | 30-50MB | 100-150MB |
| **メモリ使用量** | 30-80MB | **50-100MB** | 200+MB | 100-150MB | 150-250MB |
| **起動時間** | <0.5s | **0.5-1s** | 2-5s | 1-2s | 2-4s |
| **ScreenCaptureKit** | 直接 | **Rustクレート経由** | ネイティブモジュール必要 | ネイティブプラグイン必要 | JNI経由 |
| **Win Capture API** | N/A | **Rustクレート経由** | ネイティブモジュール必要 | ネイティブプラグイン必要 | JNI経由 |
| **Mac UI品質** | 最高 | **良好（Web）** | 良好（Web） | 中（非ネイティブ） | 中（Skia） |
| **メニューバー/トレイ** | ネイティブ | **サポート済** | サポート済 | サードパーティ（脆弱） | 限定的 |
| **グローバルホットキー** | ネイティブ | **サポート済** | サポート済 | サードパーティ（脆弱） | 限定的 |
| **オーバーレイウィンドウ** | ネイティブ | **サポート済** | サポート済 | 実験的 | サポート済 |
| **クロスプラットフォーム** | **Mac専用** | **Mac + Win** | Mac + Win | Mac + Win | Mac + Win |
| **開発速度** | 速い（1プラットフォーム） | **速い（1コードベース）** | 速い | 中 | 中 |
| **コード共有率** | 0% | **~90%** | ~95% | ~80% | ~70% |
| **自動アップデート** | Sparkle | **組込み** | 組込み | 手動 | 手動 |

### 各選択肢を除外した理由

#### Swift/SwiftUI — Mac最高品質だがWindows不可
- Mac単体なら最良の選択。CleanShot X、Raycast、Shottrは全てSwift
- **致命的問題**: Windows対応不可。Windows版を作る時に全く別のコードベースが必要
- 後からRustコアに移行するのはリアーキテクチャが必要で極めて高コスト

#### Electron — 動くが重すぎる
- Loom、Gyazo、Kap等の実績あり
- **問題1**: アプリサイズ150-300MB。常駐ユーティリティとしては肥大
- **問題2**: メモリ200MB+。8GBのMacBookでは気になるレベル
- **問題3**: 結局スクリーンキャプチャはネイティブモジュール（node-addon-api）が必要。Electronの恩恵は「UIがWeb」だけ
- **問題4**: Chromiumの巨大な依存ツリー。セキュリティアップデートの追従コスト

#### Flutter Desktop — スクリーンキャプチャに不向き
- メニューバーアプリ、グローバルホットキー、オーバーレイウィンドウのサポートが脆弱
- キャプチャ機能は結局ネイティブプラグインが必要
- 本番品質のスクリーンキャプチャツールの実績なし

#### KMP/Compose — JVMオーバーヘッドが致命的
- 起動2-4秒、メモリ150MB+。軽量ユーティリティに不適
- JNI経由のネイティブAPI呼び出しは複雑でパフォーマンスも劣る

#### Native per-platform (Rust core + Swift UI + WinUI) — 品質最高だが開発速度が犠牲
- 1Password方式。品質は最高
- **問題**: UIを2つ作る必要がある。インディー/スタートアップには開発コストが高すぎる
- 5-10人のチームがあるなら最良の選択肢

### Tauri v2が最適な理由 — 詳細

#### 1. 証明済みのアーキテクチャ

**Cap** (https://github.com/CapSoftware/Cap):
- Tauri v2 + Rust + React で構築
- macOS + Windows対応
- 4K@60fps スクリーン録画
- ScreenCaptureKit (macOS) / Windows.Graphics.Capture (Windows)
- オープンソース — コードを参考にできる

#### 2. スクリーンキャプチャのRustクレート

| クレート | 対応OS | 機能 |
|---------|--------|------|
| `screencapturekit` | macOS | ScreenCaptureKit完全バインディング。Tauri用サンプルあり |
| `windows-capture` | Windows | Windows.Graphics.Capture API。高性能 |
| `scap` | **macOS + Windows + Linux** | CapSoftware製の**統一クロスプラットフォームAPI** |
| `xcap` | macOS + Windows + Linux | スクリーンショット取得のクロスプラットフォームAPI |

**`scap`クレートが特に重要**: Cap開発チームが作った統一APIで、プラットフォーム差異を吸収。

```rust
// scap での画面キャプチャ（クロスプラットフォーム）
use scap::{capturer::Capturer, frame::Frame};

let capturer = Capturer::new(/* config */);
capturer.start_capture();
let frame: Frame = capturer.get_next_frame()?;
```

#### 3. 画像処理・GIF・OCRのRustエコシステム

| 用途 | クレート | 説明 |
|------|---------|------|
| 画像処理 | `image` | リサイズ、クロップ、フォーマット変換（PNG/JPEG/WebP） |
| GIF生成 | `gifski` | 高品質GIF（pngquantチーム製）。Gifski Mac appのエンジン |
| WebP | `webp` | WebPエンコード/デコード |
| OCR (macOS) | `objc2` + Vision framework | macOSネイティブOCR呼び出し |
| OCR (Windows) | `windows` crate + Windows.Media.OCR | WindowsネイティブOCR |
| OCR (汎用) | `uni-ocr` | macOSではVision、WindowsではWindows OCRを自動選択 |
| クリップボード | `arboard` | クロスプラットフォームクリップボード操作（画像対応） |

#### 4. Tauri v2 固有の利点

| 機能 | 状態 | 詳細 |
|------|------|------|
| System Tray | 安定 | `tauri-plugin-system-tray` — メニューバー/システムトレイ |
| Global Shortcut | 安定 | `tauri-plugin-global-shortcut` — グローバルホットキー |
| Clipboard | 安定 | `tauri-plugin-clipboard-manager` — テキスト+画像 |
| Auto Updater | 安定 | `tauri-plugin-updater` — 組込み自動アップデート |
| Shell | 安定 | `tauri-plugin-shell` — CLI実行 |
| Window Customization | 安定 | 透明ウィンドウ、フレームレス、always-on-top |
| Deep Link | 安定 | `gaze://` カスタムURLスキーム |
| Store | 安定 | ローカル設定永続化 |

#### 5. UIはReact + TypeScript

フロントエンドはWeb技術で開発可能:
- **アノテーションエディタ**: Canvas API / Konva.js / Fabric.js（Web上の画像編集ライブラリが豊富）
- **設定UI**: 通常のReact SPA
- **プレビュー**: Web技術での表示が自然
- 開発者採用のしやすさ（Rust + React/TSは人気スタック）

---

## 3. Tauri v2 の注意点・リスク

正直にリスクも記載:

| リスク | 影響 | 対策 |
|--------|------|------|
| **WebView差異** | macOS(WebKit) と Windows(WebView2/Chromium) でレンダリングが微妙に異なる | CSSはシンプルに。プラットフォーム別テスト必須 |
| **透明ウィンドウのWindows制限** | クリックスルー透過が完全でない場合がある | エリア選択UIはフルスクリーンオーバーレイで対応 |
| **Rust学習曲線** | 所有権・借用の概念が独特 | キャプチャエンジン部分は`scap`クレートが抽象化。UI側はTS |
| **エコシステムの成熟度** | Electronほどの実績・事例は少ない | Cap, Cody, Spacedrive等の実績で十分検証済み |
| **WebViewメモリ** | WebKit/WebView2が50-150MB程度消費 | Electronの200MB+よりは良い。十分許容範囲 |
| **macOS App Store** | プライベートAPI使用で審査に影響の可能性 | 直販メインの戦略と合致。App Storeは優先度低 |

---

## 4. 開発環境・ツールチェーン

### 必要なもの

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js (フロントエンド)
# pnpm推奨
npm install -g pnpm

# Tauri CLI
cargo install tauri-cli

# macOS追加 (Xcode Command Line Tools)
xcode-select --install

# Windows追加
# - Visual Studio Build Tools (C++ workload)
# - WebView2 Runtime (Windows 10+にはプリインストール)
```

### プロジェクト構成

```
gaze/
├── src-tauri/                 # Rust (Tauri backend)
│   ├── src/
│   │   ├── main.rs
│   │   ├── capture/           # スクリーンキャプチャエンジン
│   │   │   ├── mod.rs
│   │   │   ├── macos.rs       # ScreenCaptureKit wrapper
│   │   │   └── windows.rs     # Windows.Graphics.Capture wrapper
│   │   ├── pipeline/          # 画像処理パイプライン
│   │   │   ├── mod.rs
│   │   │   ├── optimizer.rs   # LLM最適化
│   │   │   ├── gif.rs         # GIFエンコード
│   │   │   └── ocr.rs         # OCR抽出
│   │   ├── clipboard.rs       # クリップボード管理
│   │   ├── mcp/               # MCP Server
│   │   │   ├── mod.rs
│   │   │   └── server.rs
│   │   └── cli.rs             # CLI インターフェース
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                       # React (Frontend)
│   ├── App.tsx
│   ├── components/
│   │   ├── AnnotationEditor/  # アノテーションUI
│   │   ├── CaptureOverlay/    # エリア選択オーバーレイ
│   │   ├── Preview/           # キャプチャプレビュー
│   │   ├── Settings/          # 設定画面
│   │   └── Tray/              # トレイメニュー
│   ├── hooks/
│   └── lib/
│       ├── tauri-commands.ts  # Rust↔TS ブリッジ
│       └── llm-profiles.ts   # LLMプロバイダ設定
├── package.json
├── tsconfig.json
├── vite.config.ts
└── README.md
```

### ビルド & 配布

```bash
# 開発
pnpm tauri dev

# ビルド（macOS .dmg）
pnpm tauri build --target universal-apple-darwin

# ビルド（Windows .msi/.exe）
pnpm tauri build --target x86_64-pc-windows-msvc

# ビルドサイズ目安
# macOS: ~15-25MB (.dmg)
# Windows: ~15-25MB (.msi)
```

---

## 5. Phase別の技術タスク

### Phase 1: Mac MVP (Month 1-2)

| タスク | 技術 | 工数目安 |
|--------|------|---------|
| Tauriプロジェクト初期化 | Tauri CLI + React + Vite | 1日 |
| メニューバー/トレイ実装 | tauri-plugin-system-tray | 2日 |
| グローバルホットキー | tauri-plugin-global-shortcut | 1日 |
| エリアキャプチャ（オーバーレイUI） | React Canvas + Tauri Window API | 5日 |
| ウィンドウキャプチャ | scap / screencapturekit crate | 3日 |
| 全画面キャプチャ | scap crate | 1日 |
| LLM最適化パイプライン | image crate（リサイズ、フォーマット変換） | 3日 |
| クリップボード自動コピー | arboard crate | 1日 |
| 基本アノテーション | Konva.js / Fabric.js on Canvas | 5日 |
| プレビュー/Quick Access Overlay | React + Tauri Window | 3日 |
| 設定UI | React | 2日 |
| macOS権限ハンドリング | ScreenCaptureKit permission API | 2日 |
| **合計** | | **~29日（6週間）** |

### Phase 2: Windows対応 (Month 5-7)

| タスク | 技術 | 工数目安 |
|--------|------|---------|
| Windowsビルド環境構築 | Visual Studio Build Tools + WebView2 | 1日 |
| Windowsキャプチャ実装 | windows-capture crate / scap | 5日 |
| Windows固有UI調整 | WebView2差異対応 | 3日 |
| システムトレイ（Windows） | 同一API（Tauri抽象化） | 1日 |
| グローバルホットキー（Windows） | 同一API | 1日 |
| Windows OCR | Windows.Media.OCR via windows crate | 3日 |
| Windows Installer (.msi) | tauri-plugin-updater + WiX | 2日 |
| Windows固有テスト | CI + 実機テスト | 3日 |
| **合計** | | **~19日（4週間）** |

**注目**: Tauri + scapの抽象化により、Windowsポーティングの工数が大幅に削減される。UIは100%共有。

### Phase 3: MCP / CLI統合 (Month 7-9)

| タスク | 技術 | 工数目安 |
|--------|------|---------|
| MCP Server実装（stdio） | Rust JSON-RPC | 5日 |
| MCPツール定義（capture_screen等） | MCP protocol spec | 3日 |
| CLI実装 | clap crate | 3日 |
| コンテキストキャプチャ | Accessibility API + git CLI | 3日 |
| テスト（Claude Code / Cursor連携） | 実動作検証 | 2日 |
| **合計** | | **~16日（3週間）** |

---

## 6. 主要Rustクレートの依存関係

```toml
# Cargo.toml (src-tauri)
[dependencies]
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-clipboard-manager = "2"
tauri-plugin-updater = "2"
tauri-plugin-shell = "2"
tauri-plugin-store = "2"

# Screen Capture
scap = "0.x"                    # クロスプラットフォーム画面キャプチャ
# or platform-specific:
# screencapturekit = "0.x"      # macOS only
# windows-capture = "1.x"       # Windows only

# Image Processing
image = "0.25"                  # 画像処理（リサイズ、クロップ、変換）
webp = "0.3"                    # WebPエンコード
gifski = "1.x"                  # 高品質GIF生成

# OCR
uni-ocr = "0.x"                 # クロスプラットフォームOCR

# Clipboard
arboard = "3"                   # クリップボード（画像対応）

# CLI
clap = { version = "4", features = ["derive"] }

# MCP Server
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }

# Utilities
base64 = "0.22"
chrono = "0.4"
```

---

## 7. 判断マトリクス — 最終確認

### 「なぜElectronではなくTauriか？」に対する回答

| 質問 | Electron | Tauri v2 |
|------|----------|---------|
| スクリーンキャプチャは？ | desktopCapturer（レガシーAPI）+ ネイティブアドオンが必要 | Rustクレートで直接ScreenCaptureKit/WGCにアクセス |
| アプリサイズは？ | 150-300MB | 15-25MB (**10倍以上小さい**) |
| メモリは？ | 200MB+ | 50-100MB (**半分以下**) |
| UIは？ | React/TS（同じ） | React/TS（同じ） |
| 開発速度は？ | やや速い（エコシステム大） | 同程度（Rustキャプチャ部分の学習コストあるが、scapが吸収） |
| 配布は？ | electron-builder | tauri-plugin-updater（組込み） |
| 実績は？ | Loom, Kap, Gyazo | **Cap（4K@60fpsスクリーン録画、本番稼働）** |

### 「なぜSwift/SwiftUIではなくTauriか？」に対する回答

| 質問 | Swift/SwiftUI | Tauri v2 |
|------|-------------|---------|
| Mac品質は？ | 最高 | 良好（Web UIだが十分） |
| Windowsは？ | **不可能** | **同一コードベースで対応** |
| スクショツールとして？ | CleanShot X, Shottrと同等の品質が可能 | Cap で十分な品質が証明済み |
| アノテーションUIは？ | AppKit/Core Graphicsで自前実装（大変） | **Canvas + Konva.js（Webの豊富なエコシステム）** |
| 開発者採用は？ | Swift開発者（Mac限定） | **Rust + React/TS（幅広い）** |

**アノテーションUIの開発速度はTauriが圧倒的に有利**。Web上のCanvas描画ライブラリ（Konva.js, Fabric.js等）は非常に成熟しており、ネイティブで同等のものを作るのは数倍の工数がかかる。

---

## 8. 結論

### 推奨技術スタック

```
Tauri v2 + Rust + React + TypeScript + Vite
```

### 推奨開発順序

```
1. Mac MVP (Month 1-2)       → キャプチャ + LLM最適化 + アノテーション
2. Mac ローンチ (Month 3-4)  → Product Hunt, ベータ配布
3. Windows (Month 5-7)       → scapクレートでポーティング
4. MCP/CLI (Month 7-9)       → Claude Code / Cursor統合
5. Cloud (Month 9-11)        → Cloudflare R2 + 共有リンク
6. iOS (Month 12+)           → Tauri v2 モバイル or SwiftUIネイティブ
```

### 参考にすべきオープンソース

| プロジェクト | 技術 | 参考ポイント |
|-------------|------|------------|
| [Cap](https://github.com/CapSoftware/Cap) | Tauri v2 + Rust | **最重要参考**。スクリーン録画の全アーキテクチャ |
| [Kap](https://github.com/wulkano/Kap) | Electron + Swift | GIF録画のUXデザイン |
| [screencapturekit crate](https://github.com/nickveld/screencapturekit-rs) | Rust | macOSキャプチャ実装 |
| [scap](https://github.com/CapSoftware/scap) | Rust | クロスプラットフォームキャプチャAPI |
| [Gifski](https://github.com/ImageOptim/gifski) | Rust | 高品質GIF生成エンジン |
