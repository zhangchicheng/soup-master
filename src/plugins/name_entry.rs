use crate::components::*;
use crate::constants::*;
use crate::resources::*;
use crate::score_store::{HighScoreStore, clean_name};
use crate::states::AppState;
use bevy::input::{ButtonState, keyboard::KeyboardInput};
use bevy::prelude::*;

/// Name entry plugin: on-screen keyboard for entering high score names.
pub struct NameEntryPlugin;

#[derive(Component)]
struct NameEntryStatus;

const KEEB: [[&str; 8]; 4] = [
    ["A", "B", "C", "D", "E", "F", "G", "H"],
    ["I", "J", "K", "L", "M", "N", "O", "P"],
    ["Q", "R", "S", "T", "U", "V", "W", "X"],
    ["Y", "Z", "*", "!", "SAVE", "", "", "DEL"],
];

impl Plugin for NameEntryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::NameEntry), setup_name_entry)
            .add_systems(
                Update,
                name_entry_input_system.run_if(in_state(AppState::NameEntry)),
            )
            .add_systems(OnExit(AppState::NameEntry), cleanup_name_entry);
    }
}

fn setup_name_entry(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut typing: MessageReader<KeyboardInput>,
) {
    typing.clear();
    // Camera for UI rendering
    commands.spawn((Camera2d, NameEntryEntity));

    let font = asset_server.load("fonts/gameboy.ttf");

    // Title
    commands.spawn((
        Text::new("ENTER YOUR NAME:"),
        TextFont {
            font: font.clone(),
            font_size: HUD_FONT_SIZE * WINDOW_SCALE,
            ..default()
        },
        TextColor(crate::theme::INK),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(50.0 * WINDOW_SCALE),
            top: Val::Px(16.0 * WINDOW_SCALE),
            ..default()
        },
        NameEntryEntity,
    ));

    // Name display
    commands.spawn((
        Text::new(".........."),
        TextFont {
            font: font.clone(),
            font_size: 16.0 * WINDOW_SCALE,
            ..default()
        },
        TextColor(crate::theme::INK),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(40.0 * WINDOW_SCALE),
            top: Val::Px(36.0 * WINDOW_SCALE),
            ..default()
        },
        NameEntryDisplay,
        NameEntryEntity,
    ));

    // Keyboard grid
    let x_offset = 50.0;
    let y_offset = 60.0;
    for (row_idx, row) in KEEB.iter().enumerate() {
        for (col_idx, key) in row.iter().enumerate() {
            if key.is_empty() {
                continue;
            }
            let x = x_offset + col_idx as f32 * 16.0;
            let y = y_offset + (row_idx as f32 + 1.0) * 16.0;

            commands.spawn((
                Text::new(key.to_string()),
                TextFont {
                    font: font.clone(),
                    font_size: HUD_FONT_SIZE * WINDOW_SCALE,
                    ..default()
                },
                TextColor(crate::theme::INK),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x * WINDOW_SCALE),
                    top: Val::Px(y * WINDOW_SCALE),
                    ..default()
                },
                NameEntryKey {
                    row: row_idx,
                    col: col_idx,
                },
                NameEntryEntity,
            ));
        }
    }
    for (text, y) in [
        ("TYPE NAME / BACKSPACE DELETE", 136.0),
        ("ENTER SAVE   ESC SKIP", 146.0),
    ] {
        let mut line = commands.spawn((
            Text::new(text),
            TextFont {
                font: font.clone(),
                font_size: 5.0 * WINDOW_SCALE,
                ..default()
            },
            TextColor(crate::theme::INK),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(28.0 * WINDOW_SCALE),
                top: Val::Px(y * WINDOW_SCALE),
                ..default()
            },
            NameEntryEntity,
        ));
        if y == 146.0 {
            line.insert(NameEntryStatus);
        }
    }
}

