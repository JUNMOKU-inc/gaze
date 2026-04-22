# 13. GUI スモークテスト チェックリスト

## Overview

このドキュメントは **Gaze の tag 済みリリースビルドを配布する前に、人間（開発者）が手作業で実施する GUI スモークテスト** です。

macOS には Tauri v2 の WKWebView を制御できる公式 WebDriver が存在しないため、起動・トレイ・グローバルショートカット・権限ダイアログといった GUI 経路は **原則として自動化できません**。本チェックリストはその穴を埋めるための最終防衛ラインです。

- **実施タイミング**: `git tag v*` を打つ直前 / 依存クレートのメジャーアップグレード後 / macOS メジャーアップデート直後
- **所要時間**: 約 5 分（初回権限ダイアログをテストする場合は +30 秒）
- **前提条件**:
  - macOS 14 (Sonoma) 以降
  - Developer ID Application で署名＋公証済みの `.app`（Team: JUMMOKU K.K / `S7U7957MTG`、Bundle ID: `dev.gazeapp.gaze`）
  - できれば **クリーンなテストアカウント**、または `tccutil` で権限リセット済み環境
  - `.pkg` 配布版ではなく、`.app` 単体を `/Applications/gaze.app` に置く前提

---

## Pre-test setup (30 秒)

```bash
# 既存バージョンのアンインストール（プロセスが残っていたら先に Quit）
pkill -x snapforge-desktop 2>/dev/null
rm -rf /Applications/gaze.app

# （任意）Screen Recording 権限をリセットして初回フローを再現
tccutil reset ScreenCapture dev.gazeapp.gaze

# （任意）Apple Events 権限もリセット
tccutil reset AppleEvents dev.gazeapp.gaze

# 新バージョンをインストール
cp -R <path-to-built-app> /Applications/gaze.app

# Gatekeeper の隔離属性を外す（署名済みでも escape hatch として）
xattr -dr com.apple.quarantine /Applications/gaze.app

# 起動
open /Applications/gaze.app
```

> **注意**: `tccutil reset ScreenCapture`（引数なし）は **全アプリの権限を消す** ので絶対に使わないこと。必ず Bundle ID を指定する。

---

## Checklist

#### T1. 起動とメニューバーアイコン
**Action**: `open /Applications/gaze.app` を実行
**Expect**:
- [ ] Dock に `gaze` アイコンが **現れない**（`LSUIElement=true` の確認）
- [ ] メニューバー右側にアイコンが **ちょうど 1 つ** 表示される（起動〜2 秒以内）
- [ ] `ps -ef | grep snapforge-desktop | grep -v grep` で **1 プロセスのみ** 表示される
- [ ] `⌘Tab` のアプリスイッチャーに `gaze` が出てこない
**If fail**: `~/Library/Application Support/dev.gazeapp.gaze/logs/gaze.$(date +%Y-%m-%d).log` の末尾 30 行を確認

#### T2. トレイメニューの全項目表示
**Action**: メニューバーの Gaze アイコンを左クリック
**Expect**: 以下の順序で項目が表示される
- [ ] `Area Capture`（ショートカット `⌥⇧2` の表示）
- [ ] `Window Capture`（`⌥⇧1`）
- [ ] `Fullscreen Capture`（`⌥⇧3`）
- [ ] `OCR Capture`（`⌥⇧4`）
- [ ] `Record GIF`（`⌥⇧5`）
- [ ] 区切り線
- [ ] `Settings...`（`⌘,`）
- [ ] 区切り線
- [ ] `Quit`（`⌘Q`）
**If fail**: `tray.rs` の `setup_tray` と Rust ログで `System tray initialized` が出ているか確認

#### T3. Screen Recording 権限ダイアログ（初回のみ）
**前提**: Pre-test setup で `tccutil reset ScreenCapture dev.gazeapp.gaze` を実行済み
**Action**: トレイメニューから `Fullscreen Capture` を選択
**Expect**:
- [ ] システムダイアログが出現し、**本文に `Gaze captures your screen to send screenshots to LLMs.` の文字列が含まれる**（`Info.plist` の `NSScreenCaptureUsageDescription`）
- [ ] `システム設定を開く` を選び、`Gaze` を ON にして **Gaze を再起動するよう促される**
- [ ] 再起動後、再度 `Fullscreen Capture` を実行するとダイアログが出ずに撮影される
**所要時間**: 初回のみ 30 〜 60 秒
**If fail**: `Info.plist` の `NSScreenCaptureUsageDescription` キーが欠けている可能性

