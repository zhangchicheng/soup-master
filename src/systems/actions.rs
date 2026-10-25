use crate::components::*;
use crate::constants::*;
use crate::events::*;
use crate::recipes::Orders;
use bevy::prelude::*;

/// Main actions system: handles station interactions (wash, prep, transfer items).
pub fn actions_system(
    mut commands: Commands,
    time: Res<Time>,
    mut players: Query<(&mut ActionState, &mut HeldItem, &FacingStation, Entity), With<Player>>,
    mut stations: Query<(
        &Station,
        &mut StationItem,
        Option<&LinkedReturnStation>,
        Option<&CrateIngredient>,
        Option<&mut DeliveryState>,
        Option<&mut DishStack>,
        Option<&mut DidDispenseItem>,
    )>,
    mut ingredients: Query<&mut Ingredient>,
    mut dishes: Query<&mut Dish>,
    mut pots: Query<&mut Pot>,
    mut soups: Query<&mut Soup>,
    item_kinds: Query<&ItemKind>,
    mut ev_delivered: MessageWriter<DishDelivered>,
    mut ev_clean: MessageWriter<CleanDishReturned>,
    mut ev_illegal: MessageWriter<IllegalAction>,
    mut ev_dispensed: MessageWriter<ItemDispensed>,
    mut orders: ResMut<Orders>,
) {
    let dt = time.delta_secs();

    for (mut action_state, mut held_item, facing_station, _player_entity) in &mut players {
        action_state.cooldown -= dt;
        if action_state.cooldown > 0.0 {
            continue;
        }

        let station_entity = match facing_station.entity {
            Some(e) => e,
            None => {
                set_activity(&mut action_state, None);
                continue;
            }
        };

        if action_state.action {
            do_action(
                dt,
                &mut action_state,
                &mut held_item,
                station_entity,
                &mut stations,
                &mut ingredients,
                &mut dishes,
                &mut ev_clean,
            );
        } else if action_state.transfer {
            set_activity(&mut action_state, None);
            // If facing an empty sink with no item in hand, redirect pickup to
            // the linked DishRack so the player doesn't need to walk there.
            let target_entity = if held_item.entity.is_none() {
                if let Ok((station, station_item, linked, _, _, _, _)) =
                    stations.get(station_entity)
                {
                    if station.kind == StationKind::Sink && station_item.entity.is_none() {
                        linked.map(|l| l.entity).unwrap_or(station_entity)
                    } else {
                        station_entity
                    }
                } else {
                    station_entity
                }
            } else {
                station_entity
            };

            do_transfer(
                &mut commands,
                &mut action_state,
                &mut held_item,
                target_entity,
                &mut stations,
                &mut ingredients,
                &mut dishes,
                &mut pots,
                &mut soups,
                &item_kinds,
                &mut ev_delivered,
                &mut ev_illegal,
                &mut ev_dispensed,
                &mut orders,
            );
        } else {
            set_activity(&mut action_state, None);
        }
    }
}

fn set_activity(action_state: &mut ActionState, activity: Option<PlayerActivity>) {
    action_state.did_change_state = action_state.activity != activity;
    action_state.activity = activity;
}

