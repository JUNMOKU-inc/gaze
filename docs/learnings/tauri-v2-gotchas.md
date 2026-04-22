# Tauri v2 プラットフォーム制約集

> 実開発で踏んだ地雷の記録。コードを書く前に確認すること。

## Window / Webview

### ウィンドウ作成はメインスレッドのみ

`WebviewWindowBuilder::build()` はメインスレッドでしか動作しない。バックグラウンドスレッドから呼ぶとデッドロックする（パニックもエラーも出ずに無言でハング）。

```rust
// NG: バックグラウンドスレッドから直接呼ぶ
std::thread::spawn(move || {
    WebviewWindowBuilder::new(app, "win", url).build(); // ← デッドロック
});

// OK: メインスレッドにディスパッチ
app.run_on_main_thread(move || {
    WebviewWindowBuilder::new(app, "win", url).build();
});
```

**発見経緯**: キャプチャ後にプレビューウィンドウを表示しようとしたが、`build()`が無言でハングした。ログにエラーもパニックも出ず、原因特定に時間がかかった。

### close() 後にウィンドウラベルが即座に解放されない

`window.close()` は非同期。同じラベル（例: `"preview"`）で新しいウィンドウを作ろうとすると、Tauriのレジストリに旧ウィンドウが残っているため**サイレントに失敗**する。

```rust
// NG: close直後に同じラベルで作成
window.close();
WebviewWindowBuilder::new(app, "preview", url).build(); // ← 失敗

// OK: 毎回ユニークなラベルを使う
let label = format!("preview-{}", counter.fetch_add(1, Ordering::Relaxed));
WebviewWindowBuilder::new(app, &label, url).build();
```

### Hidden webview は JS 実行を一時停止する場合がある

macOS の WebKit は、非表示（`.visible(false)` や `.hide()`）のウィンドウ内の WebView の JS 実行を一時停止することがある。Tauri イベント (`emit`) もリスナー (`listen`) も動作しない。

**影響**: 「hiddenでウィンドウを作成 → イベントでデータを送信 → フロントエンドがshow()を要求」という設計は構造的に壊れる。

**対策**: データが必要なウィンドウは最初から `visible(true)` で作成するか、データをHTML/initialization_scriptに埋め込む。

## macOS 固有

### NSWindow と WKWebView の両方を透明化する必要がある

ウィンドウ背景を透明にするには、NSWindow だけでなく WKWebView の `drawsBackground` も `NO` にする必要がある。片方だけだと白い矩形が残る。

```rust
// NSWindow
ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
ns_window.setOpaque(false);

// WKWebView (objc2 経由)
let _: () = msg_send![wk_view, setValue: NSNumber::numberWithBool(false),
                                forKey: ns_string!("drawsBackground")];
```

### スペース追随には NSWindowCollectionBehavior が必要

`always_on_top(true)` だけではデスクトップスペースを切り替えた時にウィンドウが追随しない。

```rust
ns_window.setCollectionBehavior(
    NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::Stationary,
);
```

### macOS のシステムショートカットとの競合

`Cmd+Shift+3/4` は macOS 標準のスクリーンショット。Tauri のグローバルショートカットで登録しても**両方が発火**する。`Alt+Shift+数字` 等を使うこと。

### グローバルショートカットが複数回発火する

Tauri v2 のグローバルショートカットは1回のキー押下で複数回コールバックが呼ばれることがある。500ms程度のデバウンスガードが必須。

```rust
static LAST: Mutex<Option<Instant>> = Mutex::new(None);

fn debounce_check() -> bool {
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if let Some(prev) = *last {
        if now.duration_since(prev).as_millis() < 500 { return false; }
    }
    *last = Some(now);
    true
}
```

## パフォーマンス

### dev build で image crate が極端に遅い

`image` crate の Lanczos3 リサイズは debug build で最適化なし → Retina フルスクリーン画像で **8秒以上**かかる。`profile.dev.package` で対象crateだけ `opt-level = 3` にする。

```toml
# Cargo.toml (workspace root)
[profile.dev.package.image]
opt-level = 3
[profile.dev.package.snapforge-pipeline]
opt-level = 3
```

**実測**: 8.9秒 → 563ms（16倍高速化）。特にリサイズが 8秒 → 152ms。

## HTML インジェクション

### document.write() のエスケープ

Rust の `format!()` で生成した HTML を JS テンプレートリテラルで `document.write()` する場合、以下の3つをエスケープする:

1. `\` → `\\` (バックスラッシュ)
2. `` ` `` → `` \` `` (バッククォート)
3. `${` → `\${` (テンプレートリテラル式展開)

### ユーザー入力の HTML エスケープ

`provider` 等のユーザー設定値を HTML に埋め込む場合、`&`, `<`, `>`, `"` をエスケープする。デスクトップアプリでも webview 内では XSS が成立し、`invoke()` 経由で Rust バックエンドにアクセスできてしまう。
