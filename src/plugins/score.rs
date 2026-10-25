use crate::systems::audio::{MenuSound, play_menu_sound};
use crate::{
    components::*,
    constants::*,
    resources::*,
    screen_art::ScreenArt,
    screen_ui::*,
    states::AppState,
    theme::{INK, PAPER},
};
use bevy::prelude::*;

pub struct ScorePlugin;
#[derive(Component)]
struct ResultStar(usize);
#[derive(Component)]
struct ResultNumber(usize);
#[derive(Resource)]
struct ReceiptAnimation {
    elapsed: f32,
    skip: bool,
}

impl Plugin for ScorePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Score), setup_score)
            .add_systems(
                Update,
                (score_input_system, animate_receipt)
                    .chain()
                    .run_if(in_state(AppState::Score)),
            )
            .add_systems(OnExit(AppState::Score), cleanup_score);
    }
}

fn setup_score(
    mut commands: Commands,
    server: Res<AssetServer>,
    art: Res<ScreenArt>,
    high_scores: Res<HighScores>,
    pending: Res<PendingScore>,
    result: Res<RoundResult>,
) {
    commands.spawn((Camera2d, ScoreScreenEntity));
    let font = server.load("fonts/gameboy.ttf");
    if pending.0.is_some() {
        let mut stars = Vec::new();
        let mut numbers = Vec::new();
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ScoreScreenEntity,
            ))
            .with_children(|root| {
                // The edited background is authored at the native 224x160 canvas.
                // Paper: x=67..156, y=28..139; horizontal rules: y=54/61/95/105/134.
                screen_image(root, art.results.clone(), 0.0, 0.0, BASE_WIDTH, BASE_HEIGHT);
                // Two centered lines of native text fit above the receipt's first rule.
                label(root, &font, "SHIFT", 92.0, 32.0, 8.0, INK);
                label(root, &font, "COMPLETE", 80.0, 42.0, 8.0, INK);
                label(
                    root,
                    &font,
                    format!("LEVEL {}", result.level + 1),
                    94.5,
                    55.0,
                    5.0,
                    INK,
                );
                label(root, &font, "ORDERS", 74.0, 64.0, 4.0, INK);
                label(root, &font, "DELIVERED", 74.0, 72.0, 4.0, INK);
                label(root, &font, "BURNT", 74.0, 81.0, 4.0, INK);
                label(root, &font, "TIPS", 74.0, 89.0, 4.0, INK);
                label(root, &font, "TOTAL", 74.0, 98.0, 5.0, INK);
                for (i, y) in [72.0, 81.0, 89.0, 98.0].into_iter().enumerate() {
                    numbers.push((
                        label(
                            root,
                            &font,
                            "",
                            if i == 3 { 121.0 } else { 119.0 },
                            y,
                            if i == 3 { 5.0 } else { 4.0 },
                            INK,
                        ),
                        i,
                    ));
                }
                for i in 0..3 {
                    let star = screen_image(
                        root,
                        art.star_empty.clone(),
                        71.0 + i as f32 * 28.0,
                        107.0,
                        26.0,
                        26.0,
                    );
                    stars.push((star, i));
                }
                label(root, &font, "ENTER MAP   R RETRY", 64.5, 141.0, 5.0, INK);
                if result.delivered > 0
                    && (high_scores.entries.len() < 5
                        || high_scores
                            .entries
                            .iter()
                            .any(|(_, score)| result.delivered > *score))
                {
                    label(root, &font, "N SAVE SCORE", 82.0, 150.0, 5.0, INK);
                }
            });
        for (entity, index) in stars {
            commands.entity(entity).insert(ResultStar(index));
        }
        for (e, i) in numbers {
            commands.entity(e).insert(ResultNumber(i));
        }
        commands.insert_resource(ReceiptAnimation {
            elapsed: 0.0,
            skip: false,
        });
    } else {
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ScoreScreenEntity,
            ))
            .with_children(|root| {
                crate::theme::checkerboard(root);
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(24.0 * WINDOW_SCALE),
                        top: Val::Px(8.0 * WINDOW_SCALE),
                        width: Val::Px(176.0 * WINDOW_SCALE),
                        height: Val::Px(144.0 * WINDOW_SCALE),
                        ..default()
                    },
                    BackgroundColor(PAPER),
                ))
                .with_children(|panel| {
                    label(panel, &font, "HIGH SCORES", 43.0, 10.0, 8.0, INK);
                    if high_scores.entries.is_empty() {
                        label(panel, &font, "NO SCORES YET", 36.0, 54.0, 8.0, INK);
                    }
                    for (i, (name, score)) in high_scores.entries.iter().enumerate() {
                        label(
                            panel,
                            &font,
                            format!("{:<12}{:>4}", name, score),
                            20.0,
                            32.0 + i as f32 * 16.0,
                            8.0,
                            INK,
                        );
                    }
                    label(
                        panel,
                        &font,
                        "ENTER / SPACE TO RETURN",
                        21.0,
                        124.0,
                        6.0,
                        INK,
                    );
                });
            });
    }
}
fn animate_receipt(
    time: Res<Time>,
    result: Res<RoundResult>,
    art: Res<ScreenArt>,
    animation: Option<ResMut<ReceiptAnimation>>,
    mut stars: Query<(&ResultStar, &mut ImageNode)>,
    mut numbers: Query<(&ResultNumber, &mut Text)>,
) {
    let Some(mut animation) = animation else {
        return;
    };
    animation.elapsed += time.delta_secs();
    if animation.skip {
        animation.elapsed = 4.0;
    }
    let ratio = ((animation.elapsed - 0.3) / 1.2).clamp(0.0, 1.0);
    let revealed = (((animation.elapsed - 1.5) / 0.4).floor().max(0.0) as usize)
        .min(Campaign::stars(result.delivered));
    for (star, mut image) in &mut stars {
        image.image = if star.0 < revealed {
            art.star.clone()
        } else {
            art.star_empty.clone()
        };
    }
    for (kind, mut text) in &mut numbers {
        **text = match kind.0 {
            0 => format!(
                "{:>2} x 10",
                (result.delivered as f32 * ratio).round() as u32
            ),
            1 => format!("{:>2} x-10", (result.burnt as f32 * ratio).round() as u32),
            2 => format!(
                "{:>7}",
                (result.delivered as f32 * 2.0 * ratio).round() as u32
            ),
            _ => format!("{:>6}", (result.total() as f32 * ratio).round() as i32),
        };
    }
}
fn score_input_system(
    mut commands: Commands,
    server: Res<AssetServer>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut pending: ResMut<PendingScore>,
    high_scores: Res<HighScores>,
    mut name: ResMut<NameEntryState>,
    mut next: ResMut<NextState<AppState>>,
    animation: Option<ResMut<ReceiptAnimation>>,
) {
    let confirm = keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space);
    if let Some(score) = pending.0 {
        if keyboard.just_pressed(KeyCode::KeyR) {
            pending.0 = None;
            next.set(AppState::Loading);
            return;
        }
        if keyboard.just_pressed(KeyCode::KeyN) {
            let index = high_scores
                .entries
                .iter()
                .position(|(_, old)| score > *old)
                .or_else(|| (high_scores.entries.len() < 5).then_some(high_scores.entries.len()));
            if score > 0 {
                if let Some(index) = index {
                    name.score_index = Some(index);
                    name.name.clear();
                    name.selected_row = 0;
                    name.selected_col = 0;
                    next.set(AppState::NameEntry);
                    return;
                }
            }
        }
        if confirm {
            if let Some(mut animation) = animation {
                if animation.elapsed < 2.8 {
                    animation.skip = true;
                    return;
                }
            }
            pending.0 = None;
            next.set(AppState::WorldMap);
        } else if keyboard.just_pressed(KeyCode::Escape) {
            pending.0 = None;
            next.set(AppState::WorldMap);
        }
    } else if confirm || keyboard.just_pressed(KeyCode::Escape) {
        play_menu_sound(&mut commands, &server, MenuSound::Back);
        next.set(AppState::Title);
    }
}
fn cleanup_score(mut commands: Commands, entities: Query<Entity, With<ScoreScreenEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<ReceiptAnimation>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn input_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            bevy::asset::AssetPlugin::default(),
        ))
        .init_asset::<AudioSource>()
        .init_state::<AppState>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<HighScores>()
        .init_resource::<NameEntryState>()
        .insert_resource(PendingScore(Some(6)))
        .add_systems(Update, score_input_system);
        app
    }
    #[test]
    fn result_can_return_to_map_or_retry_without_losing_score_for_name_entry() {
        for (key, state) in [
            (KeyCode::Enter, AppState::WorldMap),
            (KeyCode::KeyR, AppState::Loading),
            (KeyCode::KeyN, AppState::NameEntry),
        ] {
            let mut app = input_app();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            assert!(
                matches!(app.world().resource::<NextState<AppState>>(),NextState::Pending(next) if *next==state)
            );
            assert_eq!(
                app.world().resource::<PendingScore>().0,
                if key == KeyCode::KeyN { Some(6) } else { None }
            );
        }
    }
    #[test]
    fn leaving_high_scores_plays_the_back_effect() {
        let mut app = input_app();
        app.world_mut().resource_mut::<PendingScore>().0 = None;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        let handle = app
            .world_mut()
            .query::<&AudioPlayer<AudioSource>>()
            .single(app.world())
            .unwrap()
            .0
            .clone();
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(handle.id())
                .unwrap()
                .path(),
            std::path::Path::new("sounds/menu/back.wav")
        );
    }

    #[test]
    fn confirm_first_finishes_animation_then_leaves() {
        let mut app = input_app();
        app.insert_resource(ReceiptAnimation {
            elapsed: 0.5,
            skip: false,
        });
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert!(app.world().resource::<ReceiptAnimation>().skip);
        assert_eq!(app.world().resource::<PendingScore>().0, Some(6));
    }
    #[test]
    fn receipt_total_uses_deliveries_tips_and_burns() {
        assert_eq!(
            RoundResult {
                level: 0,
                delivered: 6,
                burnt: 1
            }
            .total(),
            62
        );
        assert_eq!(
            RoundResult {
                level: 0,
                delivered: 0,
                burnt: 1
            }
            .total(),
            -10
        );
    }
}
