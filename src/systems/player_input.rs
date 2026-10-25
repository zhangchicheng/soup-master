use crate::components::*;
use bevy::prelude::*;

/// Reads keyboard input and sets player velocity + action/transfer flags.
pub fn player_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut players: Query<(&mut Velocity, &mut Facing, &mut ActionState, &KeyMap)>,
) {
    for (mut vel, mut facing, mut action_state, keymap) in &mut players {
        vel.dx = 0.0;
        vel.dy = 0.0;
        action_state.action = false;
        action_state.transfer = false;
        // Movement
        if keyboard.pressed(keymap.up) && !keyboard.pressed(keymap.down) {
            facing.direction = Direction::Up;
            vel.dy = vel.speed;
        } else if keyboard.pressed(keymap.down) && !keyboard.pressed(keymap.up) {
            facing.direction = Direction::Down;
            vel.dy = -vel.speed;
        }

        if keyboard.pressed(keymap.left) && !keyboard.pressed(keymap.right) {
            facing.direction = Direction::Left;
            vel.dx = -vel.speed;
        } else if keyboard.pressed(keymap.right) && !keyboard.pressed(keymap.left) {
            facing.direction = Direction::Right;
            vel.dx = vel.speed;
        }

        if vel.dx != 0.0 && vel.dy != 0.0 {
            vel.dx *= std::f32::consts::FRAC_1_SQRT_2;
            vel.dy *= std::f32::consts::FRAC_1_SQRT_2;
        }

        // Actions
        if keyboard.pressed(keymap.action) {
            action_state.action = true;
        } else if keyboard.just_pressed(keymap.transfer) {
            action_state.transfer = true;
        }
    }
}

/// Reset velocity and action flags after processing.
pub fn player_input_cleanup_system(
    mut players: Query<(&mut Velocity, &mut ActionState, &mut DidMove)>,
) {
    for (mut vel, mut action_state, mut did_move) in &mut players {
        vel.dx = 0.0;
        vel.dy = 0.0;
        action_state.action = false;
        action_state.transfer = false;
        did_move.0 = false;
        action_state.did_change_state = false;
        action_state.did_illegal_action = false;
    }
}

/// Handle pause toggle.
pub fn pause_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    players: Query<&KeyMap>,
    mut level_state: ResMut<crate::resources::LevelState>,
) {
    for keymap in &players {
        if keyboard.just_released(keymap.pause) {
            level_state.paused = !level_state.paused;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_speed_and_single_press_transfer() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, player_input_system);
        let player = app
            .world_mut()
            .spawn((
                Velocity {
                    speed: 90.0,
                    ..default()
                },
                Facing::default(),
                ActionState {
                    action: false,
                    transfer: false,
                    cooldown: 0.0,
                    cooldown_max: 0.5,
                    activity: None,
                    did_change_state: false,
                    did_illegal_action: false,
                },
                KeyMap {
                    up: KeyCode::KeyW,
                    down: KeyCode::KeyS,
                    left: KeyCode::KeyA,
                    right: KeyCode::KeyD,
                    pause: KeyCode::KeyQ,
                    action: KeyCode::KeyV,
                    transfer: KeyCode::KeyB,
                },
            ))
            .id();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyW);
            keys.press(KeyCode::KeyD);
            keys.press(KeyCode::KeyB);
        }
        app.update();
        let velocity = app.world().get::<Velocity>(player).unwrap();
        assert!((Vec2::new(velocity.dx, velocity.dy).length() - 90.0).abs() < 0.001);
        assert!(app.world().get::<ActionState>(player).unwrap().transfer);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert!(!app.world().get::<ActionState>(player).unwrap().transfer);
    }
}
