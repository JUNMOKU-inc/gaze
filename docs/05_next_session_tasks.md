# Gaze — 次回セッション作業指示

> **[ARCHIVED 2026-03-27]** React/TypeScript は撤去済み。
> 本ファイルのタスクの多くは CaptureOverlay (React) 等の旧実装を前提としており、現行アーキテクチャでは無効。
> 参考としてのみ保持。

> 作成日: 2026-03-25
> 対象: AI Agent（Claude Code）
> 前提: 前回セッションの全コミットをgit logで確認すること

---

## 目標

実装済み機能の完成度を磨き込む。新機能追加より「動いているものを正しく・美しく動かす」に集中。

---

## 優先順位

### P0: エリアキャプチャの修正（クリティカル）

#### Task 1: frozen image背景の表示

**現状**: エリア選択オーバーレイは表示されるが、背景にフリーズ画面が表示されていない。選択エリアも暗いグレーのまま（cutout効果が見えない）。

**原因調査**:
1. `CaptureOverlay.tsx` で `state` に `frozenImage` が正しく設定されているか（polling fallbackで取得した `imageBase64` が巨大すぎてbase64デコードに失敗？）
2. `CaptureOverlay.tsx` の ready/drawing/drawn フェーズで frozen image が CSS `background-image` として設定されているか確認
3. `DimLayer.tsx` の `clip-path` polygon が正しくcutoutを作っているか（選択エリアが明るく見えるはず）

**修正方針**:
- `CaptureOverlay.tsx` のレンダリング部分を確認。frozen imageを `<div>` の `backgroundImage` として `position: fixed; inset: 0` で表示すべき
- `DimLayer` はその上に `rgba(0,0,0,0.6)` で重ねて、選択エリアを `clip-path` でくり抜く
- レイヤー順序: frozen image (z:0) → dim layer with cutout (z:1) → selection border (z:2) → handles (z:3) → HUD (z:4)

**検証方法**:
```bash
pnpm tauri dev
# メニューバー → Area Capture
# 画面がフリーズされて暗くなること
# ドラッグした範囲が明るく（元の画面が見える）こと
```

#### Task 2: エリア確定 → クリップボードフロー

**現状**: Enter/Space押下後の `confirm_area_capture` 呼び出しが正しく動作するか未確認。

**確認ポイント**:
1. `CaptureOverlay.tsx` で Enter/Space 押下時に `invoke("confirm_area_capture", ...)` が呼ばれているか
2. 引数の `x, y, w, h, scaleFactor` が正しい値か（論理ピクセル）
3. Rust側 `confirm_area_capture` が frozen bitmap をクロップ → 最適化 → クリップボードコピーできているか
4. 確定後にオーバーレイが閉じて、プレビューポップアップが表示されるか

**修正が必要な場合**:
- `CaptureOverlay.tsx` の `handleConfirm` 関数を読んで、invokeのパラメータ名がRust側の `#[tauri::command]` と一致しているか確認
- Rust側は `x: f64, y: f64, w: f64, h: f64, scale_factor: f64` を受け取る（snake_case）
- フロントエンドは `{ x, y, w, h, scaleFactor }` で送る → Tauriが自動でsnake_caseに変換するか要確認。もし変換されないなら `scale_factor` で送る必要あり

**検証方法**:
```bash
pnpm tauri dev
# Area Capture → エリア選択 → Enter
# クリップボードに画像が入っていること（⌘V で確認）
# プレビューポップアップが表示されること
```

#### Task 3: オーバーレイのEscapeキャンセル

**現状**: Escape押下でオーバーレイが閉じるか未確認。

**確認ポイント**:
1. Escapeで `dismiss_overlay` Tauriコマンドが呼ばれるか
2. オーバーレイウィンドウが非表示になるか
3. frozen bitmapがメモリから解放されるか

---

### P1: プレビューポップアップの安定化

#### Task 4: プレビューポップアップの表示改善

