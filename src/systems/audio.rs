use crate::components::*;
use crate::constants::*;
use crate::events::*;
use crate::resources::*;
use bevy::prelude::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContinuousSound {
    Washing,
    Chopping,
    Simmering,
    Boiling,
}

impl ContinuousSound {
    fn path(self) -> &'static str {
        match self {
            Self::Washing => "sounds/sfx/dishwash.wav",
            Self::Chopping => "sounds/sfx/chopxthree.wav",
            Self::Simmering => "sounds/sfx/bubbles-a.wav",
            Self::Boiling => "sounds/sfx/bubbles-b.wav",
        }
    }
}

#[derive(Component)]
pub(crate) struct GameplaySoundLoop {
    owner: Entity,
    sound: ContinuousSound,
}

/// Maintain one loop per ongoing action or stove. The existing audio pause and
/// gameplay cleanup systems own pausing and removing these playback entities.
pub fn audio_continuous_sfx_system(
    mut commands: Commands,
    server: Res<AssetServer>,
    level: Res<LevelState>,
    players: Query<(Entity, &ActionState), With<Player>>,
    stations: Query<(Entity, &Station, &StationItem)>,
    pots: Query<&Pot>,
    soups: Query<&Soup>,
    loops: Query<(Entity, &GameplaySoundLoop)>,
    mut warned: Local<HashSet<Entity>>,
) {
    if level.paused {
        return;
    }
    let mut desired = Vec::new();
    let mut warnings = HashSet::new();
    if level.startup_timer <= 0.0 && !level.done {
        for (entity, action) in &players {
            if action.action {
                let sound = match action.activity {
                    Some(PlayerActivity::Washing) => Some(ContinuousSound::Washing),
                    Some(PlayerActivity::Prepping) => Some(ContinuousSound::Chopping),
                    None => None,
                };
                if let Some(sound) = sound {
                    desired.push((entity, sound));
                }
            }
        }
        for (entity, station, item) in &stations {
            if station.kind != StationKind::Stove {
                continue;
            }
            let Some(soup) = item
                .entity
                .and_then(|item| pots.get(item).ok())
                .and_then(|pot| pot.soup)
                .and_then(|soup| soups.get(soup).ok())
            else {
                continue;
            };
            if soup.is_burnt() {
                continue;
            }
            desired.push((
                entity,
                if soup.is_cooked() {
                    ContinuousSound::Boiling
                } else {
                    ContinuousSound::Simmering
                },
            ));
            if soup.will_burn_soon() {
                warnings.insert(entity);
                if !warned.contains(&entity) {
                    commands.spawn((
                        AudioPlayer::new(server.load("sounds/sfx/burnnotice.wav")),
                        PlaybackSettings::DESPAWN,
                        GameplayEntity,
                    ));
                }
            }
        }
    }
    *warned = warnings;
    for (entity, playing) in &loops {
        if !desired.contains(&(playing.owner, playing.sound)) {
            commands.entity(entity).despawn();
        }
    }
    for (owner, sound) in desired {
        if !loops
            .iter()
            .any(|(_, playing)| playing.owner == owner && playing.sound == sound)
        {
            commands.spawn((
                AudioPlayer::new(server.load(sound.path())),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::Linear(0.5),
                    ..default()
                },
                GameplaySoundLoop { owner, sound },
                GameplayEntity,
            ));
        }
    }
}

/// Shared across menu screens; screen cleanup never owns this entity.
#[derive(Component)]
pub struct MenuMusic;

pub struct MenuMusicPlugin;
impl Plugin for MenuMusicPlugin {
    fn build(&self, app: &mut App) {
        use crate::states::AppState;
        for state in [AppState::WorldMap, AppState::Score, AppState::NameEntry] {
            app.add_systems(OnEnter(state), start_menu_music);
        }
        for state in [AppState::Loading, AppState::Gameplay] {
            app.add_systems(OnEnter(state), stop_menu_music);
        }
    }
}

pub fn ensure_menu_music(
    commands: &mut Commands,
    server: &AssetServer,
    music: &Query<(), With<MenuMusic>>,
) {
    if music.is_empty() {
        commands.spawn((
            AudioPlayer::new(server.load(MENU_BGM)),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: bevy::audio::Volume::Linear(0.2),
                ..default()
            },
            MenuMusic,
        ));
    }
}

fn start_menu_music(
    mut commands: Commands,
    server: Res<AssetServer>,
    music: Query<(), With<MenuMusic>>,
) {
    ensure_menu_music(&mut commands, &server, &music);
}

