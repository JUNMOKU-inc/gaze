# Gaze GUI — Homebrew Cask 配布ガイド

> 作成日: 2026-04-22
> ステータス: 実装準備
> 関連: `11_homebrew_distribution_guide.md` (CLI Formula 版)

---

## 0. 結論（この作業は簡単か？）

**簡単な部類。** 実装コストは合計 **3〜5時間** 程度:

| 作業 | 所要 | 難易度 | 一度きり？ |
|------|------|-------|:---:|
| Apple Developer 証明書のエクスポート＆GitHub Secrets 登録 | 30〜45分 | 🟡 中（初回のみ手順が煩雑） | ✓ |
| `release-cli.yml` に署名＋公証ステップ追加 | 1〜2時間 | 🟡 中 | ✓ |
| `homebrew-gaze` タップリポジトリ作成＋Cask追加 | 30分 | 🟢 低 | ✓ |
| 自動更新ワークフロー追加 | 30分 | 🟢 低 | ✓ |
| 動作確認（テストタグで一周） | 30分〜1h | 🟢 低 | リリース毎 |

**2回目以降のリリース**は `git tag v0.2.0 && git push --tags` だけで全て自動化されます。

---

## 1. 前提条件チェック

### 1.1 既に整っているもの

- ✅ `src-tauri/Info.plist` — 画面収録等の用途説明 (既存)
- ✅ `src-tauri/Entitlements.plist` — hardened runtime 例外 (既存)
- ✅ `.github/workflows/release-cli.yml` — `.pkg` 生成パイプライン
- ✅ Bundle ID: `dev.gazeapp.gaze`
- ✅ 配布形態: Universal `.pkg`（GUI + CLI 同梱）

### 1.2 新規に必要なもの

- ❌ **Apple Developer Program 加入**（$99/年、JUMMOKU K.K（法人）契約推奨）
- ❌ **Developer ID Application 証明書**（.app の署名用）
- ❌ **Developer ID Installer 証明書**（.pkg の署名用）
- ❌ **App-specific Password** または **App Store Connect API Key**（公証API認証用）
- ❌ CI への codesign / productsign / notarytool ステップ
- ❌ `RQ-Akiyoshi/homebrew-gaze` リポジトリ

### 1.3 Homebrew Cask で必須か？

| 要件 | 必須度 | 理由 |
|------|:---:|------|
| `.pkg` の Developer ID 署名 | 🔴 **必須** | 未署名の `.pkg` はインストーラが "identified developer" でないと拒否 |
| 公証 (Notarization) | 🟡 **強く推奨** | 公証なしだと初回起動で警告（Cask自体は通るが UX 低下） |
| Stapler（公証チケット埋込） | 🟡 推奨 | オフラインでも警告なしで開ける |
| SHA256 | 🔴 必須 | Cask の整合性検証用 |
| 安定した DL URL | 🔴 必須 | GitHub Releases で対応済 |

---

## 2. Phase 1: 署名 & 公証のセットアップ

### 2.1 Apple Developer 証明書の取得

初回のみ Mac で実行（GUI操作）:

1. **Xcode or developer.apple.com で証明書発行**
   - https://developer.apple.com/account/resources/certificates
   - "Developer ID Application" を作成（`.app` 署名用）
   - "Developer ID Installer" を作成（`.pkg` 署名用）
   - CSR は Keychain Access → Certificate Assistant → Request a Certificate from CA

2. **キーチェーンからエクスポート**（両方）
   - Keychain Access → My Certificates → 対象証明書を右クリック → Export
   - `.p12` 形式で保存（強いパスワードを設定）
   - 保存先: `~/Desktop/gaze-app-cert.p12` / `~/Desktop/gaze-installer-cert.p12`

3. **Base64 化**（CI Secret 登録用）
   ```bash
   base64 -i ~/Desktop/gaze-app-cert.p12 -o gaze-app-cert.b64
   base64 -i ~/Desktop/gaze-installer-cert.p12 -o gaze-installer-cert.b64
   ```

