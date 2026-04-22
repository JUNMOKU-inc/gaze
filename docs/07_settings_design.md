# Gaze — Settings Design

> 作成日: 2026-03-27
> ステータス: Draft

## Overview

メニューバーの「Settings...」（⌘,）から開く設定画面の設計。
設定画面はUI表示のみ。値のアプリケーション側での参照・適用は別タスクで対応する。

## Implementation Architecture

- `public/settings.html` — 自己完結型HTML（preview.htmlと同じパターン）
- `initialization_script` で現在の設定値をJSに渡す
- `invoke()` で設定変更をRust側に保存
- ADR-002準拠: `withGlobalTauri: true` + ポーリングboot

## Window Behavior

| Property | Value |
|----------|-------|
| サイズ | 560 x 480px |
| リサイズ | 不可 |
| デコレーション | あり（タイトルバー付き） |
| タイトル | Gaze Settings |
| 位置 | 画面中央 |
| シングルトン | YES（既に開いていたらフォーカス） |
| 閉じた時 | ウィンドウのみ閉じる（アプリ終了しない） |

## Settings Items

### General

| 項目 | Key | 型 | デフォルト | 説明 |
|------|-----|------|-----------|------|
| Language | `language` | enum | `en` | UI言語。`en` / `ja` |
| Launch at login | `launch_at_login` | bool | `false` | macOS ログイン時に自動起動 |

### Capture

| 項目 | Key | 型 | デフォルト | 説明 |
|------|-----|------|-----------|------|
| Auto-copy to clipboard | `auto_copy` | bool | `true` | キャプチャ後に自動でクリップボードにコピー |
| Output format | `output_format` | enum | `webp` | `png` / `webp` / `jpeg` |
| Max image dimension | `max_dimension` | object | `{ mode: "none" }` | 画像の最大サイズ制限（下記参照） |
| OCR enabled | `ocr_enabled` | bool | `true` | OCRテキスト抽出の有効/無効 |
| Smart masking | `smart_masking` | bool | `false` | メール・APIキー等の機密情報を自動マスク |

### Recording (GIF / Video)

| 項目 | Key | 型 | デフォルト | 説明 |
|------|-----|------|-----------|------|
| Max recording duration | `max_recording_sec` | number | `30` | 最大録画時間（秒）。5〜120 |
| GIF FPS | `gif_fps` | number | `10` | GIF出力時のフレームレート。5〜30 |

#### Max image dimension の詳細

3つのモードから選択:

| Mode | 説明 | 例 |
|------|------|------|
| `none` | リサイズなし（元画像のまま） | — |
| `max_width` | 幅の上限を指定。超える場合アスペクト比維持で縮小 | `{ mode: "max_width", pixels: 1568 }` |
| `max_height` | 高さの上限を指定。超える場合アスペクト比維持で縮小 | `{ mode: "max_height", pixels: 1024 }` |

- 拡大はしない（元画像が指定値より小さい場合はそのまま）
- ピクセル値の入力範囲: 100 〜 8000
- 参考プリセット表示: Claude推奨 1568px / GPT-4o推奨 2048px

### Shortcuts

| 項目 | Key | 型 | デフォルト | 説明 |
|------|-----|------|-----------|------|
| Area capture | `shortcut_area` | string | `Alt+Shift+2` | エリアキャプチャのショートカット |
| Fullscreen capture | `shortcut_fullscreen` | string | `Alt+Shift+3` | フルスクリーンキャプチャのショートカット |

- ショートカット入力はキーレコーダー方式（クリック → キー押下で記録）
- 競合チェック: macOS標準ショートカットとの衝突を警告

### Preview

| 項目 | Key | 型 | デフォルト | 説明 |
|------|-----|------|-----------|------|
| Preview position | `preview_position` | enum | `bottom_right` | プレビューの表示位置 |
| Max previews on screen | `max_previews` | number | `5` | 画面上の最大プレビュー数 (1〜10) |
| Save location | `save_location` | string | `~/Desktop` | 「Save」ボタン押下時の保存先 |

#### Preview position の選択肢