/// Handle held-action: washing dishes at sink, prepping ingredients.
fn do_action(
    dt: f32,
    action_state: &mut ActionState,
    held_item: &HeldItem,
    station_entity: Entity,
    stations: &mut Query<(
        &Station,
        &mut StationItem,
        Option<&LinkedReturnStation>,
        Option<&CrateIngredient>,
        Option<&mut DeliveryState>,
        Option<&mut DishStack>,
        Option<&mut DidDispenseItem>,
    )>,
    ingredients: &mut Query<&mut Ingredient>,
    dishes: &mut Query<&mut Dish>,
    ev_clean: &mut MessageWriter<CleanDishReturned>,
) {
    set_activity(action_state, None);
    // Player must not be holding anything to do an action at sink/prep
    if held_item.entity.is_some() {
        return;
    }

    // First query: extract what we need, then drop all borrows before re-querying.
    let (station_kind, item_entity_opt, linked_entity_opt) = {
        let Ok((station, station_item, linked, _, _, _, _)) = stations.get_mut(station_entity)
        else {
            return;
        };
        let linked_ent = linked.map(|l| l.entity);
        (station.kind, station_item.entity, linked_ent)
    };
    // All query borrows are now dropped.

    match station_kind {
        StationKind::Sink => {
            if let Some(item_entity) = item_entity_opt {
                if let Ok(mut dish) = dishes.get_mut(item_entity) {
                    if dish.clean_ratio < 1.0 {
                        set_activity(action_state, Some(PlayerActivity::Washing));
                        dish.clean_ratio = (dish.clean_ratio + dt / ACTION_TIME).min(1.0);
                        if dish.clean_ratio >= 1.0 {
                            // Dish is clean — transfer to return station (dish rack)
                            if let Some(return_entity) = linked_entity_opt {
                                drop(dish);
                                // Clear the sink
                                if let Ok((_, mut sink_item, _, _, _, _, _)) =
                                    stations.get_mut(station_entity)
                                {
                                    sink_item.entity = None;
                                }
                                // Add to dish rack
                                if let Ok((_, _, _, _, _, mut target_stack, _)) =
                                    stations.get_mut(return_entity)
                                {
                                    if let Some(ref mut stack) = target_stack.as_mut() {
                                        stack.dishes.push(item_entity);
                                    }
                                }
                                ev_clean.write(CleanDishReturned);
                                set_activity(action_state, None);
                                return;
                            }
                            set_activity(action_state, None);
                        }
                    }
                }
            }
        }
        StationKind::Prep => {
            if let Some(item_entity) = item_entity_opt {
                if let Ok(mut ingredient) = ingredients.get_mut(item_entity) {
                    if ingredient.prep_ratio < 1.0 {
                        set_activity(action_state, Some(PlayerActivity::Prepping));
                        ingredient.prep_ratio = (ingredient.prep_ratio + dt / ACTION_TIME).min(1.0);
                    } else {
                        set_activity(action_state, None);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Handle transfer: pick up / put down items between player and station.
/// Dispatches to per-station handler functions.
#[allow(clippy::too_many_arguments)]
fn do_transfer(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    station_entity: Entity,
    stations: &mut Query<(
        &Station,
        &mut StationItem,
        Option<&LinkedReturnStation>,
        Option<&CrateIngredient>,
        Option<&mut DeliveryState>,
        Option<&mut DishStack>,
        Option<&mut DidDispenseItem>,
    )>,
    ingredients: &mut Query<&mut Ingredient>,
    dishes: &mut Query<&mut Dish>,
    pots: &mut Query<&mut Pot>,
    soups: &mut Query<&mut Soup>,
    item_kinds: &Query<&ItemKind>,
    ev_delivered: &mut MessageWriter<DishDelivered>,
    ev_illegal: &mut MessageWriter<IllegalAction>,
    ev_dispensed: &mut MessageWriter<ItemDispensed>,
    orders: &mut Orders,
) {
    let Ok((
        station,
        mut station_item,
        _linked,
        crate_ingr,
        delivery_state,
        dish_stack,
        did_dispense,
    )) = stations.get_mut(station_entity)
    else {
        return;
    };

    match station.kind {
        StationKind::Counter => handle_counter_transfer(
            commands,
            action_state,
            held_item,
            &mut station_item,
            ingredients,
            dishes,
            pots,
            soups,
            item_kinds,
            ev_illegal,
        ),
        StationKind::Stove => handle_stove_transfer(
            commands,
            action_state,
            held_item,
            &mut station_item,
            ingredients,
            dishes,
            pots,
            soups,
            item_kinds,
            ev_illegal,
        ),
        StationKind::Sink => handle_sink_transfer(
            commands,
            action_state,
            held_item,
            &mut station_item,
            dishes,
            pots,
            soups,
            item_kinds,
        ),
        StationKind::Prep => handle_prep_transfer(
            commands,
            action_state,
            held_item,
            &mut station_item,
            ingredients,
            dishes,
            pots,
            soups,
            item_kinds,
            ev_illegal,
        ),
        StationKind::Delivery => handle_delivery_transfer(
            commands,
            action_state,
            held_item,
            delivery_state,
            dishes,
            item_kinds,
            ev_delivered,
            ev_illegal,
            soups,
            orders,
        ),
        StationKind::DishRack | StationKind::DishReturn => {
            handle_dish_stack_transfer(action_state, held_item, dish_stack)
        }
        StationKind::Crate => handle_crate_transfer(
            commands,
            action_state,
            held_item,
            crate_ingr,
            did_dispense,
            ev_dispensed,
            station_entity,
        ),
    }
}

// ---------------------------------------------------------------------------
// Per-station transfer handlers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn handle_counter_transfer(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    station_item: &mut StationItem,
    ingredients: &mut Query<&mut Ingredient>,
    dishes: &mut Query<&mut Dish>,
    pots: &mut Query<&mut Pot>,
    soups: &mut Query<&mut Soup>,
    item_kinds: &Query<&ItemKind>,
    ev_illegal: &mut MessageWriter<IllegalAction>,
) {
    if let Some(player_item_entity) = held_item.entity {
        let player_item_kind = item_kinds
            .get(player_item_entity)
            .copied()
            .unwrap_or(ItemKind::Ingredient);
        if station_item.entity.is_none() {
            transfer_to_station(station_item, held_item, action_state);
        } else {
            try_item_interaction(
                commands,
                action_state,
                held_item,
                player_item_entity,
                player_item_kind,
                station_item,
                ingredients,
                dishes,
                pots,
                soups,
                item_kinds,
                ev_illegal,
            );
        }
    } else if station_item.entity.is_some() {
        transfer_from_station(station_item, held_item, action_state);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_stove_transfer(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    station_item: &mut StationItem,
    ingredients: &mut Query<&mut Ingredient>,
    dishes: &mut Query<&mut Dish>,
    pots: &mut Query<&mut Pot>,
    soups: &mut Query<&mut Soup>,
    item_kinds: &Query<&ItemKind>,
    ev_illegal: &mut MessageWriter<IllegalAction>,
) {
    if let Some(player_item_entity) = held_item.entity {
        let player_item_kind = item_kinds
            .get(player_item_entity)
            .copied()
            .unwrap_or(ItemKind::Ingredient);
        if station_item.entity.is_none() && player_item_kind == ItemKind::Pot {
            transfer_to_station(station_item, held_item, action_state);
        } else {
            try_item_interaction(
                commands,
                action_state,
                held_item,
                player_item_entity,
                player_item_kind,
                station_item,
                ingredients,
                dishes,
                pots,
                soups,
                item_kinds,
                ev_illegal,
            );
        }
    } else if station_item.entity.is_some() {
        transfer_from_station(station_item, held_item, action_state);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_sink_transfer(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    station_item: &mut StationItem,
    dishes: &mut Query<&mut Dish>,
    pots: &mut Query<&mut Pot>,
    soups: &mut Query<&mut Soup>,
    item_kinds: &Query<&ItemKind>,
) {
    if let Some(player_item_entity) = held_item.entity {
        let player_item_kind = item_kinds
            .get(player_item_entity)
            .copied()
            .unwrap_or(ItemKind::Ingredient);
        if player_item_kind == ItemKind::Dish && station_item.entity.is_none() {
            // Put dirty dish in sink (only if it isn't still holding soup —
            // a freshly plated dish is "dirty" but must be delivered, not washed)
            if let Ok(dish) = dishes.get(player_item_entity) {
                if dish.clean_ratio < 1.0 && dish.soup.is_none() {
                    transfer_to_station(station_item, held_item, action_state);
                }
            }
        } else if player_item_kind == ItemKind::Pot {
            // Wash burnt soup out of pot
            if let Ok(mut pot) = pots.get_mut(player_item_entity) {
                if let Some(soup_entity) = pot.soup {
                    if let Ok(soup) = soups.get(soup_entity) {
                        if soup.is_burnt() {
                            pot.soup = None;
                            commands.entity(soup_entity).despawn();
                        }
                    }
                }
            }
        }
    } else {
        // Pick up clean dish from sink
        if let Some(item_entity) = station_item.entity {
            if let Ok(dish) = dishes.get(item_entity) {
                if dish.clean_ratio >= 1.0 {
                    transfer_from_station(station_item, held_item, action_state);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_prep_transfer(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    station_item: &mut StationItem,
    ingredients: &mut Query<&mut Ingredient>,
    dishes: &mut Query<&mut Dish>,
    pots: &mut Query<&mut Pot>,
    soups: &mut Query<&mut Soup>,
    item_kinds: &Query<&ItemKind>,
    ev_illegal: &mut MessageWriter<IllegalAction>,
) {
    if let Some(player_item_entity) = held_item.entity {
        let player_item_kind = item_kinds
            .get(player_item_entity)
            .copied()
            .unwrap_or(ItemKind::Ingredient);
        if station_item.entity.is_none() && player_item_kind == ItemKind::Ingredient {
            transfer_to_station(station_item, held_item, action_state);
        } else {
            try_item_interaction(
                commands,
                action_state,
                held_item,
                player_item_entity,
                player_item_kind,
                station_item,
                ingredients,
                dishes,
                pots,
                soups,
                item_kinds,
                ev_illegal,
            );
        }
    } else {
        // Pick up fully prepped ingredient
        if let Some(item_entity) = station_item.entity {
            if let Ok(ingredient) = ingredients.get(item_entity) {
                if ingredient.prep_ratio >= 1.0 {
                    transfer_from_station(station_item, held_item, action_state);
                }
            }
        }
    }
}

fn handle_delivery_transfer(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    delivery_state: Option<Mut<DeliveryState>>,
    dishes: &mut Query<&mut Dish>,
    item_kinds: &Query<&ItemKind>,
    ev_delivered: &mut MessageWriter<DishDelivered>,
    ev_illegal: &mut MessageWriter<IllegalAction>,
    soups: &Query<&mut Soup>,
    orders: &mut Orders,
) {
    let Some(player_item_entity) = held_item.entity else {
        return;
    };
    let player_item_kind = item_kinds
        .get(player_item_entity)
        .copied()
        .unwrap_or(ItemKind::Ingredient);
    if player_item_kind != ItemKind::Dish {
        return;
    }

    if let Ok(mut dish) = dishes.get_mut(player_item_entity) {
        if let Some(soup_entity) = dish.soup {
            let matched = soups
                .get(soup_entity)
                .ok()
                .and_then(|soup| orders.matching_index(soup));
            let Some(matched) = matched else {
                action_state.did_illegal_action = true;
                ev_illegal.write(IllegalAction);
                return;
            };
            // The soup is consumed by the customer — despawn its entity so it
            // doesn't leak (nothing references it after delivery).
            commands.entity(soup_entity).despawn();
            dish.soup = None;
            if let Some(mut del_state) = delivery_state {
                del_state
                    .dish_timers
                    .push((player_item_entity, EATING_TIME));
                del_state.total_meals_delivered += 1;
                del_state.did_change_total_meals = true;
            }
            held_item.entity = None;
            action_state.cooldown = action_state.cooldown_max;
            orders.complete(matched);
            ev_delivered.write(DishDelivered);
        } else {
            action_state.did_illegal_action = true;
            ev_illegal.write(IllegalAction);
        }
    }
}

fn handle_dish_stack_transfer(
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    dish_stack: Option<Mut<DishStack>>,
) {
    if held_item.entity.is_some() {
        return;
    }
    if let Some(mut stack) = dish_stack {
        if let Some(dish_entity) = stack.dishes.pop() {
            held_item.entity = Some(dish_entity);
            action_state.cooldown = action_state.cooldown_max;
        }
    }
}

fn handle_crate_transfer(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    crate_ingr: Option<&CrateIngredient>,
    did_dispense: Option<Mut<DidDispenseItem>>,
    ev_dispensed: &mut MessageWriter<ItemDispensed>,
    station_entity: Entity,
) {
    if held_item.entity.is_some() {
        return;
    }
    if let Some(crate_info) = crate_ingr {
        let ingredient_entity = commands
            .spawn((
                Ingredient {
                    food_type: crate_info.food_type,
                    prep_ratio: 0.0,
                },
                ItemKind::Ingredient,
                GameplayEntity,
            ))
            .id();
        held_item.entity = Some(ingredient_entity);
        action_state.cooldown = action_state.cooldown_max;
        if let Some(mut dispense_flag) = did_dispense {
            dispense_flag.0 = true;
        }
        ev_dispensed.write(ItemDispensed { station_entity });
    }
}

/// Try to interact with the item sitting on a station:
/// ingredient→pot (add to soup), dish→pot (plate soup), pot→dish (plate soup).
#[allow(clippy::too_many_arguments)]
fn try_item_interaction(
    commands: &mut Commands,
    action_state: &mut ActionState,
    held_item: &mut HeldItem,
    player_item_entity: Entity,
    player_item_kind: ItemKind,
    station_item: &mut StationItem,
    ingredients: &mut Query<&mut Ingredient>,
    dishes: &mut Query<&mut Dish>,
    pots: &mut Query<&mut Pot>,
    soups: &mut Query<&mut Soup>,
    item_kinds: &Query<&ItemKind>,
    ev_illegal: &mut MessageWriter<IllegalAction>,
) {
    let Some(station_item_entity) = station_item.entity else {
        return;
    };
    let station_item_kind = item_kinds.get(station_item_entity).copied();

    // Player holding ingredient → add to pot on station
    if player_item_kind == ItemKind::Ingredient {
        if let Ok(ItemKind::Pot) = station_item_kind {
            if let Ok(ingredient) = ingredients.get(player_item_entity) {
                if ingredient.prep_ratio >= 1.0 {
                    if let Ok(mut pot) = pots.get_mut(station_item_entity) {
                        if let Some(soup_entity) = pot.soup {
                            if let Ok(mut soup) = soups.get_mut(soup_entity) {
                                if soup.ingredients.len() < soup.max_ingredients && !soup.is_burnt()
                                {
                                    soup.ingredients.push(ingredient.food_type);
                                    soup.refresh_cook_state();
                                    commands.entity(player_item_entity).despawn();
                                    held_item.entity = None;
                                    action_state.cooldown = action_state.cooldown_max;
                                } else {
                                    action_state.did_illegal_action = true;
                                    ev_illegal.write(IllegalAction);
                                }
                            }
                        } else {
                            // Start a new soup
                            let food = ingredient.food_type;
                            let soup_entity =
                                commands.spawn((Soup::new(food), GameplayEntity)).id();
                            pot.soup = Some(soup_entity);
                            commands.entity(player_item_entity).despawn();
                            held_item.entity = None;
                            action_state.cooldown = action_state.cooldown_max;
                        }
                    }
                }
            }
        }
    }
    // Player holding clean dish → plate soup from pot on station
    else if player_item_kind == ItemKind::Dish {
        if let Ok(ItemKind::Pot) = station_item_kind {
            if let Ok(mut dish) = dishes.get_mut(player_item_entity) {
                if dish.soup.is_none() && dish.clean_ratio >= 1.0 {
                    if let Ok(mut pot) = pots.get_mut(station_item_entity) {
                        if let Some(soup_entity) = pot.soup {
                            if let Ok(soup) = soups.get(soup_entity) {
                                if soup.is_prepared() {
                                    dish.soup = Some(soup_entity);
                                    pot.soup = None;
                                    dish.clean_ratio = 0.0;
                                    action_state.cooldown = action_state.cooldown_max;
                                } else {
                                    action_state.did_illegal_action = true;
                                    ev_illegal.write(IllegalAction);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Player holding pot with soup → plate into dish on station
    else if player_item_kind == ItemKind::Pot {
        if let Ok(ItemKind::Dish) = station_item_kind {
            if let Ok(mut pot) = pots.get_mut(player_item_entity) {
                if let Some(soup_entity) = pot.soup {
                    if let Ok(soup) = soups.get(soup_entity) {
                        if soup.is_prepared() {
                            if let Ok(mut dish) = dishes.get_mut(station_item_entity) {
                                if dish.clean_ratio >= 1.0 {
                                    dish.soup = Some(soup_entity);
                                    pot.soup = None;
                                    dish.clean_ratio = 0.0;
                                    action_state.cooldown = action_state.cooldown_max;
                                }
                            }
                        } else {
                            action_state.did_illegal_action = true;
                            ev_illegal.write(IllegalAction);
                        }
                    }
                }
            }
        }
    }
}

fn transfer_to_station(
    station_item: &mut StationItem,
    held_item: &mut HeldItem,
    action_state: &mut ActionState,
) {
    station_item.entity = held_item.entity;
    held_item.entity = None;
    action_state.cooldown = action_state.cooldown_max;
}

fn transfer_from_station(
    station_item: &mut StationItem,
    held_item: &mut HeldItem,
    action_state: &mut ActionState,
) {
    held_item.entity = station_item.entity;
    station_item.entity = None;
    action_state.cooldown = action_state.cooldown_max;
}

/// Reset per-frame flags on stations.
pub fn actions_cleanup_system(
    mut crates: Query<&mut DidDispenseItem>,
    mut deliveries: Query<&mut DeliveryState>,
) {
    for mut did in &mut crates {
        did.0 = false;
    }
    for mut del in &mut deliveries {
        del.did_change_total_meals = false;
    }
}

#[cfg(test)]
mod random_order_tests {
    use super::*;
    #[test]
    fn wrong_order_is_retained_and_matching_mixed_soup_is_delivered_once() {
        let mut app = App::new();
        let mut orders = Orders::new(vec![
            FoodType::Tomato,
            FoodType::Cabbage,
            FoodType::Fish,
            FoodType::Eggplant,
            FoodType::LotusRoot,
        ]);
        orders.active = vec![
            vec![FoodType::Eggplant; 3],
            vec![FoodType::Tomato, FoodType::LotusRoot, FoodType::Fish],
        ];
        app.insert_resource(Time::<()>::default())
            .insert_resource(orders)
            .add_message::<DishDelivered>()
            .add_message::<CleanDishReturned>()
            .add_message::<IllegalAction>()
            .add_message::<ItemDispensed>()
            .add_systems(Update, actions_system);
        let station = app
            .world_mut()
            .spawn((
                Station {
                    kind: StationKind::Delivery,
                },
                StationItem { entity: None },
                DeliveryState::default(),
            ))
            .id();
        let mut soup = Soup::new(FoodType::Tomato);
        soup.ingredients.extend([FoodType::Tomato, FoodType::Fish]);
        soup.cook_time_elapsed = 24.0;
        soup.refresh_cook_state();
        let soup = app.world_mut().spawn(soup).id();
        let dish = app
            .world_mut()
            .spawn((
                Dish {
                    soup: Some(soup),
                    clean_ratio: 0.0,
                },
                ItemKind::Dish,
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                Player { index: 0 },
                ActionState {
                    transfer: true,
                    action: false,
                    cooldown: 0.0,
                    cooldown_max: ACTION_COOLDOWN,
                    activity: None,
                    did_change_state: false,
                    did_illegal_action: false,
                },
                HeldItem { entity: Some(dish) },
                FacingStation {
                    entity: Some(station),
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<HeldItem>(player).unwrap().entity,
            Some(dish)
        );
        assert_eq!(
            app.world()
                .get::<DeliveryState>(station)
                .unwrap()
                .total_meals_delivered,
            0
        );
        assert!(
            app.world()
                .get::<ActionState>(player)
                .unwrap()
                .did_illegal_action
        );
        assert!(app.world().get::<Soup>(soup).is_some());
        app.world_mut().get_mut::<Soup>(soup).unwrap().ingredients =
            vec![FoodType::Fish, FoodType::Tomato, FoodType::LotusRoot];
        app.update();
        assert!(
            app.world()
                .get::<HeldItem>(player)
                .unwrap()
                .entity
                .is_none()
        );
        assert!(app.world().get::<Dish>(dish).unwrap().soup.is_none());
        assert!(app.world().get_entity(soup).is_err());
        let delivery = app.world().get::<DeliveryState>(station).unwrap();
        assert_eq!(delivery.total_meals_delivered, 1);
        assert_eq!(delivery.dish_timers.len(), 1);
        let orders = app.world().resource::<Orders>();
        assert_eq!(orders.active.len(), 2);
        assert_eq!(orders.active[0], vec![FoodType::Eggplant; 3]);
        assert_eq!(orders.active[1].len(), 3);
        app.update();
        assert_eq!(
            app.world()
                .get::<DeliveryState>(station)
                .unwrap()
                .total_meals_delivered,
            1
        );
    }
}
