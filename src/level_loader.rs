use crate::components::*;
use crate::constants::*;
use crate::resources::*;
use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Async asset loader for LevelData (.ron files)
// ---------------------------------------------------------------------------

#[derive(Default, TypePath)]
pub struct LevelDataLoader;

impl AssetLoader for LevelDataLoader {
    type Asset = LevelData;
    type Settings = ();
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let ron_str = std::str::from_utf8(&bytes)?;
        let mut level_data: LevelData = ron::from_str(ron_str)?;
        level_data.spritesheet_image = load_context.load(level_data.spritesheet.clone());
        Ok(level_data)
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

pub struct LevelLoaderPlugin;

impl Plugin for LevelLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LevelData>()
            .init_asset_loader::<LevelDataLoader>();
    }
}

// ---------------------------------------------------------------------------
// Spawn level entities from already-loaded LevelData
// ---------------------------------------------------------------------------

/// Spawn all tile/station/player entities from pre-loaded LevelData.
pub fn spawn_level_entities(
    commands: &mut Commands,
    atlas_layouts: &mut Assets<TextureAtlasLayout>,
    level_data: &LevelData,
    sheet: &Image,
    player_count: usize,
) -> LevelState {
    let tile_w = level_data.tile_width;
    let tile_h = level_data.tile_height;
    let map_w = level_data.map_width;

    // Loading waits for the image dependency, so grid dimensions are authoritative.
    let texture_handle = level_data.spritesheet_image.clone();
    let sheet_columns = sheet.width() / tile_w as u32;
    let sheet_rows = sheet.height() / tile_h as u32;
    let layout =
        crate::tileset::layout_for_size(sheet.size(), UVec2::new(tile_w as u32, tile_h as u32))
            .expect("Atlas validated during loading");
    let layout_handle = atlas_layouts.add(layout);

    // Store tileset info as a resource
    commands.insert_resource(TilesetInfo {
        image: texture_handle.clone(),
        layout: layout_handle.clone(),
        tile_width: tile_w,
        tile_height: tile_h,
        columns: sheet_columns,
        rows: sheet_rows,
    });

    let available = level_data
        .tiles
        .iter()
        .filter_map(|id| match &level_data.tile_props.get(id)?.station {
            Some(StationDef::Crate(food)) => Some(parse_food_type(food)),
            _ => None,
        })
        .collect();
    commands.insert_resource(crate::recipes::Orders::new(available));

    // -- Collect station entities for linking later --
    let mut sink_entities: Vec<(Entity, Direction, f32, f32)> = Vec::new();
    let mut delivery_entities: Vec<(Entity, Direction, f32, f32)> = Vec::new();
    let mut dish_rack_entities: Vec<(Entity, f32, f32)> = Vec::new();
    let mut return_station_entities: Vec<(Entity, f32, f32)> = Vec::new();

    // Spawn tiles
    for (i, &tile_id) in level_data.tiles.iter().enumerate() {
        let tx = (i as u32 % map_w) as f32 * tile_w;
        let ty = (i as u32 / map_w) as f32 * tile_h;

        let tile_prop = level_data.tile_props.get(&tile_id);
        let atlas_index = match tile_prop.and_then(|prop| prop.station.as_ref()) {
            Some(StationDef::Crate(food)) => {
                crate::tileset::crate_sprite_index(parse_food_type(food))
            }
            _ => (tile_id - 1) as usize,
        };

        // Spawn tile sprite
        let mut tile_entity = commands.spawn((
            Sprite {
                image: texture_handle.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: layout_handle.clone(),
                    index: atlas_index,
                }),
                ..default()
            },
            Transform::from_xyz(tx, -ty, Z_FLOOR), // Negate Y for Bevy's Y-up
            GameplayEntity,
            MapTile { tile_id },
        ));

        if let Some(prop) = tile_prop {
            // Add collision
            if prop.collideable {
                tile_entity.insert(Collideable);
            }

            // Spawn station component if applicable
            if let Some(ref station_def) = prop.station {
                let kind = match station_def {
                    StationDef::Counter => StationKind::Counter,
                    StationDef::Stove => StationKind::Stove,
                    StationDef::Sink(_) => StationKind::Sink,
                    StationDef::Prep => StationKind::Prep,
                    StationDef::Crate(_) => StationKind::Crate,
                    StationDef::DishRack => StationKind::DishRack,
                    StationDef::DishReturn => StationKind::DishReturn,
                    StationDef::Delivery(_) => StationKind::Delivery,
                };

                tile_entity.insert((Station { kind }, StationItem { entity: None }));

                let ent = tile_entity.id();

                match station_def {
                    StationDef::Stove => {
                        // Stoves start with an empty pot
                        let pot_entity = commands
                            .spawn((Pot { soup: None }, ItemKind::Pot, GameplayEntity))
                            .id();
                        commands.entity(ent).insert(StationItem {
                            entity: Some(pot_entity),
                        });
                    }
                    StationDef::Sink(dir_str) => {
                        let dir = parse_direction(dir_str);
                        tile_entity.insert(DishLocationDirection { direction: dir });
                        sink_entities.push((ent, dir, tx, ty));
                    }
                    StationDef::Delivery(dir_str) => {
                        let dir = parse_direction(dir_str);
                        tile_entity.insert((
                            DishLocationDirection { direction: dir },
                            DeliveryState::default(),
                        ));
                        delivery_entities.push((ent, dir, tx, ty));
                    }
                    StationDef::Crate(ingredient_str) => {
                        let food_type = parse_food_type(ingredient_str);
                        tile_entity.insert((
                            CrateIngredient { food_type },
                            DidDispenseItem(false),
                            CrateLidAnimation::default(),
                        ));
                    }
                    StationDef::DishRack => {
                        tile_entity.insert(DishStack { dishes: Vec::new() });
                        let ent_id = tile_entity.id();
                        dish_rack_entities.push((ent_id, tx, ty));
                    }
                    StationDef::DishReturn => {
                        tile_entity.insert(DishStack { dishes: Vec::new() });
                        let ent_id = tile_entity.id();
                        return_station_entities.push((ent_id, tx, ty));
                    }
                    StationDef::Prep => {}
                    StationDef::Counter => {}
                }

                // Check if a dish should be placed here
                for dish_spawn in &level_data.dishes {
                    let dish_x = dish_spawn.x as f32 * tile_w;
                    let dish_y = dish_spawn.y as f32 * tile_h;
                    if (dish_x - tx).abs() < 0.1 && (dish_y - ty).abs() < 0.1 {
                        let dish_entity = commands
                            .spawn((
                                Dish {
                                    soup: None,
                                    clean_ratio: 1.0,
                                },
                                ItemKind::Dish,
                                GameplayEntity,
                            ))
                            .id();
                        commands.entity(ent).insert(StationItem {
                            entity: Some(dish_entity),
                        });
                    }
                }
            }
        }
    }

    // -- Link sinks to dish racks --
    for (sink_ent, dir, sx, sy) in &sink_entities {
        if let Some(target) =
            find_linked_station(*dir, *sx, *sy, tile_w, tile_h, &dish_rack_entities)
        {
            commands
                .entity(*sink_ent)
                .insert(LinkedReturnStation { entity: target });
        }
    }

    // -- Link delivery stations to return stations --
    for (del_ent, dir, dx, dy) in &delivery_entities {
        if let Some(target) =
            find_linked_station(*dir, *dx, *dy, tile_w, tile_h, &return_station_entities)
        {
            commands
                .entity(*del_ent)
                .insert(LinkedReturnStation { entity: target });
        }
    }

    // -- Spawn players --
    for (index, player_spawn) in level_data.players.iter().enumerate() {
        if index >= player_count {
            break;
        }
        let px = player_spawn.x as f32 * tile_w;
        let py = player_spawn.y as f32 * tile_h;

        let keymap = if index == 0 {
            KeyMap {
                up: KeyCode::KeyW,
                down: KeyCode::KeyS,
                left: KeyCode::KeyA,
                right: KeyCode::KeyD,
                pause: KeyCode::KeyQ,
                action: KeyCode::KeyV,
                transfer: KeyCode::KeyB,
            }
        } else {
            KeyMap {
                up: KeyCode::ArrowUp,
                down: KeyCode::ArrowDown,
                left: KeyCode::ArrowLeft,
                right: KeyCode::ArrowRight,
                pause: KeyCode::KeyP,
                action: KeyCode::Period,
                transfer: KeyCode::Slash,
            }
        };

        commands.spawn((
            Sprite {
                image: texture_handle.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: layout_handle.clone(),
                    index: player_sprite_index(index, Direction::Down),
                }),
                ..default()
            },
            Transform::from_xyz(px, -py, Z_PLAYER),
            Player { index },
            Facing {
                direction: Direction::Down,
            },
            Velocity {
                dx: 0.0,
                dy: 0.0,
                speed: PLAYER_SPEED,
            },
            DidMove(false),
            FacingStation { entity: None },
            HeldItem { entity: None },
            ActionState {
                action: false,
                transfer: false,
                cooldown: 0.0,
                cooldown_max: ACTION_COOLDOWN,
                activity: None,
                did_change_state: false,
                did_illegal_action: false,
            },
            keymap,
            GameplayEntity,
        ));
    }

    // Store the level data resource
    commands.insert_resource(level_data.clone());

    LevelState::new(level_data.time_limit, player_count)
}

