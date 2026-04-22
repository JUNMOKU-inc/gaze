# Gaze — 拡張アイデア集 v2
## 追加リサーチ結果 + フィーチャー深掘り

> 作成日: 2026-03-25
> 前提ドキュメント: `00_research_and_strategy.md`

---

## 1. 動画対応 — GIF & GitHub最適化

### 1.1 なぜGIFが開発者ワークフローで重要か

GitHubのREADME.mdで**インライン再生**できるアニメーション形式はGIFのみ。MP4はIssue/PRコメントにはドラッグ&ドロップで添付可能だが、READMEでは再生されない。

| コンテキスト | GIF | MP4 | WebM |
|-------------|-----|-----|------|
| GitHub README.md | インライン再生 | **非対応** | **非対応** |
| GitHub Issue/PRコメント | インライン再生 | ドラッグ&ドロップ（プレーヤー表示） | **非対応** |
| Slack | 自動再生 | プレーヤー | 非対応 |
| Discord | 自動再生 | プレーヤー | プレーヤー |
| Notion | 再生 | 再生 | 非対応 |

### 1.2 ファイルサイズ比較

| 形式 | 相対サイズ | 例（10秒録画） | 圧縮方式 |
|------|----------|---------------|---------|
| GIF | 1x (ベースライン) | ~6.6MB | フレーム単位LZW（256色制限） |
| MP4 (H.264) | **~5-10x小さい** | ~0.3-1.3MB | フレーム間差分圧縮 |
| WebM (VP9) | ~8-12x小さい | ~0.2-0.8MB | フレーム間差分圧縮 |

### 1.3 Gazeの動画戦略

**「スマートフォーマット」アプローチ**: 常にロスレス/高品質で録画し、エクスポート時に最適な形式を自動選択。

```
録画（内部: 高品質MP4）
  ├── GitHub README向け → GIF（自動最適化、256色パレット最適化、ディザリング）
  ├── GitHub Issue向け → MP4（H.264、10MB以下に自動圧縮）
  ├── LLM向け → キーフレーム抽出（重要フレームのみ静止画として出力）
  ├── Gemini向け → MP4直接送信（ネイティブ動画対応）
  └── クリップボード → GIF（汎用性最大）
```

**キーフレーム抽出のインテリジェンス**:
- フレーム間差分が大きい瞬間を自動検出（UIの状態変化、ページ遷移、エラー表示）
- マウスクリック/キーストロークのタイミングでフレームを抽出
- ユーザーが手動でフレームを選択するUIも提供
- 抽出したフレームに自動で番号ラベル（Step 1, 2, 3...）を付与

### 1.4 GitHub制限への対策

| 制限 | 値 | 対策 |
|------|-----|------|
| 画像/GIF | 10MB | 自動圧縮、フレームレート調整（15→8fps）、解像度ダウン |
| 動画(MP4) | 100MB（有料） | 通常問題なし |
| README埋め込み | GIFのみ | GIF自動変換 |

---

## 2. OCR抽出モード — 深掘り

### 2.1 なぜOCRモードが強力か

| 比較軸 | 画像をそのまま送信 | OCR→テキスト送信 |
|--------|-----------------|----------------|
| トークンコスト | 1,334トークン（1000x1000px, Claude） | ~200-500トークン（テキスト量による） |
| 数値精度 | LLMが頻繁に誤読 | OCRの方が正確 |
| コピペ可能性 | 不可 | テキストなのでそのまま利用可 |
| レイアウト情報 | 保持 | 失われる |
| 図表・チャート | 理解可能 | 不可 |

**ベストプラクティス**: ハイブリッドモード — OCRテキスト + 画像を両方送信。テキストは正確で、画像はレイアウト/視覚情報を補完。

### 2.2 実装アプローチ

macOS Vision frameworkを使用:
```swift
// VNRecognizeTextRequest で高精度OCR
// 日本語・英語・中国語等の多言語対応
// リアルタイムOCR（キャプチャと同時に実行）
```

**出力モード**:
1. **テキストのみ**: OCR結果をクリップボードにコピー（トークン最小化）
2. **テキスト + 画像**: テキストと画像を両方クリップボードに（ハイブリッド）
3. **構造化テキスト**: テーブルを検出してMarkdown表形式に変換

---

## 3. Claude Code / AIエージェント統合

