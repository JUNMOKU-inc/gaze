# Test Strategy & Implementation Plan

## Executive Summary

### テストは意味があるか？

**結論: 中程度の投資で、ターゲットを絞ったテストが最も効果的。**

#### 投資すべき理由
- **画像最適化パイプライン**はコアの差別化要素。ここのバグ（寸法誤り、トークン推定ミス）は直接ユーザー体験を損なう
- **設定の永続化**バグはデータ喪失とユーザー不満を引き起こす
- 初期段階のコードベースは高速で変化する。テストがリグレッションを防ぐ
- `capture_flow.rs`（全キャプチャが通る中核パス）にテストが0件 — 最大のリスク

#### 過剰投資を避けるべき理由
- 参考プロジェクトCap（同じTauri v2、より大規模）はRustテストフレームワーク導入を「not planned」でクローズ
- ユーザー体面のバグの大半はビジュアル/UX問題（ウィンドウ位置、透明度、ショートカット競合）で、ユニットテストでは検出不可
- Tauri v2の`test`モジュールはまだ不安定。`MockRuntime`ベースのテストはTauriアップグレードで壊れるリスク
- 個人開発プロジェクト: ウィンドウ管理のモックインフラに費やす時間は機能開発に回すべき

---

## Current State (現状)

| Metric | Value |
|--------|-------|
| **テスト総数** | 63 (46 CI実行 / 17 macOS限定で除外) |
| **テスト済みモジュール** | `optimizer.rs` (29), `capture/lib.rs` (11), エラー型 (18), `settings.rs` (5) |
| **テスト0件のモジュール** | `capture_flow.rs`, `preview_window.rs`, `clipboard.rs`, `tray.rs`, `commands/*`, `state.rs` |
| **テストフレームワーク** | 標準`#[test]`のみ（外部依存なし） |
| **dev-dependencies** | なし |

### テスト可能性マップ

```
                  テスト容易性
        高 ◄─────────────────────► 低
        │                          │
Pure    │  optimizer.rs (29)       │
Logic   │  capture/lib.rs (11)     │
        │  settings serde (5)      │
        │  error types (18)        │
        ├──────────────────────────┤
Mixed   │  capture_flow.rs (0) ⚠️   │
        │  commands/capture.rs (0) │
        │  preview_window stack(0) │
        ├──────────────────────────┤
System  │                          │  tray.rs
Glue    │                          │  clipboard.rs
        │                          │  macos.rs
        │                          │  preview positioning
        └──────────────────────────┘
```

**"Mixed" 層がゼロカバレッジかつ最高リスク** — ここにリファクタリング+テスト追加が最も効果的。

---

## Testing Pyramid

| Layer | 割合 | 対象 | ツール |
|-------|------|------|--------|
| **Unit** | ~70% | 純粋ロジック: 最適化計算、トークン推定、設定シリアライズ、エラー型、フォーマット検出 | `#[test]`, `proptest` |
| **Integration** | ~25% | クレート横断: キャプチャモック→パイプライン→エンコード出力、Tauriコマンド配線 | `tauri::test::mock_builder` |
| **E2E / Manual** | ~5% | 実キャプチャ、クリップボード、プレビューウィンドウ | 手動QA |

---

## Implementation Plan

### Phase 0: テストインフラ整備 (所要: ~30分)

**目的**: dev-dependenciesとテストユーティリティの追加

```toml
# src-tauri/Cargo.toml [dev-dependencies]
proptest = "1"
tempfile = "3"
```

```toml
# crates/snapforge-pipeline/Cargo.toml [dev-dependencies]
proptest = "1"
```

---

### Phase 1: capture_flow.rs リファクタリング + テスト (優先度: 最高)

**理由**: 全キャプチャが通る中核パス。テスト0件。バグの影響が最大。

#### Step 1.1: 純粋関数の抽出

現在の`process_captured_file()`は以下をすべて1関数で実行:
1. ファイル読み込み
2. 画像デコード
3. 最適化
4. クリップボードコピー
5. Base64エンコード
6. メタデータ構築

**リファクタリング方針**: 純粋パイプラインロジックを副作用から分離

