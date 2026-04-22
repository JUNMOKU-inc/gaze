# 15. Unit Test Quick-Wins（Phase 1）

## 1. Scope

Gaze のワークスペースには現時点で **94 本** の Rust 単体テストが走っており、主要なクレート (`snapforge-pipeline` 44, `src-tauri` 34, `snapforge-core` 21, `snapforge-capture` 11, `gaze-cli` 17) それぞれに `#[cfg(test)]` モジュールが揃っている。本ドキュメントで定義する「Quick-Win Unit Tests」とは、**1 本あたり 1 時間以内** で追加でき、クリティカルパス上のコードのうち「型で守られていない実行時ブランチ」を補強する純粋関数向けのテストを指す。I/O 不要・フィクスチャ不要・並列実行で衝突しない、という 3 要件を満たすものだけを選ぶ。これにより CI 時間を増やさず、後段のリファクタや E2E（`docs/14_cli_e2e_phase1.md`）で真に必要な投資にリソースを回せる。

Phase 1 では以下の 3 関数が対象。いずれも既に一部カバレッジがあるため、**「既存テストを読んで、真に抜けている境界条件だけを足す」** スタイルで進める。

| # | 関数 | 場所 | 既存テスト数 |
|---|------|------|-------------|
| 1 | `resolve_save_dir()` | `src-tauri/src/commands/capture.rs:103` | 6 |
| 2 | `detect_image_format()` | `crates/snapforge-core/src/output.rs:1`（本体）および `src-tauri/src/commands/capture.rs` 経由で再エクスポート | 4（core 側） |
| 3 | `process_rgba_capture_with_mode(Raw)` | `crates/snapforge-core/src/capture.rs:84` | 1 |

---

## 2. Prerequisites

各クレートの `Cargo.toml` は以下の dev-dep 状況。**追加インストールは不要**。

- `snapforge-core/Cargo.toml` … `dev-dependencies = { serde_json }`
- `src-tauri/Cargo.toml` … dev-deps セクションなし（本体 deps の `serde_json`, `base64`, `chrono` をそのまま使う）
- ワークスペースルートの `[profile.dev.package.image]` は `opt-level = 3` に設定済み。画像処理テストが debug build でも遅くならない。

テスト実行前に一度は以下を通しておくこと。

```bash
cd /Users/nodaakiyoshi/Soruce/cleanshot-lite
cargo test --workspace --exclude snapforge-capture
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

`snapforge-capture` は macOS ScreenCaptureKit 依存のため CI では excluded。ローカルの clippy は走らせて OK。

---

## 3. Quick Win 1: `resolve_save_dir()` — 残りの境界条件

### 3.1 対象シグネチャ（確認済み）

```rust
// src-tauri/src/commands/capture.rs:103
fn resolve_save_dir(save_location: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let expanded = if save_location.starts_with("~/") {
        save_location.replacen("~", &home, 1)
    } else if save_location == "~" {
        home.clone()
    } else {
        save_location.to_string()
    };
    let path = std::path::PathBuf::from(expanded);
    if path.is_dir() { path } else { std::path::PathBuf::from(home).join("Desktop") }
}
```

### 3.2 既に存在するテスト

- `resolve_save_dir_tilde_desktop` … `~/Desktop` 展開
- `resolve_save_dir_absolute_tmp` … `/tmp` パススルー
- `resolve_save_dir_nonexistent_falls_back` … 存在しない `~/...` のフォールバック
- `resolve_save_dir_tilde_only` … `~` 単体
- `resolve_save_dir_absolute_nonexistent_falls_back` … 存在しない絶対パスのフォールバック
- `resolve_save_dir_path_with_spaces` … スペース入りパス

### 3.3 追加すべきテスト（抜け漏れ）

基本形はカバー済み。`src-tauri/src/commands/capture.rs` の `#[cfg(test)] mod tests` 内の `// --- resolve_save_dir tests ---` セクションの末尾に、以下 2 本だけ追加する。

```rust
#[test]
fn resolve_save_dir_empty_string_falls_back() {
    // 空文字列は path.is_dir() が false になり Desktop へフォールバック
    let home = std::env::var("HOME").unwrap();
    let result = resolve_save_dir("");
    let expected = std::path::PathBuf::from(format!("{home}/Desktop"));
    assert_eq!(result, expected);
}

#[test]
fn resolve_save_dir_tilde_without_slash_is_literal() {
    // "~foo" は "~/" で始まらないのでそのまま literal 扱い → 存在しないので Desktop へ
    let home = std::env::var("HOME").unwrap();
    let result = resolve_save_dir("~foo");
    let expected = std::path::PathBuf::from(format!("{home}/Desktop"));
    assert_eq!(result, expected);
}
```

