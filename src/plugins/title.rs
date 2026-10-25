use crate::components::*;
use crate::constants::*;
use crate::resources::*;
use crate::states::AppState;
use crate::systems::audio::{MenuMusic, MenuSound, ensure_menu_music, play_menu_sound};
use bevy::prelude::*;
use bevy_tweening::*;
use std::time::Duration;

/// Title screen plugin: animated intro, menu selection.
pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), setup_title)
            .add_systems(
                Update,
                (title_intro_system, title_input_system).run_if(in_state(AppState::Title)),
            )
            .add_systems(OnExit(AppState::Title), cleanup_title);
    }
}

// ---------------------------------------------------------------------------
//  Animation marker components (local to this module)
// ---------------------------------------------------------------------------
#[derive(Component)]
struct TitleHeading;

#[derive(Component)]
struct MenuContainer;

/// Marks the image cursor for a menu option.
#[derive(Component)]
struct TitleMenuCursorMark {
    index: usize,
}

/// Tracks the intro animation timeline.
#[derive(Resource)]
struct TitleIntroState {
    elapsed: f32,
    intro_sound_played: bool,
    bgm_started: bool,
    menu_shown: bool,
}

impl Default for TitleIntroState {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            intro_sound_played: false,
            bgm_started: false,
            menu_shown: false,
        }
    }
}

// ---------------------------------------------------------------------------
//  Custom lens for animating Node::top
// ---------------------------------------------------------------------------

/// Lens that interpolates the `top` field of a `Node` component between two `Val::Px` values.
struct NodeTopLens {
    start: f32,
    end: f32,
}

impl Lens<Node> for NodeTopLens {
    fn lerp(&mut self, mut target: Mut<Node>, ratio: f32) {
        target.top = Val::Px(self.start + (self.end - self.start) * ratio);
    }
}

// ---------------------------------------------------------------------------
//  Setup — spawn elements at their initial (off-screen) positions
// ---------------------------------------------------------------------------

fn setup_title(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera for UI rendering
    commands.spawn((Camera2d, TitleScreenEntity));

    // Boot sound (plays immediately)
    commands.spawn((
        AudioPlayer::new(asset_server.load("sounds/sfx/start.wav")),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Despawn,
            ..default()
        },
        TitleScreenEntity,
    ));

    let font = asset_server.load("fonts/gameboy.ttf");

    // Black background container — all UI elements are children of this
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(crate::theme::PAPER),
            TitleScreenEntity,
        ))
        .with_children(|bg| {
            crate::theme::checkerboard(bg);
            // Editable font title — starts off-screen at y=-100 base
            // Tween: delay 1.5s, then slide from y=-100 to y=2 over 0.7s
            let title_tween = Delay::new(Duration::from_secs_f32(1.5)).then(Tween::new(
                EaseFunction::QuadraticInOut,
                Duration::from_secs_f32(0.7),
                NodeTopLens {
                    start: -100.0 * WINDOW_SCALE,
                    end: 2.0 * WINDOW_SCALE,
                },
            ));
            bg.spawn((
                Text::new("Soup\nMaster"),
                bevy::text::LineHeight::RelativeToFont(0.95),
                TextFont {
                    font: asset_server.load("fonts/FatPix-SVG.ttf"),
                    font_size: 28.0 * WINDOW_SCALE,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                TextColor(crate::theme::RED),
                TextShadow {
                    offset: Vec2::splat(WINDOW_SCALE),
                    color: crate::theme::INK,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0 * WINDOW_SCALE),
                    top: Val::Px(-100.0 * WINDOW_SCALE), // off-screen
                    width: Val::Px((BASE_WIDTH - 16.0) * WINDOW_SCALE),
                    height: Val::Px(52.0 * WINDOW_SCALE),
                    ..default()
                },
                TitleHeading,
                TweenAnim::new(title_tween),
            ));

            // Native 80x64 bowl, horizontally centered above the menu.
            // Tween: delay 1.0s, then slide from y=-64 to y=58 over 1.0s
            let soup_tween = Delay::new(Duration::from_secs_f32(1.0)).then(Tween::new(
                EaseFunction::QuadraticInOut,
                Duration::from_secs_f32(1.0),
                NodeTopLens {
                    start: -64.0 * WINDOW_SCALE,
                    end: 58.0 * WINDOW_SCALE,
                },
            ));
            bg.spawn((
                ImageNode {
                    image: asset_server.load("images/ui/soup_bowl.png"),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(((BASE_WIDTH - 80.0) / 2.0) * WINDOW_SCALE),
                    top: Val::Px(-64.0 * WINDOW_SCALE), // off-screen
                    width: Val::Px(80.0 * WINDOW_SCALE),
                    height: Val::Px(64.0 * WINDOW_SCALE),
                    ..default()
                },
                TweenAnim::new(soup_tween),
            ));

            // Menu container — initially hidden (Visibility::Hidden)
            bg.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(MENU_X * WINDOW_SCALE),
                    top: Val::Px(MENU_Y * WINDOW_SCALE),
                    width: Val::Px(216.0 * WINDOW_SCALE),
                    height: Val::Px(22.0 * WINDOW_SCALE),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(2.0 * WINDOW_SCALE)),
                    overflow: Overflow::visible(),
                    ..default()
                },
                BackgroundColor(crate::theme::RED),
                Visibility::Hidden,
                MenuContainer,
            ))
            .with_children(|menu| {
                let items = ["1 Chef", "2 Chefs", "High Scores"];
                for (i, item) in items.iter().enumerate() {
                    // Reserve the cursor space even for unselected options.
                    menu.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            flex_shrink: 0.0,
                            ..default()
                        },
                        TitleMenuItem,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(asset_server.load("images/ui/chef_cursor.png")),
                            Node {
                                width: Val::Px(16.0 * WINDOW_SCALE),
                                height: Val::Px(16.0 * WINDOW_SCALE),
                                margin: UiRect::right(Val::Px(WINDOW_SCALE)),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            if i == 0 {
                                Visibility::Inherited
                            } else {
                                Visibility::Hidden
                            },
                            TitleMenuCursorMark { index: i },
                        ));
                        // Menu label beside the cursor.
                        row.spawn((
                            Text::new(item.to_string()),
                            TextFont {
                                font: font.clone(),
                                font_size: 7.0 * WINDOW_SCALE,
                                ..default()
                            },
                            TextLayout::new_with_justify(Justify::Left),
                            TextColor(crate::theme::PAPER),
                        ));
                    });
                }
            });
        });

    commands.insert_resource(MenuSelection::new(3));
    commands.insert_resource(TitleIntroState::default());
}