```rust
// 新: テスト可能な純粋関数
pub fn process_image_bytes(
    raw_bytes: &[u8],
    provider: LlmProvider,
) -> Result<ProcessedCapture, CaptureFlowError> {
    // decode → optimize → base64 → metadata
    // クリップボードなし、ファイルシステムなし
}

// 新: 副作用を含むオーケストレーター（薄いラッパー）
pub async fn process_captured_file(
    path: &Path,
    provider: LlmProvider,
) -> Result<ProcessedCapture, CaptureFlowError> {
    let bytes = std::fs::read(path)?;
    let result = process_image_bytes(&bytes, provider)?;
    copy_rgba_to_clipboard(&result.rgba, result.width, result.height)?;
    Ok(result)
}
```

#### Step 1.2: テスト追加 (目標: 10-15テスト)

| テスト | 内容 |
|--------|------|
| `process_image_bytes_valid_png` | 有効なPNG → 正常なメタデータ返却 |
| `process_image_bytes_valid_webp` | 有効なWebP → 正常処理 |
| `process_image_bytes_invalid` | 不正バイト列 → エラー |
| `process_image_bytes_empty` | 空バイト → エラー |
| `process_image_bytes_dimensions` | 出力寸法がプロバイダ制約内 |
| `process_image_bytes_base64_roundtrip` | Base64エンコード→デコードで元画像復元 |
| `capture_metadata_serialization` | メタデータJSON round-trip |
| `temp_capture_path_format` | パス形式の検証 |
| `temp_capture_path_uniqueness` | 連続呼び出しでユニーク |

---

### Phase 2: preview_window.rs 純粋ロジック抽出 + テスト (優先度: 高)

**理由**: スタック管理とデバウンスロジックがグローバル状態に埋もれている。

#### Step 2.1: スタック管理の純粋関数化

```rust
// 新: テスト可能な純粋関数
pub fn compute_eviction_targets(
    stack: &[String],
    max_previews: usize,
) -> Vec<String> {
    // 最大数超過時に削除すべきラベルを返す
}

pub fn compute_window_position(
    slot_index: usize,
    monitor_width: f64,
    monitor_height: f64,
) -> (f64, f64) {
    // ウィンドウ位置計算
}
```

#### Step 2.2: デバウンスの純粋関数化

```rust
pub fn should_debounce(
    last_capture: Option<Instant>,
    now: Instant,
    threshold: Duration,
) -> bool
```

#### Step 2.3: テスト追加 (目標: 8-10テスト)

| テスト | 内容 |
|--------|------|
| `eviction_empty_stack` | 空スタック → 削除なし |
| `eviction_under_limit` | 制限以下 → 削除なし |
| `eviction_at_limit` | ちょうど制限 → 1つ削除(新規分) |
| `eviction_over_limit` | 超過 → 古い順に削除 |
| `debounce_first_capture` | 初回 → 通過 |
| `debounce_within_threshold` | 閾値内 → ブロック |
| `debounce_after_threshold` | 閾値超過 → 通過 |
| `window_position_slot_0` | 最初のスロット位置 |
| `window_position_slot_n` | N番目のスロット位置 |
| `window_position_edge` | 画面端でのクランプ |

---

### Phase 3: commands/capture.rs フォーマット検出テスト (優先度: 中)

**理由**: `save_capture_to_desktop()`のマジックバイト検出はデータ破損に直結。

#### テスト追加 (目標: 5-7テスト)

| テスト | 内容 |
|--------|------|
| `detect_format_png` | PNG magic bytes → `.png`拡張子 |
| `detect_format_webp` | RIFF+WEBP magic → `.webp`拡張子 |
| `detect_format_jpeg` | JPEG magic → `.jpg`拡張子 |
| `detect_format_unknown` | 不明バイト → フォールバック |
| `base64_decode_valid` | 有効なBase64 → 正常デコード |
| `base64_decode_invalid` | 不正なBase64 → エラー |
| `desktop_path_construction` | パス生成の検証 |

---

### Phase 4: proptest でオプティマイザーの堅牢性強化 (優先度: 中)