### 2.2 公証認証情報の取得

**推奨: App Store Connect API Key**（長期安定・ローテーション容易）

1. https://appstoreconnect.apple.com/access/api
2. Keys → Generate API Key（Role: Developer 以上）
3. `.p8` ファイルをダウンロード（**再ダウンロード不可**）
4. Key ID、Issuer ID をメモ

代替案: Apple ID + App-specific Password（簡単だが個人 Apple ID 依存）
- https://appleid.apple.com/ → Sign-In and Security → App-Specific Passwords → 生成

### 2.3 GitHub Secrets の登録

```bash
# リポジトリは RQ-Akiyoshi/gaze 想定
REPO=RQ-Akiyoshi/gaze

# .p12 Base64（.app 用）
gh secret set APPLE_APP_CERT_P12_BASE64 --repo $REPO < gaze-app-cert.b64
gh secret set APPLE_APP_CERT_PASSWORD --repo $REPO  # .p12 パスフレーズ

# .p12 Base64（.pkg installer 用）
gh secret set APPLE_INSTALLER_CERT_P12_BASE64 --repo $REPO < gaze-installer-cert.b64
gh secret set APPLE_INSTALLER_CERT_PASSWORD --repo $REPO

# キーチェーン一時パスワード
gh secret set KEYCHAIN_PASSWORD --repo $REPO  # 任意のランダム文字列

# 公証（API Key 方式）
gh secret set APPLE_API_KEY_ID --repo $REPO       # 10桁英数
gh secret set APPLE_API_ISSUER_ID --repo $REPO    # UUID 形式
gh secret set APPLE_API_KEY_P8 --repo $REPO       # .p8 ファイル内容そのまま

# チームID（証明書に紐付く Developer ID team）
gh secret set APPLE_TEAM_ID --repo $REPO          # 10桁英数
```

> ⚠️ `gh secret set --body -` は stdinリダイレクト扱いされません。ファイル内容を渡す場合は `--body "$(cat file)"` か `gh secret set NAME < file`（旧形式）を使用。今回の例ではリダイレクト `< file` を使用しています。

### 2.4 release-cli.yml への署名・公証ステップ追加

`build-pkg` ジョブに以下を追加（既存ステップの前後に挿入）:

```yaml
# build-pkg ジョブ先頭に追加
- name: Import signing certificates
  env:
    APP_CERT_B64: ${{ secrets.APPLE_APP_CERT_P12_BASE64 }}
    APP_CERT_PW:  ${{ secrets.APPLE_APP_CERT_PASSWORD }}
    INST_CERT_B64: ${{ secrets.APPLE_INSTALLER_CERT_P12_BASE64 }}
    INST_CERT_PW:  ${{ secrets.APPLE_INSTALLER_CERT_PASSWORD }}
    KC_PW: ${{ secrets.KEYCHAIN_PASSWORD }}
  run: |
    KC=build.keychain
    security create-keychain -p "$KC_PW" "$KC"
    security default-keychain -s "$KC"
    security unlock-keychain -p "$KC_PW" "$KC"
    security set-keychain-settings -lut 21600 "$KC"

    echo "$APP_CERT_B64"  | base64 -d > app.p12
    echo "$INST_CERT_B64" | base64 -d > inst.p12

    security import app.p12  -k "$KC" -P "$APP_CERT_PW"  -T /usr/bin/codesign
    security import inst.p12 -k "$KC" -P "$INST_CERT_PW" -T /usr/bin/productsign
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KC_PW" "$KC"

    rm -f app.p12 inst.p12

# Universal .app 作成後、pkgbuild の前に追加
- name: Sign .app bundle
  env:
    TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
  run: |
    # Developer ID Application 証明書の識別子 (CN)
    IDENTITY="Developer ID Application: ${TEAM_ID}"  # 実際は "Developer ID Application: Your Org (TEAM_ID)"

    # Entitlements 適用・hardened runtime で署名
    codesign --force --deep --options runtime \
      --entitlements src-tauri/Entitlements.plist \
      --timestamp \
      --sign "$IDENTITY" \
      Gaze.app

    codesign --verify --strict --verbose=2 Gaze.app
    spctl --assess --type execute --verbose=2 Gaze.app || true  # 公証前なので reject が正常

# 既存の productbuild の後に「署名済み pkg を再生成」ステップを追加
# （または productbuild に --sign フラグを追加する形でもOK）
- name: Sign .pkg with productsign
  env:
    TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
    VERSION: ${{ needs.create-release.outputs.version }}
  run: |
    IDENTITY="Developer ID Installer: ${TEAM_ID}"
    productsign --sign "$IDENTITY" --timestamp \
      "gaze-v${VERSION}.pkg" "gaze-v${VERSION}-signed.pkg"
    mv "gaze-v${VERSION}-signed.pkg" "gaze-v${VERSION}.pkg"
    pkgutil --check-signature "gaze-v${VERSION}.pkg"

# Upload 前に公証ステップを追加
- name: Notarize .pkg
  env:
    API_KEY_ID: ${{ secrets.APPLE_API_KEY_ID }}
    API_ISSUER: ${{ secrets.APPLE_API_ISSUER_ID }}
    API_KEY_P8: ${{ secrets.APPLE_API_KEY_P8 }}
    VERSION:    ${{ needs.create-release.outputs.version }}
  run: |
    mkdir -p ~/.appstoreconnect/private_keys
    echo "$API_KEY_P8" > ~/.appstoreconnect/private_keys/AuthKey_${API_KEY_ID}.p8

    xcrun notarytool submit "gaze-v${VERSION}.pkg" \
      --key ~/.appstoreconnect/private_keys/AuthKey_${API_KEY_ID}.p8 \
      --key-id  "$API_KEY_ID" \
      --issuer  "$API_ISSUER" \
      --wait

    xcrun stapler staple "gaze-v${VERSION}.pkg"
    xcrun stapler validate "gaze-v${VERSION}.pkg"
```

> 💡 **`--timestamp`** は公証の必須条件。**`--options runtime`** (hardened runtime) も必須。

---

## 3. Phase 2: Tap リポジトリのセットアップ

既存ドキュメント `11_homebrew_distribution_guide.md §2` と同じ手順。同じ tap を Formula (CLI) と Cask (GUI) で共有する構成:

```
RQ-Akiyoshi/homebrew-gaze/
├── Formula/
│   └── gaze.rb          # CLI Formula（既存予定）
├── Casks/
│   └── gaze.rb          # GUI Cask（本ガイドの対象）
└── .github/
    └── workflows/
        ├── update-formula.yml
        └── update-cask.yml
```

`TAP_GITHUB_TOKEN` (Fine-grained PAT) は既に `.github/workflows/release-cli.yml` で使用済みなので再利用。

---

## 4. Phase 3: Cask ファイル作成

### 4.1 `Casks/gaze.rb`

```ruby
cask "gaze" do
  version "0.1.0"
  sha256 "PLACEHOLDER_SHA256"

  url "https://github.com/RQ-Akiyoshi/gaze/releases/download/v#{version}/gaze-v#{version}.pkg"
  name "Gaze"
  desc "LLM-optimized screen capture for Mac"
  homepage "https://gazeapp.dev"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :ventura"

  pkg "gaze-v#{version}.pkg",
      choices: [
        { "choiceIdentifier" => "app", "choiceAttribute" => "selected", "attributeSetting" => 1 },
        { "choiceIdentifier" => "cli", "choiceAttribute" => "selected", "attributeSetting" => 1 }
      ]

  uninstall pkgutil: [
    "dev.gazeapp.gaze",
    "dev.gazeapp.gaze.cli"
  ],
            delete:  [
              "/Applications/Gaze.app",
              "/usr/local/bin/gaze"
            ]

  zap trash: [
    "~/Library/Application Support/Gaze",
    "~/Library/Caches/dev.gazeapp.gaze",
    "~/Library/Logs/Gaze",
    "~/Library/Preferences/dev.gazeapp.gaze.plist"
  ]
end
```

