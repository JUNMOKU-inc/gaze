# Gaze CLI — Homebrew 配布ガイド

> 作成日: 2026-03-31
> ステータス: 実装準備

---

## 1. 配布方式の選択

### 1.1 Homebrew Tap vs homebrew-core

| 観点 | Homebrew Tap (自前リポジトリ) | homebrew-core (公式) |
|------|---------------------------|---------------------|
| 審査 | なし。即日公開可能 | あり。PR レビュー必須 |
| 要件 | なし | GitHub ≥75 stars or ≥30 forks/watchers、安定版リリース必須、"beta" 不可 |
| ビルド | ソースでもバイナリでも可 | ソースビルド必須 (`depends_on "rust" => :build`) |
| インストールコマンド | `brew tap xxx/gaze && brew install gaze` | `brew install gaze` |
| 自動更新 | 自前 CI で Formula を更新 | BrewTestBot が自動でボトル作成 |

**→ ベータは Tap で開始。75+ stars 到達後に homebrew-core へ申請。**

### 1.2 Formula vs Cask

- **Formula**: CLI バイナリ向け。`/opt/homebrew/bin/` に配置
- **Cask**: `.app` バンドル向け。`/Applications/` に配置

Gaze は GUI (Tauri .app) と CLI の2つの成果物を持つ。

| 成果物 | 配布方式 | インストール先 |
|--------|---------|-------------|
| `gaze` CLI バイナリ | **Formula** (本ドキュメント) | `/opt/homebrew/bin/gaze` |
| `Gaze.app` デスクトップアプリ | **Cask** (将来、別ドキュメント) | `/Applications/Gaze.app` |

---

## 2. Tap リポジトリの作成

### 2.1 リポジトリ命名規則

Homebrew Tap は `homebrew-` プレフィクスが **必須**。

```
GitHub リポジトリ名: RQ-Akiyoshi/homebrew-gaze
brew tap コマンド:   brew tap RQ-Akiyoshi/gaze
                     → 自動的に github.com/RQ-Akiyoshi/homebrew-gaze を参照
```

### 2.2 作成手順

```bash
# 方法 A: brew tap-new で雛形生成 (テスト用 GitHub Actions 付き)
brew tap-new RQ-Akiyoshi/gaze
cd "$(brew --repository RQ-Akiyoshi/gaze)"
gh repo create RQ-Akiyoshi/homebrew-gaze --push --public --source .

# 方法 B: 手動作成 (最小構成)
mkdir -p homebrew-gaze/Formula
cd homebrew-gaze
git init
# Formula/gaze.rb を作成 (後述)
gh repo create RQ-Akiyoshi/homebrew-gaze --push --public --source .
```

### 2.3 ディレクトリ構成

```
homebrew-gaze/
├── Formula/
│   └── gaze.rb              # Formula 本体
├── .github/
│   └── workflows/
│       └── test.yml          # brew test-bot (任意)
└── README.md
```

---

## 3. Formula の記述

### 3.1 プリビルドバイナリ方式 (推奨)

ユーザーの環境に Rust 不要。インストールが数秒で完了する。

```ruby
# Formula/gaze.rb
class Gaze < Formula
  desc "LLM-optimized screen capture CLI"
  homepage "https://gaze.dev"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/RQ-Akiyoshi/gaze/releases/download/v#{version}/gaze-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    else
      url "https://github.com/RQ-Akiyoshi/gaze/releases/download/v#{version}/gaze-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64"
    end
  end

  def install
    bin.install "gaze"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gaze --version")
  end
end
```

**sha256 の取得方法:**
```bash
shasum -a 256 gaze-v0.1.0-aarch64-apple-darwin.tar.gz
shasum -a 256 gaze-v0.1.0-x86_64-apple-darwin.tar.gz
```

### 3.2 ソースビルド方式 (homebrew-core 申請時)

homebrew-core は プリビルドバイナリを受け付けない。ソースからビルドする Formula が必要。

```ruby
# homebrew-core 用 (将来)
class Gaze < Formula
  desc "LLM-optimized screen capture CLI"
  homepage "https://gaze.dev"
  url "https://github.com/RQ-Akiyoshi/gaze/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "PLACEHOLDER_SHA256_SOURCE"
  license "MIT"

  depends_on "rust" => :build
  depends_on :macos  # macOS 専用 (ScreenCaptureKit 依存)

  def install
    # std_cargo_args = ["--locked", "--root", prefix, "--path", "."]
    # workspace 内の特定パッケージを指定
    system "cargo", "install", *std_cargo_args(path: "crates/gaze-cli")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gaze --version")
  end
end
```

