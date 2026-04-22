# CLI E2E Phase 1 — No-Real-Capture Integration Tests

## 1. Goal

`gaze` CLI（`crates/gaze-cli/`）は現状 `src/lib.rs` 内の unit / in-module テスト（17本、`FakeBackend` ベース）でロジックを検証しているが、**実バイナリを起動して stdout / stderr / exit code / ファイルシステム副作用を検証する E2E 層**がまだ存在しない。`docs/12_cli_e2e_test_cases.csv` には 48 ケースが定義されているが、その多くは実画面キャプチャや手動操作を要求する。

Phase 1 では、**画面キャプチャや GUI を一切必要としない 12 ケース**を `crates/gaze-cli/tests/*.rs`（Cargo の integration test ディレクトリ）に実装する。ヘッドレス CI で常時グリーンに保ち、リリースパイプラインの最初の gate として機能させるのが目的。Phase 2 以降の real-capture / permission / clipboard 系はスコープ外。

## 2. Prerequisites

- Rust toolchain: workspace ルートの `rust-toolchain.toml` / `Cargo.toml` に従う（MSRV は workspace 既定。`cargo --version` が動けば十分）
- `gaze` バイナリがビルドできること:
  ```bash
  cargo build -p gaze-cli
  ```
- macOS / Linux / Windows いずれでも通る想定（Phase 1 は real capture を呼ばないため OS 依存なし）
- 追加の外部ツール（`jq`、`sips`、ImageMagick 等）は **不要**。フィクスチャは Rust の `image` crate で生成する

## 3. Dependencies to add

`crates/gaze-cli/Cargo.toml` の `[dev-dependencies]` に以下を追記する。既に `image` と `tempfile` は存在するので、差分だけ足すこと。

```toml
[dev-dependencies]
image.workspace = true
tempfile = "3"
assert_cmd = "2"        # Command を作って stdout/stderr/exit を assert
predicates = "3"        # predicate::str::contains 等のマッチャ
assert_fs = "1"         # 一時ディレクトリとファイル存在/中身の assert
serde_json.workspace = true  # JSON 出力のパース
# 任意: insta = "1"     # スナップショットテスト（--help 固定化）
```

補足:
- `assert_cmd` は `Command::cargo_bin("gaze")` で同一ワークスペースの `[[bin]]` を解決する。`cargo test` 実行時に自動ビルドされる
- `predicates` は `assert_cmd` 本体では露出しない述語 API を提供（`predicate::path::exists()` など）
- `assert_fs` は temp dir をテスト終了時に自動削除する。`tempfile` と併用可能
- ドキュメント: <https://docs.rs/assert_cmd/>, <https://docs.rs/predicates/>, <https://docs.rs/assert_fs/>, <https://rust-cli.github.io/book/tutorial/testing.html>

## 4. Directory layout

```
crates/gaze-cli/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   └── main.rs
└── tests/
    ├── common/
    │   └── mod.rs          # フィクスチャ生成ヘルパ
    ├── e2e_smoke.rs        # --help / --version / version subcommand / unknown subcommand
    ├── e2e_capture_args.rs # capture サブコマンドの arg validation（no-capture）
    ├── e2e_list_args.rs    # list サブコマンドの arg validation
    └── e2e_optimize.rs     # optimize サブコマンド（実ファイル入出力あり、画面キャプチャなし）
```

フィクスチャ用バイナリファイルはコミットしない。テスト内で `tempfile::TempDir` に生成する方針（再現性と diff ノイズ回避のため）。`tests/common/mod.rs` のヘルパ:

