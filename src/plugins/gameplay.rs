use crate::components::*;
use crate::constants::*;
use crate::events::*;
use crate::resources::*;
use crate::states::*;
use crate::systems;
use bevy::prelude::*;

/// Gameplay plugin: orchestrates level loading, all gameplay systems, and state transitions.
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Campaign>()
            .init_resource::<RoundResult>()
            .add_message::<DishDelivered>()
            .add_message::<CleanDishReturned>()
            .add_message::<IllegalAction>()
            .add_message::<ItemDispensed>()
            // Loading state: async asset loading
            .add_systems(OnEnter(AppState::Loading), start_loading)
            .add_systems(
                Update,
                check_level_loaded.run_if(in_state(AppState::Loading)),
            )
            .add_systems(OnEnter(AppState::Gameplay), setup_gameplay)
            .add_systems(
                Update,
                (
                    gameplay_phase_system,
                    systems::player_input::pause_input_system,
                    systems::render_hud::update_pause_label,
                    systems::audio::audio_pause_system,
                    // Core gameplay: only runs after startup countdown and when not paused/done
                    (
                        systems::player_input::player_input_system,
                        systems::movement::movement_system,
                        systems::actions::actions_system,
                        systems::cook_timer::cook_timer_system,
                        systems::cook_timer::counter_soup_display_system,
                        systems::deliver_timer::deliver_timer_system,
                    )
                        .chain()
                        .run_if(is_actively_playing),
                    // Timer + HUD: only ticks during active play
                    (
                        systems::render_hud::render_hud_system,
                        systems::render_hud::animate_order_cards,
                        systems::render_hud::update_hud_text,
                    )
                        .chain()
                        .run_if(is_actively_playing),
                    // Rendering: always runs (map animates during startup too)
                    (
                        systems::render_map::update_player_sprites,
                        systems::render_map::animate_tiles,
                        systems::render_map::animate_crate_lids,
                        systems::render_map::render_items_system,
                        systems::audio::audio_bgm_system,
                        systems::audio::audio_continuous_sfx_system,
                        systems::audio::audio_sfx_system,
                    )
                        .chain()
                        .run_if(not(is_paused)),
                    systems::player_input::player_input_cleanup_system,
                    systems::actions::actions_cleanup_system,
                    // Stop all audio once the level is done ("TIME'S UP!" phase)
                    systems::audio::audio_cleanup_system,
                )
                    .chain()
                    .run_if(in_state(AppState::Gameplay)),
            )
            .add_systems(OnExit(AppState::Gameplay), cleanup_gameplay);
    }
}

fn is_paused(level_state: Option<Res<LevelState>>) -> bool {
    level_state.map_or(false, |ls| ls.paused)
}

/// Returns true only when actually playing: startup done, not paused, not ended.
fn is_actively_playing(level_state: Option<Res<LevelState>>) -> bool {
    level_state.map_or(false, |ls| {
        ls.startup_timer <= 0.0 && !ls.paused && !ls.done
    })
}

/// Start async loading of level data.
fn start_loading(mut commands: Commands, asset_server: Res<AssetServer>, campaign: Res<Campaign>) {
    let handle =
        asset_server.load::<LevelData>(crate::plugins::world_map::LEVEL_PATHS[campaign.selected]);
    commands.insert_resource(LevelLoadHandle(handle));
}

/// Wait for level data to finish loading, then transition to Gameplay.
fn check_level_loaded(
    level_handle: Option<Res<LevelLoadHandle>>,
    level_data_assets: Res<Assets<LevelData>>,
    images: Res<Assets<Image>>,
    server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Some(handle_res) = level_handle else {
        return;
    };
    let Some(level) = level_data_assets.get(&handle_res.0) else {
        return;
    };
    let Some(image) = images.get(&level.spritesheet_image) else {
        if matches!(
            server.get_load_state(level.spritesheet_image.id()),
            Some(bevy::asset::LoadState::Failed(_))
        ) {
            error!("Cannot load kitchen atlas '{}'", level.spritesheet);
            next_state.set(AppState::WorldMap);
        }
        return;
    };
    match crate::tileset::layout_for_size(
        image.size(),
        UVec2::new(level.tile_width as u32, level.tile_height as u32),
    ) {
        Ok(_) => next_state.set(AppState::Gameplay),
        Err(error) => {
            error!("Cannot load kitchen atlas '{}': {error}", level.spritesheet);
            next_state.set(AppState::WorldMap);
        }
    }
}

