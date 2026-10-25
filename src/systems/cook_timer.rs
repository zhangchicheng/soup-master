use crate::components::*;
use bevy::prelude::*;

/// Advances cook timers on soups in pots on stoves.
pub fn cook_timer_system(
    time: Res<Time>,
    stoves: Query<(&Station, &StationItem)>,
    pots: Query<&Pot>,
    mut soups: Query<&mut Soup>,
    mut level: ResMut<crate::resources::LevelState>,
) {
    let dt = time.delta_secs();

    for (station, station_item) in &stoves {
        if station.kind != StationKind::Stove {
            continue;
        }
        if let Some(item_entity) = station_item.entity {
            if let Ok(pot) = pots.get(item_entity) {
                if let Some(soup_entity) = pot.soup {
                    if let Ok(mut soup) = soups.get_mut(soup_entity) {
                        let was_burnt = soup.is_burnt();
                        soup.cook_time_elapsed += dt;
                        soup.refresh_cook_state();
                        if !was_burnt && soup.is_burnt() {
                            level.burnt_batches += 1;
                        }
                    }
                }
            }
        }
    }
}

/// Update cook_ratio on soups in pots on counters too (for display only, not cooking).
pub fn counter_soup_display_system(
    stations: Query<(&Station, &StationItem)>,
    pots: Query<&Pot>,
    mut soups: Query<&mut Soup>,
) {
    for (station, station_item) in &stations {
        if station.kind != StationKind::Counter {
            continue;
        }
        if let Some(item_entity) = station_item.entity {
            if let Ok(pot) = pots.get(item_entity) {
                if let Some(soup_entity) = pot.soup {
                    if let Ok(mut soup) = soups.get_mut(soup_entity) {
                        soup.refresh_cook_state();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_burnt_batch_is_counted_once() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(1));
        app.insert_resource(time)
            .insert_resource(crate::resources::LevelState::new(120.0, 1))
            .add_systems(Update, cook_timer_system);
        let mut soup = Soup::new(FoodType::Tomato);
        soup.cook_time_elapsed = 12.0;
        let soup = app.world_mut().spawn(soup).id();
        let pot = app.world_mut().spawn(Pot { soup: Some(soup) }).id();
        app.world_mut().spawn((
            Station {
                kind: StationKind::Stove,
            },
            StationItem { entity: Some(pot) },
        ));
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<crate::resources::LevelState>()
                .burnt_batches,
            1
        );
    }
}