### 4.2 SHA256 の取得

リリース後:
```bash
shasum -a 256 gaze-v0.1.0.pkg
```

自動化は §5 で実装。

### 4.3 ローカル検証

Cask を書き換えた直後に:
```bash
brew audit --cask ./Casks/gaze.rb
brew style --fix Casks/gaze.rb
brew install --cask ./Casks/gaze.rb   # ローカルファイルから直接インストール
brew uninstall --cask gaze
```

---

## 5. Phase 4: 自動更新ワークフロー

### 5.1 メインリポジトリ側: `release-cli.yml` に追加

既存の `update-homebrew` ジョブ（Formula 用）に加えて:

```yaml
update-cask:
  needs: [create-release, build-pkg, publish-release]
  runs-on: ubuntu-latest
  steps:
    - name: Download .pkg and compute SHA256
      run: |
        VERSION=${{ needs.create-release.outputs.version }}
        curl -fsSL -o gaze.pkg \
          "https://github.com/${{ github.repository }}/releases/download/v${VERSION}/gaze-v${VERSION}.pkg"
        echo "sha256=$(shasum -a 256 gaze.pkg | cut -d' ' -f1)" >> "$GITHUB_OUTPUT"
      id: sha

    - name: Dispatch update event to tap
      uses: peter-evans/repository-dispatch@v3
      with:
        token: ${{ secrets.TAP_GITHUB_TOKEN }}
        repository: RQ-Akiyoshi/homebrew-gaze
        event-type: update-cask
        client-payload: |
          {
            "version": "${{ needs.create-release.outputs.version }}",
            "sha256":  "${{ steps.sha.outputs.sha256 }}"
          }
```

### 5.2 Tap 側: `homebrew-gaze/.github/workflows/update-cask.yml`

```yaml
name: Update Cask

on:
  repository_dispatch:
    types: [update-cask]

permissions:
  contents: write

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Rewrite Cask
        env:
          VERSION: ${{ github.event.client_payload.version }}
          SHA256:  ${{ github.event.client_payload.sha256 }}
        run: |
          python3 - <<PY
          import re, os
          path = "Casks/gaze.rb"
          src = open(path).read()
          src = re.sub(r'version "[^"]+"', f'version "{os.environ["VERSION"]}"', src, count=1)
          src = re.sub(r'sha256 "[^"]+"',  f'sha256 "{os.environ["SHA256"]}"',  src, count=1)
          open(path, "w").write(src)
          PY

      - name: Commit & push
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add Casks/gaze.rb
          git commit -m "cask gaze ${{ github.event.client_payload.version }}"
          git push
```

---

## 6. Phase 5: 公式ドキュメント & LP 更新

### 6.1 README / LP インストール手順

```markdown
## Install

### Homebrew (macOS, 推奨)
```bash
brew tap RQ-Akiyoshi/gaze
brew install --cask gaze         # GUI + CLI
```

### CLI のみ
```bash
brew install gaze                # Formula 側（CLI バイナリのみ）
```

### 手動 (.pkg 直接)
[Releases ページ](https://github.com/RQ-Akiyoshi/gaze/releases/latest) から `.pkg` をダウンロード
```

### 6.2 LP ヒーロー CTA を `brew install --cask` に変更

`website/ja/index.html` の CTA ボタン:
```html
<code>brew tap RQ-Akiyoshi/gaze && brew install --cask gaze</code>
```

---

## 7. 実装チェックリスト