fn stop_menu_music(mut commands: Commands, music: Query<Entity, With<MenuMusic>>) {
    for entity in &music {
        commands.entity(entity).despawn();
    }
}

/// Loop the kitchen music until the round ends or gameplay is exited.
pub fn audio_bgm_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut audio_state: ResMut<GameAudio>,
    level_state: Res<LevelState>,
) {
    if level_state.done {
        return;
    }

    // Start BGM if not yet started
    if audio_state.current_bgm.is_none() {
        let source = asset_server.load(GAME_BGM);
        let entity = commands
            .spawn((
                AudioPlayer::new(source),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::Linear(0.2),
                    ..default()
                },
                GameplayEntity,
            ))
            .id();
        audio_state.current_bgm = Some(entity);
    }
}

/// Play one-shot SFX based on events.
pub fn audio_sfx_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut ev_delivered: MessageReader<DishDelivered>,
    mut ev_clean: MessageReader<CleanDishReturned>,
    mut ev_illegal: MessageReader<IllegalAction>,
    mut ev_dispensed: MessageReader<ItemDispensed>,
) {
    for _ in ev_delivered.read() {
        commands.spawn((
            AudioPlayer::new(asset_server.load("sounds/sfx/deliverychime.wav")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                ..default()
            },
            GameplayEntity,
        ));
    }

    for _ in ev_clean.read() {
        commands.spawn((
            AudioPlayer::new(asset_server.load("sounds/sfx/cleanplate.wav")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                ..default()
            },
            GameplayEntity,
        ));
    }

    for _ in ev_illegal.read() {
        commands.spawn((
            AudioPlayer::new(asset_server.load("sounds/sfx/badidea.wav")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                ..default()
            },
            GameplayEntity,
        ));
    }

    for _ in ev_dispensed.read() {
        // Crate dispense doesn't have a specific SFX in the original,
        // but the animation plays. We can add one if needed.
    }
}

/// Stop all audio when the level ends.
pub fn audio_cleanup_system(
    mut commands: Commands,
    audio_entities: Query<Entity, With<AudioPlayer<AudioSource>>>,
    level_state: Res<LevelState>,
) {
    if level_state.done {
        for entity in &audio_entities {
            commands.entity(entity).despawn();
        }
    }
}

/// Audio sinks need an explicit pause; skipping the playback systems alone
/// leaves background music and ongoing effects running.
pub fn audio_pause_system(level: Res<LevelState>, sinks: Query<&AudioSink, With<GameplayEntity>>) {
    for sink in &sinks {
        if level.paused {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

/// Short menu effects outlive screen changes and despawn after playback.
pub enum MenuSound {
    Enter,
    Hover,
    Confirm,
    Back,
}

pub fn play_menu_sound(commands: &mut Commands, server: &AssetServer, sound: MenuSound) {
    let path = match sound {
        MenuSound::Enter => "sounds/menu/confirm.wav",
        MenuSound::Hover => "sounds/menu/hover.wav",
        MenuSound::Confirm => "sounds/menu/confirm.wav",
        MenuSound::Back => "sounds/menu/back.wav",
    };
    commands.spawn((
        AudioPlayer::new(server.load(path)),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Despawn,
            ..default()
        },
    ));
}

#[cfg(test)]
mod music_tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    #[test]
    fn relax_keeps_the_same_player_across_menu_screens_and_stops_for_gameplay() {
        use crate::states::AppState;
        use bevy::{ecs::system::RunSystemOnce, state::app::StatesPlugin};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<AudioSource>()
            .init_state::<AppState>()
            .add_plugins(MenuMusicPlugin);
        app.update();
        // The title intro requests music once its initial animation reaches 1.5s.
        app.world_mut().run_system_once(start_menu_music).unwrap();
        let music = |app: &mut App| {
            app.world_mut()
                .query_filtered::<Entity, With<MenuMusic>>()
                .single(app.world())
                .unwrap()
        };
        let original = music(&mut app);
        let player = app
            .world()
            .get::<AudioPlayer<AudioSource>>(original)
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(player.0.id())
                .unwrap()
                .path(),
            std::path::Path::new(MENU_BGM)
        );
        for state in [
            AppState::WorldMap,
            AppState::Score,
            AppState::Title,
            AppState::Score,
            AppState::NameEntry,
            AppState::Score,
            AppState::WorldMap,
        ] {
            app.world_mut()
                .resource_mut::<NextState<AppState>>()
                .set(state);
            app.update();
            app.world_mut().run_system_once(start_menu_music).unwrap();
            assert_eq!(
                music(&mut app),
                original,
                "Menu navigation must retain the same playback entity"
            );
        }
        for state in [AppState::Loading, AppState::Gameplay] {
            app.world_mut()
                .resource_mut::<NextState<AppState>>()
                .set(state);
            app.update();
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<MenuMusic>>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Score);
        app.update();
        let restarted = music(&mut app);
        assert_ne!(restarted, original);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::WorldMap);
        app.update();
        assert_eq!(music(&mut app), restarted);
    }

    #[test]
    fn kitchen_music_loops_without_restarting_near_the_end_and_stops_with_round() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_resource::<GameAudio>()
            .insert_resource(LevelState::new(120.0, 1))
            .add_systems(Update, (audio_bgm_system, audio_cleanup_system).chain());
        app.update();
        let entity = app.world().resource::<GameAudio>().current_bgm.unwrap();
        let player = app.world().get::<AudioPlayer<AudioSource>>(entity).unwrap();
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(player.0.id())
                .unwrap()
                .path(),
            std::path::Path::new(GAME_BGM)
        );
        assert!(matches!(
            app.world().get::<PlaybackSettings>(entity).unwrap().mode,
            bevy::audio::PlaybackMode::Loop
        ));
        app.world_mut().resource_mut::<LevelState>().time_remaining = 10.0;
        app.update();
        assert_eq!(
            app.world().resource::<GameAudio>().current_bgm,
            Some(entity)
        );
        assert_eq!(
            app.world_mut()
                .query::<&AudioPlayer<AudioSource>>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut().resource_mut::<LevelState>().done = true;
        app.update();
        assert!(app.world().get_entity(entity).is_err());
    }
}

