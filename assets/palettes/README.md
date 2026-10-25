# Nostalgos 12 游戏配色

原始调色板：项目根目录 `nostalgos-12-1x.png`，未修改。项目副本为 `nostalgos-12.png`；`nostalgos-12.gpl` 可导入 Aseprite、GIMP 等编辑器。

共 12 色，按原图顺序：

`#DC6250 #DEADA5 #DAD4C9 #FFD183 #EEB24A #55927F #21525A #272A32 #2152A5 #5A8BDE #B89CE9 #844790`

## 已替换的素材

- `assets/images/**/*.png`：当前运行图片，按厨房、UI、结算、世界地图分类；不使用的旧图已移至 `backups/asset-cleanup-20261023T072027Z/`。
- `art/source/food/{Veggies_GB,Deli_GB,Fruit_GB}.aseprite`：直接修改原生可编辑文件，保留图层、动画帧、时长、透明度；每个文件载入相同调色板，另有透明索引。
- `src/theme.rs`、HUD、姓名输入与星星状态：使用调色板中的界面颜色。厨房颜色已经保存进图集，不再在加载时覆盖；未获得的星星读取独立 `star-empty.png`，避免乘色产生额外颜色。

PNG 尺寸和每个像素的 alpha 均与备份一致。转换禁用抖动，保留轮廓和布局。厨房沿用原先按物件分类的配色，映射到新色阶，区分木头、蔬菜、金属与两位厨师。其它 PNG 使用确定性的最近色映射。Aseprite 用逐像素最近色转换，保留原生编辑结构。

`art/source/` 和 `backups/` 不进入运行素材包，原件备份保留原色和结构，游戏不读取这些文件。重新运行地图准备脚本时会自动将导出的小图块转到当前调色板。

## 原素材备份与恢复

换色前完整备份：`backups/pre-nostalgos-12-20261017T065347Z/`。

其中 `assets/images/` 保存所有原 PNG 和 Aseprite，`src/` 保存换色前代码，`scripts/` 保存换色前准备脚本，`SHA256.json` 保存文件校验值。另有转换工具自动创建的 `backups/palette-*/` 素材备份。

若恢复旧版本的完整显示效果，需要同时恢复该备份里的 `assets/images/`、`src/` 和 `scripts/`；旧版本依赖运行时染色，单独恢复旧图集无法还原当时的厨房配色。恢复前请备份后续修改。

## 校验与再次转换

```bash
# 只校验所有 PNG 的可见像素均属于这 12 色，不修改文件
python3 scripts/recolor_assets.py

# 对新增加/修改过的素材再次换色；每次先自动备份 assets/images
python3 scripts/recolor_assets.py --apply
```

需要 ImageMagick；转换原生源文件还需要支持 Lua 的 Aseprite。`--legacy-kitchen` 仅用于首次迁移原版四色绿调图集，已经换色的图集不要再使用此参数。

地图编辑预览：`previews/world-map-editor.png`。实际 UI 实体的 CPU 布局预览：`previews/world-map-tiled-layout.png`、`previews/results-painted-layout.png`；文字由 ImageMagick 渲染，非 GPU 游戏截图。