```rust
// crates/gaze-cli/tests/common/mod.rs
use image::{DynamicImage, ImageBuffer, Rgba};
use std::path::Path;

#[allow(dead_code)]
pub fn write_png(path: &Path, width: u32, height: u32) {
    let img = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    });
    DynamicImage::ImageRgba8(img)
        .save_with_format(path, image::ImageFormat::Png)
        .expect("write png fixture");
}

#[allow(dead_code)]
pub fn write_jpeg(path: &Path, width: u32, height: u32) {
    let img = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba([(x % 256) as u8, (y % 256) as u8, 200, 255])
    });
    DynamicImage::ImageRgba8(img)
        .to_rgb8()
        .save_with_format(path, image::ImageFormat::Jpeg)
        .expect("write jpeg fixture");
}

#[allow(dead_code)]
pub fn write_garbage(path: &Path) {
    std::fs::write(path, b"this is not an image").expect("write garbage fixture");
}
```

## 5. Test case table

| CSV ID | File | Test function | Summary | Expected |
|--------|------|---------------|---------|----------|
| CLI-E2E-001 | e2e_smoke.rs | `help_lists_all_subcommands` | `gaze --help` | exit 0, stdout に `capture` `list` `optimize` `version` |
| CLI-E2E-002 | e2e_smoke.rs | `version_flag_prints_semver` | `gaze --version` | exit 0, stdout に `gaze <version>` |
| CLI-E2E-003 | e2e_smoke.rs | `version_subcommand_prints_semver` | `gaze version` | exit 0, stdout に `gaze <version>` |
| CLI-E2E-004 | e2e_smoke.rs | `unknown_subcommand_returns_usage_error` | `gaze no-such-command` | exit 2, stderr に usage エラー |
| CLI-E2E-005 | e2e_capture_args.rs | `capture_format_path_requires_output` | `gaze capture --format path` | exit 2, stderr に `--format path requires --output` |
| CLI-E2E-006 | e2e_capture_args.rs | `capture_area_mode_rejects_window_id` | `gaze capture --mode area --window 7` | exit 2, stderr に `--window cannot be combined with area capture mode` |
| CLI-E2E-007 | e2e_capture_args.rs | `capture_window_mode_rejects_display_id` | `gaze capture --mode window --display 1` | exit 2, stderr に組み合わせエラー |
| CLI-E2E-008 | e2e_capture_args.rs | `capture_rejects_display_and_window_together` | `gaze capture --display 1 --window 7` | exit 2, stderr に `--display cannot be combined with --window` |
| CLI-E2E-043 | e2e_list_args.rs | `list_without_target_returns_usage_error` | `gaze list` | exit 2, stderr に usage エラー |
| CLI-E2E-036 | e2e_optimize.rs | `optimize_format_path_requires_output` | `gaze optimize <png> --format path` | exit 2, stderr に `--format path requires --output` |
| CLI-E2E-037 | e2e_optimize.rs | `optimize_missing_input_returns_runtime_error` | `gaze optimize /no/such.png` | exit 1, stderr に `Failed to read input file` |
| CLI-E2E-038 | e2e_optimize.rs | `optimize_invalid_image_returns_runtime_error` | `gaze optimize <garbage>` | exit 1, stderr に decode failure 系メッセージ |

補足: CLI-E2E-032/033/034/035/045 は optimize の正常系で real-capture を要求しないため Phase 1.5 で追加可能だが、画像処理パイプラインの挙動に依存するため Phase 1 では **arg validation と error path に絞る**（上記 3 ケース）。

## 6. Test case details

### e2e_smoke.rs

```rust
// crates/gaze-cli/tests/e2e_smoke.rs
use assert_cmd::Command;
use predicates::prelude::*;

fn gaze() -> Command {
    Command::cargo_bin("gaze").expect("gaze binary should be built by cargo test")
}

#[test]
fn help_lists_all_subcommands() {
    // CLI-E2E-001: smoke test — binary launches and help advertises every subcommand
    gaze()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("capture"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("optimize"))
        .stdout(predicate::str::contains("version"))
        .stdout(predicate::str::contains("LLM-optimized screen capture CLI"));
}

#[test]
fn version_flag_prints_semver() {
    // CLI-E2E-002: `--version` uses clap's builtin version flag
    let expected = format!("gaze {}", env!("CARGO_PKG_VERSION"));
    gaze()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(expected));
}

#[test]
fn version_subcommand_prints_semver() {
    // CLI-E2E-003: custom `version` subcommand writes to stdout via writeln!
    let expected = format!("gaze {}", env!("CARGO_PKG_VERSION"));
    gaze()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(expected));
}

#[test]
fn unknown_subcommand_returns_usage_error() {
    // CLI-E2E-004: clap should emit usage error to stderr and exit with code 2
    gaze()
        .arg("no-such-command")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("error"));
}
```

