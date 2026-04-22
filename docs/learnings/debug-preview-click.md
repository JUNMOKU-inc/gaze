# Preview Window Click Debug Log

## Goal
プレビューポップアップの画像クリックでexpand/collapseする機能を実装

## Architecture
- プレビューウィンドウは **Rust-driven HTML** (`preview_window.rs`)
- `WebviewWindowBuilder::new()` で `preview.html` をロード
- `initialization_script()` で `window.__GAZE_DATA__` と `window.__GAZE_LABEL__` を設定
- `window.eval("document.open();document.write(...)；document.close()")` でHTML全体を差し替え
- 既存のJS (Copy/Save/Dismiss) はHTML内の `<script>` タグに記述

## Experiments

### Experiment 1: onclick attribute on .thumb-area
- **What**: HTMLの `.thumb-area` に `onclick="toggleExpand()"` を追加
- **Result**: クリックしても何も起きない
- **Analysis**: onclick属性が効いていない

### Experiment 2: accept_first_mouse(true)
- **What**: macOSの非フォーカスウィンドウへの初回クリック問題を疑い、`accept_first_mouse(true)` を追加
- **Result**: 変化なし
- **Analysis**: フォーカス問題ではない

### Experiment 3: showToast debug in toggleExpand
- **What**: `toggleExpand()` 冒頭に `showToast('click!')` を追加
- **Result**: toastが変化しない
- **Analysis**: `toggleExpand()` 関数自体が呼ばれていない → JS関数が未定義

### Experiment 4: Separate eval() with addEventListener
- **What**: `document.write()` の**後**に別の `window.eval()` でクリックハンドラを注入
  ```js
  setTimeout(function() {
      var ta = document.querySelector('.thumb-area');
      ta.style.border = '2px solid lime';
      ta.addEventListener('click', function() {
          document.getElementById('toast-text').textContent = 'CLICK DETECTED!';
      });
  }, 300);
  ```
- **Result**: 緑枠表示 YES, クリック検出 YES
- **Analysis**: `eval()` 経由のJS実行とDOMイベントは正常に動作する

### Experiment 5: Check __TAURI__ availability
- **What**: `document.write()` 後に `typeof window.__TAURI__` と `typeof window.__GAZE_DATA__` を表示
- **Result**: `TAURI=undefined DATA=object`
- **Analysis**:
  - `document.write()` が Tauri IPC bridge (`window.__TAURI__`) を破壊する
  - `initialization_script` で設定した `window.__GAZE_DATA__` は生存する
  - **既存のCopy/Save/Dismissも実は動いていなかった可能性大**

### Experiment 6: Save invoke reference in initialization_script
- **What**: `initialization_script` に `window.__GAZE_INVOKE__ = window.__TAURI__.core.invoke` を追加。JS側で `__GAZE_INVOKE__` を使用。
- **Result**: 動かない
- **Analysis**: `invoke` 関数の参照は保存できても、その内部で使われるIPC channelが `document.write()` で破壊されている可能性

## Key Finding
**`document.write()` は WKWebView の Tauri IPC bridge を完全に破壊する。**

`window.__TAURI__` オブジェクト自体が消失し、`initialization_script` で保存した関数参照も内部依存が壊れて動作しない。

## Root Cause (Confirmed)

`document.open()` + `document.write()` はブラウザ仕様上、documentを完全に再作成する。
WKWebViewではTauriのIPC bridgeは `WKUserScript` (`AtDocumentStart`) で注入されるが、
`document.open()` は新しいnavigationとして扱われないため、IPC初期化スクリプトが再実行されない。
結果、`window.__TAURI__` が永久に消失する。

- Tauri Issue #8926 で確認済み
- MDN: `document.open()` removes all event listeners and nodes
- `initialization_script` で設定した値 (`__GAZE_DATA__`) は window object に直接代入されたため生存
- `invoke` 関数参照を保存しても、内部で使う `window.webkit.messageHandlers.ipc` が破壊されているため動作しない

## Solution

`document.documentElement.innerHTML = ...` に変更する。
- DOMノードのみ置換し、IPC bridgeは保持される
- `<script>` タグは innerHTML 経由では実行されないが、別途 `eval()` で注入するため問題なし
- HTMLテンプレートから `<!DOCTYPE html><html>...</html>` ラッパーを除去し、head+body部分のみ返す