| Value | 説明 |
|-------|------|
| `bottom_right` | 右下（デフォルト） |
| `bottom_left` | 左下 |
| `top_right` | 右上 |
| `top_left` | 左上 |

4隅をビジュアルで選べるUI（4つの正方形をクリック）。

## UI Layout

```
┌─────────────────────────────────────────┐
│ Gaze Settings                       [x] │
├─────────────────────────────────────────┤
│                                         │
│  General                                │
│  ┌─────────────────────────────────────┐│
│  │ Language          [English ▾]       ││
│  │ Launch at login   [  toggle  ]      ││
│  └─────────────────────────────────────┘│
│                                         │
│  Capture                                │
│  ┌─────────────────────────────────────┐│
│  │ Auto-copy         [  toggle  ]      ││
│  │ Output format     [WebP ▾]          ││
│  │ Max dimension     [None ▾]          ││
│  │   └ pixels        [1568] px         ││
│  │ OCR              [  toggle  ]       ││
│  │ Smart masking    [  toggle  ]       ││
│  └─────────────────────────────────────┘│
│                                         │
│  Recording                              │
│  ┌─────────────────────────────────────┐│
│  │ Max duration      [30] sec          ││
│  │ GIF FPS           [10 ▾]            ││
│  └─────────────────────────────────────┘│
│                                         │
│  Shortcuts                              │
│  ┌─────────────────────────────────────┐│
│  │ Area capture      [⌥⇧2] [Record]   ││
│  │ Fullscreen        [⌥⇧3] [Record]   ││
│  └─────────────────────────────────────┘│
│                                         │
│  Preview                                │
│  ┌─────────────────────────────────────┐│
│  │ Position          [■ □]  (4隅)      ││
│  │                   [□ □]             ││
│  │ Max on screen     [5 ▾]             ││
│  │ Save location     [~/Desktop] [...]  ││
│  └─────────────────────────────────────┘│
│                                         │
└─────────────────────────────────────────┘
```

## CLI / MCP Considerations

設定値はGUIだけでなく、CLI・MCP Server からも参照される。以下の原則を守る:

### 原則: 設定はデフォルト値、CLIフラグで上書き可能

```bash
# settings.json のデフォルト値を使用
gaze capture

# CLIフラグでその場だけ上書き
gaze capture --format png --max-width 2048 --quiet
```

### CLI向け追加フラグ（settings.htmlでは設定しない）

| Flag | 説明 |
|------|------|
| `--quiet` / `-q` | プレビューポップアップを表示しない（CLI/MCP利用時） |
| `--output <mode>` | 出力先: `clipboard`（デフォルト）/ `stdout` / `file` |
| `--json` | メタデータ + base64 をJSON形式でstdoutに出力（MCP向け） |
| `--format <fmt>` | 出力フォーマットを一時的に上書き |
| `--max-width <px>` | 最大幅を一時的に上書き |
| `--max-height <px>` | 最大高さを一時的に上書き |

### settings.rs の設計指針

```rust
pub struct Settings {
    // ... all fields
}

impl Settings {
    /// Load from settings.json, falling back to defaults
    pub fn load() -> Self { ... }

    /// Merge with CLI overrides (CLI flags take precedence)
    pub fn with_overrides(self, overrides: CliOverrides) -> Self { ... }
}
```

設定の読み込み順序: **デフォルト値 → settings.json → CLIフラグ** （後勝ち）

## Data Storage

設定はJSON形式でアプリデータディレクトリに保存:

```
~/Library/Application Support/com.gaze.Gaze/settings.json
```

## Files to Create/Modify

| Action | File | Description |
|--------|------|-------------|
| Create | `public/settings.html` | 設定画面UI |
| Create | `src-tauri/src/settings.rs` | 設定の読み書き + Tauriコマンド |
| Modify | `src-tauri/src/lib.rs` | settings モジュール追加 + コマンド登録 |
| Modify | `src-tauri/src/tray.rs` | Settings... メニューハンドラ実装 |
| Modify | `src-tauri/capabilities/default.json` | settings-* ウィンドウパターン追加 |
