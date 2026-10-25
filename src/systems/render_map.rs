use crate::components::*;
use crate::constants::*;
use crate::level_loader::player_sprite_index;
use crate::resources::*;
use bevy::prelude::*;
use std::collections::HashSet;

use crate::tileset::*;

/// Update player sprite atlas index based on facing direction and walk animation.
pub fn update_player_sprites(
    time: Res<Time>,
    mut players: Query<(&Player, &Facing, &DidMove, &mut Sprite)>,
) {
    for (player, facing, did_move, mut sprite) in &mut players {
        let base_index = player_sprite_index(player.index, facing.direction);
        if let Some(ref mut atlas) = sprite.texture_atlas {
            if did_move.0 {
                let frame = (time.elapsed_secs() * 10.0) as usize % PLAYER_WALK_FRAMES;
                atlas.index = base_index + frame;
            } else {
                atlas.index = base_index;
            }
        }
    }
}

/// Animate animated tiles (stove fire, sink water, delivery station).
pub fn animate_tiles(time: Res<Time>, mut tiles: Query<(&Station, &mut Sprite)>) {
    let elapsed = time.elapsed_secs();

    for (station, mut sprite) in &mut tiles {
        if let Some(ref mut atlas) = sprite.texture_atlas {
            match station.kind {
                StationKind::Stove => {
                    let frame = (elapsed * 3.0) as usize % 2;
                    atlas.index = 1 * COLS + 5 + frame;
                }
                StationKind::Sink => {
                    let frame = (elapsed * 3.0) as usize % 4;
                    atlas.index = 1 * COLS + 7 + frame;
                }
                StationKind::Delivery => {
                    let frame = (elapsed * 3.0) as usize % 3;
                    atlas.index = 1 * COLS + frame;
                }
                _ => {}
            }
        }
    }
}