**ガード対象**: バイナリの rpath / startup 問題、`--version` / `version` 両方の分岐、clap のトップレベル usage エラーを EXIT_USAGE(2) にマッピングする `run_cli` の分岐（`src/lib.rs:250-263`）。

### e2e_capture_args.rs

```rust
// crates/gaze-cli/tests/e2e_capture_args.rs
use assert_cmd::Command;
use predicates::prelude::*;

fn gaze() -> Command {
    Command::cargo_bin("gaze").expect("gaze binary should be built by cargo test")
}

#[test]
fn capture_format_path_requires_output() {
    // CLI-E2E-005: validate_output_requirements() is a runtime check (not clap-level),
    // so we exercise the full run_cli path. `--format path` without --output => EXIT_USAGE.
    gaze()
        .args(["capture", "--format", "path"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains(
            "--format path requires --output",
        ));
}

#[test]
fn capture_area_mode_rejects_window_id() {
    // CLI-E2E-006: resolve_capture_target() forbids --window when --mode area.
    // Because validate runs before permission check, this must NOT attempt a capture.
    gaze()
        .args(["capture", "--mode", "area", "--window", "7"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains(
            "--window cannot be combined with area capture mode",
        ));
}

#[test]
fn capture_window_mode_rejects_display_id() {
    // CLI-E2E-007: --display is full-capture only; combining with --mode window => usage error.
    gaze()
        .args(["capture", "--mode", "window", "--display", "1"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains(
            "--display can only be used with full capture mode",
        ));
}

#[test]
fn capture_rejects_display_and_window_together() {
    // CLI-E2E-008: --display and --window are mutually exclusive even in full mode.
    gaze()
        .args(["capture", "--display", "1", "--window", "7"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains(
            "--display cannot be combined with --window",
        ));
}
```

**ガード対象**: `validate_output_requirements` と `resolve_capture_target`（`src/lib.rs:423-477`）。これらは backend 呼び出し前に走るので real capture 不要。**注意**: `ensure_permission` は resolve より前に走るため、権限がない環境では先に runtime error(1) になる可能性がある。Phase 1 の CI ではこのケースに遭遇しないが、もし問題になれば `FakeBackend` を通す CLI 内 test に移すこと（lib.rs の既存 13 番目のテスト `invalid_capture_argument_combo_returns_usage_error` が同等のカバレッジを持つ）。

**注記**: E2E テストは実バイナリ経由なので `ensure_permission` が `snapforge_capture::has_permission()` を呼ぶ。macOS で権限未付与だと E2E-005〜008 は exit 1 に落ちる。CI（GitHub Actions macOS ランナー）では画面収録権限が常に false を返すため、`run_capture` 冒頭の `ensure_permission` が先にエラーを返してしまう。対策として**lib.rs の `run_capture` を再確認し、arg validation をパーミッションチェックより前に移す PR を別途検討する**か、E2E-005〜008 は Phase 2 に回して Phase 1 では E2E-001〜004, 043, 036〜038 の 8 ケースに縮小する。最新の lib.rs では `ensure_permission` → `validate_output_requirements` → `resolve_capture_target` の順（`src/lib.rs:298-308`）なので、本 Phase でもこの制約は残る。**推奨**: Phase 1 着手時に `run_capture` の順序を `validate_output_requirements` → `resolve_capture_target` → `ensure_permission` に並べ替える小 PR を先行マージし、その後 E2E を入れる。