### 3.1 MCP Server としてのGaze

これが**最大の差別化ポイント**になりうる。GazeをMCP Serverとして公開すれば、Claude Code、Cursor、VS Code等のAIコーディングツールから直接呼び出せる。

**設定例 (`.mcp.json`)**:
```json
{
  "mcpServers": {
    "gaze": {
      "command": "/Applications/Gaze.app/Contents/MacOS/gaze-mcp",
      "args": ["--stdio"]
    }
  }
}
```

**公開するMCPツール**:

| ツール名 | 説明 | 返り値 |
|---------|------|--------|
| `capture_screen` | 全画面キャプチャ | base64画像 |
| `capture_window` | 指定ウィンドウをキャプチャ | base64画像 |
| `capture_area` | 指定座標のエリアキャプチャ | base64画像 |
| `capture_window_by_name` | アプリ名/タイトルでウィンドウ検索・キャプチャ | base64画像 |
| `list_windows` | 全ウィンドウ一覧 | JSON配列 |
| `ocr_screen` | 画面のテキストをOCR抽出 | テキスト |
| `ocr_area` | 指定エリアのテキストをOCR抽出 | テキスト |
| `record_gif` | GIF録画開始/停止 | ファイルパス |
| `capture_with_context` | スクショ + メタデータ（URL, アプリ名, git branch） | JSON |

### 3.2 ユースケース: AIエージェントがスクショを必要とする場面

#### a) UIデバッグ・ビジュアルフィードバックループ

```
開発者: 「このCSSを直して。今の表示はこうなっている」
Claude Code:
  1. gaze.capture_window_by_name("Chrome") でブラウザをキャプチャ
  2. 画像を分析して問題を特定
  3. CSSを修正
  4. 再度キャプチャして修正結果を確認
  5. 「修正しました。Before/Afterはこちらです」
```

#### b) ビジュアルリグレッションテスト

```
開発者: 「UIが壊れていないか確認して」
Claude Code:
  1. npm run dev を起動
  2. gaze.capture_window_by_name("localhost:3000")
  3. 前回のベースラインスクショと比較
  4. 差分を検出して報告
```

#### c) エラーダイアログのキャプチャ・診断

```
開発者: 「アプリがエラーを出している。画面を見て」
Claude Code:
  1. gaze.capture_screen() で全画面取得
  2. エラーダイアログを検出
  3. gaze.ocr_area() でエラーメッセージをテキスト抽出
  4. エラーの原因を診断して修正提案
```

#### d) レスポンシブデザイン検証

```
Claude Code:
  1. ブラウザを各サイズにリサイズ
  2. gaze.capture_window("Chrome") を各サイズで実行
  3. モバイル/タブレット/デスクトップの表示を比較検証
```

#### e) ドキュメント・チュートリアル自動生成

```
開発者: 「この操作手順をドキュメント化して」
Claude Code:
  1. gaze.record_gif() で操作を録画
  2. キーフレームを抽出
  3. 各ステップにキャプション付きのMarkdownドキュメントを生成
  4. GIFをGitHubにアップロード可能な形式で出力
```

### 3.3 CLI インターフェース

MCP Serverに加えて、CLIからも同じ機能を呼び出せるようにする:

```bash
# 全画面キャプチャ
gaze capture --output /tmp/screen.png

# ウィンドウ指定キャプチャ
gaze capture --window "Chrome" --optimize claude --output /tmp/chrome.png

# OCR
gaze ocr --area "100,200,800,600"

# GIF録画
gaze record --duration 10 --output /tmp/demo.gif

# コンテキスト付きキャプチャ
gaze capture --with-context --output /tmp/capture.json
# → { "image": "base64...", "url": "https://...", "app": "Chrome", "git_branch": "feat/new-ui" }
```

Claude Codeは `Bash` ツールからこれを直接呼び出し、`Read` ツールで画像を表示できる。

### 3.4 既存のスクショMCPサーバーとの差別化

| 既存MCP | 限界 | Gazeの優位性 |
|---------|------|-----------------|
| universal-screenshot-mcp | 基本的なOS screencaptureのみ | LLM最適化、アノテーション、OCR |
| macos-screen-mcp | ウィンドウ一覧・キャプチャのみ | 動画、バースト、コンテキスト |
| ScreenMonitorMCP | リアルタイム監視特化 | オンデマンド + 最適化パイプライン |