fn setup_gameplay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    selected_player_count: Res<SelectedPlayerCount>,
    level_handle: Res<LevelLoadHandle>,
    level_data_assets: Res<Assets<LevelData>>,
    images: Res<Assets<Image>>,
) {
    let player_count = selected_player_count.0.max(1);

    let level_data = level_data_assets
        .get(&level_handle.0)
        .expect("Level data should be loaded before entering Gameplay");

    // Spawn level entities from the pre-loaded data
    let hud_sheet = level_data.spritesheet_image.clone();
    let level_state = crate::level_loader::spawn_level_entities(
        &mut commands,
        &mut atlas_layouts,
        level_data,
        images
            .get(&level_data.spritesheet_image)
            .expect("Kitchen image loaded before Gameplay"),
        player_count,
    );

    commands.insert_resource(level_state);
    commands.insert_resource(GameAudio::default());
    commands.remove_resource::<LevelLoadHandle>();

    // Set up 2D camera with pixel-perfect scaling
    // Sprites are center-anchored in Bevy, so offset by half a tile to align
    // the grid with the viewport edges (Love2D anchors at top-left).
    let half_tile = TILE_SIZE / 2.0;
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(crate::theme::INK),
            ..default()
        },
        Transform::from_xyz(
            TILE_SIZE * 7.0 - half_tile,      // Center on the 14-tile-wide map
            -(BASE_HEIGHT / 2.0) + half_tile, // Center on the 10-tile-high map (Y negated)
            999.0,
        ),
        Projection::from(OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            scaling_mode: bevy::camera::ScalingMode::Fixed {
                width: BASE_WIDTH,
                height: BASE_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
        GameplayEntity,
    ));

    // Spawn HUD
    systems::render_hud::spawn_hud(commands, asset_server, hud_sheet);
}

#[derive(Component)]
struct PhaseOverlayRoot;

fn spawn_phase_overlay(commands: &mut Commands, font: Handle<Font>, text: &str) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            PhaseOverlayRoot,
            GameplayEntity,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new(text),
                TextFont {
                    font,
                    font_size: 16.0 * WINDOW_SCALE,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                TextColor(crate::theme::INK),
                PhaseOverlayText,
            ));
        });
}

/// Manages the gameplay phases: Ready → Go → Playing → Time's Up → Score screen.
fn gameplay_phase_system(
    mut level_state: ResMut<LevelState>,
    time: Res<Time>,
    mut next_state: ResMut<NextState<AppState>>,
    mut pending_score: ResMut<PendingScore>,
    mut campaign: ResMut<Campaign>,
    mut result: ResMut<RoundResult>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut phase_text: Query<&mut Text, With<PhaseOverlayText>>,
    phase_entities: Query<Entity, With<PhaseOverlayRoot>>,
    mut hud_roots: Query<&mut Visibility, With<crate::systems::render_hud::HudRoot>>,
) {
    let dt = time.delta_secs();
    if level_state.paused {
        return;
    }

    if level_state.startup_timer > 0.0 {
        // Starting phase
        level_state.startup_timer -= dt;

        if phase_entities.is_empty() && level_state.startup_timer > 0.0 {
            let font = asset_server.load("fonts/gameboy.ttf");
            let text = if level_state.startup_timer > 1.0 {
                "READY?"
            } else {
                "GO!"
            };
            spawn_phase_overlay(&mut commands, font, text);

            // Play ready-go sound
            commands.spawn((
                AudioPlayer::new(asset_server.load("sounds/sfx/readygo.wav")),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Despawn,
                    ..default()
                },
                GameplayEntity,
            ));
        }

        for mut text in &mut phase_text {
            if level_state.startup_timer > 1.0 {
                **text = "READY?".to_string();
            } else {
                **text = "GO!".to_string();
            }
        }

        if level_state.startup_timer <= 0.0 {
            // Remove overlay text
            for entity in &phase_entities {
                commands.entity(entity).despawn();
            }
            // Reveal HUD now that "Go!" is done
            for mut vis in &mut hud_roots {
                *vis = Visibility::Inherited;
            }
        }
    } else if level_state.done {
        // Ending phase
        level_state.shutdown_timer -= dt;

        if phase_entities.is_empty() {
            let font = asset_server.load("fonts/gameboy.ttf");
            spawn_phase_overlay(&mut commands, font, "TIME'S UP!");
        }

        if level_state.shutdown_timer <= 0.0 {
            pending_score.0 = Some(level_state.score);
            campaign.record(level_state.score);
            *result = RoundResult {
                level: campaign.selected,
                delivered: level_state.score,
                burnt: level_state.burnt_batches,
            };
            next_state.set(AppState::Score);
        }
    }
}