fn name_entry_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut typing: MessageReader<KeyboardInput>,
    store: Res<HighScoreStore>,
    mut name_state: ResMut<NameEntryState>,
    mut high_scores: ResMut<HighScores>,
    mut pending_score: ResMut<PendingScore>,
    mut next_state: ResMut<NextState<AppState>>,
    mut name_display: Query<&mut Text, With<NameEntryDisplay>>,
    mut key_texts: Query<(&NameEntryKey, &mut TextColor)>,
    mut status: Query<&mut Text, (With<NameEntryStatus>, Without<NameEntryDisplay>)>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        pending_score.0 = None;
        next_state.set(AppState::WorldMap);
        return;
    }
    for event in typing.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if event.key_code == KeyCode::Backspace {
            name_state.name.pop();
        } else if let Some(text) = &event.text {
            // Space remains the on-screen keyboard's selection key.
            for character in text
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || "-_!*".contains(*c))
            {
                if name_state.name.len() < name_state.max_length {
                    name_state.name.push(character.to_ascii_uppercase());
                }
            }
        }
    }
    let mut save_requested = keyboard.just_pressed(KeyCode::Enter);
    // Navigation
    if keyboard.just_released(KeyCode::ArrowRight) {
        move_selection(&mut name_state, "right");
    }
    if keyboard.just_released(KeyCode::ArrowLeft) {
        move_selection(&mut name_state, "left");
    }
    if keyboard.just_released(KeyCode::ArrowUp) {
        move_selection(&mut name_state, "up");
    }
    if keyboard.just_released(KeyCode::ArrowDown) {
        move_selection(&mut name_state, "down");
    }

    // Selection
    if keyboard.just_released(KeyCode::Space) {
        let selected = KEEB[name_state.selected_row][name_state.selected_col];
        if selected.len() == 1 && name_state.name.len() < name_state.max_length {
            name_state.name.push_str(selected);
        } else if selected == "DEL" && !name_state.name.is_empty() {
            name_state.name.pop();
        } else if selected == "SAVE" {
            save_requested = true;
        }
    }
    if save_requested && !clean_name(&name_state.name).is_empty() {
        if let Some(score) = pending_score.0 {
            let mut saved = high_scores.clone();
            saved.entries.push((clean_name(&name_state.name), score));
            saved.entries.sort_by(|a, b| b.1.cmp(&a.1));
            saved.entries.truncate(5);
            match store.save(&saved) {
                Ok(()) => {
                    *high_scores = saved;
                    pending_score.0 = None;
                    next_state.set(AppState::Score);
                    return;
                }
                Err(error) => {
                    warn!("Cannot save high scores: {error}");
                    for mut text in &mut status {
                        **text = "SAVE FAILED - ENTER TO RETRY".to_string();
                    }
                }
            }
        }
    }

    // Update name display
    for mut text in &mut name_display {
        let dots = ".".repeat(name_state.max_length.saturating_sub(name_state.name.len()));
        **text = format!("{}{}", name_state.name, dots);
    }

    // Update key highlighting
    for (key, mut color) in &mut key_texts {
        if key.row == name_state.selected_row && key.col == name_state.selected_col {
            *color = TextColor(crate::theme::RED);
        } else {
            *color = TextColor(crate::theme::INK);
        }
    }
}

fn move_selection(state: &mut NameEntryState, direction: &str) {
    match direction {
        "right" => {
            state.selected_col += 1;
            if state.selected_col >= 8 {
                state.selected_col = 0;
            }
        }
        "left" => {
            if state.selected_col == 0 {
                state.selected_col = 7;
            } else {
                state.selected_col -= 1;
            }
        }
        "up" => {
            if state.selected_row == 0 {
                state.selected_row = 3;
            } else {
                state.selected_row -= 1;
            }
        }
        "down" => {
            state.selected_row += 1;
            if state.selected_row >= 4 {
                state.selected_row = 0;
            }
        }
        _ => {}
    }

    // Skip empty cells
    let selected = KEEB[state.selected_row][state.selected_col];
    if selected.is_empty() {
        if direction == "up" || direction == "down" {
            move_selection(state, "left");
        } else {
            move_selection(state, direction);
        }
    }
}

fn cleanup_name_entry(mut commands: Commands, entities: Query<Entity, With<NameEntryEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::Key;

    #[test]
    fn typed_name_is_saved_and_a_failed_write_can_be_retried_without_losing_it() {
        let folder = std::env::temp_dir().join(format!(
            "soup-master-name-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let blocked = folder.join("blocked");
        std::fs::write(&blocked, "file in place of directory").unwrap();
        let mut app = App::new();
        app.add_message::<KeyboardInput>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<NameEntryState>()
            .init_resource::<HighScores>()
            .insert_resource(PendingScore(Some(4)))
            .insert_resource(HighScoreStore(blocked.join("scores.json")))
            .insert_resource(NextState::<AppState>::default())
            .add_systems(Update, name_entry_input_system);
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyS,
            logical_key: Key::Character("soup".into()),
            state: ButtonState::Pressed,
            text: Some("soup".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(app.world().resource::<NameEntryState>().name, "SOUP");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert_eq!(app.world().resource::<PendingScore>().0, Some(4));
        assert!(app.world().resource::<HighScores>().entries.is_empty());
        assert_eq!(app.world().resource::<NameEntryState>().name, "SOUP");
        std::fs::remove_file(&blocked).unwrap();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.reset_all();
        keys.press(KeyCode::Enter);
        app.update();
        assert_eq!(app.world().resource::<PendingScore>().0, None);
        assert_eq!(
            app.world()
                .resource::<HighScoreStore>()
                .load()
                .unwrap()
                .entries,
            vec![("SOUP".into(), 4)]
        );
        assert!(matches!(
            app.world().resource::<NextState<AppState>>(),
            NextState::Pending(AppState::Score)
        ));
        std::fs::remove_dir_all(folder).unwrap();
    }
}