### e2e_list_args.rs

```rust
// crates/gaze-cli/tests/e2e_list_args.rs
use assert_cmd::Command;
use predicates::prelude::*;

fn gaze() -> Command {
    Command::cargo_bin("gaze").expect("gaze binary should be built by cargo test")
}

#[test]
fn list_without_target_returns_usage_error() {
    // CLI-E2E-043: `list` has a required `<TARGET>` subcommand (displays|windows).
    // clap should reject missing target and exit with code 2.
    gaze()
        .arg("list")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Usage").or(predicate::str::contains("usage")));
}
```

**ガード対象**: clap の required subcommand 検証。backend は呼ばれない。

### e2e_optimize.rs

```rust
// crates/gaze-cli/tests/e2e_optimize.rs
mod common;

use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

fn gaze() -> Command {
    Command::cargo_bin("gaze").expect("gaze binary should be built by cargo test")
}

#[test]
fn optimize_format_path_requires_output() {
    // CLI-E2E-036: optimize shares validate_output_requirements() with capture.
    // Must fail before reading the input file, so input path can be bogus.
    let tmp = assert_fs::TempDir::new().unwrap();
    let input = tmp.child("input.png");
    common::write_png(input.path(), 200, 150);

    gaze()
        .args([
            "optimize",
            input.path().to_str().unwrap(),
            "--format",
            "path",
        ])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains(
            "--format path requires --output",
        ));
}

#[test]
fn optimize_missing_input_returns_runtime_error() {
    // CLI-E2E-037: nonexistent input => std::fs::read fails => RunError::runtime => exit 1.
    gaze()
        .args(["optimize", "/tmp/gaze-e2e-no-such-input.png"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Failed to read input file"));
}

#[test]
fn optimize_invalid_image_returns_runtime_error() {
    // CLI-E2E-038: non-image bytes => snapforge_core::process_image_bytes decode failure => exit 1.
    let tmp = assert_fs::TempDir::new().unwrap();
    let bogus = tmp.child("not_image.bin");
    common::write_garbage(bogus.path());

    gaze()
        .args(["optimize", bogus.path().to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::is_empty().not());
}
```

**ガード対象**: `run_optimize` 内の validate → read → process の順序（`src/lib.rs:386-411`）。E2E-037 は I/O エラー、E2E-038 は `image` crate のデコード失敗パス。

## 7. How to run

```bash
# ライブラリ内の既存 17 テストのみ実行
cargo test -p gaze-cli --lib

# 単一の E2E ファイル実行
cargo test -p gaze-cli --test e2e_smoke
cargo test -p gaze-cli --test e2e_capture_args
cargo test -p gaze-cli --test e2e_list_args
cargo test -p gaze-cli --test e2e_optimize

# gaze-cli の全テスト（lib + integration）
cargo test -p gaze-cli

# integration test だけまとめて実行
cargo test -p gaze-cli --tests

# ワークスペース全体（CI と同等。ただし snapforge-capture は除外）
cargo test --workspace --exclude snapforge-capture
```

`cargo test -p gaze-cli --test <name>` の `<name>` は `tests/<name>.rs` のファイル名（拡張子なし）と一致する。`assert_cmd::Command::cargo_bin("gaze")` は初回実行時に自動で `gaze-cli` の `[[bin]]` をビルドする。

## 8. Definition of Done

