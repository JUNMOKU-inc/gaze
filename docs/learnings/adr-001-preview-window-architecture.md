# ADR-001: プレビューウィンドウを Rust-driven 静的 HTML に変更

> Date: 2026-03-27
> Status: Accepted
> Deciders: @RQ-Akiyoshi + Claude Code + Opus review

## Context

スクリーンキャプチャ後に表示されるプレビューポップアップが、2回目以降のキャプチャで表示されない不具合が繰り返し発生していた。

### 旧アーキテクチャ（React ベース）

```
Rust: キャプチャ完了
  → metadata を AppState に保存
  → ensure_preview_window (webview を hidden で作成)
  → 300ms sleep ← ハック
  → emit("capture-complete") イベント
React (hidden webview 内):
  → イベント受信 → addCapture → activeCaptures 変化
  → useEffect → invoke("show_preview_window")
Rust:
  → window.show()
```

### 問題の構造

1. **制御権の分散**: Rust がウィンドウを作成し、React が表示タイミングを決め、Rust が show() する。3者間の調整が脆い。
2. **hidden webview の JS 一時停止**: macOS WebKit は hidden ウィンドウの JS を停止する場合がある → イベントが届かない → show() も呼ばれない。
3. **300ms sleep**: React マウント完了を sleep で待つのは本質的に不安定。
4. **close() のライフサイクル不一致**: close() 後もウィンドウラベルがレジストリに残る。

修正を5回以上試みたが、パッチではなく構造的な問題であることが判明。

## Decision

**React を排除し、Rust から直接インライン HTML を生成してウィンドウに注入する方式に変更。**

### 新アーキテクチャ

```
Rust: キャプチャ完了
  → ユニークラベルで新 webview 作成 (visible: true)
  → initialization_script でメタデータ注入
  → document.write() で HTML 全体を置換
  → macOS: NSWindow 透明化 + スペース追随設定
```

- **1オーナー**: Rust がウィンドウのライフサイクルを 100% 制御
- **イベント不要**: データは HTML にインライン
- **sleep 不要**: ウィンドウは最初から visible
- **スタッキング対応**: 各キャプチャが独立したウィンドウ（最大5つ）

## Alternatives Considered

### A: Full native (objc2/cocoa NSWindow)

- 利点: webview なし、完璧な制御
- 欠点: objc2 バインディングの複雑さ、macOS 専用、UI 変更にコンパイル必要
- 却下理由: 開発コストに見合わない。CSS で十分な UI にネイティブは過剰。

### C: 現行アーキテクチャ修正（React 維持）

- 利点: 最小変更
- 欠点: hidden webview 問題は構造的に残る。show() のタイミング問題も残る。
- 却下理由: 5回修正して直せなかった。パッチではなく設計変更が必要。

## Consequences

### Positive

- 2回目以降のキャプチャが確実に表示される
- 300ms sleep ハック廃止
- React/Zustand/イベントリスナーの複雑な状態管理が不要に
- スタッキング（複数プレビュー同時表示）が自然に実現

### Negative

- HTML テンプレートが Rust コード内に埋め込まれている（~230行）→ `include_str!()` で分離可能
- React コンポーネント（PreviewPopup, PreviewStack, useAutoDismiss 等）が死にコードになる
- 将来アノテーション等のリッチ UI が必要な場合、別途 React ウィンドウを作る必要がある

### Risks

- `document.write()` のエスケープ漏れによる表示崩壊 → P0 レビューで対応済み
- objc2 のバージョン互換性 → `msg_send!` の API が v0.5 と v0.6 で異なる

## Learnings

1. **「2つのランタイムでウィンドウ表示を共同管理する」設計は避ける** — 必ずどちらかが全権を持つべき。
2. **Tauri の webview は「アプリケーションウィンドウ」向き** — 通知的な ephemeral UI には過剰。使うなら Rust 側で完全制御する。
3. **問題を3回以上パッチで修正しようとしたら、設計を疑う** — 今回は5回目でようやくアーキテクチャ変更を決断した。最初から立ち止まるべきだった。