**注意点:**
- `std_cargo_args` は Homebrew が提供するヘルパー。`--locked`, `--root`, `--path` を自動展開
- `path: "crates/gaze-cli"` で workspace 内の特定クレートを指定
- `Cargo.lock` を Git に含めること (`--locked` が要求する)
- Homebrew が Rust ツールチェインを自動提供。ユーザーの Rust は使わない
- ビルドには約3-5分かかる (ユーザー環境依存)

---

## 4. GitHub Actions リリースパイプライン

### 4.1 全体フロー

```
git tag v0.1.0 && git push --tags
  │
  ├─ [Job 1] create-release
  │    └─ GitHub Release をドラフト作成
  │
  ├─ [Job 2] build-cli (matrix: aarch64 + x86_64)
  │    ├─ cargo build --release
  │    ├─ strip バイナリ
  │    ├─ tar.gz 作成
  │    └─ GitHub Release にアップロード
  │
  ├─ [Job 3] publish-release
  │    └─ ドラフトを正式公開
  │
  └─ [Job 4] update-homebrew-formula
       ├─ sha256 計算
       └─ homebrew-gaze リポジトリの Formula を更新
```

### 4.2 ワークフロー定義

```yaml
# .github/workflows/release-cli.yml
name: Release CLI

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

env:
  CARGO_TERM_COLOR: always

jobs:
  # -------------------------------------------------------
  # 1. GitHub Release をドラフト作成
  # -------------------------------------------------------
  create-release:
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.meta.outputs.version }}
    steps:
      - uses: actions/checkout@v4

      - name: Extract version
        id: meta
        run: echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

      - name: Create draft release
        run: gh release create "$GITHUB_REF_NAME" --draft --verify-tag --title "$GITHUB_REF_NAME"
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  # -------------------------------------------------------
  # 2. macOS バイナリビルド (aarch64 + x86_64)
  # -------------------------------------------------------
  build-cli:
    needs: create-release
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-latest           # arm64 ランナー
          - target: x86_64-apple-darwin
            os: macos-13               # 最後の Intel ランナー
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --package gaze-cli --target ${{ matrix.target }}

      - name: Strip and package
        run: |
          strip target/${{ matrix.target }}/release/gaze
          tar czf gaze-v${{ needs.create-release.outputs.version }}-${{ matrix.target }}.tar.gz \
            -C target/${{ matrix.target }}/release gaze

      - name: Upload to release
        run: |
          gh release upload "$GITHUB_REF_NAME" \
            gaze-v${{ needs.create-release.outputs.version }}-${{ matrix.target }}.tar.gz
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  # -------------------------------------------------------
  # 3. ドラフトを正式公開
  # -------------------------------------------------------
  publish-release:
    needs: [create-release, build-cli]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Publish release
        run: gh release edit "$GITHUB_REF_NAME" --draft=false
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  # -------------------------------------------------------
  # 4. Homebrew Formula を自動更新
  # -------------------------------------------------------
  update-homebrew:
    needs: [create-release, publish-release]
    runs-on: ubuntu-latest
    steps:
      - name: Download release assets and compute SHA256
        run: |
          VERSION=${{ needs.create-release.outputs.version }}
          for target in aarch64-apple-darwin x86_64-apple-darwin; do
            curl -fsSL -o "gaze-${target}.tar.gz" \
              "https://github.com/${{ github.repository }}/releases/download/v${VERSION}/gaze-v${VERSION}-${target}.tar.gz"
            shasum -a 256 "gaze-${target}.tar.gz" >> checksums.txt
          done
          cat checksums.txt

      - name: Extract SHA256 values
        id: sha
        run: |
          echo "arm64=$(grep aarch64 checksums.txt | cut -d' ' -f1)" >> "$GITHUB_OUTPUT"
          echo "x86_64=$(grep x86_64 checksums.txt | cut -d' ' -f1)" >> "$GITHUB_OUTPUT"

      - name: Update Homebrew Formula
        uses: peter-evans/repository-dispatch@v3
        with:
          token: ${{ secrets.TAP_GITHUB_TOKEN }}
          repository: RQ-Akiyoshi/homebrew-gaze
          event-type: update-formula
          client-payload: |
            {
              "version": "${{ needs.create-release.outputs.version }}",
              "sha256_arm64": "${{ steps.sha.outputs.arm64 }}",
              "sha256_x86_64": "${{ steps.sha.outputs.x86_64 }}"
            }
```

### 4.3 Tap 側の自動更新ワークフロー