- [ ] `crates/gaze-cli/Cargo.toml` に `assert_cmd`, `predicates`, `assert_fs` を追加した
- [ ] `crates/gaze-cli/tests/common/mod.rs` にフィクスチャヘルパを実装した
- [ ] 12 本の E2E テストがすべて green（`cargo test -p gaze-cli --tests`）
- [ ] 既存の lib テスト 17 本にリグレッションなし（`cargo test -p gaze-cli --lib`）
- [ ] `cargo test --workspace --exclude snapforge-capture` が全 green
- [ ] `cargo fmt --all -- --check` 通過
- [ ] `cargo clippy -- -D warnings` で新規警告ゼロ
- [ ] `.github/workflows/ci.yml` が既に `cargo test` を呼んでいれば追加変更不要。明示的な `--tests` job が欲しければ追加（Phase 1 では必須ではない）
- [ ] CSV（`docs/12_cli_e2e_test_cases.csv`）の該当 12 行に「実装済み: Phase 1」のメモを手動で追記（別カラムを足すか PR description で紐付け）
- [ ] **事前 PR**: `run_capture` / `run_optimize` 内で `validate_output_requirements` と `resolve_capture_target` を `ensure_permission` より前に移動（E2E-005〜008 がヘッドレス CI で通るようにするため）

## 9. Optional — Snapshot testing with `insta`

`--help` の出力は UX ドキュメント相当で変更時に必ずレビューすべき。`insta` を使うと回帰を可視化できる:

```toml
# [dev-dependencies] 追記
insta = { version = "1", features = ["yaml"] }
```

```rust
// e2e_smoke.rs に追加
#[test]
fn help_output_matches_snapshot() {
    let output = Command::cargo_bin("gaze")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!("gaze_help", stdout);
}
```

初回実行後 `cargo insta review` で承認。以降 `--help` が変わると diff が表示される。Phase 1 では optional、Phase 2 で本採用を検討。

## 10. Out of scope (Phase 2+)

以下は Phase 1 で **実装しない**。Phase 2 以降で段階的に追加する:

- **Real capture 系**: CLI-E2E-013〜022, 028, 029, 031, 041, 042, 046, 047（`capture_fullscreen` / `capture_window` が成功する前提）
- **Interactive capture**: CLI-E2E-024〜027, 044（`screencapture -i` を呼び出すため `auto` にしても TTY 依存）
- **Clipboard 副作用**: CLI-E2E-019, 039（`arboard` が GUI session を要求、CI で不安定）
- **Permission gate**: CLI-E2E-009, 010, 030（TCC 操作が必要で `manual` 指定）
- **Optimize 正常系**: CLI-E2E-032〜035, 040, 045（画像パイプラインの期待寸法・WebP マジックバイト検証。Phase 1.5 候補）
- **エンコーディング**: CLI-E2E-048（日本語ウィンドウタイトルが実在する前提）
- **GUI / Tauri preview**: このリポジトリの Tauri 側（`src-tauri/`）のテストは別トラック
- **MCP サーバ E2E**: `src-tauri/src/mcp/` の JSON-RPC テストは別ファイルで計画

### Phase 1.5 で増やす候補

Optimize 正常系（CLI-E2E-032, 033, 034, 035, 045）は real capture 不要で、`common::write_png` / `write_jpeg` で fixture を作れば実装可能。WebP マジックバイト（`b"RIFF"` 先頭 + `b"WEBP"` at offset 8）や PNG マジックバイト（`b"\x89PNG\r\n\x1a\n"`）の検証を加えて Phase 1.5 として提出するとよい。CLI capture の `--format base64` 出力は **WebP エンコード済み base64**（PNG ではない）である点に注意 — `--raw` 付きのときのみ PNG バイトになる（lib.rs の `CaptureProcessingMode::Raw` 経路）。

## References

- [assert_cmd docs](https://docs.rs/assert_cmd/)
- [predicates docs](https://docs.rs/predicates/)
- [assert_fs docs](https://docs.rs/assert_fs/)
- [Rust CLI book — Testing](https://rust-cli.github.io/book/tutorial/testing.html)
- [insta docs](https://insta.rs/)
- Source: `crates/gaze-cli/src/lib.rs`（exit code 定数: `EXIT_SUCCESS=0` / `EXIT_FAILURE=1` / `EXIT_USAGE=2` / `EXIT_CANCELLED=3`）
- Test case master: `docs/12_cli_e2e_test_cases.csv`
