# React/TypeScript Removal Plan

Date: 2026-03-27
Status: Draft

## Goal

React/TypeScript/Vite ベースのアプリ UI を削除し、現状の実装実態に合わせて Tauri + Rust 中心の構成へ整理する。

今回の目的は「プレビューが既に Rust-driven HTML に移行済みである前提で、不要になった React/TypeScript の残骸と、それに紐づくビルド・テスト・ドキュメントの依存を外すこと」である。

## Current State Summary

- ランタイム上のプレビュー UI はすでに Rust 主導で動作している。
  - `src-tauri/src/preview_window.rs`
  - `WebviewWindowBuilder` で `preview.html` を開き、Rust が HTML/JS を注入する構成
- Tauri アプリ本体はデフォルトウィンドウなしで起動している。
  - `src-tauri/tauri.conf.json`
  - `src-tauri/src/lib.rs`
- トレイの `Settings...` は未実装で、React 製の設定画面は現状どこからも使われていない。
  - `src-tauri/src/tray.rs`
- 一方でビルド配線はまだ React/Vite 前提で残っている。
  - `src-tauri/tauri.conf.json`
  - `package.json`
  - `vite.config.ts`
  - `tsconfig.json`
- `src/` 配下の React コードは旧プレビュー実装の残骸が中心で、現行アーキテクチャとは整合していない。
  - `src/App.tsx`
  - `src/main.tsx`
  - `src/components/PreviewPopup/**`
  - `src/hooks/**`
  - `src/stores/**`

## Recommended Direction

推奨方針は次の通り。

- React/TypeScript/Vite/Vitest/Tailwind/Zustand をアプリ本体から完全に外す
- `preview.html` だけを最小の静的アセットとして残す
- Tauri の `frontendDist` を React ビルド成果物ではなく、静的アセット置き場に向ける
- 開発・ビルド・CI を Rust 中心に再編する

この方針を採るなら、React/TypeScript は「将来また必要になった時に別ウィンドウ用として再導入する」扱いに切り替える。

## In Scope

- 旧 React プレビュー実装の削除
- React エントリポイントの削除
- Vite/React/Vitest/Tailwind/Zustand 設定の削除
- Tauri のフロントエンド配線を静的アセット前提に変更
- Node / pnpm 依存の削減または除去
- CI、Git hooks、README、`CLAUDE.md` の更新
- 古い開発メモや次回タスクメモのうち、現在の実装と衝突する記述の整理

## Out Of Scope

- `src-tauri/src/preview_window.rs` の UI/UX 改修そのもの
- キャプチャ機能、画像最適化機能、トレイ機能の新規実装
- `Settings...`、アノテーション、OCR など未実装機能の新規 UI 追加
- `website/` 配下の静的サイト
- 過去の ADR や調査資料を歴史ごと書き換えること

## Removal Targets

### 1. React Runtime and UI Source

削除候補:

- `src/App.tsx`
- `src/main.tsx`
- `src/components/**`
- `src/hooks/**`
- `src/stores/**`
- `src/lib/**`
- `src/styles/index.css`
- `src/test/setup.ts`
- `src/vite-env.d.ts`
- `index.html`

考慮:

- `src/lib/**` は現状 React 側専用だが、削除前に Rust 側や docs から参照がないことを最終確認する
- `index.html` は React ルート用であり、静的 `preview.html` に切り替えた後は不要になる

### 2. Static Asset Boundary

維持対象:

- `public/preview.html`

考慮:

- 現在の `preview.html` はほぼ空だが、`WebviewUrl::App("preview.html")` のロード先として必要
- これを消すなら、Tauri 側で別の URL 戦略に変更する必要がある
- 最小変更なら `preview.html` は残し、Tauri が直接参照する静的アセットとして扱うのが安全

### 3. Build and Tooling

削除または再設計対象:

- `package.json`
- `pnpm-lock.yaml`
- `package-lock.json`
- `vite.config.ts`
- `vitest.config.ts`
- `tsconfig.json`
- `tsconfig.node.json`
- `biome.json`
- `lefthook.yml`