```yaml
# homebrew-gaze/.github/workflows/update-formula.yml
name: Update Formula

on:
  repository_dispatch:
    types: [update-formula]

permissions:
  contents: write

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Update Formula
        run: |
          VERSION="${{ github.event.client_payload.version }}"
          SHA_ARM64="${{ github.event.client_payload.sha256_arm64 }}"
          SHA_X86="${{ github.event.client_payload.sha256_x86_64 }}"

          cat > Formula/gaze.rb << FORMULA
          class Gaze < Formula
            desc "LLM-optimized screen capture CLI"
            homepage "https://gaze.dev"
            version "${VERSION}"
            license "MIT"

            on_macos do
              if Hardware::CPU.arm?
                url "https://github.com/RQ-Akiyoshi/gaze/releases/download/v#{version}/gaze-v#{version}-aarch64-apple-darwin.tar.gz"
                sha256 "${SHA_ARM64}"
              else
                url "https://github.com/RQ-Akiyoshi/gaze/releases/download/v#{version}/gaze-v#{version}-x86_64-apple-darwin.tar.gz"
                sha256 "${SHA_X86}"
              end
            end

            def install
              bin.install "gaze"
            end

            test do
              assert_match version.to_s, shell_output("#{bin}/gaze --version")
            end
          end
          FORMULA

      - name: Commit and push
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add Formula/gaze.rb
          git commit -m "gaze ${{ github.event.client_payload.version }}"
          git push
```

### 4.4 必要な GitHub Secrets

| Secret 名 | 設定場所 | 用途 |
|-----------|---------|------|
| `GITHUB_TOKEN` | 自動提供 | リリース作成・アセットアップロード |
| `TAP_GITHUB_TOKEN` | メインリポジトリの Settings > Secrets | Tap リポジトリへの `repository_dispatch` 発火。`repo` スコープの PAT が必要 |

**PAT (Personal Access Token) の作成手順:**
1. GitHub Settings > Developer settings > Personal access tokens > Fine-grained tokens
2. Repository access: `RQ-Akiyoshi/homebrew-gaze` のみ
3. Permissions: Contents (Read and write)
4. 生成したトークンをメインリポジトリの Secrets に `TAP_GITHUB_TOKEN` として登録

---

## 5. macOS 固有の考慮事項

### 5.1 コード署名と Gatekeeper

| 配布方法 | コード署名 | Notarization | Gatekeeper |
|---------|-----------|-------------|------------|
| **Homebrew (Tap/core)** | 不要 | 不要 | Homebrew がバイパス (`xattr -d com.apple.quarantine`) |
| curl インストーラ | 推奨 | 推奨 | 署名なしだとブロックされる可能性あり |
| Web サイト直接 DL | 必須 | 必須 | 署名+公証なしだと「開発元不明」警告 |

**→ Homebrew 配布なら署名/公証は不要。** curl インストーラを併用する場合は Apple Developer ID ($99/年) でのコード署名を検討。

### 5.2 画面収録権限 (Screen Recording Permission)

CLI バイナリが ScreenCaptureKit を使用するため、macOS が画面収録権限を要求する。

- 初回実行時にシステムダイアログが表示される
- `/opt/homebrew/bin/gaze` (or `/usr/local/bin/gaze`) としてパス単位で権限管理される
- ターミナルアプリ (Terminal.app, iTerm2 等) に付与された権限を CLI が継承するケースもある
- `gaze capture` 初回実行時に権限がなければ、適切なエラーメッセージとシステム設定へのガイドを表示すべき

```
$ gaze capture
Error: Screen recording permission required.

Grant permission:
  1. Open System Settings → Privacy & Security → Screen Recording
  2. Enable "gaze" (or your terminal app)
  3. Restart your terminal

Or run: open "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
```

### 5.3 バイナリサイズ最適化

Homebrew ダウンロードの体感速度に影響するため、バイナリサイズを最小化する。

```toml
# Cargo.toml (workspace root) に追加
[profile.release]
strip = true          # デバッグシンボル除去
lto = true            # Link-Time Optimization
codegen-units = 1     # 最適化強化 (ビルド時間とトレードオフ)
panic = "abort"       # unwinding 除去
```

目安: Rust CLI バイナリは通常 5-15 MB。上記設定で 3-8 MB 程度に削減可能。

---

## 6. curl インストーラ (Homebrew の補完)

Homebrew を使わないユーザー向け。

### 6.1 インストーラスクリプト