fn cleanup_gameplay(mut commands: Commands, entities: Query<Entity, With<GameplayEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<LevelState>();
    commands.remove_resource::<LevelData>();
    commands.remove_resource::<GameAudio>();
    commands.remove_resource::<crate::recipes::Orders>();
    commands.remove_resource::<TilesetInfo>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn loading_waits_for_the_atlas_image_before_entering_gameplay() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<LevelData>()
            .init_asset::<Image>()
            .init_state::<AppState>()
            .add_systems(Update, check_level_loaded);
        let image_handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .reserve_handle();
        let mut level: LevelData =
            ron::from_str(include_str!("../../assets/levels/level01.ron")).unwrap();
        level.spritesheet_image = image_handle.clone();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<LevelData>>()
            .add(level);
        app.insert_resource(LevelLoadHandle(handle));
        app.update();
        assert!(matches!(
            app.world().resource::<NextState<AppState>>(),
            NextState::Unchanged
        ));
        let image = Image::from_buffer(
            include_bytes!("../../assets/images/kitchen/tileset.png"),
            bevy::image::ImageType::Extension("png"),
            bevy::image::CompressedImageFormats::NONE,
            true,
            bevy::image::ImageSampler::nearest(),
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        )
        .unwrap();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(image_handle.id(), image)
            .unwrap();
        app.update();
        assert!(matches!(
            app.world().resource::<NextState<AppState>>(),
            NextState::Pending(AppState::Gameplay)
        ));
    }
    #[test]
    fn gameplay_schedule_preserves_input_until_transfer_and_freezes_on_pause() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<LevelData>()
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<AudioSource>()
            .init_asset::<TextureAtlasLayout>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<PendingScore>()
            .insert_resource(SelectedPlayerCount(1))
            .init_state::<AppState>()
            .add_plugins(GameplayPlugin);
        let mut level: LevelData =
            ron::from_str(include_str!("../../assets/levels/level01.ron")).unwrap();
        let image = Image::from_buffer(
            include_bytes!("../../assets/images/kitchen/tileset.png"),
            bevy::image::ImageType::Extension("png"),
            bevy::image::CompressedImageFormats::NONE,
            true,
            bevy::image::ImageSampler::nearest(),
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        )
        .unwrap();
        let sheet_size = image.size();
        level.spritesheet_image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let handle = app
            .world_mut()
            .resource_mut::<Assets<LevelData>>()
            .add(level);
        app.insert_resource(LevelLoadHandle(handle));
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Gameplay);
        app.update();
        app.world_mut().resource_mut::<LevelState>().startup_timer = 0.0;
        let sheet = app.world().resource::<TilesetInfo>();
        assert_eq!(
            (sheet.columns, sheet.rows),
            (sheet_size.x / 16, sheet_size.y / 16)
        );
        assert_eq!(
            app.world()
                .resource::<Assets<TextureAtlasLayout>>()
                .get(&sheet.layout)
                .unwrap()
                .textures
                .len(),
            (sheet_size.x / 16 * (sheet_size.y / 16)) as usize
        );

        let player = app
            .world_mut()
            .query_filtered::<Entity, With<Player>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world()
                .get::<Sprite>(player)
                .unwrap()
                .texture_atlas
                .as_ref()
                .unwrap()
                .index,
            121
        );
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(32.0, -128.0, Z_PLAYER);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(
            app.world()
                .get::<HeldItem>(player)
                .unwrap()
                .entity
                .is_some()
        );
        // Every real crate dispenses its food and prepared foods can enter a pot.
        // Exercise empty and existing soups through the actual schedule.
        for food in [FoodType::Cabbage, FoodType::Fish, FoodType::Eggplant] {
            let crate_position = app
                .world_mut()
                .query::<(&CrateIngredient, &Transform)>()
                .iter(app.world())
                .find(|(crate_info, _)| crate_info.food_type == food)
                .unwrap()
                .1
                .translation;
            app.world_mut().get_mut::<HeldItem>(player).unwrap().entity = None;
            app.world_mut()
                .get_mut::<ActionState>(player)
                .unwrap()
                .cooldown = 0.0;
            app.world_mut().get_mut::<Facing>(player).unwrap().direction = Direction::Down;
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = crate_position + Vec3::new(0.0, 16.0, Z_PLAYER);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            let mut item = app.world().get::<HeldItem>(player).unwrap().entity.unwrap();
            assert_eq!(app.world().get::<Ingredient>(item).unwrap().food_type, food);
            app.world_mut()
                .get_mut::<Ingredient>(item)
                .unwrap()
                .prep_ratio = 1.0;
            for filled in [false, true] {
                let pot = app
                    .world_mut()
                    .query_filtered::<Entity, With<Pot>>()
                    .iter(app.world())
                    .next()
                    .unwrap();
                let soup = if filled {
                    Some(app.world_mut().spawn(Soup::new(FoodType::Tomato)).id())
                } else {
                    None
                };
                app.world_mut().get_mut::<Pot>(pot).unwrap().soup = soup;
                // Face the left stove at (0,5); its pot is looked up below.
                let stove = app
                    .world_mut()
                    .query::<(&Station, &StationItem, &Transform)>()
                    .iter(app.world())
                    .find(|(_, item, _)| item.entity == Some(pot))
                    .unwrap()
                    .2
                    .translation;
                app.world_mut()
                    .get_mut::<Transform>(player)
                    .unwrap()
                    .translation = stove + Vec3::new(16.0, 0.0, Z_PLAYER);
                app.world_mut().get_mut::<Facing>(player).unwrap().direction = Direction::Left;
                app.world_mut()
                    .get_mut::<ActionState>(player)
                    .unwrap()
                    .cooldown = 0.0;
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .reset_all();
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::KeyB);
                app.update();
                assert_eq!(app.world().get::<HeldItem>(player).unwrap().entity, None);
                let soup_entity = app.world().get::<Pot>(pot).unwrap().soup.unwrap();
                let expected = if filled {
                    vec![FoodType::Tomato, food]
                } else {
                    vec![food]
                };
                assert_eq!(
                    app.world().get::<Soup>(soup_entity).unwrap().ingredients,
                    expected
                );
                assert!(app.world().get_entity(item).is_err());
                // Refill the hand for the next branch.
                item = app
                    .world_mut()
                    .spawn((
                        Ingredient {
                            food_type: food,
                            prep_ratio: 1.0,
                        },
                        ItemKind::Ingredient,
                        GameplayEntity,
                    ))
                    .id();
                app.world_mut().get_mut::<HeldItem>(player).unwrap().entity = Some(item);
            }
        }
        let remaining = app.world().resource::<LevelState>().time_remaining;
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.press(KeyCode::KeyQ);
            keys.release(KeyCode::KeyQ);
        }
        app.update();
        let level = app.world().resource::<LevelState>();
        assert!(level.paused);
        assert_eq!(level.time_remaining, remaining);
    }
}