考慮:

- `src-tauri/tauri.conf.json` は現状 `beforeDevCommand: "pnpm dev"`、`beforeBuildCommand: "pnpm build"`、`frontendDist: "../dist"` に依存している
- React を削除するなら、Tauri が dev/build の両方でどの静的ディレクトリを読むかを先に決める必要がある
- `package.json` を完全削除するか、最小限の task runner として残すかは最初に決める
- `package-lock.json` と `pnpm-lock.yaml` が共存しているため、削除時に package manager 方針も整理する

### 4. CI and Local Automation

更新対象:

- `.github/workflows/ci.yml`
- `lefthook.yml`

考慮:

- 現在の CI は frontend job を前提としている
- React/Vitest を削除すると `pnpm lint`、`pnpm test:coverage` は成立しなくなる
- 代替として Rust job のみへ寄せるか、静的アセット存在チェック程度の軽量検証へ置き換える必要がある

### 5. Documentation

更新対象:

- `README.md`
- `CLAUDE.md`
- `docs/05_next_session_tasks.md`

確認・整理対象:

- `docs/03_frontend_coding_guidelines.md`
- `docs/02_development_order_and_tech_selection.md`
- `docs/02_tauri_v2_research.md`

考慮:

- `README.md` と `CLAUDE.md` は現行運用を示す一次資料なので、必ず更新する
- ADR や旧調査資料は歴史的記録として残してよいが、現状仕様と誤解されないように扱う
- `docs/03_frontend_coding_guidelines.md` は削除よりも「将来リッチ UI を再導入する時の参考資料」として退避・注記する方が安全

## Impact Analysis

### Runtime Impact

- 現行の capture -> optimize -> preview の実行経路はほぼ Rust 側のみで完結しているため、React 削除による直接的なランタイム影響は小さい
- 最大の注意点は `preview.html` を Tauri が読み込めなくならないこと

### Build Impact

- もっとも影響が大きいのはここ
- 現状 `tauri dev` / `tauri build` は Vite の成果物または dev server を前提としている
- React 削除後は Tauri の frontend asset 戦略を先に切り替えないと、アプリ起動やバンドルが壊れる

### Developer Workflow Impact

- `pnpm install`
- `pnpm build`
- `pnpm test`
- `pnpm lint`

上記の前提が崩れる。

代替候補:

- `cargo tauri dev`
- `cargo tauri build`
- `cargo fmt`
- `cargo clippy`
- `cargo test`

### Test Impact

- 既存の TypeScript テストはほぼ React/UI 周辺に集中している
- これらは React 削除と同時に消える
- カバレッジ目標を CI 上でどう扱うかを再定義する必要がある

### Documentation Impact

- 現在の README と `CLAUDE.md` は実装実態より React/TS への言及が強い
- このまま React を消すとドキュメントの方が嘘になるため、コード削除と同時に修正が必要

## Key Decisions To Make Before Implementation

### Decision 1. Static Asset Directory

候補:

- `public/` をそのまま Tauri の静的配布ディレクトリとして使う
- `ui-static/` のような専用ディレクトリを新設する

推奨:

- 最小変更のため `public/` 再利用を優先

### Decision 2. Node Toolchain Policy

候補:

- Node / pnpm / package.json を完全撤去する
- package.json は残すが、React/Vite/Vitest 依存だけ消す

推奨:

- React/TypeScript 削除を明確にするなら完全撤去寄り
- ただし team が `pnpm tauri dev` に依存しているなら、移行コストを見て段階的に削る

### Decision 3. Handling of Frontend Docs

候補:

- 完全削除
- `archive` 扱いにする
- 将来の rich UI 再導入用ガイドとして保持する

推奨:

- 一次資料は更新
- 設計資料は archive または historical 扱いで保持

## Proposed Execution Plan

### Phase 1. Freeze the Target Architecture