pub use crate::tileset::player_sprite_index;

fn parse_direction(s: &str) -> Direction {
    match s {
        "left" => Direction::Left,
        "right" => Direction::Right,
        "up" => Direction::Up,
        "down" => Direction::Down,
        other => panic!("Unknown direction: {other}"),
    }
}

fn parse_food_type(s: &str) -> FoodType {
    match s {
        "tomato" => FoodType::Tomato,
        "lotus_root" | "lotus" => FoodType::LotusRoot,
        "meat" => FoodType::Meat,
        "cabbage" => FoodType::Cabbage,
        "fish" => FoodType::Fish,
        "eggplant" => FoodType::Eggplant,
        other => panic!("Unknown food type: {other}"),
    }
}

/// Walk in a direction from (sx, sy) looking for a station in the target list.
fn find_linked_station(
    dir: Direction,
    sx: f32,
    sy: f32,
    tile_w: f32,
    tile_h: f32,
    targets: &[(Entity, f32, f32)],
) -> Option<Entity> {
    let (dx, dy) = match dir {
        Direction::Left => (-tile_w, 0.0),
        Direction::Right => (tile_w, 0.0),
        Direction::Up => (0.0, -tile_h),
        Direction::Down => (0.0, tile_h),
    };

    let mut cx = sx + dx;
    let mut cy = sy + dy;

    // Walk up to 20 tiles in that direction looking for a match
    for _ in 0..20 {
        for (ent, tx, ty) in targets {
            if (cx - tx).abs() < 0.1 && (cy - ty).abs() < 0.1 {
                return Some(*ent);
            }
        }
        cx += dx;
        cy += dy;
    }
    None
}

#[cfg(test)]
mod atlas_loading_tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    #[test]
    fn ron_loader_loads_the_real_png_as_a_retained_dependency() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), LevelLoaderPlugin))
            .init_asset::<Image>()
            .register_asset_loader(bevy::image::ImageLoader::new(
                bevy::image::CompressedImageFormats::NONE,
            ));
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<LevelData>("levels/level01.ron");
        for _ in 0..250 {
            app.update();
            if let Some(level) = app.world().resource::<Assets<LevelData>>().get(&handle) {
                if let Some(image) = app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(&level.spritesheet_image)
                {
                    assert_eq!(image.width(), 176);
                    assert!(image.height() >= 240);
                    let layout =
                        crate::tileset::layout_for_size(image.size(), UVec2::splat(16)).unwrap();
                    assert_eq!(
                        layout.textures.len(),
                        (image.width() / 16 * (image.height() / 16)) as usize
                    );
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("LevelData must load images/kitchen/tileset.png before the kitchen is spawned");
    }
}