**現状**: フルスクリーンキャプチャ時にプレビューポップアップが表示されるが、以下の問題あり:
- クリックすると消える（auto-dismiss or フォーカス喪失）
- サムネイルが正しく表示されるが、情報行（サイズ、トークン数）のデバッグUIが残っている

**修正**:
1. `src/App.tsx` からデバッグUI（"Loading...", "debugInfo", "Active captures:"）を削除し、正式なPreviewStack表示に戻す
2. プレビューウィンドウのフォーカス喪失で閉じないようにする
3. auto-dismissタイマーが正しく動作しているか確認（5秒後にフェードアウト）
4. ホバー時にタイマーが一時停止するか確認

#### Task 5: ホバーアクションボタンの動作確認

**現状**: PreviewPopupにはホバーで表示されるアクションボタン（Copy, Edit, OCR, Save, Dismiss）が実装されているが、動作未確認。

**確認ポイント**:
- ホバーでボタンが表示されるか
- 各ボタンの onClick ハンドラが実装されているか（少なくともCopy/Dismissは動くべき）
- キーボードショートカット（E, T, S, Esc）が動くか

---

### P2: フルスクリーンキャプチャの改善

#### Task 6: macOS標準ショートカットとの競合解消

**現状**: `⌘⇧3` はmacOS標準のスクリーンショットと競合する可能性がある。

**検討**:
- macOS標準スクショを無効化する方法（`defaults write com.apple.screencapture`）
- または別のショートカットにデフォルト変更（`⌘⇧+` や `⌃⇧3` など）
- 設定画面で変更可能にする（将来対応でOK）

#### Task 7: フラッシュアニメーション

**UX仕様**: キャプチャ確定時に画面が白くフラッシュする（80ms）。現在未実装。
- フルスクリーンキャプチャ: 全画面フラッシュ
- エリアキャプチャ: 選択エリアのみフラッシュ

---

### P3: コード品質

#### Task 8: デバッグコードの清掃

以下のファイルにデバッグ用の `console.log` や一時UIが残っている:
- `src/App.tsx` — debugInfo state, "Loading...", "Active captures:" 表示
- `src/components/CaptureOverlay/CaptureOverlay.tsx` — "Waiting for overlay-show event..." 表示、console.log

これらを削除し、正式な状態に戻す。ただし**P0タスク完了後**に行うこと（デバッグに必要な場合があるため）。

#### Task 9: テスト補強

現在のテスト: Rust 8 + Frontend 16 = 24テスト

追加すべきテスト:
- `overlayReducer` のユニットテスト（各状態遷移）
- `DimensionHUD` のトークン推定テスト
- `confirm_area_capture` の境界値テスト（ゼロサイズ、範囲外座標）
- `clipboard.rs` のテスト（モック可能な範囲で）

#### Task 10: codex review

全P0-P1タスク完了後に実行:
```bash
codex review --diff "HEAD~5..HEAD"
```

---

## 作業手順

1. **まず `git log --oneline -15` で前回の状態を確認**
2. **P0タスク（1→2→3）を順番に完了** — 各タスク完了ごとにコミット
3. **`pnpm tauri dev` で動作確認** — 実際に起動して手動テスト
4. **P1タスク（4→5）** — プレビューポップアップの安定化
5. **P3 Task 8** — デバッグコード清掃
6. **P3 Task 10** — codex review
7. **全テスト確認**: `cargo clippy && cargo test && pnpm typecheck && pnpm test`
8. **コミット & プッシュ**

---

## 注意事項

- Rust変更後は `pnpm tauri dev` の再起動が必要（HMRはフロントエンドのみ）
- `touch src-tauri/src/lib.rs` で強制再コンパイルを発動させられる
- macOS Screen Recording権限: ターミナルアプリ（Terminal.app）に付与する方式（CLAUDE.md参照）
- `tccutil reset ScreenCapture` は**絶対に使わない**（全アプリの権限がリセットされる）
- エージェントチーム活用: 独立タスクは並行実行可能（例: Task 1とTask 5は独立）
