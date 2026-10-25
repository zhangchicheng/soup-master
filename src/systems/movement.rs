use crate::components::*;
use crate::constants::*;
use bevy::prelude::*;

/// Resolve each axis separately so sliding along a row of counters is stable.
/// Small steps prevent crossing a tile during a slow frame.
pub fn movement_system(
    time: Res<Time>,
    mut players: Query<
        (
            &mut Transform,
            &Velocity,
            &Facing,
            &mut DidMove,
            &mut FacingStation,
        ),
        With<Player>,
    >,
    collideables: Query<
        (&Transform, Option<&Station>, Entity),
        (With<Collideable>, Without<Player>),
    >,
) {
    for (mut transform, vel, facing, mut did_move, mut facing_station) in &mut players {
        let start = transform.translation.truncate();
        let delta = Vec2::new(vel.dx, vel.dy) * time.delta_secs();
        let steps = (delta.abs().max_element() / (TILE_SIZE / 4.0))
            .ceil()
            .max(1.0) as usize;
        let step = delta / steps as f32;
        let mut position = start;
        for _ in 0..steps {
            for axis in 0..2 {
                let movement = step[axis];
                if movement == 0.0 {
                    continue;
                }
                position[axis] += movement;
                for (tile, _, _) in &collideables {
                    let center = tile.translation.truncate();
                    let extent = Vec2::new(
                        (PLAYER_WIDTH + TILE_SIZE) / 2.0,
                        (PLAYER_HEIGHT + TILE_SIZE) / 2.0,
                    );
                    let overlap = extent - (position - center).abs();
                    if overlap.x > 0.0 && overlap.y > 0.0 {
                        position[axis] = center[axis] - movement.signum() * extent[axis];
                    }
                }
            }
        }
        did_move.0 = position.distance_squared(start) > 0.000001;
        transform.translation.x = position.x;
        transform.translation.y = position.y;

        // Probe the facing side even while standing still. Never keep a stale station.
        let direction = match facing.direction {
            Direction::Up => Vec2::Y,
            Direction::Down => Vec2::NEG_Y,
            Direction::Left => Vec2::NEG_X,
            Direction::Right => Vec2::X,
        };
        facing_station.entity = collideables
            .iter()
            .filter(|(_, station, _)| station.is_some())
            .filter_map(|(tile, _, entity)| {
                let offset = tile.translation.truncate() - position;
                let forward = offset.dot(direction);
                let sideways = offset.perp_dot(direction).abs();
                (forward > 0.0 && forward <= TILE_SIZE + 2.0 && sideways < TILE_SIZE * 0.65)
                    .then_some((entity, offset.length_squared()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(entity, _)| entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn world(delta: Vec2) -> (App, Entity, Entity) {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(0.1));
        app.insert_resource(time)
            .add_systems(Update, movement_system);
        let station = app
            .world_mut()
            .spawn((
                Transform::from_xyz(16.0, 0.0, 0.0),
                Station {
                    kind: StationKind::Counter,
                },
                Collideable,
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                Player { index: 0 },
                Transform::default(),
                Velocity {
                    dx: delta.x,
                    dy: delta.y,
                    speed: PLAYER_SPEED,
                },
                Facing {
                    direction: Direction::Right,
                },
                DidMove(false),
                FacingStation::default(),
            ))
            .id();
        (app, player, station)
    }

    #[test]
    fn stationary_player_can_interact_and_turn_away() {
        let (mut app, player, station) = world(Vec2::ZERO);
        app.update();
        assert_eq!(
            app.world().get::<FacingStation>(player).unwrap().entity,
            Some(station)
        );
        app.world_mut().get_mut::<Facing>(player).unwrap().direction = Direction::Left;
        app.update();
        assert_eq!(
            app.world().get::<FacingStation>(player).unwrap().entity,
            None
        );
    }

    #[test]
    fn large_step_does_not_cross_counter() {
        let (mut app, player, _) = world(Vec2::new(1000.0, 0.0));
        app.update();
        assert_eq!(
            app.world().get::<Transform>(player).unwrap().translation.x,
            0.0
        );
    }
}