---

## 4. クラウド共有 & コラボレーション

### 4.1 クラウド機能の価値

| 用途 | 説明 | ターゲット |
|------|------|----------|
| **即時共有** | キャプチャ→1クリックで共有リンク生成 | 全ユーザー |
| **チーム共有ライブラリ** | プロジェクト単位でスクショを共有・検索 | チーム |
| **バグレポート共有** | スクショ+コンテキスト+コンソールログをまとめて送信 | 開発チーム |
| **クライアント共有** | パスワード付きリンクで外部にデザイン共有 | デザイナー |

### 4.2 共有リンクの仕組み

```
キャプチャ → クラウドアップロード → 短縮URL生成 → クリップボードにコピー

例: https://gaze.app/s/abc123
```

**リンクの機能**:
- **OGプレビュー**: Slack/Discord/Twitterでリッチプレビュー表示（1200x630pxサムネイル自動生成）
- **有効期限**: 1時間 / 24時間 / 7日 / 30日 / 無期限
- **パスワード保護**: オプションでパスワード設定
- **閲覧分析**: 閲覧回数、閲覧者のタイムスタンプ
- **自動削除**: 期限切れで自動クリーンアップ

### 4.3 プライバシー設計

CleanShot Cloudの最大の不満は「24時間で動画が自動削除される」「1GB制限がすぐ枯渇する」こと。

**Gazeのアプローチ**:
- **ローカルファースト**: デフォルトは全てローカル保存。クラウドはオプトイン
- **E2E暗号化**: アップロード時にクライアント側で暗号化。サーバーは暗号文しか保持しない
- **セルフホストオプション**: 企業向けにS3/R2互換のセルフホスト設定
- **明確な保持ポリシー**: 無料=7日、Pro=90日、Team=無制限（明示的に表示）

### 4.4 クラウド料金プラン

| Tier | ストレージ | 保持期間 | 価格 |
|------|----------|---------|------|
| Free | 100MB | 7日 | $0 |
| Pro | 5GB | 90日 | $2/mo（Pro+に含む） |
| Team | 50GB/チーム | 無制限 | $5/user/mo |
| Enterprise | 無制限 | 無制限 | カスタム |

### 4.5 バックエンド技術

- **ストレージ**: Cloudflare R2（S3互換、エグレス無料、グローバルCDN）
- **API**: Cloudflare Workers（エッジコンピューティング、低レイテンシ）
- **認証**: Apple Sign In + email（シンプル）
- **暗号化**: libsodium（E2E encryption）

---

## 5. スマホ ↔ PC 連携

### 5.1 ユースケース

| シーン | 説明 | 需要 |
|--------|------|------|
| **スマホ画面をMacに送信** | モバイルアプリのUI問題をMac上のLLMに送りたい | 高 |
| **スマホで撮った写真をLLM最適化** | 書類、ホワイトボード、名刺等をLLM用に最適化 | 高 |
| **Macのキャプチャをスマホで確認** | 外出先でチームメンバーのスクショを確認 | 中 |
| **モバイルバグレポート** | スマホアプリのバグをスクショ+コンテキストでMacに送信 | 高 |
| **ペアリングでリアルタイム転送** | スマホをサブキャプチャデバイスとして使用 | 中 |

### 5.2 技術アプローチ

#### Option A: iCloud / CloudKit同期（推奨）

```
iPhone Gaze → iCloud → Mac Gaze
                     ↓
              LLM最適化パイプライン
```

- **利点**: Appleエコシステムにネイティブ、ゼロ設定、自動同期
- **実装**: CloudKit + NSUbiquitousKeyValueStore
- **制約**: Apple端末のみ（Android非対応）

#### Option B: ローカルネットワーク（Bonjour/mDNS）

```
iPhone Gaze ←→ [同一Wi-Fi] ←→ Mac Gaze
         MultipeerConnectivity / Bonjour
```

- **利点**: クラウド不要、超低レイテンシ、プライバシー最大
- **実装**: MultipeerConnectivity framework
- **制約**: 同一ネットワーク必須

#### Option C: クラウド経由（前述のクラウド機能を利用）

```
iPhone Gaze → Cloudflare R2 → Mac Gaze
```

