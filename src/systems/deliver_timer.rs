use crate::components::*;
use bevy::prelude::*;

/// Counts down delivery timers and distributes dirty dishes across return stations.
pub fn deliver_timer_system(
    time: Res<Time>,
    mut deliveries: Query<(&mut DeliveryState, Option<&LinkedReturnStation>)>,
    mut return_stacks: Query<(Entity, &Station, &mut DishStack)>,
) {
    let dt = time.delta_secs();

    for (mut del_state, linked) in &mut deliveries {
        let preferred = linked.map(|link| link.entity);
        del_state.dish_timers.retain_mut(|(dish, timer)| {
            *timer -= dt;
            if *timer > 0.0 {
                return true;
            }
            // Re-evaluate after every dish, including returns in the same frame.
            // Prefer an empty rack, then the least occupied; keep the authored
            // delivery link as the tie-breaker without using clean dish racks.
            let target = return_stacks
                .iter()
                .filter(|(_, station, _)| station.kind == StationKind::DishReturn)
                .min_by_key(|(entity, _, stack)| {
                    (
                        stack.dishes.len(),
                        Some(*entity) != preferred,
                        entity.to_bits(),
                    )
                })
                .map(|(entity, _, _)| entity);
            if let Some(target) = target {
                let (_, _, mut stack) = return_stacks.get_mut(target).unwrap();
                stack.dishes.push(*dish);
                false
            } else {
                // Keep the pending dish if no return station currently exists.
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(0.1));
        app.insert_resource(time)
            .add_systems(Update, deliver_timer_system);
        app
    }

    #[test]
    fn dirty_dishes_fill_empty_returns_before_stacking_and_reuse_freed_slots() {
        let mut app = app();
        let returns = (0..2)
            .map(|_| {
                app.world_mut()
                    .spawn((
                        Station {
                            kind: StationKind::DishReturn,
                        },
                        DishStack::default(),
                    ))
                    .id()
            })
            .collect::<Vec<_>>();
        let clean_rack = app
            .world_mut()
            .spawn((
                Station {
                    kind: StationKind::DishRack,
                },
                DishStack::default(),
            ))
            .id();
        let dishes = (0..4)
            .map(|_| app.world_mut().spawn_empty().id())
            .collect::<Vec<_>>();
        let delivery = app
            .world_mut()
            .spawn((
                DeliveryState {
                    dish_timers: vec![(dishes[0], 0.05), (dishes[1], 0.05)],
                    ..default()
                },
                LinkedReturnStation { entity: returns[0] },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<DishStack>(returns[0]).unwrap().dishes,
            vec![dishes[0]]
        );
        assert_eq!(
            app.world().get::<DishStack>(returns[1]).unwrap().dishes,
            vec![dishes[1]]
        );
        assert!(
            app.world()
                .get::<DeliveryState>(delivery)
                .unwrap()
                .dish_timers
                .is_empty()
        );
        // With all returns occupied, stack without losing a dish.
        app.world_mut()
            .get_mut::<DeliveryState>(delivery)
            .unwrap()
            .dish_timers
            .push((dishes[2], 0.0));
        app.update();
        assert_eq!(
            app.world().get::<DishStack>(returns[0]).unwrap().dishes,
            vec![dishes[0], dishes[2]]
        );
        // The next dish goes to the newly emptied return, not the linked one.
        app.world_mut()
            .get_mut::<DishStack>(returns[1])
            .unwrap()
            .dishes
            .pop();
        app.world_mut()
            .get_mut::<DeliveryState>(delivery)
            .unwrap()
            .dish_timers
            .push((dishes[3], 0.0));
        app.update();
        assert_eq!(
            app.world().get::<DishStack>(returns[1]).unwrap().dishes,
            vec![dishes[3]]
        );
        assert!(
            app.world()
                .get::<DishStack>(clean_rack)
                .unwrap()
                .dishes
                .is_empty()
        );
    }

    #[test]
    fn missing_return_keeps_dish_pending_until_a_return_is_available() {
        let mut app = app();
        let dish = app.world_mut().spawn_empty().id();
        let delivery = app
            .world_mut()
            .spawn(DeliveryState {
                dish_timers: vec![(dish, 0.0)],
                ..default()
            })
            .id();
        app.update();
        assert_eq!(
            app.world()
                .get::<DeliveryState>(delivery)
                .unwrap()
                .dish_timers
                .len(),
            1
        );
        let rack = app
            .world_mut()
            .spawn((
                Station {
                    kind: StationKind::DishReturn,
                },
                DishStack::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<DishStack>(rack).unwrap().dishes,
            vec![dish]
        );
        assert!(
            app.world()
                .get::<DeliveryState>(delivery)
                .unwrap()
                .dish_timers
                .is_empty()
        );
    }
}