**理由**: 現在29テストあるが、プロパティベースでエッジケースを網羅的に検証可能。

```rust
proptest! {
    #[test]
    fn dimensions_never_exceed_max(w in 1u32..20000, h in 1u32..20000) {
        let (out_w, out_h) = calculate_optimal_dimensions(w, h, LlmProvider::Claude);
        prop_assert!(out_w <= 1568);
        prop_assert!(out_h <= 1568);
    }

    #[test]
    fn aspect_ratio_preserved(w in 100u32..10000, h in 100u32..10000) {
        let (out_w, out_h) = calculate_optimal_dimensions(w, h, LlmProvider::Claude);
        let original = w as f64 / h as f64;
        let result = out_w as f64 / out_h as f64;
        prop_assert!((original - result).abs() < 0.02);
    }

    #[test]
    fn token_estimate_monotonic(w in 1u32..5000, h in 1u32..5000) {
        // 解像度が上がればトークン数も増える
        let small = LlmProvider::Claude.estimate_tokens(w, h);
        let large = LlmProvider::Claude.estimate_tokens(w * 2, h * 2);
        prop_assert!(large >= small);
    }
}
```

目標: 5-8 proptestケース

---

### Phase 5: settings.rs 異常系テスト強化 (優先度: 低)

既に5テストあるが、異常系を強化:

| テスト | 内容 |
|--------|------|
| `malformed_json` | 壊れたJSON → デフォルト値にフォールバック |
| `invalid_field_values` | 不正な値 → エラーまたはデフォルト |
| `missing_config_dir` | ディレクトリ不在 → 自動作成 |
| `concurrent_save_load` | 並行アクセス → データ整合性 |

---

### Phase 6: Tauri コマンド配線テスト (優先度: 低、リスクあり)

**注意**: Tauri v2の`test`モジュールはまだ不安定。最小限に留める。

```rust
#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn all_commands_registered() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                // 全コマンドをリスト
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        drop(app);
    }
}
```

目標: 3-5テスト（コマンド登録の検証のみ）

---

## やらないこと

| 対象 | 理由 |
|------|------|
| `tray.rs` のテスト | 100% Tauriグルーコード。テスト不可。 |
| `clipboard.rs` のモックインフラ | ROI低。CIで不安定。トレイト導入は将来の拡張時に。 |
| `macos.rs` のテスト | OS APIグルー。`CaptureEngine`トレイトで分離済み。 |
| プレビューウィンドウの描画テスト | HTML/CSS/JSは手動QAのみ。 |
| `mockall` 導入 | トレイト面が小さい。手書きモックで十分。 |
| E2Eテストフレームワーク | 個人開発規模では過剰。 |

---

## Target Metrics

| Metric | 現状 | 目標 |
|--------|------|------|
| テスト総数 | 63 | ~100-110 |
| CI実行テスト | 46 | ~85-95 |
| テスト0件の高リスクモジュール | 3 (`capture_flow`, `preview_window`, `commands/capture`) | 0 |
| テスト実行時間 | <1秒 | <3秒 |
| dev-dependencies | 0 | 2 (`proptest`, `tempfile`) |

---

## Implementation Order & Effort Estimate

| Phase | 内容 | 新テスト数 | リファクタリング |
|-------|------|-----------|----------------|
| **Phase 0** | テストインフラ整備 | 0 | なし |
| **Phase 1** | capture_flow リファクタリング + テスト | 10-15 | `process_image_bytes`抽出 |
| **Phase 2** | preview_window 純粋ロジック + テスト | 8-10 | スタック管理・デバウンス抽出 |
| **Phase 3** | フォーマット検出テスト | 5-7 | フォーマット検出関数抽出 |
| **Phase 4** | proptest追加 | 5-8 | なし |
| **Phase 5** | settings異常系テスト | 4-5 | なし |
| **Phase 6** | Tauriコマンド配線テスト | 3-5 | なし |

**推奨実行順序**: Phase 0 → 1 → 2 → 3 → 4 → 5 → 6

Phase 1-3が最もROIが高い。Phase 4-6は追加的な堅牢性向上。