### 3.4 実行コマンド

```bash
cargo test -p snapforge-desktop resolve_save_dir
```

**見積もり: 30 分（読み 20 分 + 実装 5 分 + 確認 5 分）**

> 前提条件: `HOME` 環境変数が設定されていること。macOS/Linux の通常環境で常に真。CI も問題なし。

---

## 4. Quick Win 2: `detect_image_format()` — core 側に WAVE 検出を追加

### 4.1 対象シグネチャ（確認済み）

```rust
// crates/snapforge-core/src/output.rs:1
pub fn detect_image_format(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        "webp"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpeg"
    } else {
        "png"
    }
}
```

### 4.2 既存テスト（`crates/snapforge-core/src/output.rs` の `#[cfg(test)] mod tests`）

- `detect_png_format`
- `detect_webp_format`
- `detect_jpeg_format`
- `detect_unknown_format_falls_back_to_png` … `b"RIFF1234"` (8 バイト) と空と乱数

### 4.3 追加すべきテスト（抜け漏れ）

**RIFF 長さチェック境界** と **RIFF+wrong-magic（WAVE 等）** の 2 ケースを追加。`output.rs` の `mod tests` の最後に以下を append。

```rust
#[test]
fn detect_riff_exactly_11_bytes_falls_back() {
    // `bytes.len() >= 12` の境界: 11 バイトは false 側 → png
    let mut header = Vec::from(b"RIFF" as &[u8]);
    header.extend_from_slice(&[0x00; 4]); // size
    header.extend_from_slice(b"WEB"); // 1 バイト足りない (合計 11)
    assert_eq!(detect_image_format(&header), "png");
}

#[test]
fn detect_riff_wave_falls_back_to_png() {
    // RIFF コンテナ + WAVE マジック (実在する .wav ファイルを想定)
    let mut wav = Vec::from(b"RIFF" as &[u8]);
    wav.extend_from_slice(&[0x00; 4]);
    wav.extend_from_slice(b"WAVE");
    assert_eq!(detect_image_format(&wav), "png");
}

#[test]
fn detect_riff_avi_falls_back_to_png() {
    // RIFF + AVI (動画コンテナ)
    let mut avi = Vec::from(b"RIFF" as &[u8]);
    avi.extend_from_slice(&[0x00; 4]);
    avi.extend_from_slice(b"AVI ");
    assert_eq!(detect_image_format(&avi), "png");
}
```

> 注: `src-tauri/src/commands/capture.rs` 側にも同名のラッパーテスト群があるが、本体は `snapforge-core` なので **core 側にだけ足す**（重複を避ける）。

### 4.4 実行コマンド

```bash
cargo test -p snapforge-core detect_
```

**見積もり: 15 分**

---

## 5. Quick Win 3: `process_rgba_capture_with_mode(Raw)` — Claude 以外の provider を確認

### 5.1 対象シグネチャ（確認済み）

```rust
// crates/snapforge-core/src/capture.rs:84
pub fn process_rgba_capture_with_mode(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    provider: LlmProvider,
    mode: CaptureProcessingMode,
) -> Result<ProcessedCapture, CaptureFlowError>
```

Raw モードでは resize せず `encode_png` でそのまま PNG 化、`optimized_width/height` には入力サイズをそのまま入れる。

### 5.2 既存テスト（`crates/snapforge-core/src/capture.rs`）

- `process_rgba_capture_raw_preserves_dimensions_and_encodes_png` … Claude + 25×10 入力で寸法保持 + PNG エンコード確認

### 5.3 追加すべきテスト（抜け漏れ）

Raw モードは provider に依存しない挙動のはずだが、**token_estimate は provider 依存** なので Gpt4o / Gemini での分岐を少なくとも 1 本カバーしておく。100×200 という仕様書指定サイズで dimensions 保持を検証する形で追加。`capture.rs` の `mod tests` の末尾に以下を append。