#### T4. Option+Shift+3 フルスクリーンキャプチャ
**Action**: 任意のアプリを最前面にして `⌥⇧3` を押下
**Expect**:
- [ ] シャッター音 or 視覚的フラッシュなしでも **1 秒以内にプレビューウィンドウがスクリーン右下に表示** される
- [ ] プレビューには **撮影画像のサムネイル**、ファイルサイズ、推定トークン数、プロバイダー名 (`claude`/`gpt`/`gemini`) が表示される
- [ ] プレビューは **Dock アイコンを生やさない**（T1 と同じ状態を維持）
- [ ] ログに `Global shortcut: fullscreen capture` が 1 回だけ記録される（500ms デバウンス確認）
**If fail**: ログで `Fullscreen capture failed` を grep

#### T5. Option+Shift+2 エリアキャプチャ
**Action**: `⌥⇧2` を押下
**Expect**:
- [ ] 画面がわずかに暗転し、**十字カーソル**（crosshair）に変化する
- [ ] ドラッグで矩形選択 → リリース時にプレビューが表示される
- [ ] `Esc` で選択をキャンセルすると **プレビューは出ず、ログに `Area capture cancelled by user` が出力**
**所要時間**: 約 10 秒

#### T6. Option+Shift+1 ウィンドウキャプチャ
**Action**: Safari / Finder など 2 枚以上のウィンドウを開いた状態で `⌥⇧1` を押下
**Expect**:
- [ ] 各ウィンドウ上にマウスオーバーでハイライトが出る or ウィンドウ選択 UI が現れる
- [ ] 選択したウィンドウ **1 枚だけ** が撮影され、プレビューに出る（背景やメニューバーは写り込まない）
- [ ] キャンセル時に `Window capture cancelled by user` がログに残る

#### T7. クリップボード確認
**Action**: T4〜T6 のいずれかの直後に以下を実行
```bash
osascript -e 'the clipboard as «class PNGf»' | head -c 8 | xxd
```
**Expect**:
- [ ] 出力の先頭が `8950 4e47 0d0a 1a0a`（PNG マジックバイト `\x89PNG\r\n\x1a\n`）になる
- [ ] Settings で `autoCopy` が false の場合は **何も貼り付かない**（クリップボードは撮影前の内容のまま）
**If fail**: `clipboard.rs` と settings の `auto_copy` を確認

#### T8. Settings ウィンドウと永続化
**Action**: トレイ `Settings...` を選択 → `Default provider` を `gpt` に変更 → ウィンドウを閉じる → Gaze を Quit → 再起動 → Settings を再度開く
**Expect**:
- [ ] Settings ウィンドウは 560x520 の固定サイズで中央に表示され、リサイズ不可
- [ ] セクションが `General / Capture / Recording / Shortcuts / Preview` の順で並ぶ
- [ ] `Default provider` を変更してウィンドウを閉じると `~/Library/Application Support/dev.gazeapp.gaze/settings.json` が更新される
- [ ] `cat` で確認: `"defaultProvider": "gpt"` が含まれる
- [ ] 再起動後も選択が保持されている
- [ ] `Launch at login` を ON → 再ログインで Gaze が自動起動する（チェックは手動で十分）

#### T9. GIF 録画 start/stop（サウンド確認）
**Action**: `⌥⇧5` を 1 回押す → 3 秒待つ → もう一度 `⌥⇧5`
**Expect**:
- [ ] 1 回目: **Submarine 音** が鳴り、画面の隅に録画中インジケーター（赤ドット等）が表示される
- [ ] 2 回目: **Hero 音** が鳴り、インジケーターが消え、プレビューに GIF が表示される（再生ループ）
- [ ] ログに `GIF recording started` → `GIF recording saved` が順に記録される
- [ ] 連打してもデバウンス（500ms）で 1 回だけトグルされる
**If fail**: `afplay /System/Library/Sounds/Submarine.aiff` が単体で鳴るか確認