/// Play the four authored lid frames once after a successful ingredient pickup.
/// Runs before actions_cleanup_system clears DidDispenseItem, and pauses with play.
pub fn animate_crate_lids(
    time: Res<Time>,
    mut crates: Query<(
        &DidDispenseItem,
        &mut CrateLidAnimation,
        &CrateIngredient,
        &mut Sprite,
    )>,
) {
    const FRAME_SECONDS: f32 = 0.1;
    for (dispensed, mut animation, ingredient, mut sprite) in &mut crates {
        if ingredient.food_type == FoodType::Meat {
            continue; // Meat has no authored lid strip yet.
        }
        if dispensed.0 {
            animation.elapsed = Some(0.0);
        } else if let Some(elapsed) = &mut animation.elapsed {
            *elapsed += time.delta_secs();
        }
        let Some(elapsed) = animation.elapsed else {
            continue;
        };
        let frame = (elapsed / FRAME_SECONDS) as usize;
        if let Some(atlas) = &mut sprite.texture_atlas {
            if frame < ATLAS_CRATE_LID_FRAMES {
                atlas.index = crate_sprite_index(ingredient.food_type) + 1 + frame;
            } else {
                atlas.index = crate_sprite_index(ingredient.food_type);
                animation.elapsed = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Item rendering system
// ---------------------------------------------------------------------------

/// Renders items on stations and held by players by spawning/updating/despawning
/// `ItemVisual` sprite entities.
///
/// Matches the original Love2D rendering order:
/// 1. Sink overlays (filled sink quad)
/// 2. Items on stoves (pots with soup state + cooking indicators)
/// 3. Items on prep stations (prep overlay + item)
/// 4. Items on counters
/// 5. Top dish on dish racks / return stations
/// 6. Items held by players
pub fn render_items_system(
    mut commands: Commands,
    time: Res<Time>,
    tileset: Option<Res<TilesetInfo>>,
    stations: Query<
        (&Station, &StationItem, &Transform, Entity),
        (
            Without<ActivityOverlay>,
            Without<ItemVisual>,
            Without<OverlayVisual>,
        ),
    >,
    stack_stations: Query<
        (&Station, &DishStack, &Transform),
        (
            Without<ActivityOverlay>,
            Without<ItemVisual>,
            Without<OverlayVisual>,
        ),
    >,
    players: Query<
        (
            Entity,
            &HeldItem,
            &Transform,
            &Facing,
            &ActionState,
            &FacingStation,
        ),
        With<Player>,
    >,
    pots: Query<&Pot>,
    dishes: Query<&Dish>,
    ingredients: Query<&Ingredient>,
    soups: Query<&Soup>,
    mut existing_visuals: Query<
        (Entity, &ItemVisual, &mut Sprite, &mut Transform),
        (
            Without<Player>,
            Without<Station>,
            Without<OverlayVisual>,
            Without<ActivityOverlay>,
        ),
    >,
    mut existing_overlays: Query<
        (Entity, &OverlayVisual, &mut Sprite, &mut Transform),
        (
            Without<Player>,
            Without<Station>,
            Without<ItemVisual>,
            Without<ActivityOverlay>,
        ),
    >,
    mut existing_activity_overlays: Query<
        (Entity, &ActivityOverlay, &mut Sprite, &mut Transform),
        (
            Without<Player>,
            Without<Station>,
            Without<ItemVisual>,
            Without<OverlayVisual>,
        ),
    >,
    station_transforms: Query<
        &Transform,
        (
            With<Station>,
            Without<Player>,
            Without<ActivityOverlay>,
            Without<ItemVisual>,
            Without<OverlayVisual>,
        ),
    >,
) {
    let Some(tileset) = tileset else { return };

    let elapsed = time.elapsed_secs();

    // Track which item entities need visuals this frame
    // Each entry: (item_entity, world_x, world_y, atlas_index, z_layer)
    let mut desired_visuals: Vec<(Entity, f32, f32, usize, f32)> = Vec::new();
    // For stations that need multiple overlays (cooking progress etc.), we use
    // a synthetic entity approach via a separate list
    let mut extra_sprites: Vec<(f32, f32, usize, f32)> = Vec::new();
    // Desired activity overlays: (player_entity, x, y, atlas_index)
    let mut desired_activity: Vec<(Entity, f32, f32, usize)> = Vec::new();

    // --- Items on stations (via StationItem) ---
    for (station, station_item, transform, _station_entity) in &stations {
        let Some(item_entity) = station_item.entity else {
            continue;
        };
        let sx = transform.translation.x;
        let sy = transform.translation.y;

        match station.kind {
            StationKind::Sink => {
                // In Love2D: draw filledSinkQuad over the station tile
                // The item is a dish being washed; we show the filled sink overlay
                desired_visuals.push((item_entity, sx, sy, ATLAS_FILLED_SINK, Z_ITEM));
            }
            StationKind::Stove => {
                // Draw pot item on stove
                if let Some(atlas_idx) =
                    get_pot_atlas_index(&pots, &soups, item_entity, true, elapsed)
                {
                    desired_visuals.push((item_entity, sx, sy, atlas_idx, Z_ITEM));

                    // Draw cooking indicators above pot (if soup exists, not burnt, on station)
                    if let Ok(pot) = pots.get(item_entity) {
                        if let Some(soup_entity) = pot.soup {
                            if let Ok(soup) = soups.get(soup_entity) {
                                if !soup.is_burnt() {
                                    add_pot_contents(&mut extra_sprites, soup, sx, sy);

                                    // Keep the original half-tile position and draw over the ingredient markers.
                                    if let Some(progress_idx) =
                                        get_cooking_progress_index(soup, true, elapsed)
                                    {
                                        extra_sprites.push((
                                            sx,
                                            sy + TILE_SIZE * 0.5,
                                            progress_idx,
                                            Z_OVERLAY + 1.0,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            StationKind::Prep => {
                // In Love2D: draw prepReadyQuad over station + draw the ingredient
                extra_sprites.push((sx, sy, ATLAS_PREP_READY, Z_ITEM));
                if let Some(atlas_idx) = get_item_atlas_index(
                    &pots,
                    &dishes,
                    &ingredients,
                    &soups,
                    item_entity,
                    false,
                    elapsed,
                ) {
                    desired_visuals.push((item_entity, sx, sy, atlas_idx, Z_ITEM + 0.1));
                }
            }
            StationKind::Counter => {
                // Draw item on counter
                if let Some(atlas_idx) = get_item_atlas_index(
                    &pots,
                    &dishes,
                    &ingredients,
                    &soups,
                    item_entity,
                    false,
                    elapsed,
                ) {
                    desired_visuals.push((item_entity, sx, sy, atlas_idx, Z_ITEM));

                    // If it's a pot with soup, show cooking indicators
                    if let Ok(pot) = pots.get(item_entity) {
                        if let Some(soup_entity) = pot.soup {
                            if let Ok(soup) = soups.get(soup_entity) {
                                if !soup.is_burnt() {
                                    add_pot_contents(&mut extra_sprites, soup, sx, sy);

                                    if let Some(progress_idx) =
                                        get_cooking_progress_index(soup, false, elapsed)
                                    {
                                        extra_sprites.push((
                                            sx,
                                            sy + TILE_SIZE * 0.5,
                                            progress_idx,
                                            Z_OVERLAY + 1.0,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                // For other station types (Delivery, Crate, etc.) — no item rendering
            }
        }
    }

    // --- Dish racks and return stations (DishStack) ---
    for (station, dish_stack, transform) in &stack_stations {
        if !matches!(
            station.kind,
            StationKind::DishRack | StationKind::DishReturn
        ) {
            continue;
        }
        // Show the TOP of the stack — handle_dish_stack_transfer pops from the end
        if let Some(&top_dish) = dish_stack.dishes.last() {
            let sx = transform.translation.x;
            let sy = transform.translation.y;
            if let Some(atlas_idx) = get_dish_atlas_index(&dishes, top_dish) {
                desired_visuals.push((top_dish, sx, sy, atlas_idx, Z_ITEM));
            }
        }
    }

    // --- Items held by players + activity animations ---
    for (player_entity, held_item, transform, facing, action_state, facing_station) in &players {
        if let Some(item_entity) = held_item.entity {
            let px = transform.translation.x;
            // In Love2D: when facing up or right, held item is drawn at y - 4
            // In Bevy Y-up: that becomes y + 4
            let py = match facing.direction {
                Direction::Up | Direction::Right => transform.translation.y + 4.0,
                _ => transform.translation.y,
            };
            // Z-order: facing down/left = item in front of player; up/right = behind player
            let item_z = match facing.direction {
                Direction::Down | Direction::Left => Z_PLAYER + 0.1,
                Direction::Up | Direction::Right => Z_PLAYER - 0.1,
            };
            if let Some(atlas_idx) = get_item_atlas_index(
                &pots,
                &dishes,
                &ingredients,
                &soups,
                item_entity,
                false,
                elapsed,
            ) {
                desired_visuals.push((item_entity, px, py, atlas_idx, item_z));
            }
            if let Ok(pot) = pots.get(item_entity) {
                if let Some(soup_entity) = pot.soup {
                    if let Ok(soup) = soups.get(soup_entity) {
                        if !soup.is_burnt() {
                            add_pot_contents(&mut extra_sprites, soup, px, py);
                        }
                    }
                }
            }
        }

        // Activity animation overlay (chopping / washing) drawn at the facing station
        if let Some(activity) = &action_state.activity {
            if let Some(station_entity) = facing_station.entity {
                if let Ok(station_tf) = station_transforms.get(station_entity) {
                    let sx = station_tf.translation.x;
                    let sy = station_tf.translation.y;
                    let atlas_idx = match activity {
                        PlayerActivity::Prepping => {
                            let frame = (elapsed / 0.03) as usize % ATLAS_CHOPPING_FRAMES;
                            ATLAS_CHOPPING_BASE + frame
                        }
                        PlayerActivity::Washing => {
                            let frame = (elapsed / 0.3) as usize % ATLAS_WASHING_FRAMES;
                            ATLAS_WASHING_BASE + frame
                        }
                    };
                    desired_activity.push((player_entity, sx, sy, atlas_idx));
                }
            }
        }
    }

    // --- Sync visual entities ---
    // All updates use direct mutable query access (immediate, same frame).
    // Spawn/despawn via commands is only used when entities need to be created or destroyed.

    // 1. Update or despawn persistent activity overlays
    let mut updated_activities: HashSet<Entity> = HashSet::new();
    for (vis_entity, activity_overlay, mut sprite, mut atf) in &mut existing_activity_overlays {
        if let Some((_, x, y, atlas_idx)) = desired_activity
            .iter()
            .find(|(pe, ..)| *pe == activity_overlay.player_entity)
        {
            atf.translation = Vec3::new(*x, *y, Z_OVERLAY);
            if let Some(ref mut atlas) = sprite.texture_atlas {
                atlas.index = *atlas_idx;
            }
            updated_activities.insert(activity_overlay.player_entity);
        } else {
            commands.entity(vis_entity).despawn();
        }
    }

    // 2. Update existing item visuals or despawn stale ones
    let mut updated_items: HashSet<Entity> = HashSet::new();
    for (visual_entity, item_visual, mut sprite, mut tf) in &mut existing_visuals {
        if let Some((_, x, y, atlas_idx, z)) = desired_visuals
            .iter()
            .find(|(e, ..)| *e == item_visual.item_entity)
        {
            tf.translation = Vec3::new(*x, *y, *z);
            if let Some(ref mut atlas) = sprite.texture_atlas {
                atlas.index = *atlas_idx;
            }
            updated_items.insert(item_visual.item_entity);
        } else {
            commands.entity(visual_entity).despawn();
        }
    }

    // 3. Pool-update overlay visuals (cooking indicators, ingredient counters, prep marks)
    let extra_count = extra_sprites.len();
    let mut reused = 0;
    for (vis_entity, _overlay, mut sprite, mut tf) in &mut existing_overlays {
        if reused < extra_count {
            let (x, y, atlas_idx, z) = &extra_sprites[reused];
            tf.translation = Vec3::new(*x, *y, *z);
            if let Some(ref mut atlas) = sprite.texture_atlas {
                atlas.index = *atlas_idx;
            }
            reused += 1;
        } else {
            commands.entity(vis_entity).despawn();
        }
    }

    // 4. Spawn new item visuals for items that don't have one yet
    for (item_entity, x, y, atlas_idx, z) in &desired_visuals {
        if !updated_items.contains(item_entity) {
            commands.spawn((
                Sprite {
                    image: tileset.image.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: tileset.layout.clone(),
                        index: *atlas_idx,
                    }),
                    ..default()
                },
                Transform::from_xyz(*x, *y, *z),
                ItemVisual {
                    item_entity: *item_entity,
                },
                GameplayEntity,
            ));
        }
    }

    // 5. Spawn new persistent activity overlays
    for (player_entity, x, y, atlas_idx) in &desired_activity {
        if !updated_activities.contains(player_entity) {
            commands.spawn((
                Sprite {
                    image: tileset.image.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: tileset.layout.clone(),
                        index: *atlas_idx,
                    }),
                    ..default()
                },
                Transform::from_xyz(*x, *y, Z_OVERLAY),
                ActivityOverlay {
                    player_entity: *player_entity,
                },
                GameplayEntity,
            ));
        }
    }

    // 6. Spawn remaining overlay sprites that couldn't be pooled
    for i in reused..extra_count {
        let (x, y, atlas_idx, z) = &extra_sprites[i];
        commands.spawn((
            Sprite {
                image: tileset.image.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: tileset.layout.clone(),
                    index: *atlas_idx,
                }),
                ..default()
            },
            Transform::from_xyz(*x, *y, *z),
            OverlayVisual,
            GameplayEntity,
        ));
    }
}

// ---------------------------------------------------------------------------
// Helper functions to determine atlas indices
// ---------------------------------------------------------------------------

/// Get the atlas index for a pot based on its soup state.
fn get_pot_atlas_index(
    pots: &Query<&Pot>,
    soups: &Query<&Soup>,
    pot_entity: Entity,
    on_stove: bool,
    elapsed: f32,
) -> Option<usize> {
    let pot = pots.get(pot_entity).ok()?;
    if let Some(soup_entity) = pot.soup {
        if let Ok(soup) = soups.get(soup_entity) {
            if soup.is_burnt() {
                Some(ATLAS_BURNT_POT)
            } else if on_stove && soup.will_burn_soon() {
                let frame = (elapsed * 3.0) as usize % ATLAS_WARNING_POT_FRAMES;
                Some(ATLAS_WARNING_POT_BASE + frame)
            } else if on_stove {
                // Boiling animation: 3 frames
                let frame = (elapsed * 3.0) as usize % 3;
                Some(ATLAS_BOILING_BASE + frame)
            } else {
                Some(ATLAS_FILLED_POT)
            }
        } else {
            Some(ATLAS_EMPTY_POT)
        }
    } else {
        Some(ATLAS_EMPTY_POT)
    }
}

/// Get the atlas index for a dish based on its state.
fn get_dish_atlas_index(dishes: &Query<&Dish>, dish_entity: Entity) -> Option<usize> {
    let dish = dishes.get(dish_entity).ok()?;
    if dish.soup.is_some() {
        Some(ATLAS_FILLED_BOWL)
    } else if dish.clean_ratio < 1.0 {
        Some(ATLAS_DIRTY_BOWL)
    } else {
        Some(ATLAS_EMPTY_BOWL)
    }
}

/// Get the atlas index for an ingredient based on type and prep state.
fn get_ingredient_atlas_index(ingredients: &Query<&Ingredient>, entity: Entity) -> Option<usize> {
    let ingredient = ingredients.get(entity).ok()?;
    Some(ingredient_sprite_index(
        ingredient.food_type,
        ingredient.prep_ratio >= 1.0,
    ))
}

/// Get the atlas index for any item (pot, dish, or ingredient).
fn get_item_atlas_index(
    pots: &Query<&Pot>,
    dishes: &Query<&Dish>,
    ingredients: &Query<&Ingredient>,
    soups: &Query<&Soup>,
    item_entity: Entity,
    on_stove: bool,
    elapsed: f32,
) -> Option<usize> {
    // Try pot
    if pots.get(item_entity).is_ok() {
        return get_pot_atlas_index(pots, soups, item_entity, on_stove, elapsed);
    }
    // Try dish
    if dishes.get(item_entity).is_ok() {
        return get_dish_atlas_index(dishes, item_entity);
    }
    // Try ingredient
    if ingredients.get(item_entity).is_ok() {
        return get_ingredient_atlas_index(ingredients, item_entity);
    }
    None
}

fn add_pot_contents(overlays: &mut Vec<(f32, f32, usize, f32)>, soup: &Soup, x: f32, y: f32) {
    // Match the original 16x16 counter: first lower-left, then top, then lower-right.
    // Food icons are centered on the top slot in the atlas; shift them to each slot.
    const OFFSETS: [(f32, f32); 3] = [(-4.0, -5.0), (0.0, 0.0), (4.0, -5.0)];
    for (food, (dx, dy)) in soup.ingredients.iter().zip(OFFSETS) {
        overlays.push((
            x + dx,
            y + TILE_SIZE + dy,
            ingredient_icon_index(*food),
            Z_OVERLAY,
        ));
    }
    if let Some(empty_slots) = get_ingredient_counter_index(soup) {
        overlays.push((x, y + TILE_SIZE, empty_slots, Z_OVERLAY));
    }
}

/// Show how many ingredient slots are still empty.
fn get_ingredient_counter_index(soup: &Soup) -> Option<usize> {
    match soup.max_ingredients.saturating_sub(soup.ingredients.len()) {
        2 => Some(ATLAS_MISSING_TWO),
        1 => Some(ATLAS_MISSING_ONE),
        _ => None, // 3 = max, no counter shown
    }
}

/// Get cooking progress overlay index.
fn get_cooking_progress_index(soup: &Soup, on_stove: bool, elapsed: f32) -> Option<usize> {
    if on_stove && soup.will_burn_soon() {
        // Burn warning animation: 2 frames
        let frame = (elapsed * 3.0) as usize % 2;
        Some(ATLAS_BURN_WARN_BASE + frame)
    } else if soup.is_cooked() {
        Some(ATLAS_COOK_DONE)
    } else if soup.cook_ratio >= 0.75 {
        Some(ATLAS_COOK_75)
    } else if soup.cook_ratio >= 0.50 {
        Some(ATLAS_COOK_50)
    } else if soup.cook_ratio >= 0.25 {
        Some(ATLAS_COOK_25)
    } else {
        None
    }
}

#[cfg(test)]
mod crate_lid_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn pickup_plays_four_frames_then_restores_only_its_crate_and_can_restart() {
        for food in [
            FoodType::Tomato,
            FoodType::Cabbage,
            FoodType::Fish,
            FoodType::Eggplant,
            FoodType::LotusRoot,
        ] {
            check_crate_animation(food);
        }
    }

    fn check_crate_animation(food: FoodType) {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_systems(Update, animate_crate_lids);
        let spawn_crate = |world: &mut World| {
            world
                .spawn((
                    DidDispenseItem(false),
                    CrateLidAnimation::default(),
                    CrateIngredient { food_type: food },
                    Sprite {
                        texture_atlas: Some(TextureAtlas {
                            layout: Handle::default(),
                            index: crate_sprite_index(food),
                        }),
                        ..default()
                    },
                ))
                .id()
        };
        let active = spawn_crate(app.world_mut());
        let other = spawn_crate(app.world_mut());
        let index = |app: &App, entity| {
            app.world()
                .get::<Sprite>(entity)
                .unwrap()
                .texture_atlas
                .as_ref()
                .unwrap()
                .index
        };
        let step = |app: &mut App, seconds| {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(seconds));
            app.update();
        };
        step(&mut app, 0.1);
        assert_eq!(index(&app, active), crate_sprite_index(food));
        app.world_mut()
            .get_mut::<DidDispenseItem>(active)
            .unwrap()
            .0 = true;
        step(&mut app, 0.1);
        app.world_mut()
            .get_mut::<DidDispenseItem>(active)
            .unwrap()
            .0 = false;
        assert_eq!(index(&app, active), crate_sprite_index(food) + 1);
        for frame in 1..ATLAS_CRATE_LID_FRAMES {
            step(&mut app, 0.101);
            assert_eq!(index(&app, active), crate_sprite_index(food) + 1 + frame);
            assert_eq!(index(&app, other), crate_sprite_index(food));
        }
        step(&mut app, 0.101);
        assert_eq!(index(&app, active), crate_sprite_index(food));
        assert!(
            app.world()
                .get::<CrateLidAnimation>(active)
                .unwrap()
                .elapsed
                .is_none()
        );
        app.world_mut()
            .get_mut::<DidDispenseItem>(active)
            .unwrap()
            .0 = true;
        step(&mut app, 0.1);
        assert_eq!(index(&app, active), crate_sprite_index(food) + 1);
        // A second successful pickup while animating restarts at the first frame.
        step(&mut app, 0.2);
        assert_eq!(index(&app, active), crate_sprite_index(food) + 1);
    }
}

#[cfg(test)]
mod player_animation_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn both_players_cycle_four_frames_in_each_direction_and_stop_on_first_frame() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_systems(Update, update_player_sprites);
        let players: Vec<_> = (0..2)
            .map(|index| {
                app.world_mut()
                    .spawn((
                        Player { index },
                        Facing::default(),
                        DidMove(true),
                        Sprite {
                            texture_atlas: Some(TextureAtlas {
                                layout: Handle::default(),
                                index: 0,
                            }),
                            ..default()
                        },
                    ))
                    .id()
            })
            .collect();
        for (direction, row) in [
            (Direction::Down, 11),
            (Direction::Right, 12),
            (Direction::Up, 13),
            (Direction::Left, 14),
        ] {
            *app.world_mut().resource_mut::<Time>() = Time::default();
            for &player in &players {
                app.world_mut().get_mut::<Facing>(player).unwrap().direction = direction;
                app.world_mut().get_mut::<DidMove>(player).unwrap().0 = true;
            }
            for tick in 0..5 {
                if tick > 0 {
                    app.world_mut()
                        .resource_mut::<Time>()
                        .advance_by(Duration::from_millis(101));
                }
                app.update();
                for (index, &player) in players.iter().enumerate() {
                    let atlas = app
                        .world()
                        .get::<Sprite>(player)
                        .unwrap()
                        .texture_atlas
                        .as_ref()
                        .unwrap();
                    assert_eq!(atlas.index, row * 11 + index * 4 + tick % 4);
                }
            }
            for &player in &players {
                app.world_mut().get_mut::<DidMove>(player).unwrap().0 = false;
            }
            app.update();
            for (index, &player) in players.iter().enumerate() {
                let atlas = app
                    .world()
                    .get::<Sprite>(player)
                    .unwrap()
                    .texture_atlas
                    .as_ref()
                    .unwrap();
                assert_eq!(atlas.index, row * 11 + index * 4);
            }
        }
    }
}

#[cfg(test)]
mod pot_marker_tests {
    use super::*;
    #[test]
    fn contents_show_each_ingredient_and_missing_slots_disappear_when_full() {
        let mut soup = Soup::new(FoodType::Tomato);
        assert_eq!(get_ingredient_counter_index(&soup), Some(6 * 11 + 8));
        let mut partial = Vec::new();
        add_pot_contents(&mut partial, &soup, 32.0, 48.0);
        assert_eq!(
            partial,
            vec![
                (28.0, 59.0, 7 * 11 + 5, Z_OVERLAY),
                (32.0, 64.0, 6 * 11 + 8, Z_OVERLAY),
            ]
        );
        soup.ingredients.push(FoodType::Fish);
        assert_eq!(get_ingredient_counter_index(&soup), Some(6 * 11 + 9));
        soup.ingredients.push(FoodType::Tomato);
        assert_eq!(get_ingredient_counter_index(&soup), None);
        let mut icons = Vec::new();
        add_pot_contents(&mut icons, &soup, 32.0, 48.0);
        assert_eq!(
            icons.iter().map(|icon| icon.2).collect::<Vec<_>>(),
            [7 * 11 + 5, 7 * 11 + 8, 7 * 11 + 5]
        );
        assert_eq!(
            icons
                .iter()
                .map(|icon| (icon.0, icon.1))
                .collect::<Vec<_>>(),
            [(28.0, 59.0), (32.0, 64.0), (36.0, 59.0)]
        );
    }
}

#[cfg(test)]
mod warning_pot_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn pot_frame(
        In((entity, on_stove, elapsed)): In<(Entity, bool, f32)>,
        pots: Query<&Pot>,
        soups: Query<&Soup>,
    ) -> Option<usize> {
        get_pot_atlas_index(&pots, &soups, entity, on_stove, elapsed)
    }

    #[test]
    fn red_pot_loops_four_frames_only_during_stove_warning() {
        let mut world = World::new();
        let mut soup = Soup::new(FoodType::Tomato);
        soup.state = CookState::WillBurn;
        let soup_entity = world.spawn(soup).id();
        let pot = world
            .spawn(Pot {
                soup: Some(soup_entity),
            })
            .id();
        for (elapsed, expected) in [
            (0.0, 183),
            (0.34, 184),
            (0.67, 185),
            (1.0, 186),
            (1.34, 183),
        ] {
            assert_eq!(
                world
                    .run_system_once_with(pot_frame, (pot, true, elapsed))
                    .unwrap(),
                Some(expected)
            );
        }
        assert_eq!(
            world
                .run_system_once_with(pot_frame, (pot, false, 1.0))
                .unwrap(),
            Some(ATLAS_FILLED_POT)
        );
        world.get_mut::<Soup>(soup_entity).unwrap().state = CookState::Cooked;
        assert_eq!(
            world
                .run_system_once_with(pot_frame, (pot, true, 0.34))
                .unwrap(),
            Some(ATLAS_BOILING_BASE + 1)
        );
        world.get_mut::<Soup>(soup_entity).unwrap().state = CookState::Burnt;
        assert_eq!(
            world
                .run_system_once_with(pot_frame, (pot, true, 0.34))
                .unwrap(),
            Some(ATLAS_BURNT_POT)
        );
    }
}
