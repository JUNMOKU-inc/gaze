# ADR-002: プレビューウィンドウのIPC bridge保全とJS実行アーキテクチャ

> 作成日: 2026-03-27
> ステータス: **Accepted**
> 関連: ADR-001 (Preview Window Architecture)

## Context

プレビューポップアップに画像クリックでexpand/collapse機能を追加する際、Tauri IPCが動作しない問題が発生。
6回の実験を経て根本原因を特定し、アーキテクチャを全面的に見直した。

## 問題: `document.write()` が Tauri IPC bridge を破壊する

### 旧アーキテクチャ（動作しない）

```
Rust: WebviewWindowBuilder::new(app, label, "preview.html")
  → .initialization_script("window.__GAZE_DATA__ = ...")
  → .build()
  → window.eval("document.open(); document.write(html); document.close()")
```

**なぜダメか:**

1. `document.open()` はブラウザ仕様上、ドキュメントを完全に再作成する（MDN参照）
2. WKWebView の Tauri IPC bridge は `WKUserScript` (`AtDocumentStart`) で注入される
3. `document.open()` は新しいnavigationとして扱われないため、IPC初期化スクリプトが**再実行されない**
4. 結果、`window.__TAURI__` が**永久に消失**する
5. `initialization_script` で設定した値 (`__GAZE_DATA__`) は `window` オブジェクトに直接代入されたため生存するが、`__TAURI__` は消える

**Tauri Issue #8926 で確認済みの既知問題。**

### 試みたが失敗したアプローチ

| アプローチ | 結果 | 理由 |
|-----------|------|------|
| `document.write()` 内の `<script>` タグ | JS未実行 | WKWebView は document.write() 内のscriptを実行しない |
| `document.documentElement.innerHTML = html` | IPC消失 | head内のTauri bridge scriptタグが除去される |
| `initialization_script` で invoke 関数参照を保存 | invoke失敗 | 関数参照は残るが、内部の `window.webkit.messageHandlers.ipc` が破壊済み |
| `eval()` 後に別の `eval()` でJS注入 | DOM操作は可能、IPC不可 | `__TAURI__` 自体が存在しない |

### 決定打: `withGlobalTauri` 未設定

上記の問題に加え、根本的な設定不足があった:

- **`tauri.conf.json` に `"withGlobalTauri": true` が未設定だった**
- React側は `@tauri-apps/api` からimportするためバンドラー経由でIPC bridgeが利用可能
- plain HTML（preview.html）はバンドラーなしのため、グローバル `window.__TAURI__` が必要
- この設定なしでは、どのアプローチを使っても `window.__TAURI__` は plain HTML ページで利用不可

## 正しいアーキテクチャ（現行）

```
Rust: WebviewWindowBuilder::new(app, label, "preview.html")
  → .initialization_script("window.__GAZE_DATA__ = ...; window.__GAZE_LABEL__ = ...")
  → .accept_first_mouse(true)
  → .build()
  → position_window() / configure_macos_window()
  // eval() は一切不要
```

```
preview.html: 自己完結型のページ
  → DOMContentLoaded 後に boot() 関数が起動
  → window.__GAZE_DATA__ と window.__TAURI__ の存在を16msポーリングで待機
  → 両方揃ったらデータをDOMに反映し、イベントリスナーを登録
```

**なぜこれが正しいか:**

1. **IPC bridge が保持される**: ページを差し替えないため、Tauri が注入した `window.__TAURI__` が生存
2. **`<script>` タグがネイティブ実行**: `document.write()` や `innerHTML` 経由ではなく、ページ自身のscriptなので確実に実行
3. **タイミング安全**: ポーリングで `__TAURI__` と `__GAZE_DATA__` の両方を待つため、初期化順序に依存しない
4. **コードが簡潔**: Rust側のHTML/JS生成コード ~280行を削除、preview.html に集約

## 必須設定チェックリスト

新しいWebViewウィンドウ（plain HTML）を作成する際は以下を**必ず**確認:

### tauri.conf.json
```json
{
  "app": {
    "withGlobalTauri": true  // ← 必須
  }
}
```

### capabilities/default.json
```json
{
  "windows": ["main", "preview-*"]  // ← ウィンドウラベルパターンを含める
}
```

### Rust (WebviewWindowBuilder)
```rust
WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::App("page.html".into()))
    .accept_first_mouse(true)       // macOS: 非フォーカスウィンドウでもクリック受付
    .initialization_script(format!(  // データ注入
        "window.__MY_DATA__ = {json};",
    ))
    .build()
// ⚠️ window.eval() で document.write() / innerHTML を使わないこと
```

### HTML
```html
<script>
function boot(attempt) {
    // __TAURI__ と データ の両方を待つ
    if (!window.__TAURI__ || !window.__MY_DATA__) {
        if (attempt < 200) setTimeout(function() { boot(attempt + 1); }, 16);
        return;
    }
    var invoke = window.__TAURI__.core.invoke;
    // ここでDOM操作とイベントリスナー登録
}
boot(0);
</script>
```

## 禁止事項

| 操作 | 影響 |
|------|------|
| `document.open()` / `document.write()` | IPC bridge 完全破壊 |
| `document.documentElement.innerHTML = ...` | head内のTauri bridge script除去 |
| `document.body.innerHTML = ...` + `eval(js)` | script実行タイミング不定、IPC不安定 |
| `withGlobalTauri` 未設定でplain HTML | `window.__TAURI__` 未注入 |

## 実験ログ

6回の実験の詳細は `docs/learnings/debug-preview-click.md` に記録。