#[cfg(test)]
mod sfx_tests {
    use super::*;
    use bevy::{asset::AssetPlugin, time::TimeUpdateStrategy};
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        let mut level = LevelState::new(120.0, 2);
        level.startup_timer = 0.0;
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .insert_resource(level)
            .insert_resource(crate::recipes::Orders::new(vec![FoodType::Tomato]))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                250,
            )))
            .add_message::<DishDelivered>()
            .add_message::<CleanDishReturned>()
            .add_message::<IllegalAction>()
            .add_message::<ItemDispensed>();
        app.update();
        app
    }

    fn sounds(app: &mut App, path: &str) -> Vec<Entity> {
        let mut query = app
            .world_mut()
            .query::<(Entity, &AudioPlayer<AudioSource>)>();
        let server = app.world().resource::<AssetServer>();
        query
            .iter(app.world())
            .filter_map(|(entity, player)| {
                (server.get_path(player.0.id()).unwrap().path() == std::path::Path::new(path))
                    .then_some(entity)
            })
            .collect()
    }

    #[test]
    fn washing_loops_while_held_then_stops_and_plays_clean_plate_once() {
        let mut app = app();
        app.add_systems(
            Update,
            (
                crate::systems::actions::actions_system,
                audio_continuous_sfx_system,
                audio_sfx_system,
            )
                .chain()
                .run_if(|level: Res<LevelState>| !level.paused),
        );
        let rack = app
            .world_mut()
            .spawn((
                Station {
                    kind: StationKind::DishRack,
                },
                StationItem::default(),
                DishStack::default(),
            ))
            .id();
        let dish = app
            .world_mut()
            .spawn((
                Dish {
                    soup: None,
                    clean_ratio: 0.0,
                },
                ItemKind::Dish,
            ))
            .id();
        let sink = app
            .world_mut()
            .spawn((
                Station {
                    kind: StationKind::Sink,
                },
                StationItem { entity: Some(dish) },
                LinkedReturnStation { entity: rack },
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                Player { index: 0 },
                ActionState {
                    action: true,
                    transfer: false,
                    cooldown: 0.0,
                    cooldown_max: ACTION_COOLDOWN,
                    activity: None,
                    did_change_state: false,
                    did_illegal_action: false,
                },
                HeldItem::default(),
                FacingStation { entity: Some(sink) },
            ))
            .id();
        app.update();
        let washing = sounds(&mut app, "sounds/sfx/dishwash.wav");
        assert_eq!(washing.len(), 1);
        assert!(matches!(
            app.world()
                .get::<PlaybackSettings>(washing[0])
                .unwrap()
                .mode,
            bevy::audio::PlaybackMode::Loop
        ));
        app.update();
        assert_eq!(sounds(&mut app, "sounds/sfx/dishwash.wav"), washing);
        app.world_mut()
            .get_mut::<ActionState>(player)
            .unwrap()
            .action = false;
        app.update();
        assert!(sounds(&mut app, "sounds/sfx/dishwash.wav").is_empty());
        assert!(sounds(&mut app, "sounds/sfx/cleanplate.wav").is_empty());
        app.world_mut()
            .get_mut::<ActionState>(player)
            .unwrap()
            .action = true;
        app.update();
        let resumed = sounds(&mut app, "sounds/sfx/dishwash.wav");
        assert_eq!(resumed.len(), 1);
        app.world_mut().resource_mut::<LevelState>().paused = true;
        let ratio = app.world().get::<Dish>(dish).unwrap().clean_ratio;
        app.update();
        assert_eq!(sounds(&mut app, "sounds/sfx/dishwash.wav"), resumed);
        assert_eq!(app.world().get::<Dish>(dish).unwrap().clean_ratio, ratio);
        app.world_mut().resource_mut::<LevelState>().paused = false;
        for _ in 0..10 {
            app.update();
        }
        assert!(sounds(&mut app, "sounds/sfx/dishwash.wav").is_empty());
        let clean = sounds(&mut app, "sounds/sfx/cleanplate.wav");
        assert_eq!(clean.len(), 1);
        assert!(matches!(
            app.world().get::<PlaybackSettings>(clean[0]).unwrap().mode,
            bevy::audio::PlaybackMode::Despawn
        ));
        assert_eq!(
            app.world().get::<DishStack>(rack).unwrap().dishes,
            vec![dish]
        );
        assert!(
            app.world()
                .get::<StationItem>(sink)
                .unwrap()
                .entity
                .is_none()
        );
        app.update();
        assert_eq!(sounds(&mut app, "sounds/sfx/cleanplate.wav"), clean);
    }

    #[test]
    fn chopping_and_stove_sounds_follow_activity_and_warning_does_not_repeat_every_frame() {
        let mut app = app();
        app.add_systems(Update, audio_continuous_sfx_system);
        let player = app
            .world_mut()
            .spawn((
                Player { index: 0 },
                ActionState {
                    action: true,
                    transfer: false,
                    cooldown: 0.0,
                    cooldown_max: ACTION_COOLDOWN,
                    activity: Some(PlayerActivity::Prepping),
                    did_change_state: false,
                    did_illegal_action: false,
                },
            ))
            .id();
        let soup = app.world_mut().spawn(Soup::new(FoodType::Tomato)).id();
        let pot = app.world_mut().spawn(Pot { soup: Some(soup) }).id();
        app.world_mut().spawn((
            Station {
                kind: StationKind::Stove,
            },
            StationItem { entity: Some(pot) },
        ));
        app.update();
        let chopping = sounds(&mut app, "sounds/sfx/chopxthree.wav");
        let simmering = sounds(&mut app, "sounds/sfx/bubbles-a.wav");
        assert_eq!(chopping.len(), 1);
        assert_eq!(simmering.len(), 1);
        app.update();
        assert_eq!(sounds(&mut app, "sounds/sfx/chopxthree.wav"), chopping);
        assert_eq!(sounds(&mut app, "sounds/sfx/bubbles-a.wav"), simmering);
        app.world_mut()
            .get_mut::<ActionState>(player)
            .unwrap()
            .activity = None;
        app.world_mut().get_mut::<Soup>(soup).unwrap().state = CookState::Cooked;
        app.update();
        assert!(sounds(&mut app, "sounds/sfx/chopxthree.wav").is_empty());
        assert!(sounds(&mut app, "sounds/sfx/bubbles-a.wav").is_empty());
        let boiling = sounds(&mut app, "sounds/sfx/bubbles-b.wav");
        assert_eq!(boiling.len(), 1);
        app.world_mut().get_mut::<Soup>(soup).unwrap().state = CookState::WillBurn;
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(sounds(&mut app, "sounds/sfx/bubbles-b.wav"), boiling);
        assert_eq!(sounds(&mut app, "sounds/sfx/burnnotice.wav").len(), 1);
        app.world_mut().get_mut::<Soup>(soup).unwrap().state = CookState::Burnt;
        app.update();
        assert!(sounds(&mut app, "sounds/sfx/bubbles-b.wav").is_empty());
        app.world_mut().resource_mut::<LevelState>().done = true;
        app.world_mut()
            .get_mut::<ActionState>(player)
            .unwrap()
            .activity = Some(PlayerActivity::Prepping);
        app.update();
        assert!(sounds(&mut app, "sounds/sfx/chopxthree.wav").is_empty());
    }
}