### Phase 1: 署名・公証
- [ ] Apple Developer Program 加入（JUMMOKU K.K（法人））
- [ ] Developer ID Application / Installer 証明書発行
- [ ] `.p12` エクスポート → Base64 化
- [ ] App Store Connect API Key 発行（`.p8`）
- [ ] GitHub Secrets 全10項目登録（§2.3）
- [ ] `release-cli.yml` に署名・公証ステップを追加（§2.4）
- [ ] テストタグ `v0.1.0-rc.1` でドライラン
- [ ] `spctl --assess` と `stapler validate` で成功確認

### Phase 2: タップリポジトリ
- [ ] `RQ-Akiyoshi/homebrew-gaze` リポジトリ作成（既存なら Casks/ 追加）
- [ ] `Casks/gaze.rb` 配置（§4.1）
- [ ] `brew audit --cask` / `brew style --fix` でバリデーション
- [ ] ローカル環境で `brew install --cask ./Casks/gaze.rb` → 起動確認

### Phase 3: 自動化
- [ ] メインリポジトリに `update-cask` ジョブ追加（§5.1）
- [ ] Tap リポジトリに `update-cask.yml` 追加（§5.2）
- [ ] 本番タグ `v0.1.0` でフルパイプライン検証

### Phase 4: ドキュメント
- [ ] README に brew install 手順記載
- [ ] LP ヒーロー CTA を `brew install --cask gaze` に変更
- [ ] changelog.html に配布開始を告知

---

## 8. トラブルシューティング

| 症状 | 原因 | 対処 |
|------|------|------|
| `codesign` が `-1011` エラー | キーチェーンのパスワード問題 | `security set-key-partition-list` の実行確認 |
| `notarytool submit` が "Invalid" | 未hardened-runtime / timestamp欠け | `--options runtime --timestamp` をcodesignに必須 |
| 公証で "The signature algorithm is not supported" | SHA-1 タイムスタンプ使用 | `--timestamp` は RFC3161 (SHA-256)、明示フラグ不要 |
| `brew install --cask` で "not notarized" 警告 | stapler未適用 | `xcrun stapler staple` を必ず実行 |
| Cask で "sha256 mismatch" | pkgへの再署名後SHA256変化 | 署名→stapler→SHA256計算の順に注意 |
| 初回起動で画面収録権限が無い | macOS TCC標準動作 | `11_homebrew_distribution_guide.md §5.2` 参照 |

---

## 9. 設計判断ログ

| 判断 | 採用案 | 却下案 | 理由 |
|------|--------|--------|------|
| 配布形態 | `.pkg` (Universal) | `.app + .zip` | 既存パイプラインが .pkg、CLI 同梱可能 |
| 公証方式 | App Store Connect API Key | Apple ID + App-specific PW | ローテーション容易、長期安定 |
| 証明書保管 | GitHub Secrets (Base64) | S3/R2 保管 | 標準パターン、追加インフラ不要 |
| タップ構成 | 単一リポで Formula + Cask 並存 | Cask 専用リポ分離 | メンテコスト削減、既存 TAP_GITHUB_TOKEN 再利用 |
| Cask 更新 | repository_dispatch | PR ベース (bump-homebrew-formula-action) | Formula 用と同じパターンで統一 |
| 公証ステップ失敗時 | リリース全体を失敗扱い | スキップしてリリース | 公証なしの配布は UX 破綻するため厳格運用 |

---

## 10. 次のアクション

最小で開始する場合:

1. **Apple Developer 加入** が最大のクリティカルパス（最大48時間審査）
2. 並行して Phase 2（タップ・Cask 雛形）を進められる
3. Secrets が揃い次第 Phase 1（CI統合）を実装
4. `v0.1.0-rc.1` タグで end-to-end 検証
5. 問題なければ `v0.1.0` でリリース → LP 更新

**Tips**: `v0.1.0-rc.N` タグを何度でも切り直して試せる。本番タグを汚す前に 3〜5 回は RC で回すのが安全。