- **利点**: ネットワーク不問、Android対応可能
- **制約**: レイテンシ、クラウド依存

#### 推奨: Option A + B のハイブリッド

- 同一ネットワーク時: Bonjour/MultipeerConnectivityで即時転送（AirDrop的UX）
- 外出時/別ネットワーク時: iCloud経由で自動同期
- クラウド共有: 非Apple端末ユーザーへの共有はクラウドリンク

### 5.3 iPhone版の機能

**Mac版との差別化** — iPhoneは「キャプチャデバイス」として特化:

| 機能 | 説明 |
|------|------|
| **カメラ→LLM最適化** | ホワイトボード、書類、名刺を撮影→歪み補正→LLM最適化→Macに送信 |
| **スクリーンショット転送** | iOS標準スクショを検出→自動でMacに転送→LLM最適化 |
| **Share Extension** | 任意のアプリから「Gazeで送信」で即座にMacに転送 |
| **QRコードペアリング** | Mac画面のQRをスマホで読むだけで接続 |
| **ライブプレビュー** | スマホカメラのリアルタイム映像をMac上でプレビュー → ベストショットをキャプチャ |

### 5.4 Continuity Camera 連携

macOS Ventura以降の**Continuity Camera**を活用:
- iPhoneをMacのWebカメラとして使用する既存機能
- Gazeはこれを拡張し「iPhoneカメラで物理世界をキャプチャ→Mac上のLLMに送信」を実現
- Desk View（俯瞰撮影）でドキュメントスキャンに特化したモードも可能

---

## 6. コンテキストキャプチャ — メタデータ自動収集

### 6.1 なぜコンテキストが重要か

スクリーンショットは「ある瞬間の画面」だが、AIにとって有用なのは「その瞬間の状況全体」。

**自動収集可能なコンテキスト**:

| メタデータ | 取得方法 (macOS) | 用途 |
|-----------|----------------|------|
| アクティブURL | Accessibility API / AppleScript | ページの特定 |
| アプリ名 + ウィンドウタイトル | `NSWorkspace.shared.frontmostApplication` | コンテキスト |
| Git branch + commit | `git rev-parse --abbrev-ref HEAD` | コードの状態 |
| 直近のターミナル出力 | iTerm2 API / Accessibility API | エラーコンテキスト |
| 画面解像度 + スケール | `NSScreen.main` | 再現性 |
| タイムスタンプ (UTC + ローカル) | Foundation | 時系列 |
| クリップボード内容 | `NSPasteboard.general` | 関連情報 |
| 環境変数（選択的） | `ProcessInfo.processInfo.environment` | デバッグ情報 |

### 6.2 出力フォーマット

```json
{
  "capture": {
    "image": "base64:...",
    "format": "png",
    "dimensions": {"width": 1920, "height": 1080, "scale": 2},
    "optimized_for": "claude",
    "token_estimate": 1334
  },
  "context": {
    "timestamp": "2026-03-25T11:30:00+09:00",
    "app": "Google Chrome",
    "window_title": "localhost:3000 - My App",
    "url": "http://localhost:3000/dashboard",
    "git": {
      "branch": "feat/new-dashboard",
      "commit": "a1b2c3d",
      "dirty": true
    },
    "terminal_last_output": "Error: Cannot read property 'map' of undefined\n    at Dashboard.tsx:42"
  },
  "ocr_text": "Dashboard\nTotal Users: 1,234\nError: Failed to load chart data"
}
```

---

## 7. AI搭載機能 — スマートツール

### 7.1 プレキャプチャ・スマートリダクション

**Scribeのアプローチに触発**: 機密情報をキャプチャする**前に**ブラーをかける。画像ファイルに機密情報が一切残らない。

**自動検出対象**:
- APIキー/トークン（`sk-`, `ghp_`, `Bearer`, `AKIA...`）
- メールアドレス
- IPアドレス・内部ホスト名
- クレジットカード番号
- `.env`ファイルの値
- データベース接続文字列

**実装**: Vision frameworkのOCR + 正規表現パターンマッチングで検出 → ブラーレイヤーをリアルタイムでオーバーレイ → キャプチャ。

### 7.2 スマートアノテーション

**AIが「注目すべき場所」を自動提案**:
- エラーメッセージを赤い矩形で強調
- UIの変更点をハイライト
- フォーカスエリアに自動でアロー追加