// ---------------------------------------------------------------------------
//  Intro animation system — drives the timed intro sequence
// ---------------------------------------------------------------------------

/// Timeline from the original Love2D title.lua:
///   t=0.0: boot sound (already played in setup)
///   t=1.0: soup slides y=-64 → y=58 over 1.0s (in-out-quad)
///   t=1.2: intro/fall sound
///   t=1.5: BGM starts; title slides y=-100→y=2 (0.7s)
///   t=2.0: menu appears
fn title_intro_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    mut intro: ResMut<TitleIntroState>,
    music: Query<(), With<MenuMusic>>,
    mut menu_q: Query<&mut Visibility, With<MenuContainer>>,
) {
    intro.elapsed += time.delta_secs();
    let t = intro.elapsed;

    // --- Intro/fall sound at t=1.2 ---
    if t >= 1.2 && !intro.intro_sound_played {
        intro.intro_sound_played = true;
        commands.spawn((
            AudioPlayer::new(asset_server.load("sounds/sfx/fall.wav")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                ..default()
            },
            TitleScreenEntity,
        ));
    }

    // --- BGM at t=1.5 ---
    if t >= 1.5 && !intro.bgm_started {
        intro.bgm_started = true;
        ensure_menu_music(&mut commands, &asset_server, &music);
    }

    // --- Show the menu at t=2.0 ---
    if t >= 2.0 && !intro.menu_shown {
        intro.menu_shown = true;
        for mut vis in &mut menu_q {
            *vis = Visibility::Inherited;
        }
    }
}

// ---------------------------------------------------------------------------
//  Input system — only active after intro finishes
// ---------------------------------------------------------------------------

