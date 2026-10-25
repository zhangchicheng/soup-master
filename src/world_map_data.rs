//! Tiled JSON is the source of truth for art placement and road connectivity.
use crate::constants::{BASE_HEIGHT, BASE_WIDTH};
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};

pub const MAP_PATH: &str = "maps/overworld.tmj";
const H_FLIP: u32 = 0x8000_0000;
const V_FLIP: u32 = 0x4000_0000;
const UNSUPPORTED_FLAGS: u32 = 0x3000_0000;
fn yes() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Property {
    pub name: String,
    pub value: serde_json::Value,
}
fn int(properties: &[Property], name: &str) -> Option<i64> {
    properties.iter().find(|p| p.name == name)?.value.as_i64()
}
fn boolean(properties: &[Property], name: &str) -> bool {
    properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.as_bool())
        .unwrap_or(false)
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tile {
    pub id: u32,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub class: String,
    pub image: String,
    pub imagewidth: u32,
    pub imageheight: u32,
    #[serde(default)]
    pub properties: Vec<Property>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct Tileset {
    pub firstgid: u32,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub tiles: Vec<Tile>,
}
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Object {
    pub id: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub class: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub gid: Option<u32>,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub properties: Vec<Property>,
}
impl Object {
    pub fn class(&self) -> &str {
        if self.kind.is_empty() {
            &self.class
        } else {
            &self.kind
        }
    }
    /// Tiled tile objects use a bottom-left anchor, point objects use their point.
    pub fn center(&self) -> Vec2 {
        if self.gid.is_some() {
            Vec2::new(self.x + self.width / 2.0, self.y - self.height / 2.0)
        } else {
            Vec2::new(self.x, self.y)
        }
    }
    pub fn top_left(&self) -> Vec2 {
        Vec2::new(
            self.x,
            self.y - if self.gid.is_some() { self.height } else { 0.0 },
        )
    }
}
#[derive(Deserialize, Debug, Clone)]
pub struct Layer {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub data: Vec<u32>,
    #[serde(default)]
    pub objects: Vec<Object>,
    #[serde(default)]
    pub properties: Vec<Property>,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub offsetx: f32,
    #[serde(default)]
    pub offsety: f32,
}
#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct WorldMapData {
    pub width: u32,
    pub height: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub orientation: String,
    #[serde(default)]
    pub infinite: bool,
    pub tilesets: Vec<Tileset>,
    pub layers: Vec<Layer>,
}
#[derive(Debug, Clone)]
pub struct LevelNode {
    pub index: usize,
    pub name: String,
    pub center: Vec2,
    pub cell: IVec2,
    pub width: f32,
    pub height: f32,
}
#[derive(Resource, Debug, Clone)]
pub struct MapRuntime {
    pub data: WorldMapData,
    pub roads: HashMap<IVec2, u8>,
    pub levels: Vec<LevelNode>,
    pub spawn: IVec2,
    pub car: Object,
}
impl WorldMapData {
    pub fn tile(&self, gid: u32) -> Option<&Tile> {
        let clean = gid & !(H_FLIP | V_FLIP | UNSUPPORTED_FLAGS);
        let set = self
            .tilesets
            .iter()
            .filter(|s| s.firstgid <= clean)
            .max_by_key(|s| s.firstgid)?;
        set.tiles.iter().find(|t| t.id == clean - set.firstgid)
    }
    pub fn flipped(gid: u32) -> (bool, bool) {
        (gid & H_FLIP != 0, gid & V_FLIP != 0)
    }
    pub fn cell(&self, p: Vec2) -> IVec2 {
        IVec2::new(
            (p.x / self.tilewidth as f32).floor() as i32,
            (p.y / self.tileheight as f32).floor() as i32,
        )
    }
    pub fn cell_center(&self, c: IVec2) -> Vec2 {
        (c.as_vec2() + Vec2::splat(0.5)) * Vec2::new(self.tilewidth as f32, self.tileheight as f32)
    }
    pub fn origin(&self) -> Vec2 {
        Vec2::new(
            ((BASE_WIDTH - (self.width * self.tilewidth) as f32) / 2.0).max(0.0),
            ((BASE_HEIGHT - (self.height * self.tileheight) as f32) / 2.0).max(0.0),
        )
    }
    pub fn image_path(image: &str) -> Result<String, String> {
        let mut parts = vec!["maps"];
        if image.starts_with('/') || image.contains(':') {
            return Err("Image paths must be relative to assets/maps".into());
        }
        for part in image.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err("Image path escapes assets".into());
                    }
                }
                other => parts.push(other),
            }
        }
        Ok(parts.join("/"))
    }
    pub fn runtime(&self) -> Result<MapRuntime, String> {
        if self.orientation != "orthogonal"
            || self.infinite
            || self.tilewidth != 16
            || self.tileheight != 16
            || self.width == 0
            || self.height == 0
            || self.width > 128
            || self.height > 128
        {
            return Err("Use a finite orthogonal map, 16x16 tiles, at most 128x128 cells".into());
        }
        if self.tilesets.iter().any(|s| s.source.is_some()) {
            return Err(
                "Embed the tileset in Tiled before saving (external TSJ is not supported)".into(),
            );
        }
        let mut gids = HashSet::new();
        for set in &self.tilesets {
            for tile in &set.tiles {
                if set.firstgid == 0
                    || !gids.insert(set.firstgid + tile.id)
                    || tile.imagewidth == 0
                    || tile.imageheight == 0
                {
                    return Err("Invalid or duplicate tile id/image size".into());
                }
                Self::image_path(&tile.image)?;
            }
        }
        let mut roads = HashMap::new();
        let mut levels = Vec::new();
        let mut spawn = None;
        let mut car = None;
        for layer in &self.layers {
            if layer.offsetx != 0.0 || layer.offsety != 0.0 {
                return Err(format!("Layer {} must have zero pixel offset", layer.name));
            }
            if !layer.visible {
                continue;
            }
            match layer.kind.as_str() {
                "tilelayer" => {
                    if layer.width != self.width
                        || layer.height != self.height
                        || layer.data.len() != (self.width * self.height) as usize
                    {
                        return Err(format!(
                            "Layer {} has invalid dimensions or non-CSV data",
                            layer.name
                        ));
                    }
                    let navigation = boolean(&layer.properties, "navigation");
                    for (i, gid) in layer
                        .data
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|(_, g)| *g != 0)
                    {
                        if gid & UNSUPPORTED_FLAGS != 0 {
                            return Err("Use the supplied directional road tiles instead of diagonal tile rotations".into());
                        }
                        let tile = self
                            .tile(gid)
                            .ok_or_else(|| format!("Unknown tile GID {gid}"))?;
                        if navigation {
                            // Terrain/decorative tiles can cover a navigation layer
                            // without becoming drivable. Explicit road metadata is
                            // still validated, including malformed masks.
                            let decorative = matches!(tile.kind.as_str(), "Terrain" | "Decoration")
                                || matches!(tile.class.as_str(), "Terrain" | "Decoration");
                            if decorative && !tile.properties.iter().any(|p| p.name == "road_mask")
                            {
                                continue;
                            }
                            let mut mask = int(&tile.properties, "road_mask")
                                .filter(|m| (0..=15).contains(m))
                                .ok_or_else(|| {
                                    format!(
                                        "Layer '{}' at cell {},{}: tile '{}' (GID {}) needs integer road_mask 0..15; use a road tile with connection properties",
                                        layer.name, i as u32 % self.width, i as u32 / self.width,
                                        tile.image, gid & 0x0fff_ffff
                                    )
                                })? as u8;
                            let (h, v) = Self::flipped(gid);
                            if h {
                                mask = (mask & 5) | ((mask & 2) << 2) | ((mask & 8) >> 2);
                            }
                            if v {
                                mask = (mask & 10) | ((mask & 1) << 2) | ((mask & 4) >> 2);
                            }
                            let cell = IVec2::new(
                                i as i32 % self.width as i32,
                                i as i32 / self.width as i32,
                            );
                            if roads.insert(cell, mask).is_some() {
                                return Err("Only one navigation tile may occupy a cell".into());
                            }
                        }
                    }
                }
                "objectgroup" => {
                    for object in layer.objects.iter().filter(|o| o.visible) {
                        if !object.x.is_finite() || !object.y.is_finite() || object.rotation != 0.0
                        {
                            return Err(format!(
                                "Object {} needs finite position and zero rotation",
                                object.id
                            ));
                        }
                        if let Some(gid) = object.gid {
                            if gid & UNSUPPORTED_FLAGS != 0 || self.tile(gid).is_none() {
                                return Err(format!("Unknown/rotated tile object {}", object.id));
                            }
                        }
                        match object.class() {
                            "Level" => {
                                let index = int(&object.properties, "level")
                                    .filter(|i| (0..3).contains(i))
                                    .ok_or("Level objects need integer property level=0,1,2")?
                                    as usize;
                                levels.push(LevelNode {
                                    index,
                                    name: object.name.clone(),
                                    center: object.center(),
                                    cell: self.cell(object.center()),
                                    width: if object.width > 0.0 {
                                        object.width
                                    } else {
                                        36.0
                                    },
                                    height: if object.height > 0.0 {
                                        object.height
                                    } else {
                                        26.0
                                    },
                                });
                            }
                            "Car" => {
                                if spawn.replace(self.cell(object.center())).is_some() {
                                    return Err("Use exactly one Car object".into());
                                }
                                car = Some(object.clone());
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    return Err(format!(
                        "Unsupported layer type {}: use tilelayer or objectgroup",
                        layer.kind
                    ));
                }
            }
        }
        let spawn = spawn.ok_or("Map needs one Car object")?;
        if !roads.contains_key(&spawn) {
            return Err(format!(
                "Car must start on a road tile: its center is in cell {},{}. Move the Car object so its center is over a road",
                spawn.x, spawn.y
            ));
        }
        if levels.len() != 3 || levels.iter().map(|l| l.index).collect::<HashSet<_>>().len() != 3 {
            return Err("Map needs one Level object for each level=0,1,2".into());
        }
        let mut car = car.unwrap();
        if car.width <= 0.0 {
            car.width = 28.0;
        }
        if car.height <= 0.0 {
            car.height = 20.0;
        }
        let map = MapRuntime {
            data: self.clone(),
            roads,
            levels,
            spawn,
            car,
        };
        for (&cell, &mask) in &map.roads {
            for (direction, bit, opposite) in DIRECTIONS {
                if mask & bit != 0
                    && map
                        .roads
                        .get(&(cell + direction))
                        .is_none_or(|m| m & opposite == 0)
                {
                    return Err(format!(
                        "Road connector at {},{} has no matching neighbor",
                        cell.x, cell.y
                    ));
                }
            }
        }
        let reached = map.reachable(spawn);
        for level in &map.levels {
            // Storefront artwork can sit beside the road. Entering a kitchen
            // already uses a one-cell Manhattan interaction range.
            if !reached.contains(&level.cell)
                && !DIRECTIONS
                    .iter()
                    .any(|(direction, _, _)| reached.contains(&(level.cell + *direction)))
            {
                return Err(format!(
                    "Level {} is not on or next to a reachable road",
                    level.index + 1
                ));
            }
        }
        Ok(map)
    }
}
pub const DIRECTIONS: [(IVec2, u8, u8); 4] = [
    (IVec2::NEG_Y, 1, 4),
    (IVec2::X, 2, 8),
    (IVec2::Y, 4, 1),
    (IVec2::NEG_X, 8, 2),
];
impl MapRuntime {
    pub fn can_step(&self, from: IVec2, direction: IVec2) -> bool {
        let Some((_, bit, opposite)) = DIRECTIONS.iter().find(|(d, _, _)| *d == direction) else {
            return false;
        };
        self.roads.get(&from).is_some_and(|m| m & bit != 0)
            && self
                .roads
                .get(&(from + direction))
                .is_some_and(|m| m & opposite != 0)
    }
    pub fn reachable(&self, start: IVec2) -> HashSet<IVec2> {
        let mut reached = HashSet::from([start]);
        let mut pending = VecDeque::from([start]);
        while let Some(cell) = pending.pop_front() {
            for (d, _, _) in DIRECTIONS {
                if self.can_step(cell, d) && reached.insert(cell + d) {
                    pending.push_back(cell + d);
                }
            }
        }
        reached
    }
    pub fn scroll_for(&self, position: Vec2) -> Vec2 {
        let extent = Vec2::new(
            (self.data.width * self.data.tilewidth) as f32,
            (self.data.height * self.data.tileheight) as f32,
        );
        (position - Vec2::new(BASE_WIDTH, BASE_HEIGHT) / 2.0).clamp(
            Vec2::ZERO,
            (extent - Vec2::new(BASE_WIDTH, BASE_HEIGHT)).max(Vec2::ZERO),
        )
    }
    pub fn nearby_level(&self, cell: IVec2) -> Option<&LevelNode> {
        self.levels
            .iter()
            .filter_map(|l| {
                let d = (l.cell - cell).abs().element_sum();
                (d <= 1).then_some((d, l))
            })
            .min_by_key(|(d, l)| (*d, l.index))
            .map(|(_, l)| l)
    }
}
#[derive(Default, TypePath)]
pub struct WorldMapLoader;
impl AssetLoader for WorldMapLoader {
    type Asset = WorldMapData;
    type Settings = ();
    type Error = Box<dyn std::error::Error + Send + Sync>;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let map: WorldMapData = serde_json::from_slice(&bytes)?;
        map.runtime().map_err(std::io::Error::other)?;
        Ok(map)
    }
    fn extensions(&self) -> &[&str] {
        &["tmj"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn example() -> WorldMapData {
        serde_json::from_str(include_str!("../assets/maps/overworld.tmj")).unwrap()
    }
    #[test]
    fn edited_map_roads_connect_all_kitchens() {
        let map = example().runtime().unwrap();
        assert!(map.roads.contains_key(&map.spawn));
        assert_eq!(map.levels.len(), 3);
        let reached = map.reachable(map.spawn);
        assert_eq!(reached.len(), map.roads.len());
        for level in &map.levels {
            assert!(
                reached
                    .iter()
                    .any(|cell| (*cell - level.cell).abs().element_sum() <= 1)
            );
        }
    }
    #[test]
    fn roadside_kitchen_preserves_its_art_position_and_can_be_entered() {
        let mut data = example();
        let runtime = data.runtime().unwrap();
        // Find an empty roadside cell, independent of the user's painted route.
        let roadside = runtime
            .roads
            .keys()
            .flat_map(|cell| {
                DIRECTIONS
                    .iter()
                    .map(move |(direction, _, _)| *cell + *direction)
            })
            .find(|cell| {
                cell.x >= 0
                    && cell.y >= 0
                    && cell.x < data.width as i32
                    && cell.y < data.height as i32
                    && !runtime.roads.contains_key(cell)
            })
            .unwrap();
        let center = data.cell_center(roadside);
        let level = data
            .layers
            .iter_mut()
            .flat_map(|l| &mut l.objects)
            .find(|o| o.class() == "Level")
            .unwrap();
        let index = int(&level.properties, "level").unwrap() as usize;
        level.gid = None;
        level.x = center.x;
        level.y = center.y;
        let mut map = data.runtime().unwrap();
        let level = map.levels.iter().find(|l| l.index == index).unwrap();
        assert_eq!(level.center, center);
        assert_eq!(level.cell, roadside);
        map.levels.retain(|level| level.index == index);
        assert!(map.reachable(map.spawn).iter().any(|cell| {
            (*cell - roadside).abs().element_sum() == 1
                && map
                    .nearby_level(*cell)
                    .is_some_and(|level| level.index == index)
        }));
    }
    #[test]
    fn disconnected_road_or_offroad_level_is_rejected() {
        let mut data = example();
        let map = data.runtime().unwrap();
        let removed = *map.roads.keys().find(|cell| **cell != map.spawn).unwrap();
        let index = (removed.y as u32 * data.width + removed.x as u32) as usize;
        data.layers
            .iter_mut()
            .find(|l| l.name == "Roads")
            .unwrap()
            .data[index] = 0;
        assert!(data.runtime().unwrap_err().contains("matching neighbor"));
        let mut data = example();
        let level = data
            .layers
            .iter_mut()
            .flat_map(|l| &mut l.objects)
            .find(|o| o.class() == "Level")
            .unwrap();
        level.gid = None;
        level.x = 0.0;
        level.y = 0.0;
        assert!(data.runtime().unwrap_err().contains("reachable road"));
    }
    #[test]
    fn terrain_and_decoration_on_roads_are_visible_but_not_drivable() {
        let original = example();
        let expected = original.runtime().unwrap();
        for kind in ["Terrain", "Decoration"] {
            let mut data = original.clone();
            // Exercise both Tiled's legacy `type` and newer `class` spellings.
            let tile = data
                .tilesets
                .iter_mut()
                .flat_map(|s| &mut s.tiles)
                .find(|t| t.image.ends_with("/ground.png"))
                .unwrap();
            tile.kind.clear();
            tile.class = kind.into();
            let terrain_gid = data
                .tilesets
                .iter()
                .find_map(|s| {
                    s.tiles
                        .iter()
                        .find(|t| t.image.ends_with("/ground.png"))
                        .map(|t| s.firstgid + t.id)
                })
                .unwrap();
            let empty = (0..data.width * data.height)
                .find(|i| {
                    !expected.roads.contains_key(&IVec2::new(
                        (*i % data.width) as i32,
                        (*i / data.width) as i32,
                    ))
                })
                .unwrap();
            data.layers
                .iter_mut()
                .find(|l| l.name == "Roads")
                .unwrap()
                .data[empty as usize] = terrain_gid;
            let map = data.runtime().unwrap();
            assert_eq!(map.roads, expected.roads);
            assert_eq!(
                map.data
                    .layers
                    .iter()
                    .find(|l| l.name == "Roads")
                    .unwrap()
                    .data[empty as usize],
                terrain_gid
            );
        }
    }
    #[test]
    fn malformed_road_masks_report_layer_coordinates_and_image() {
        let mut data = example();
        let (index, gid) = data
            .layers
            .iter()
            .find(|l| l.name == "Roads")
            .unwrap()
            .data
            .iter()
            .copied()
            .enumerate()
            .find(|(_, gid)| {
                *gid != 0
                    && data
                        .tile(*gid)
                        .unwrap()
                        .properties
                        .iter()
                        .any(|p| p.name == "road_mask")
            })
            .unwrap();
        let image = data.tile(gid).unwrap().image.clone();
        for set in &mut data.tilesets {
            for tile in &mut set.tiles {
                if tile.image == image {
                    tile.properties.retain(|p| p.name != "road_mask");
                }
            }
        }
        let error = data.runtime().unwrap_err();
        assert!(error.contains("Roads"));
        assert!(error.contains(&format!(
            "cell {},{}",
            index as u32 % data.width,
            index as u32 / data.width
        )));
        assert!(error.contains(&image));
        assert!(error.contains("integer road_mask 0..15"));
    }
    #[test]
    fn paths_and_tile_object_anchors_follow_tiled() {
        assert_eq!(
            WorldMapData::image_path("../images/world_map/tiles/ground.png").unwrap(),
            "images/world_map/tiles/ground.png"
        );
        assert!(WorldMapData::image_path("../../secrets").is_err());
        let object = Object {
            gid: Some(1000),
            x: 16.0,
            y: 48.0,
            width: 24.0,
            height: 32.0,
            ..Default::default()
        };
        assert_eq!(object.top_left(), Vec2::new(16.0, 16.0));
        assert_eq!(object.center(), Vec2::new(28.0, 32.0));
    }
}

#[cfg(test)]
mod loading_tests {
    use super::*;
    #[test]
    fn asset_loader_reads_tmj_from_disk_without_recompiling_map() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<WorldMapData>()
            .init_asset_loader::<WorldMapLoader>();
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<WorldMapData>(MAP_PATH);
        let mut loaded = false;
        for _ in 0..200 {
            app.update();
            if let Some(map) = app.world().resource::<Assets<WorldMapData>>().get(&handle) {
                assert_eq!(map.runtime().unwrap().levels.len(), 3);
                loaded = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(
            loaded,
            "Tiled map failed to load from assets/maps/overworld.tmj"
        );
    }
    #[test]
    fn large_map_camera_is_clamped_to_the_map_edges() {
        let mut map = super::tests::example().runtime().unwrap();
        assert_eq!(map.scroll_for(Vec2::new(200.0, 140.0)), Vec2::ZERO);
        map.data.width = 28;
        map.data.height = 20;
        assert_eq!(
            map.scroll_for(Vec2::new(300.0, 200.0)),
            Vec2::new(188.0, 120.0)
        );
        assert_eq!(
            map.scroll_for(Vec2::new(999.0, 999.0)),
            Vec2::new(224.0, 160.0)
        );
    }
}