### 7.3 Screenshot-to-Code

キャプチャしたUIを即座にReact/HTML/Tailwindコードに変換:
- 「Copy as Code」ボタン
- LLM APIを呼び出してコード生成
- デザインモックアップ → 実装の橋渡し

### 7.4 デュアルクリップボード

キャプチャ時に**画像とテキストを同時に**クリップボードに格納:

| ペースト先 | 出力 |
|-----------|------|
| Slack / Discord | 画像 |
| ターミナル / コードエディタ | OCRテキスト |
| GitHub Issue | Markdown（`![screenshot](url)` + テキスト引用） |
| LLMアプリ | 最適化画像 |

ペースト先のアプリを自動判別して最適な形式で出力する**コンテキストアウェア・ペースト**。

---

## 8. インテグレーション・エコシステム

### 8.1 優先インテグレーション

| 優先度 | 連携先 | 機能 | 実装 |
|--------|--------|------|------|
| 1 | **Claude Code / Cursor** | MCP Server | stdio MCP |
| 2 | **GitHub** | Issue/PRにスクショ直接添付 | GitHub API |
| 3 | **Slack** | ワンキーでチャンネルに共有 | Slack API |
| 4 | **Linear / Jira** | スクショからバグチケット自動生成 | REST API |
| 5 | **Notion** | ドキュメントに直接挿入 | Notion API |
| 6 | **Raycast** | Raycast拡張機能 | Raycast Extension API |
| 7 | **Figma** | デザインとの比較diff | Figma API |

### 8.2 ワンキー・バグレポート

**キラーフィーチャー**: 1キーストロークで完全なバグレポートを生成

```
⌘+Shift+B (カスタマイズ可能)
  ↓
1. 画面キャプチャ
2. コンソールログ取得（ブラウザMCP経由）
3. ネットワークリクエスト取得
4. Git branch + commit取得
5. 環境情報収集
  ↓
Linear/GitHub Issueを自動作成:
  - タイトル: AI生成（スクショからエラーを読み取り）
  - 画像: 最適化スクショ添付
  - 説明: コンテキスト情報をMarkdownで構造化
  - ラベル: "bug" 自動付与
```

### 8.3 Webhook / API

開発者が自動化に使えるWebhook:

```bash
# キャプチャイベントをWebhookで送信
POST https://your-server.com/webhook
{
  "event": "capture",
  "image_url": "https://gaze.app/s/abc123",
  "context": { ... },
  "timestamp": "2026-03-25T11:30:00Z"
}
```

---

## 9. 追加マネタイゼーション

### 9.1 改訂版料金プラン

| Tier | 価格 | 内容 |
|------|------|------|
| **Free** | $0 | 基本キャプチャ（1日20回）、基本アノテーション、ローカル保存のみ |
| **Pro** | $19 一括 | フル機能、LLM最適化、OCR、バースト、GIF録画、1年アップデート |
| **Pro+** | $4.99/mo | Pro + クラウド共有(5GB) + スマホ連携 + AI機能（リダクション、スマートアノテーション） |
| **Team** | $8/user/mo | Pro+ + チーム共有ライブラリ + インテグレーション（Slack, GitHub, Linear） + 管理者機能 |
| **API/MCP** | Pro+に含む | CLI + MCP Server + Webhook（商用利用は別途） |

### 9.2 収益ドライバー

```
Free → Pro（一括$19）: LLM最適化の価値で転換
Pro → Pro+（$4.99/mo）: クラウド + スマホ連携 + AI機能で転換
個人 → Team（$8/user/mo）: チーム共有 + インテグレーションで転換
```

---

## 10. 改訂版ロードマップ

### Phase 1: MVP — Core Capture (Month 1-2)
- [ ] Swift/SwiftUI プロジェクトセットアップ
- [ ] ScreenCaptureKit でエリア/ウィンドウ/全画面キャプチャ
- [ ] LLM最適化パイプライン（Claude/GPT/Gemini プロファイル）
- [ ] クリップボード自動コピー
- [ ] メニューバーUI + グローバルホットキー
- [ ] 基本アノテーション（矢印、矩形、テキスト、番号、ブラー）