#### T10. マルチディスプレイ環境
**前提**: 外付けディスプレイが 1 枚以上接続されている
**Action**: システム設定 → ディスプレイ → 配置 → 白いバー（プライマリメニューバー）を内蔵 → 外付けにドラッグで移動
**Expect**:
- [ ] **トレイアイコンが新しいプライマリディスプレイのメニューバーに移動する**（Gaze 再起動が必要な場合あり）
- [ ] `⌥⇧3` で撮影した画像はカーソルがあるディスプレイのものになる（全画面モード）
- [ ] プレビューウィンドウはカーソルのある display の `bottom_right` に表示される

#### T11. Quit と再起動後の復元
**Action**: トレイ `Quit`（`⌘Q`）→ `ps -ef | grep snapforge-desktop` でプロセス消滅を確認 → `open /Applications/gaze.app` で再起動
**Expect**:
- [ ] Quit 後、プロセスが **完全に消える**（子プロセス含めて 0 件）
- [ ] 再起動後、T8 で変更した `defaultProvider: gpt` が Settings に反映されている
- [ ] T1 と同じ初期状態に戻っている（メニューバーアイコン 1 つ、Dock なし）

#### T12. OCR Capture は **反応しないこと** を確認
**Action**: トレイから `OCR Capture` をクリック、および `⌥⇧4` を押下
**Expect**:
- [ ] **クリック／ショートカットのいずれも何も起こらない**（撮影ダイアログ・プレビューが出ない、クリップボードも変化しない）
- [ ] ログに `Tray: OCR capture requested (not yet implemented)` が記録される（トレイ経由の場合）
- [ ] `⌥⇧4` はそもそもグローバルショートカット登録されていないため無反応
**メモ**: 未実装機能の確認項目。**クラッシュや誤動作がないこと** を担保する。

---

## Known issues / not to be alarmed

- **OCR Capture は未実装**: T12 で「反応しない」のが正しい挙動
- **DMG 配布は廃止**: 現行は `.pkg` 配布。本チェックリストは **`.app` を `/Applications` に直置きした状態** を前提
- **Unsigned ビルドでは macOS 15 以降でトレイアイコンが表示されない**: 必ず Developer ID 署名＋公証済みの成果物をテストすること
- **初回起動時は `open` 経由で起動する**: Finder ダブルクリックだと Gatekeeper チェックが走って 2 秒ほど遅延する場合あり
- **`tccutil reset ScreenCapture`（引数なし）は全アプリの権限を吹き飛ばす**: 必ず `dev.gazeapp.gaze` を付ける

---

## If something fails

1. **ログを確認**
   ```bash
   tail -n 100 "$HOME/Library/Application Support/dev.gazeapp.gaze/logs/gaze.$(date +%Y-%m-%d).log"
   ```
2. **設定ファイルを確認**
   ```bash
   cat "$HOME/Library/Application Support/dev.gazeapp.gaze/settings.json"
   ```
3. **クラッシュレポートを取得**
   ```bash
   ls -lt "$HOME/Library/Logs/DiagnosticReports/" | grep -i gaze | head
   open "$HOME/Library/Logs/DiagnosticReports/"
   ```
4. **GitHub Issue テンプレート**

   ```markdown
   ### Environment
   - macOS: (例: 15.3.1)
   - Gaze version: (例: v0.2.0, git sha: xxxxxxx)
   - Display setup: (内蔵のみ / 外付け 1 枚 / 2 枚 …)

   ### 失敗した項目
   T? 〜 T?

   ### 再現手順
   1. …
   2. …

   ### 期待値 / 実際の挙動

   ### ログ抜粋
   ```
   (gaze.log の該当箇所 20 行程度)
   ```

   ### クラッシュレポート
   （あれば添付）
   ```

---

## Exit criteria

- **12 項目中 11 項目以上 PASS でリリース OK**
- ただし **T12（OCR 未実装）は「反応しないこと」を確認する項目なので失敗ゼロが条件**、スキップではなく必ず実施する
- **T3（初回権限ダイアログ）は権限リセット環境でのみ必須**、継続環境では見送り可
- **T10（マルチディスプレイ）は外付けディスプレイ未接続なら PASS 扱いにして良い**（ただしリリースノートに明記すること）
- PASS 10 以下の場合はリリース中止、該当項目を Issue 化して修正してから再テスト
