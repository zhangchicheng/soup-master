use crate::systems::audio::{MenuSound, play_menu_sound};
use crate::{
    resources::*, screen_art::ScreenArt, screen_ui::*, states::AppState, theme::INK,
    world_map_data::*,
};
use bevy::{asset::LoadState, prelude::*};
use std::collections::HashMap;

pub struct WorldMapPlugin;
#[derive(Component)]
struct MapScreen;
#[derive(Component)]
struct MapRoot;
#[derive(Component)]
struct MapWorld;
#[derive(Component)]
struct MapCar;
#[derive(Component)]
struct MapStop {
    level: usize,
    normal: Handle<Image>,
}
#[derive(Component)]
struct StopFlash(usize);
#[derive(Resource, Default)]
struct StopPulse {
    active: Option<usize>,
    elapsed: f32,
}
#[derive(Component)]
struct LoadingText;
#[derive(Resource)]
struct MapLoad {
    handle: Handle<WorldMapData>,
    ready: bool,
    failed: bool,
}
#[derive(Resource, Default)]
pub struct MapImages(pub HashMap<String, Handle<Image>>);

pub const LEVEL_PATHS: [&str; 3] = [
    "levels/level01.ron",
    "levels/level02.ron",
    "levels/level03.ron",
];

/// Persists across kitchen visits; movement is driven by the painted road tiles.
#[derive(Resource, Default, Debug)]
pub struct MapDrive {
    pub cell: IVec2,
    pub position: Vec2,
    pub target: Option<IVec2>,
    pub facing: i32,
    valid: bool,
}
impl MapDrive {
    pub fn reset(&mut self, map: &MapRuntime) {
        self.cell = map.spawn;
        self.position = map.data.cell_center(self.cell);
        self.target = None;
        self.facing = 1;
        self.valid = true;
    }
    pub fn try_move(&mut self, map: &MapRuntime, direction: IVec2) {
        if self.target.is_none() && map.can_step(self.cell, direction) {
            self.target = Some(self.cell + direction);
            if direction.x != 0 {
                self.facing = direction.x;
            }
        }
    }
    pub fn advance(&mut self, map: &MapRuntime, dt: f32) {
        let Some(target) = self.target else {
            return;
        };
        let destination = map.data.cell_center(target);
        self.position = self.position.move_towards(destination, 64.0 * dt.max(0.0));
        if self.position.distance_squared(destination) < 0.0001 {
            self.cell = target;
            self.position = destination;
            self.target = None;
        }
    }
}
impl Plugin for WorldMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenArt>()
            .init_resource::<Campaign>()
            .init_resource::<RoundResult>()
            .init_resource::<MapDrive>()
            .init_resource::<StopPulse>()
            .init_asset::<WorldMapData>()
            .init_asset_loader::<WorldMapLoader>()
            .add_systems(OnEnter(AppState::WorldMap), start_map)
            .add_systems(
                Update,
                (
                    map_escape,
                    build_map,
                    (
                        map_controls,
                        drive_car,
                        update_stop_appearance,
                        animate_stop_highlight,
                    )
                        .chain()
                        .run_if(resource_exists::<MapRuntime>),
                )
                    .chain()
                    .run_if(in_state(AppState::WorldMap)),
            )
            .add_systems(OnExit(AppState::WorldMap), cleanup_map);
    }
}
fn map_label(
    root: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    text: impl Into<String>,
    x: f32,
    y: f32,
    size: f32,
) -> Entity {
    let entity = label(root, font, text, x, y, size, INK);
    root.commands().entity(entity).insert(TextShadow {
        offset: Vec2::splat(4.0),
        color: crate::theme::PAPER,
    });
    entity
}
fn start_map(mut commands: Commands, server: Res<AssetServer>) {
    commands.spawn((Camera2d, MapScreen));
    commands.insert_resource(StopPulse::default());
    commands.insert_resource(MapLoad {
        handle: server.load(MAP_PATH),
        ready: false,
        failed: false,
    });
    let font = server.load("fonts/gameboy.ttf");
    let mut text = Entity::PLACEHOLDER;
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                overflow: Overflow::clip(),
                ..default()
            },
            MapRoot,
            MapScreen,
        ))
        .with_children(|root| {
            text = map_label(root, &font, "LOADING MAP...", 8.0, 70.0, 8.0);
        });
    commands.entity(text).insert(LoadingText);
}
fn map_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    server: Res<AssetServer>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        play_menu_sound(&mut commands, &server, MenuSound::Back);
        next.set(AppState::Title);
    }
}
fn build_map(
    mut commands: Commands,
    server: Res<AssetServer>,
    maps: Res<Assets<WorldMapData>>,
    mut loading: ResMut<MapLoad>,
    art: Res<ScreenArt>,
    campaign: Res<Campaign>,
    mut drive: ResMut<MapDrive>,
    root: Single<Entity, With<MapRoot>>,
    mut loading_text: Query<(Entity, &mut Text), With<LoadingText>>,
) {
    if loading.ready || loading.failed {
        return;
    }
    let Some(data) = maps.get(&loading.handle) else {
        if matches!(server.load_state(loading.handle.id()), LoadState::Failed(_)) {
            loading.failed = true;
            for (_, mut text) in &mut loading_text {
                **text = "MAP COULD NOT LOAD\nESC TO RETURN".into();
            }
        }
        return;
    };
    let map = match data.runtime() {
        Ok(map) => map,
        Err(error) => {
            error!("Invalid world map: {error}");
            loading.failed = true;
            for (_, mut text) in &mut loading_text {
                **text = "MAP COULD NOT LOAD\nESC TO RETURN".into();
            }
            return;
        }
    };
    for (entity, _) in &loading_text {
        commands.entity(entity).despawn();
    }
    let mut images = MapImages::default();
    for set in &map.data.tilesets {
        for tile in &set.tiles {
            let path = WorldMapData::image_path(&tile.image).expect("validated asset path");
            images.0.insert(path.clone(), server.load(path));
        }
    }
    if !drive.valid || !map.roads.contains_key(&drive.cell) {
        drive.reset(&map);
    } else {
        drive.position = map.data.cell_center(drive.cell);
        drive.target = None;
    }
    let font = server.load("fonts/gameboy.ttf");
    let origin = map.data.origin();
    let mut car_entity = Entity::PLACEHOLDER;
    commands.entity(*root).with_children(|screen| {
        screen
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                MapWorld,
            ))
            .with_children(|root| {
                for layer in map.data.layers.iter().filter(|l| l.visible) {
                    if layer.kind == "tilelayer" {
                        for (i, gid) in layer
                            .data
                            .iter()
                            .copied()
                            .enumerate()
                            .filter(|(_, g)| *g != 0)
                        {
                            let tile = map.data.tile(gid).unwrap();
                            let image =
                                images.0[&WorldMapData::image_path(&tile.image).unwrap()].clone();
                            let p = origin
                                + Vec2::new(
                                    (i as u32 % map.data.width * map.data.tilewidth) as f32,
                                    (i as u32 / map.data.width * map.data.tileheight) as f32,
                                );
                            let entity = screen_image(
                                root,
                                image.clone(),
                                p.x,
                                p.y,
                                map.data.tilewidth as f32,
                                map.data.tileheight as f32,
                            );
                            let (flip_x, flip_y) = WorldMapData::flipped(gid);
                            if flip_x || flip_y {
                                root.commands().entity(entity).insert(ImageNode {
                                    image,
                                    flip_x,
                                    flip_y,
                                    ..default()
                                });
                            }
                        }
                    } else {
                        let mut objects: Vec<_> = layer
                            .objects
                            .iter()
                            .filter(|o| o.visible && o.class() != "Level" && o.class() != "Car")
                            .collect();
                        objects.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.id.cmp(&b.id)));
                        for object in objects {
                            if let Some(tile) = object.gid.and_then(|gid| map.data.tile(gid)) {
                                let p = origin + object.top_left();
                                let image = images.0
                                    [&WorldMapData::image_path(&tile.image).unwrap()]
                                    .clone();
                                let entity = screen_image(
                                    root,
                                    image.clone(),
                                    p.x,
                                    p.y,
                                    object.width,
                                    object.height,
                                );
                                let (flip_x, flip_y) = WorldMapData::flipped(object.gid.unwrap());
                                if flip_x || flip_y {
                                    root.commands().entity(entity).insert(ImageNode {
                                        image,
                                        flip_x,
                                        flip_y,
                                        ..default()
                                    });
                                }
                            }
                        }
                    }
                }
                for level in &map.levels {
                    let center = origin + level.center;
                    let badge_tile = map
                        .data
                        .layers
                        .iter()
                        .flat_map(|l| &l.objects)
                        .find(|o| o.class() == "Level" && o.center() == level.center)
                        .and_then(|o| o.gid)
                        .and_then(|gid| map.data.tile(gid));
                    let badge_image = map
                        .data
                        .layers
                        .iter()
                        .flat_map(|l| &l.objects)
                        .find(|o| o.class() == "Level" && o.center() == level.center)
                        .and_then(|o| o.gid)
                        .and_then(|g| map.data.tile(g))
                        .and_then(|t| images.0.get(&WorldMapData::image_path(&t.image).ok()?))
                        .cloned()
                        .unwrap_or_else(|| art.badge.clone());
                    let stop = screen_image(
                        root,
                        if level.index <= campaign.unlocked() {
                            badge_image.clone()
                        } else {
                            art.stop_locked.clone()
                        },
                        center.x - level.width / 2.0,
                        center.y - level.height / 2.0,
                        level.width,
                        level.height,
                    );
                    root.commands().entity(stop).insert((
                        Name::new(level.name.clone()),
                        MapStop {
                            level: level.index,
                            normal: badge_image,
                        },
                    ));
                    // The supplied flash is a 20x18 transparent outline around
                    // the 18x16 stop. Keep its border outside the scaled storefront.
                    let source_size = badge_tile
                        .map(|tile| Vec2::new(tile.imagewidth as f32, tile.imageheight as f32))
                        .unwrap_or(Vec2::new(18.0, 16.0));
                    let flash_size =
                        Vec2::new(level.width, level.height) * Vec2::new(20.0, 18.0) / source_size;
                    let flash = screen_image(
                        root,
                        art.stop_flash.clone(),
                        center.x - flash_size.x / 2.0,
                        center.y - flash_size.y / 2.0,
                        flash_size.x,
                        flash_size.y,
                    );
                    root.commands()
                        .entity(flash)
                        .insert((StopFlash(level.index), Visibility::Hidden));
                    map_label(
                        root,
                        &font,
                        (level.index + 1).to_string(),
                        center.x + 11.0,
                        center.y - 4.0,
                        6.0,
                    );
                }
                let car_image = map
                    .car
                    .gid
                    .and_then(|g| map.data.tile(g))
                    .and_then(|t| images.0.get(&WorldMapData::image_path(&t.image).ok()?))
                    .cloned()
                    .unwrap_or_else(|| art.van.clone());
                let p = origin + drive.position;
                car_entity = screen_image(
                    root,
                    car_image.clone(),
                    p.x - map.car.width / 2.0,
                    p.y - map.car.height / 2.0,
                    map.car.width,
                    map.car.height,
                );
                // Apply orientation on creation too, before the first movement frame.
                root.commands().entity(car_entity).insert(ImageNode {
                    image: car_image,
                    flip_x: drive.facing > 0,
                    ..default()
                });
            });
    });
    commands.entity(car_entity).insert(MapCar);
    commands.insert_resource(images);
    commands.insert_resource(map);
    loading.ready = true;
}
fn map_controls(
    mut commands: Commands,
    server: Res<AssetServer>,
    keyboard: Res<ButtonInput<KeyCode>>,
    map: Res<MapRuntime>,
    mut drive: ResMut<MapDrive>,
    mut campaign: ResMut<Campaign>,
    mut pending: ResMut<PendingScore>,
    mut next: ResMut<NextState<AppState>>,
) {
    if drive.target.is_some() {
        return;
    }
    if let Some(level) = map.nearby_level(drive.cell) {
        campaign.selected = level.index;
        if (keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space))
            && level.index <= campaign.unlocked()
        {
            play_menu_sound(&mut commands, &server, MenuSound::Enter);
            pending.0 = None;
            next.set(AppState::Loading);
            return;
        }
    }
    let direction = if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) {
        IVec2::NEG_Y
    } else if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) {
        IVec2::Y
    } else if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        IVec2::NEG_X
    } else if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        IVec2::X
    } else {
        return;
    };
    drive.try_move(&map, direction);
}
fn drive_car(
    time: Res<Time>,
    map: Res<MapRuntime>,
    mut drive: ResMut<MapDrive>,
    mut cars: Query<(&mut Node, &mut ImageNode), (With<MapCar>, Without<MapWorld>)>,
    mut world: Query<&mut Node, (With<MapWorld>, Without<MapCar>)>,
) {
    drive.advance(&map, time.delta_secs());
    let p = map.data.origin() + drive.position;
    let scroll = map.scroll_for(drive.position);
    for mut node in &mut world {
        node.left = Val::Px(-scroll.x.round() * 4.0);
        node.top = Val::Px(-scroll.y.round() * 4.0);
    }
    for (mut node, mut image) in &mut cars {
        node.left = Val::Px((p.x - map.car.width / 2.0).round() * 4.0);
        node.top = Val::Px((p.y - map.car.height / 2.0).round() * 4.0);
        image.flip_x = drive.facing > 0;
    }
}
fn update_stop_appearance(
    campaign: Res<Campaign>,
    art: Res<ScreenArt>,
    mut stops: Query<(&MapStop, &mut ImageNode)>,
) {
    let unlocked = campaign.unlocked();
    for (stop, mut image) in &mut stops {
        let texture = if stop.level <= unlocked {
            &stop.normal
        } else {
            &art.stop_locked
        };
        if image.image != *texture {
            image.image = texture.clone();
        }
    }
}
fn animate_stop_highlight(
    time: Res<Time>,
    map: Res<MapRuntime>,
    drive: Res<MapDrive>,
    campaign: Res<Campaign>,
    mut pulse: ResMut<StopPulse>,
    mut flashes: Query<(&StopFlash, &mut Visibility)>,
) {
    // Exactly the same selection/range and stationary condition as map_controls.
    let active = if drive.target.is_none() {
        map.nearby_level(drive.cell)
            .filter(|level| level.index <= campaign.unlocked())
            .map(|level| level.index)
    } else {
        None
    };
    if active != pulse.active {
        pulse.active = active;
        pulse.elapsed = 0.0;
    } else {
        pulse.elapsed = (pulse.elapsed + time.delta_secs()) % 0.9;
    }
    for (flash, mut visibility) in &mut flashes {
        *visibility = if active == Some(flash.0) && pulse.elapsed < 0.45 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
fn cleanup_map(mut commands: Commands, entities: Query<Entity, With<MapScreen>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<MapLoad>();
    commands.remove_resource::<MapRuntime>();
    commands.remove_resource::<MapImages>();
}

#[cfg(test)]
mod level_tests {
    use super::*;
    use std::collections::HashSet;
    #[test]
    fn every_kitchen_has_reachable_stations_and_pickable_starting_dishes() {
        for source in [
            include_str!("../../assets/levels/level01.ron"),
            include_str!("../../assets/levels/level02.ron"),
            include_str!("../../assets/levels/level03.ron"),
        ] {
            let level: LevelData = ron::from_str(source).unwrap();
            assert_eq!(
                level.tiles.len(),
                (level.map_width * level.map_height) as usize
            );
            for food in ["tomato", "cabbage", "fish", "eggplant", "lotus_root"] {
                assert!(
                    level.tiles.iter().any(|id| matches!(
                        level.tile_props.get(id).and_then(|prop| prop.station.as_ref()),
                        Some(StationDef::Crate(name)) if name == food
                    )),
                    "Missing food crate: {food}"
                );
            }
            let w = level.map_width as i32;
            let h = level.map_height as i32;
            let start = (level.players[0].x as i32, level.players[0].y as i32);
            let mut reached = HashSet::from([start]);
            let mut pending = vec![start];
            while let Some((x, y)) = pending.pop() {
                for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                    if nx >= 0
                        && ny >= 0
                        && nx < w
                        && ny < h
                        && !level
                            .tile_props
                            .get(&level.tiles[(ny * w + nx) as usize])
                            .is_some_and(|prop| prop.collideable)
                        && reached.insert((nx, ny))
                    {
                        pending.push((nx, ny));
                    }
                }
            }
            for (i, id) in level.tiles.iter().enumerate() {
                if level
                    .tile_props
                    .get(id)
                    .and_then(|prop| prop.station.as_ref())
                    .is_some_and(|station| !matches!(station, StationDef::Counter))
                {
                    let x = i as i32 % w;
                    let y = i as i32 / w;
                    assert!(
                        [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                            .iter()
                            .any(|p| reached.contains(p)),
                        "station at {x},{y} is inaccessible"
                    );
                }
            }
            for dish in &level.dishes {
                let id = level.tiles[(dish.y * level.map_width + dish.x) as usize];
                assert!(matches!(
                    level.tile_props[&id].station,
                    Some(StationDef::Counter)
                ));
            }
        }
    }
}

#[cfg(test)]
mod movement_tests {
    use super::*;
    fn example() -> MapRuntime {
        serde_json::from_str::<WorldMapData>(include_str!("../../assets/maps/overworld.tmj"))
            .unwrap()
            .runtime()
            .unwrap()
    }
    #[test]
    fn stop_outline_blinks_only_for_the_nearby_enterable_kitchen() {
        use bevy::time::TimeUpdateStrategy;
        let map = example();
        let approach = |index: usize| {
            *map.reachable(map.spawn)
                .iter()
                .find(|cell| map.nearby_level(**cell).is_some_and(|l| l.index == index))
                .unwrap()
        };
        let first = approach(0);
        let locked = approach(2);
        let far = *map
            .roads
            .keys()
            .find(|cell| map.nearby_level(**cell).is_none())
            .unwrap();
        let mut drive = MapDrive::default();
        drive.reset(&map);
        drive.cell = first;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(100),
            ))
            .insert_resource(map)
            .insert_resource(drive)
            .init_resource::<Campaign>()
            .init_resource::<StopPulse>()
            .add_systems(Update, animate_stop_highlight);
        let first_flash = app
            .world_mut()
            .spawn((StopFlash(0), Visibility::Hidden))
            .id();
        let locked_flash = app
            .world_mut()
            .spawn((StopFlash(2), Visibility::Hidden))
            .id();
        let visible = |app: &App, entity| {
            *app.world().get::<Visibility>(entity).unwrap() == Visibility::Visible
        };
        app.update();
        assert!(visible(&app, first_flash));
        assert!(!visible(&app, locked_flash));
        let mut on = false;
        let mut off = false;
        for _ in 0..12 {
            app.update();
            on |= visible(&app, first_flash);
            off |= !visible(&app, first_flash);
        }
        assert!(
            on && off,
            "An eligible stop must alternate between lit and unlit"
        );
        app.world_mut().resource_mut::<MapDrive>().cell = locked;
        app.update();
        assert!(!visible(&app, first_flash));
        assert!(!visible(&app, locked_flash));
        app.world_mut().resource_mut::<Campaign>().best = [3, 3, 0];
        app.update();
        assert!(visible(&app, locked_flash));
        app.world_mut().resource_mut::<MapDrive>().target = Some(first);
        app.update();
        assert!(
            !visible(&app, locked_flash),
            "A moving car cannot enter a stop"
        );
        let mut drive = app.world_mut().resource_mut::<MapDrive>();
        drive.target = None;
        drive.cell = far;
        drop(drive);
        app.update();
        assert!(!visible(&app, first_flash) && !visible(&app, locked_flash));
    }
    #[test]
    fn driving_stays_on_connected_roads_and_does_not_teleport() {
        let map = example();
        let mut car = MapDrive::default();
        car.reset(&map);
        assert_eq!(car.facing, 1, "A new trip starts facing right");
        let directions = [IVec2::NEG_X, IVec2::X, IVec2::NEG_Y, IVec2::Y];
        let blocked = *directions
            .iter()
            .find(|d| !map.can_step(map.spawn, **d))
            .unwrap();
        let direction = *directions
            .iter()
            .find(|d| map.can_step(map.spawn, **d))
            .unwrap();
        let destination = map.spawn + direction;
        car.try_move(&map, blocked);
        assert!(car.target.is_none());
        car.try_move(&map, direction);
        assert_eq!(car.target, Some(destination));
        car.advance(&map, 0.125);
        assert_eq!(car.cell, map.spawn);
        assert_eq!(
            car.position,
            map.data.cell_center(map.spawn) + direction.as_vec2() * 8.0
        );
        car.try_move(&map, blocked);
        assert_eq!(car.target, Some(destination));
        car.advance(&map, 10.0);
        assert_eq!(car.cell, destination);
        assert!(car.target.is_none());
        let blocked = *directions
            .iter()
            .find(|d| !map.can_step(destination, **d))
            .unwrap();
        car.try_move(&map, blocked);
        assert!(car.target.is_none());
    }
    #[test]
    fn tiled_scene_builds_and_locked_kitchen_cannot_be_entered() {
        use bevy::{asset::AssetPlugin, state::app::StatesPlugin};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<AudioSource>()
            .init_state::<AppState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<PendingScore>()
            .add_plugins(WorldMapPlugin);
        let data = example().data;
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<WorldMapData>(MAP_PATH);
        app.world_mut()
            .resource_mut::<Assets<WorldMapData>>()
            .insert(handle.id(), data)
            .unwrap();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::WorldMap);
        app.update();
        assert_eq!(app.world().resource::<MapRuntime>().levels.len(), 3);
        let locked_image = app.world().resource::<ScreenArt>().stop_locked.clone();
        let stops: Vec<_> = app
            .world_mut()
            .query::<(&MapStop, &ImageNode)>()
            .iter(app.world())
            .map(|(stop, image)| (stop.level, image.image.clone(), stop.normal.clone()))
            .collect();
        assert_eq!(stops.len(), 3);
        for (index, image, normal) in stops {
            assert_eq!(
                image,
                if index == 0 {
                    normal
                } else {
                    locked_image.clone()
                }
            );
        }

        assert!(
            app.world_mut()
                .query::<&ImageNode>()
                .iter(app.world())
                .count()
                > 140
        );
        let map = app.world().resource::<MapRuntime>();
        let level = map.levels.iter().find(|l| l.index == 2).unwrap();
        let cell = *map
            .reachable(map.spawn)
            .iter()
            .filter(|cell| (**cell - level.cell).abs().element_sum() <= 1)
            .min_by_key(|cell| (**cell - level.cell).abs().element_sum())
            .unwrap();
        let position = map.data.cell_center(cell);
        {
            let mut car = app.world_mut().resource_mut::<MapDrive>();
            car.cell = cell;
            car.position = position;
            car.target = None;
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::WorldMap
        );
        assert!(matches!(
            app.world().resource::<NextState<AppState>>(),
            NextState::Unchanged
        ));
        assert_eq!(
            app.world_mut()
                .query::<&AudioPlayer<AudioSource>>()
                .iter(app.world())
                .count(),
            0,
            "Locked kitchens must not play the enter sound"
        );
        app.world_mut().resource_mut::<Campaign>().best = [3, 3, 0];
        *app.world_mut().resource_mut::<ButtonInput<KeyCode>>() = ButtonInput::default();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert!(matches!(
            app.world().resource::<NextState<AppState>>(),
            NextState::Pending(AppState::Loading)
        ));
        assert_eq!(app.world().resource::<Campaign>().selected, 2);
        let (confirmation, source) = app
            .world_mut()
            .query::<(Entity, &AudioPlayer<AudioSource>)>()
            .single(app.world())
            .map(|(entity, player)| (entity, player.0.clone()))
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(source.id())
                .unwrap()
                .path(),
            std::path::Path::new("sounds/menu/confirm.wav")
        );
        assert!(matches!(
            app.world()
                .get::<PlaybackSettings>(confirmation)
                .unwrap()
                .mode,
            bevy::audio::PlaybackMode::Despawn
        ));

        for (stop, image) in app
            .world_mut()
            .query::<(&MapStop, &ImageNode)>()
            .iter(app.world())
        {
            assert_eq!(
                image.image, stop.normal,
                "Unlocked stops must restore the normal storefront"
            );
        }
        app.update();
        assert!(
            app.world()
                .get::<AudioPlayer<AudioSource>>(confirmation)
                .is_some(),
            "The enter sound must survive map cleanup to finish playing"
        );
    }
}
