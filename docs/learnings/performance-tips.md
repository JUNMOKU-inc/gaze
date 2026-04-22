# パフォーマンス最適化パターン

> Gaze 開発で得たパフォーマンス知見。

## 計測手法

### tracing スパンによるステップ計測

各処理ステップを `tracing::info_span!` で囲み、`FmtSpan::CLOSE` で自動的に `time.busy` を記録する。手動の `Instant::now()` より保守性が高い。

```rust
let result = {
    let _s = tracing::info_span!("step.resize", from = %"3420x2214", to = %"1568x1015").entered();
    img.resize_exact(w, h, FilterType::Lanczos3)
};
// ログ: step.resize: close time.busy=152ms
```

### ログの2層出力

- **コンソール**: 人間が読みやすいフォーマット（開発時）
- **JSON ファイル**: 構造化データ（分析・サポート用）
- 保存先: `~/Library/Application Support/com.gaze.Gaze/logs/`
- 日次ローテーション、最大7日保持

## 実測結果（2026-03-26）

### Retina フルスクリーン (3420×2214) キャプチャパイプライン

| ステップ | debug (未最適化) | opt-level=3 | 改善率 |
|---------|-----------------|-------------|--------|
| screencapture 実行 | ~380ms | ~380ms | - |
| PNG デコード | ~390ms | ~379ms | - |
| RGBA 変換 | ~22ms | 3.9ms | 5.6x |
| **Lanczos3 リサイズ** | **~8秒** | **152ms** | **53x** |
| WebP エンコード | (上に含む) | 9.0ms | - |
| クリップボードコピー | ~163ms | 7.1ms | 23x |
| base64 エンコード | ~3ms | 3.0ms | - |
| **合計** | **~8.9秒** | **563ms** | **16x** |

### ボトルネックの法則

画像処理パイプラインでは:
1. **リサイズが支配的** — Lanczos3 は計算量 O(n²) で画像サイズに敏感
2. **debug build の最適化なしは致命的** — CPU バウンドな処理は 10-50倍遅くなる
3. **クリップボードの二重デコードは無駄** — RGBA → encode → decode → RGBA は避ける

## 適用パターン

### dev profile で画像クレートだけ最適化

```toml
[profile.dev.package.image]
opt-level = 3
[profile.dev.package.snapforge-pipeline]
opt-level = 3
```

ビルド時間への影響は初回のみ数秒増。日常の開発体験は変わらない。

### クリップボードコピーで二重デコードを回避

`optimize_image()` の戻り値に `encoded` (WebP/PNG) と `rgba` (生データ) の両方を含めることで、クリップボードコピー時にデコードし直す必要がなくなる。

```rust
pub struct OptimizeResult {
    pub encoded: Vec<u8>,  // LLM 入力用
    pub rgba: Vec<u8>,     // クリップボード用（二重デコード回避）
    pub width: u32,
    pub height: u32,
}
```

163ms → 7ms の改善。