### Phase 2: AI Features (Month 3-4)
- [ ] OCR抽出モード（Vision framework）
- [ ] GIF/動画録画 + キーフレーム抽出
- [ ] バーストモード
- [ ] スマートクロップ
- [ ] トークンコスト見積もり表示
- [ ] プロバイダプロファイル切替UI

### Phase 3: Developer Integration (Month 4-5)
- [ ] MCP Server（Claude Code / Cursor対応）
- [ ] CLI ツール
- [ ] コンテキストキャプチャ（URL, app, git branch）
- [ ] Product Hunt / Hacker News ローンチ

### Phase 4: Cloud & Sharing (Month 6-8)
- [ ] クラウドアップロード + 共有リンク（Cloudflare R2）
- [ ] OGプレビュー生成
- [ ] 有効期限 + パスワード保護
- [ ] スマートリダクション（プレキャプチャ）

### Phase 5: Mobile & Ecosystem (Month 9-12)
- [ ] iPhone版（キャプチャ + Mac転送）
- [ ] iCloud同期 + Bonjour ローカル転送
- [ ] Slack / GitHub / Linear インテグレーション
- [ ] Raycast拡張機能
- [ ] ワンキー・バグレポート

### Phase 6: Team & Scale (Month 12+)
- [ ] チーム共有ライブラリ
- [ ] 管理者ダッシュボード
- [ ] セルフホストオプション
- [ ] Webhook / API
- [ ] Screenshot-to-Code（AI）

---

## 11. 競合との最終比較マトリクス

| 機能 | CleanShot X | LazyScreenshots | Loom | **Gaze** |
|------|------------|----------------|------|-------------|
| スクリーンショット | ◎ | ○ | ✗ | ◎ |
| GIF録画 | ◎ | ✗ | ✗ | ◎ |
| 動画録画 | ◎ | ✗ | ◎ | ○ |
| LLM最適化 | ✗ | △（ペーストのみ） | ✗ | **◎** |
| OCR | ○ | ✗ | ✗ | **◎** |
| MCP Server | ✗ | ✗ | ✗ | **◎** |
| CLI | ✗ | ✗ | ✗ | **◎** |
| コンテキストキャプチャ | ✗ | ✗ | △（トランスクリプト） | **◎** |
| スマートリダクション | ✗ | ✗ | ✗ | **◎** |
| クラウド共有 | ◎ | ✗ | ◎ | ○ |
| スマホ連携 | ✗ | ✗ | ○ | **◎** |
| チーム機能 | △ | ✗ | ◎ | ○ |
| 1キー・バグレポート | ✗ | ✗ | ✗ | **◎** |
| 価格 | $29+$19/yr | $29 | $0-24/mo | **$19（or $4.99/mo）** |

---

## Appendix: 追加調査情報源

### MCP / AIエージェント連携
- [Claude Code MCP Documentation](https://code.claude.com/docs/en/mcp)
- [macos-screen-mcp](https://github.com/jhead/macos-screen-mcp) — macOS特化スクショMCP
- [universal-screenshot-mcp](https://github.com/sethbang/mcp-screenshot-server) — マルチOS対応
- [Visual Feedback Loop - Agentic Coding Handbook](https://tweag.github.io/agentic-coding-handbook/WORKFLOW_VISUAL_FEEDBACK/)
- [Anthropic Computer Use Tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool)

### GIF / 動画
- [GitHub: Video/GIF support in markdown](https://github.com/orgs/community/discussions/8864)
- [Gifox](https://gifox.app) — Mac GIF録画ツール
- [Kap](https://getkap.co) — オープンソースGIF/動画録画
- [Gifski](https://gif.ski) — 高品質GIF変換

### クラウド共有
- [CleanShot Cloud](https://cleanshot.com/pricing)
- [Droplr](https://droplr.com/) — スクショ共有SaaS
- [Zipline](https://github.com/diced/zipline) — セルフホスト共有サーバー
- [Cloudflare R2](https://developers.cloudflare.com/r2/) — S3互換エグレス無料ストレージ

### AI機能
- [Scribe Smart Blur](https://scribehow.com/library/smart-blur) — プレキャプチャリダクション
- [DocsHound Auto Annotations](https://docshound.com/use-cases/screenshot-annotations) — AI自動アノテーション
- [screenshot-to-code](https://github.com/abi/screenshot-to-code) — UIスクショ→コード変換