- 「今後しばらく Settings / Annotation / Overlay を React で実装しない」ことを明文化する
- `preview.html` を残す前提で静的アセット構成を確定する
- Node を完全撤去するか、暫定残置するかを決める

### Phase 2. Rewire Tauri to Static Assets

- `src-tauri/tauri.conf.json` を更新する
- React の dev/build コマンド依存を外す
- `preview.html` が dev/build/bundle の各経路で読めることを確認する

### Phase 3. Remove Dead React Code

- `src/` 配下と `index.html` を削除する
- React 固有のスタイル、store、hook、テストを削除する
- 削除後に参照切れがないことを `rg` で確認する

### Phase 4. Remove Tooling and Dependency Residue

- `package.json` の不要依存を除去、またはファイル自体を削除する
- lockfile 方針を一本化する
- `vite.config.ts`、`vitest.config.ts`、`tsconfig*.json`、`biome.json` を削除または再設計する
- `lefthook.yml` を Rust 中心へ更新する

### Phase 5. Update CI and Docs

- `.github/workflows/ci.yml` を Rust 中心に組み替える
- `README.md` を現行アーキテクチャに合わせる
- `CLAUDE.md` を「Tauri v2 + Rust + static preview asset」前提へ更新する
- 旧 React 前提ドキュメントに historical 注記を入れるか、参照導線を弱める

### Phase 6. Verification

- `cargo tauri dev` で起動できる
- トレイ表示が出る
- グローバルショートカットからキャプチャできる
- プレビューが表示される
- Copy / Save / Close / Expand / Collapse が動く
- `cargo tauri build` で preview asset を含んだ bundle が作れる
- `rg` で React/Vite/Vitest 参照が残っていないことを確認する

## Risks and Mitigations

### Risk 1. Tauri Dev Flow Breakage

内容:

- dev server 前提の設定を外した結果、`tauri dev` が起動しない可能性がある

対策:

- まず `frontendDist` を静的ディレクトリに切り替えた小変更だけで動作確認する
- React 削除はその後に行う

### Risk 2. Hidden Asset Dependency on `preview.html`

内容:

- `preview.html` を軽視して消すと、プレビューウィンドウ生成が失敗する

対策:

- `preview.html` は最後まで残す
- 代替 URL 戦略に移行するなら別タスクとして切り出す

### Risk 3. Documentation Drift

内容:

- コードだけ削って README / `CLAUDE.md` を更新しないと、以後の開発判断を誤らせる

対策:

- コード削除と同じ PR で一次資料も更新する

### Risk 4. Future UI Reintroduction Cost

内容:

- 設定画面やアノテーション実装時に再び web UI スタックが必要になる可能性がある

対策:

- 今回は「現状不要な stack を外す」ことを優先する
- 必要になれば別ウィンドウ専用に再導入する前提で ADR を残す

## Files Expected To Change

高確率で変更または削除されるファイル:

- `src-tauri/tauri.conf.json`
- `public/preview.html`
- `README.md`
- `CLAUDE.md`
- `.github/workflows/ci.yml`
- `lefthook.yml`
- `package.json`
- `pnpm-lock.yaml`
- `package-lock.json`
- `vite.config.ts`
- `vitest.config.ts`
- `tsconfig.json`
- `tsconfig.node.json`
- `biome.json`
- `index.html`
- `src/**`

変更有無を最終確認するファイル:

- `docs/03_frontend_coding_guidelines.md`
- `docs/05_next_session_tasks.md`
- `docs/02_development_order_and_tech_selection.md`

## Exit Criteria

この計画の完了条件は次の通り。

- アプリ本体に React/TypeScript/Vite 依存が残っていない
- `preview.html` を含む最小静的アセットだけで Tauri が起動・ビルドできる
- CI とローカルフックが新構成に一致している
- README と `CLAUDE.md` が現行アーキテクチャを正しく説明している
- 旧 React 実装はコードベース上に残っていないか、残す場合でも historical と明記されている