```bash
#!/bin/sh
# install.sh — Gaze CLI installer
set -e

REPO="RQ-Akiyoshi/gaze"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) ;;
  *) echo "Error: Gaze CLI currently supports macOS only."; exit 1 ;;
esac

case "$ARCH" in
  arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
  x86_64)        TARGET="x86_64-apple-darwin" ;;
  *)             echo "Error: Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Fetch latest version
LATEST="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' | cut -d'"' -f4)"

if [ -z "$LATEST" ]; then
  echo "Error: Could not determine latest version."
  exit 1
fi

VERSION="${LATEST#v}"
URL="https://github.com/$REPO/releases/download/$LATEST/gaze-v${VERSION}-${TARGET}.tar.gz"

echo "Installing gaze v${VERSION} for ${TARGET}..."
echo "  From: $URL"
echo "  To:   $INSTALL_DIR/gaze"

# Download and extract
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fsSL "$URL" | tar xz -C "$TMP"

# Install
mkdir -p "$INSTALL_DIR"
mv "$TMP/gaze" "$INSTALL_DIR/gaze"
chmod +x "$INSTALL_DIR/gaze"

echo ""
echo "Installed gaze v${VERSION} to $INSTALL_DIR/gaze"
"$INSTALL_DIR/gaze" --version
```

### 6.2 使い方

```bash
# デフォルト (/usr/local/bin)
curl -fsSL https://gaze.dev/install.sh | sh

# カスタムディレクトリ
curl -fsSL https://gaze.dev/install.sh | INSTALL_DIR=~/.local/bin sh
```

---

## 7. cargo-binstall サポート (ゼロコスト追加)

### 7.1 概要

`cargo-binstall` は `cargo install` の代替で、ソースビルドの代わりに GitHub Releases からプリビルドバイナリをダウンロードする。Rust 開発者にリーチする追加チャネル。

### 7.2 設定

```toml
# crates/gaze-cli/Cargo.toml に追加
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/gaze-v{ version }-{ target }{ archive-suffix }"
bin-dir = "{ bin }{ binary-ext }"
pkg-fmt = "tgz"
```

### 7.3 使い方

```bash
# cargo-binstall がインストール済みなら
cargo binstall gaze-cli

# 通常の cargo install (ソースビルド、フォールバック)
cargo install gaze-cli
```

GitHub Releases のアーカイブ命名が `gaze-v{version}-{target}.tar.gz` であれば、
`cargo-binstall` が自動検出するため、`[package.metadata.binstall]` がなくても動作するケースが多い。明示的に書いておく方が確実。

---

## 8. 実装チェックリスト

### Phase 1: ベータリリース準備

- [ ] `RQ-Akiyoshi/homebrew-gaze` リポジトリ作成
- [ ] `Formula/gaze.rb` (プリビルドバイナリ方式) 作成
- [ ] `.github/workflows/release-cli.yml` をメインリポジトリに追加
- [ ] `homebrew-gaze/.github/workflows/update-formula.yml` 作成
- [ ] `TAP_GITHUB_TOKEN` (Fine-grained PAT) を作成・登録
- [ ] `[profile.release]` にサイズ最適化設定を追加
- [ ] `[package.metadata.binstall]` を `crates/gaze-cli/Cargo.toml` に追加
- [ ] `install.sh` を作成し Web サイトに配置
- [ ] テストリリース実行: `git tag v0.1.0-rc.1 && git push --tags`
- [ ] `brew tap RQ-Akiyoshi/gaze && brew install gaze` で動作確認
- [ ] `curl -fsSL ... | sh` で動作確認
- [ ] `gaze capture` の画面収録権限フローを確認

### Phase 2: homebrew-core 申請 (75+ stars 到達後)

- [ ] ソースビルド方式の Formula を作成
- [ ] `Cargo.lock` が Git 管理されていることを確認
- [ ] macOS 3バージョン + Linux でビルド可能なことを確認
- [ ] homebrew-core に PR 提出

---

## 9. 設計判断ログ

| 判断 | 採用案 | 却下案 | 理由 |
|------|--------|--------|------|
| 初期配布 | Homebrew Tap (プリビルド) | homebrew-core | ≥75 stars 要件、beta 不可制約 |
| Formula 方式 | プリビルドバイナリ | ソースビルド | インストール数秒 vs 3-5分。ユーザー体験優先 |
| Tap 更新 | repository_dispatch + 自動生成 | 手動 Formula 編集 | リリースごとの手作業を排除 |
| コード署名 | Homebrew 配布では不要 | Apple Developer ID 署名 | Homebrew が Gatekeeper をバイパスするため |
| curl インストーラ | 併用 (Homebrew 補完) | Homebrew のみ | Homebrew を使わない層のカバー |
| cargo-binstall | 対応 (ゼロコスト) | 非対応 | Cargo.toml に3行追加するだけ |