fn title_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut menu: ResMut<MenuSelection>,
    mut cursors: Query<(&TitleMenuCursorMark, &mut Visibility)>,
    mut next_state: ResMut<NextState<AppState>>,
    mut selected_player_count: ResMut<SelectedPlayerCount>,
    mut pending_score: ResMut<PendingScore>,
    intro: Option<Res<TitleIntroState>>,
) {
    // Don't accept input until intro is done
    if let Some(ref intro) = intro {
        if !intro.menu_shown {
            return;
        }
    }

    if keyboard.just_released(KeyCode::ArrowUp)
        || keyboard.just_released(KeyCode::KeyW)
        || keyboard.just_released(KeyCode::ArrowLeft)
        || keyboard.just_released(KeyCode::KeyA)
    {
        menu.up();
        play_menu_sound(&mut commands, &asset_server, MenuSound::Hover);
    }
    if keyboard.just_released(KeyCode::ArrowDown)
        || keyboard.just_released(KeyCode::KeyS)
        || keyboard.just_released(KeyCode::ArrowRight)
        || keyboard.just_released(KeyCode::KeyD)
    {
        menu.down();
        play_menu_sound(&mut commands, &asset_server, MenuSound::Hover);
    }

    for (cursor_mark, mut visibility) in &mut cursors {
        *visibility = if cursor_mark.index == menu.index {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    if keyboard.just_released(KeyCode::Space) || keyboard.just_released(KeyCode::Enter) {
        play_menu_sound(&mut commands, &asset_server, MenuSound::Confirm);

        match menu.index {
            0 => {
                selected_player_count.0 = 1;
                next_state.set(AppState::WorldMap);
            }
            1 => {
                selected_player_count.0 = 2;
                next_state.set(AppState::WorldMap);
            }
            2 => {
                pending_score.0 = None;
                next_state.set(AppState::Score);
            }
            _ => {}
        }
    }
}

fn cleanup_title(mut commands: Commands, entities: Query<Entity, With<TitleScreenEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<TitleIntroState>();
}

#[cfg(test)]
mod menu_sound_tests {
    use super::*;
    use bevy::{asset::AssetPlugin, ecs::system::RunSystemOnce};

    #[test]
    fn menu_uses_red_background_and_paper_text() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_asset::<Image>()
            .init_asset::<Font>();
        app.world_mut().run_system_once(setup_title).unwrap();
        let menu = app
            .world_mut()
            .query_filtered::<Entity, With<MenuContainer>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<BackgroundColor>(menu).unwrap().0,
            crate::theme::RED
        );
        let colors = app
            .world_mut()
            .query::<(&Text, &TextColor)>()
            .iter(app.world())
            .filter(|(text, _)| ["1 Chef", "2 Chefs", "High Scores"].contains(&text.0.as_str()))
            .map(|(_, color)| color.0.to_srgba())
            .collect::<Vec<_>>();
        assert_eq!(colors.len(), 3);
        for color in colors {
            assert_eq!(
                (
                    (color.red * 255.0).round() as u8,
                    (color.green * 255.0).round() as u8,
                    (color.blue * 255.0).round() as u8
                ),
                (218, 212, 201)
            );
        }
    }

    #[test]
    fn menu_uses_new_effects_and_confirmation_survives_screen_cleanup() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SelectedPlayerCount>()
            .init_resource::<PendingScore>()
            .insert_resource(MenuSelection::new(3))
            .insert_resource(NextState::<AppState>::default())
            .add_systems(Update, title_input_system);
        for (key, expected) in [
            (KeyCode::ArrowDown, "sounds/menu/hover.wav"),
            (KeyCode::ArrowUp, "sounds/menu/hover.wav"),
            (KeyCode::Enter, "sounds/menu/confirm.wav"),
        ] {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(key);
            keys.release(key);
            app.update();
            let sounds: Vec<_> = app
                .world_mut()
                .query::<(Entity, &AudioPlayer<AudioSource>, &PlaybackSettings)>()
                .iter(app.world())
                .map(|(entity, player, settings)| {
                    assert!(matches!(settings.mode, bevy::audio::PlaybackMode::Despawn));
                    (entity, player.0.clone())
                })
                .collect();
            assert_eq!(sounds.len(), 1);
            let (entity, handle) = &sounds[0];
            assert_eq!(
                app.world()
                    .resource::<AssetServer>()
                    .get_path(handle.id())
                    .unwrap()
                    .path(),
                std::path::Path::new(expected)
            );
            app.world_mut().run_system_once(cleanup_title).unwrap();
            assert!(
                app.world()
                    .get::<AudioPlayer<AudioSource>>(*entity)
                    .is_some(),
                "Menu effects must finish across screen changes"
            );
            app.world_mut().despawn(*entity);
        }
    }
}
