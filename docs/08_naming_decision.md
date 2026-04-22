# サービス名決定記録: Gaze

> 決定日: 2026-03-27
> 旧名称: Feedshot
> 新名称: Gaze
> ステータス: 確定

---

## 1. 決定の背景

- Feedshotは仮名称として使用していた
- サービスの本質は「スクリーンショットツール」ではなく「AIに画面を見せ、判断させ、行動につなげる視覚入力基盤」
- "shot" が入る名前はスクリーンショット専用に見え、OCR・録画・watch・差分検知・MCP・Enterprise展開まで含む将来の拡張性に制限がかかる
- CLI-firstアーキテクチャのため、毎日何十回も打つコマンド名としての短さ・打ちやすさが重要

## 2. 検討プロセス

### 2.1 命名ブリーフの策定
- `docs/06_service_naming_brief.md` として評価軸を定義
- コンセプト適合性、拡張性、CLI適性、ブランド性、日本語運用性、衝突リスクの6軸

### 2.2 候補の収集
2つの独立した提案プロセスを実施:

**提案A（短い名前重視）**: Gaze, Lenz, Optic, Loupe, Vizshot, Iris, Aperture, Sight, Viu 等18案
**提案B（複合語重視）**: SightLoop, SightFeed, SightDock, ViewFlow, ContextPort, Visport 等16案

### 2.3 エージェントチーム投票
5つの異なるペルソナで人気投票を実施:

| ペルソナ | 1位 | 2位 | 3位 |
|---------|-----|-----|-----|
| シニアバックエンドエンジニア（Go/Rust、CLI重視） | Gaze | Loupe | Lenz |
| スタートアップCTO（ブランド・チーム導入重視） | Gaze | Loupe | Lenz |
| インディー開発者/OSSマニア（Unix哲学重視） | Gaze | Lenz | Loupe |
| プロダクトデザイナー（ブランド・感性重視） | Gaze | Loupe | Lenz |
| 日本語DevRel（日本語運用性重視） | Loupe | Gaze | Lenz |

**結果**: Gaze が5人中4人の1位、全員のTop3入り（14pt/15pt満点）

## 3. Gazeを選んだ理由

### 3.1 名前の意味
"Gaze" = 「じっと見つめる、凝視する」

AIが画面を注意深く、持続的に見る — このプロダクトの本質を動詞1語、4文字で表現できる。

### 3.2 評価

| 評価軸 | スコア | 理由 |
|--------|--------|------|
| コンセプト適合性 | ★★★★★ | 「AIが見て判断する」をそのまま表現 |
| 拡張性 | ★★★★★ | capture/ocr/watch/mask/record/diffすべてが「gazeの行為」として成立 |
| CLI適性 | ★★★★★ | 4文字。`gaze capture`, `brew install gaze` |
| ブランド性 | ★★★★★ | 短く、静かで、力がある。ロゴは瞳孔モチーフで展開可能 |
| 日本語運用性 | ★★★★ | 「ゲイズ」は馴染み薄いが、4文字の短さで補える |
| 衝突リスク | ★★★★ | npmにファイルウォッチャーが存在するが非活発。Homebrew/crates.ioはクリーン |

### 3.3 CLIコマンドとしての使用例

```bash
brew install gaze

gaze capture                    # 画面キャプチャ
gaze capture --area             # 範囲選択
gaze ocr                       # OCRテキスト抽出
gaze watch --interval 5s       # 画面監視
gaze mask --pii                # 機密情報マスク
gaze record --duration 10s     # 録画
gaze diff before.png after.png # 差分検知

# パイプライン
gaze capture | gaze ocr | pbcopy
gaze capture --burst 5 | gaze mask --pii | claude
```

### 3.4 ブランドコピー

- **日本語**: 「Gaze — AIに画面を見せよう。」
- **英語**: 「Gaze — Show your screen to AI.」
- **サブコピー**: 「Gaze — Let AI see what you see.」

## 4. 次にやるべきこと

- [ ] ドメイン取得: `gaze.dev` or `getgaze.dev` or `usegaze.dev`
- [ ] GitHub organization/repo名の確保
- [ ] Homebrew formula名の確認
- [ ] crates.io パッケージ名の確認
- [ ] ロゴ・ビジュアルアイデンティティの設計
- [ ] OGP画像の更新

## 5. 不採用になった候補と理由

| 候補 | 不採用理由 |
|------|-----------|
| SightLoop | 9文字はCLIで長い。全投票者が「長い」と評価 |
| SightFeed | 9文字。"feed"が一方向的。SNS/メディア系に見える |
| Feedshot | "shot"がスクショ専用に閉じる。中長期の拡張性に懸念 |
| Lenz | Lensのtypoに見える。毎回「LensじゃなくてLenz」の説明コスト |
| Optic | Useoptic（optic.dev）との衝突。開発者ツール領域で埋もれる |
| Loupe | 日本語では最強だが「道具」のニュアンスが強く、AIの主体性が薄い |

## 参考資料

- `docs/06_service_naming_brief.md` — 命名作業指示書
- `docs/07_service_naming_proposal.md` — 提案書（SightLoop推奨）
