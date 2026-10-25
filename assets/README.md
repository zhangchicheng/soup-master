# Soup Master 运行资源

游戏直接加载以下素材，图片保持用户提供的原图与像素，不做重新绘制或调色。

| 目录 | 内容 |
| --- | --- |
| `images/kitchen/` | 厨房 tileset，包括角色、食材、厨具和动画 |
| `images/ui/` | 封面汤碗、厨师帽光标、订单底图 |
| `images/results/` | 结算背景和原始尺寸的亮星、空星 |
| `images/world_map/tiles/` | 地形、道路及可拼接图块 |
| `images/world_map/objects/` | 小车、树、山、关卡标记 |
| `fonts/` | 游戏文字和标题字体 |
| `sounds/music/` | 菜单/地图及厨房背景音乐 |
| `sounds/menu/` | 确认、返回、移动选择音效 |
| `sounds/sfx/` | 洗碗、切菜、煮汤、交餐、开场等游戏音效 |
| `levels/` | 三关的 RON 布局与食材箱设置 |
| `maps/` | Tiled 世界地图与编辑器配套文件 |
| `palettes/` | Nostalgos 12 调色板，供编辑素材使用 |

厨房的可编辑贴图现在是 `images/kitchen/tileset.png`。地图图片原路径保留，Tiled 内的相对引用保持有效；图块表中的素材即使暂时没摆在地图上，也会保留供拼接使用。

未使用素材原件在 `../backups/asset-cleanup-20261023T072027Z/assets/`，完整移动清单在该目录的 `manifest.json`。Aseprite 源文件、NPC 和食材源图的可编辑副本放在 `../art/source/`，不进入发布包。

`python3 scripts/build_web.py --audit`（在项目根目录运行）检查全部运行引用；Web 发布只打包这些引用，不包含备份、源文件、调色板或 Tiled 编辑器会话。