```rust
#[test]
fn process_rgba_capture_raw_preserves_100x200_dimensions_claude() {
    let rgba = vec![200u8; 100 * 200 * 4];
    let processed = process_rgba_capture_with_mode(
        &rgba,
        100,
        200,
        LlmProvider::Claude,
        CaptureProcessingMode::Raw,
    )
    .unwrap();

    // Raw モードは resize しない
    assert_eq!(processed.metadata.original_width, 100);
    assert_eq!(processed.metadata.original_height, 200);
    assert_eq!(processed.metadata.optimized_width, 100);
    assert_eq!(processed.metadata.optimized_height, 200);
    // 再エンコード結果は PNG
    assert_eq!(
        crate::detect_image_format(&processed.encoded),
        "png"
    );
}

#[test]
fn process_rgba_capture_raw_all_providers_preserve_dimensions() {
    // Raw モードは provider に依存せず寸法を保持するが token 推定は provider ごとに変わる
    let rgba = vec![64u8; 100 * 200 * 4];
    let mut tokens = Vec::new();
    for provider in [LlmProvider::Claude, LlmProvider::Gpt4o, LlmProvider::Gemini] {
        let processed = process_rgba_capture_with_mode(
            &rgba,
            100,
            200,
            provider,
            CaptureProcessingMode::Raw,
        )
        .unwrap();
        assert_eq!(processed.metadata.optimized_width, 100);
        assert_eq!(processed.metadata.optimized_height, 200);
        tokens.push(processed.metadata.token_estimate);
    }
    // Provider ごとに異なるトークン推定式を使うので値は 3 種類揃う
    assert!(tokens[0] > 0 && tokens[1] > 0 && tokens[2] > 0);
}
```

### 5.4 実行コマンド

```bash
cargo test -p snapforge-core process_rgba_capture_raw
```

**見積もり: 20 分**

---

## 6. Order of Execution（推奨順）

依存の軽いものから着手すると詰まったときに戻しやすい。

1. **Quick Win 2（detect_image_format）** … 最軽量。`snapforge-core` だけで完結し、ビルド時間も最短。
2. **Quick Win 3（process_rgba_capture_with_mode Raw）** … 同じ `snapforge-core` クレート内。#2 のビルドキャッシュが効く。
3. **Quick Win 1（resolve_save_dir）** … `src-tauri` 全体のリビルドが必要なので最後。

各段階で `cargo test -p <crate>` を通してから次に進むこと。

---

## 7. Definition of Done

以下の全てが green になって初めて PR に含めてよい。

- [ ] 追加した全テストが `cargo test --workspace --exclude snapforge-capture` で pass
- [ ] `cargo clippy --workspace -- -D warnings` に新しい警告が出ない
- [ ] `cargo fmt --all -- --check` 通過
- [ ] **本番コードを一時的に壊してテストが赤くなることを手元で確認**（例: `resolve_save_dir` の `~/` 展開を消すと `resolve_save_dir_tilde_without_slash_is_literal` 以外の既存テストが落ちる、`detect_image_format` の `&bytes[8..12] == b"WEBP"` を `b"WAVE"` に変えると新テストが落ちる、等）。検証後は revert する
- [ ] テストの命名が既存規約（`snake_case`, 動詞を含む、`_fallback` / `_preserves_` 等の suffix）と一致

---

## 8. What NOT to do in this phase

明示的な **非ゴール**。以下は別 phase で扱うので本 PR には混ぜないこと。

- `src-tauri/src/capture_flow.rs` や `src-tauri/src/preview_window.rs` のテスト追加 → Phase 2 にて実施予定。モックが必要で 1 時間に収まらない
- `assert_cmd` / `trycmd` などの E2E ハーネス導入 → `docs/14_cli_e2e_phase1.md` のスコープ
- プロダクションコードのリファクタ（関数分割、API 変更） → 本 phase は **テスト追加のみ**
- `snapforge-capture` への macOS テスト追加 → CI 対象外
- Notion / ドキュメント更新 → 実装 merge 後に別途

---

## 9. Follow-up List（次 phase 以降で扱うべき候補）

本 phase の範囲外だが、quick-win パターンで将来追加したいもの。

- `process_image_bytes()` に対し、実 PNG（`image` クレートで in-memory 生成）を流した round-trip テスト強化
- `save_capture_to_desktop()` のフォーマット検出 + ファイル名生成を `tempfile` クレート導入の上でファイル I/O まで検証
- `preview_window::debounce_check()` のタイミング境界（500ms デバウンス）を `std::time::Instant` の差し替えで検証
- `CommandError` の `Display` / `Serialize` 実装に対する snapshot test（`insta` 導入検討）
- `LlmProvider::estimate_tokens` の proptest 拡張（現在 pipeline 側にあるが境界ケース追加）

---

## Appendix: 参考リンク

- `docs/03_rust_coding_guidelines.md` — エラー処理、テストスタイル、プラットフォーム抽象化
- `docs/09_test_strategy.md` — ワークスペース全体のテスト戦略
- `docs/14_cli_e2e_phase1.md` — E2E テスト（本 phase と排他）
- 既存テストが格納されたファイル:
  - `src-tauri/src/commands/capture.rs` 末尾
  - `crates/snapforge-core/src/output.rs` 末尾
  - `crates/snapforge-core/src/capture.rs` 末尾
