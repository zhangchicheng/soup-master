# 可拼接的世界地图（Tiled）

游戏现在读取 `overworld.tmj`，不再使用整张地图背景和写死的小车路线。用 Tiled 直接打开这个文件，或打开 `overworld.tiled-project` 后选择地图。

## 四层分别怎么改

| 图层 | 用途 | 编辑方法 |
| --- | --- | --- |
| Terrain | 地面、网格水面、点状草地 | 用图章/填充工具画 tile |
| Roads | 可行驶道路，包含直路、转角、T 路口、十字路口和端点 | 用方向图块拼接，或选择 Terrain Sets 中的 Roads 配合 Terrain Brush 自动衔接 |
| Scenery | 独立的树、山 | 选 Objects 图集，用 Insert Tile 对象工具放置；已有对象可直接拖动、复制和缩放 |
| Levels and car | 三个关卡入口、小车出生位置 | 拖动现有对象，保留类型和属性 |

地图网格为 **16×16**。地形 PNG 为 16×16，新道路原图为 **8×8**，Tiled 和游戏都按最近邻放大到一格显示；保留原图便于修改。图片在 `assets/images/world_map/tiles/`。对象图片在 `assets/images/world_map/objects/`，树 24×32、山 32×40、小车 28×20、店铺入口 stop.png 18×16、闪烁轮廓 stop_flash.png 20×18。可直接用像素编辑器修改这些原生尺寸 PNG。运行图块仍可直接编辑；整理前的旧素材原件保存在项目 `backups/` 下，源文件工作副本见 `art/README.md`。

`Roads` 图层有 `navigation=true` 属性。带 `Terrain` / `Decoration` 类型（或 class）的地面和装饰图片可以放在该层，仅显示，不参与道路连接；带 `Road` 类型的图块必须有有效的 `road_mask`。道路 tile 的 `road_mask` 属性表示连接方向：N=1、E=2、S=4、W=8，相加得到 0～15。例如：竖直道路为 5，水平道路为 10，北+东转角为 3，十字路口为 15。当前 tile 表先列 road-1～6，再列由这六张裁切/组合出的端点、路口和独立路段，已覆盖全部组合；你通常不需要手改这个属性。旧 road-00～15 已退出 tile 表并移至备份。

| 原图 | 方向 | road_mask |
| --- | --- | --- |
| road-1.png | 横路（东、西） | 10 |
| road-2.png | 竖路（北、南） | 5 |
| road-3.png | 北、东转角 | 3 |
| road-4.png | 北、西转角 | 9 |
| road-5.png | 南、东转角 | 6 |
| road-6.png | 南、西转角 | 12 |

配套图块在 `tiles/connections/`。端点沿用当前直路的完整外观，通过 `road_mask` 限制行驶方向，不显示额外的半格标记。修改六张原图后运行 `python3 scripts/prepare_road_tiles.py` 更新配套图块；原图本身由地图直接读取。旧道路和替换前地图备份在 `backups/pre-road-replacement-20261017T070658Z/`。

## 小车怎样操作

- 方向键或 WASD 直接开车，每次沿路移动一格，按住可连续行驶。
- 到路口时选择方向；小车不能驶入草地、水面或没有相互连接的路段。
- 停在关卡所在格或紧邻的一格，已解锁店铺外沿的 stop_flash 会交替亮灭（每 0.45 秒切换）；按 Enter / Space 进入。移动中、驶离或尚未解锁时不闪烁。
- 未解锁的店铺显示 stop_locked.png，不能进入；前一关获得一星后解锁并恢复 stop.png，靠近时显示 stop_flash 闪烁外沿。
- Esc 返回标题。进厨房再返回时会保留停车位置。

## 关卡和出生点的对象属性

`Level` 对象的整数属性 `level` 为 0、1、2，分别关联 `level01.ron`、`level02.ron`、`level03.ron`。对象名称用于显示关卡名；你可以改名称、位置和外观。当前保留三个入口；新增厨房数量需要同时扩展关卡/进度数据。

`Car` 对象必须恰好一个。它的中心决定出生格，必须位于道路上；关卡对象中心可以放在从出生点可达的道路格，或与道路上下左右紧邻的一格（店铺靠路边）；斜对角不算。Tiled 的 tile object 使用左下角为锚点，编辑器会自动处理，拖动时看对象中心落在哪一格即可。

保存文件后重启游戏，数据从磁盘读取，**不需要改 Rust 或重新编译地图**。不需要运行素材准备脚本。

可以在 Tiled 中扩大地图，镜头会跟随小车滚动，地图使用完整的 224×160 画面，不再预留顶部说明栏。示例为 14×10 格，当前支持最大 128×128 格。扩大后须填好 Terrain 图层，并继续连接 Roads。

## 支持的 Tiled 格式

使用有限的 Orthogonal 地图、16×16 网格、JSON 数组/CSV 图层数据、嵌入式 image collection tileset、Tile Layer 和 Object Layer；保持图层像素偏移为 0。示例已设好。

水平和垂直翻转可用；方向道路已有独立图块，建议直接选择对应图块，不使用对角旋转。当前不读取外置 TSJ、无限地图、group layer、base64 压缩图层或旋转对象。如果在 Tiled 中将图集导出成了外置 TSJ，请先 Embed Tileset 再保存。

断路、没有匹配的连接方向、出生点在路外、关卡不可达等问题会在加载时被拒绝，具体原因见终端。可先运行：

```sh
cargo run -- --validate-map assets/maps/overworld.tmj
```

地图格式依据：[Tiled JSON Map Format](https://doc.mapeditor.org/en/stable/reference/json-map-format/)。

## 美术来源与重新准备素材

道路使用用户提供的 road-1～6；其余地图 tile/object 美术使用内置 imagegen，以 `ref.jpeg` 为图像参考生成。历史原稿说明请查看对应的 `backups/` 目录；当前发布只读取图块和对象 PNG。`scripts/prepare_map_tiles.py` 对地形/对象原图采样，并调用新道路配套图块准备脚本，不再导出旧道路；它会覆盖已准备的 PNG，因此只在你要从原图重新导出素材时手动运行。日常拼地图或直接修改小 PNG 不需要运行。重新准备后会自动将输出映射到 `assets/palettes/nostalgos-12.png` 的 12 色，保留原稿。

## 线条图块

`Lines` 图集中已加入 `tiles/line-1.png` 和 `tiles/line-2.png`，保留 8×8 原图，编辑器和游戏按 16×16 网格放大显示。可画在 Terrain 或额外的普通 Tile Layer 上；这些是装饰图块，不带道路连接属性。也可放在 Roads 图层，但只显示图片，不作为可行驶道路。
