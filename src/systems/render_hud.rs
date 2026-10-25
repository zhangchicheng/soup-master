use crate::components::*;
use crate::constants::*;
use crate::recipes::Orders;
use crate::resources::LevelState;
use crate::theme::{INK, PAPER};
use crate::tileset::order_ingredient_rect;
use bevy::prelude::*;

#[derive(Component)]
pub struct HudRoot;
#[derive(Component)]
pub struct PauseLabel;
#[derive(Component)]
pub struct RecipeSlot {
    pub order: usize,
    pub ingredient: usize,
}

#[derive(Component)]
pub struct OrderCard {
    order: usize,
    id: Option<u64>,
    x: f32,
}

/// Keep the older order on the right; new orders enter from the left even when recipes match.
pub fn animate_order_cards(
    time: Res<Time>,
    orders: Res<Orders>,
    mut cards: Query<(&mut OrderCard, &mut Node)>,
) {
    let previous = cards
        .iter()
        .map(|(card, _)| (card.id, card.x))
        .collect::<Vec<_>>();
    // Rebind every card first so a surviving order keeps its current position.
    // Waiting cards stay fully outside the screen until the older card has stopped.
    for (mut card, _) in &mut cards {
        let id = orders.order_id(card.order);
        if card.id != id {
            card.x = previous
                .iter()
                .find(|(old_id, _)| old_id.is_some() && *old_id == id)
                .map(|(_, x)| *x)
                .unwrap_or(-32.0);
            card.id = id;
        }
    }
    let moving_order = cards
        .iter()
        .filter(|(card, _)| {
            card.id.is_some()
                && card.x < (crate::recipes::MAX_ACTIVE_ORDERS - 1 - card.order) as f32 * 32.0
        })
        .map(|(card, _)| card.order)
        .min();
    for (mut card, mut node) in &mut cards {
        if Some(card.order) == moving_order {
            let target = (crate::recipes::MAX_ACTIVE_ORDERS - 1 - card.order) as f32 * 32.0;
            card.x = (card.x + time.delta_secs() * 32.0).min(target);
        }
        node.left = Val::Px(card.x.round() * WINDOW_SCALE);
    }
}

pub fn render_hud_system(
    time: Res<Time>,
    mut level_state: ResMut<LevelState>,
    deliveries: Query<&DeliveryState>,
) {
    level_state.time_remaining = (level_state.time_remaining - time.delta_secs()).max(0.0);
    level_state.done = level_state.time_remaining == 0.0;
    level_state.score = deliveries.iter().map(|d| d.total_meals_delivered).sum();
}

pub fn update_hud_text(
    level_state: Res<LevelState>,
    mut time_text: Query<(&mut Text, &mut TextColor), With<HudTimeText>>,
    orders: Res<Orders>,
    mut icons: Query<(&RecipeSlot, &mut ImageNode, &mut Visibility)>,
) {
    for (slot, mut image, mut visibility) in &mut icons {
        let food = orders
            .active
            .get(slot.order)
            .and_then(|recipe| recipe.get(slot.ingredient));
        *visibility = if food.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some(food) = food {
            image.rect = Some(order_ingredient_rect(*food));
        }
    }
    let seconds = level_state.time_remaining.ceil() as u32;
    for (mut text, mut color) in &mut time_text {
        **text = format!("{:02}:{:02}", seconds / 60, seconds % 60);
        color.0 = INK;
    }
}

pub fn update_pause_label(
    level: Res<LevelState>,
    mut labels: Query<&mut Visibility, With<PauseLabel>>,
) {
    for mut visibility in &mut labels {
        *visibility = if level.paused {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn spawn_hud(mut commands: Commands, asset_server: Res<AssetServer>, sheet: Handle<Image>) {
    let font = asset_server.load("fonts/gameboy.ttf");
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Visibility::Hidden,
            HudRoot,
            GameplayEntity,
        ))
        .with_children(|parent| {
            // Two native 32×32 cards fit the reserved 64×32 corner.
            for order in 0..crate::recipes::MAX_ACTIVE_ORDERS {
                parent
                    .spawn((
                        ImageNode::new(asset_server.load("images/ui/order.png")),
                        OrderCard {
                            order,
                            id: None,
                            x: -32.0,
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-32.0 * WINDOW_SCALE),
                            top: Val::Px(0.0),
                            width: Val::Px(32.0 * WINDOW_SCALE),
                            height: Val::Px(32.0 * WINDOW_SCALE),
                            ..default()
                        },
                    ))
                    .with_children(|card| {
                        for slot in 0..3 {
                            card.spawn((
                                ImageNode::new(sheet.clone()),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px((1.0 + slot as f32 * 11.0) * WINDOW_SCALE),
                                    top: Val::Px(21.0 * WINDOW_SCALE),
                                    width: Val::Px(8.0 * WINDOW_SCALE),
                                    height: Val::Px(8.0 * WINDOW_SCALE),
                                    ..default()
                                },
                                RecipeSlot {
                                    order,
                                    ingredient: slot,
                                },
                                Visibility::Inherited,
                            ));
                        }
                    });
            }
            parent.spawn((
                Text::new("02:00"),
                TextFont {
                    font: font.clone(),
                    font_size: 10.0 * WINDOW_SCALE,
                    ..default()
                },
                TextColor(INK),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(5.0 * WINDOW_SCALE),
                    top: Val::Px(3.0 * WINDOW_SCALE),
                    ..default()
                },
                HudTimeText,
            ));
        });
    commands.spawn((
        Text::new("PAUSED\nQ / P TO RESUME"),
        TextFont {
            font,
            font_size: 12.0 * WINDOW_SCALE,
            ..default()
        },
        TextColor(PAPER),
        BackgroundColor(INK),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(40.0 * WINDOW_SCALE),
            top: Val::Px(64.0 * WINDOW_SCALE),
            padding: UiRect::all(Val::Px(6.0 * WINDOW_SCALE)),
            ..default()
        },
        Visibility::Hidden,
        PauseLabel,
        GameplayEntity,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn both_order_cards_show_their_own_foods_and_refill_on_completion() {
        let mut app = App::new();
        let mut orders = Orders::new(vec![FoodType::Fish]);
        orders.active = vec![vec![FoodType::Tomato; 3], vec![FoodType::Fish; 3]];
        app.insert_resource(orders)
            .insert_resource(LevelState::new(120.0, 1))
            .add_systems(Update, update_hud_text);
        let mut cards = Vec::new();
        for order in 0..2 {
            let icon = app
                .world_mut()
                .spawn((
                    RecipeSlot {
                        order,
                        ingredient: 2,
                    },
                    ImageNode::default(),
                    Visibility::Inherited,
                ))
                .id();
            cards.push(icon);
        }
        app.update();
        assert_eq!(
            app.world()
                .get::<ImageNode>(cards[0])
                .unwrap()
                .rect
                .unwrap()
                .min,
            Vec2::new(68.0, 260.0)
        );
        assert_eq!(
            app.world()
                .get::<ImageNode>(cards[1])
                .unwrap()
                .rect
                .unwrap()
                .min,
            Vec2::new(4.0, 260.0)
        );
        app.world_mut().resource_mut::<Orders>().complete(0);
        app.update();
        assert_eq!(
            app.world()
                .get::<ImageNode>(cards[0])
                .unwrap()
                .rect
                .unwrap()
                .min,
            Vec2::new(4.0, 260.0)
        );
        assert_eq!(
            app.world()
                .get::<ImageNode>(cards[1])
                .unwrap()
                .rect
                .unwrap()
                .min,
            Vec2::new(4.0, 260.0)
        );
        assert_eq!(
            *app.world().get::<Visibility>(cards[1]).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn orders_move_one_at_a_time_at_start_and_after_single_or_double_completion() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Orders::new(vec![FoodType::Fish]))
            .add_systems(Update, animate_order_cards);
        let cards = (0..2)
            .map(|order| {
                app.world_mut()
                    .spawn((
                        OrderCard {
                            order,
                            id: None,
                            x: 0.0,
                        },
                        Node::default(),
                    ))
                    .id()
            })
            .collect::<Vec<_>>();
        let step = |app: &mut App, seconds| {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(seconds));
            app.update();
        };
        let positions = |app: &App| {
            cards
                .iter()
                .map(|entity| app.world().get::<OrderCard>(*entity).unwrap().x)
                .collect::<Vec<_>>()
        };
        step(&mut app, 0.0);
        assert_eq!(positions(&app), vec![-32.0, -32.0]);
        step(&mut app, 1.0);
        assert_eq!(positions(&app), vec![0.0, -32.0]);
        step(&mut app, 1.0);
        assert_eq!(positions(&app), vec![32.0, -32.0]);
        step(&mut app, 0.5);
        assert_eq!(positions(&app), vec![32.0, -16.0]);
        step(&mut app, 0.5);
        assert_eq!(positions(&app), vec![32.0, 0.0]);
        let old_id = app.world().resource::<Orders>().order_id(1);
        app.world_mut().resource_mut::<Orders>().complete(0);
        assert_eq!(app.world().resource::<Orders>().order_id(0), old_id);
        assert_ne!(app.world().resource::<Orders>().order_id(1), old_id);
        step(&mut app, 0.0);
        assert_eq!(positions(&app), vec![0.0, -32.0]);
        step(&mut app, 0.5);
        assert_eq!(positions(&app), vec![16.0, -32.0]);
        step(&mut app, 10.0);
        assert_eq!(positions(&app), vec![32.0, -32.0]);
        step(&mut app, 1.0);
        assert_eq!(positions(&app), vec![32.0, 0.0]);
        app.world_mut().resource_mut::<Orders>().complete(1);
        step(&mut app, 0.0);
        assert_eq!(positions(&app), vec![32.0, -32.0]);
        step(&mut app, 1.0);
        assert_eq!(positions(&app), vec![32.0, 0.0]);

        // Both visible orders complete before the HUD gets its next update.
        app.world_mut().resource_mut::<Orders>().complete(0);
        app.world_mut().resource_mut::<Orders>().complete(0);
        step(&mut app, 0.0);
        assert_eq!(positions(&app), vec![-32.0, -32.0]);
        step(&mut app, 0.5);
        assert_eq!(positions(&app), vec![-16.0, -32.0]);
        // Replacing the waiting order must not restart the moving one.
        app.world_mut().resource_mut::<Orders>().complete(1);
        step(&mut app, 0.5);
        assert_eq!(positions(&app), vec![0.0, -32.0]);
        step(&mut app, 1.0);
        assert_eq!(positions(&app), vec![32.0, -32.0]);
        step(&mut app, 1.0);
        assert_eq!(positions(&app), vec![32.0, 0.0]);

        // Deliver the other original order on the very next animation frame.
        app.world_mut().resource_mut::<Orders>().complete(0);
        step(&mut app, 0.5);
        assert_eq!(positions(&app), vec![16.0, -32.0]);
        app.world_mut().resource_mut::<Orders>().complete(0);
        step(&mut app, 0.5);
        assert_eq!(positions(&app), vec![-16.0, -32.0]);
        step(&mut app, 1.5);
        assert_eq!(positions(&app), vec![32.0, -32.0]);
        step(&mut app, 1.0);
        assert_eq!(positions(&app), vec![32.0, 0.0]);
    }

    #[test]
    fn final_frame_delivery_counts_and_timer_stops_at_zero() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(0.1));
        let mut level = LevelState::new(0.05, 1);
        level.startup_timer = 0.0;
        app.insert_resource(time)
            .insert_resource(level)
            .add_systems(Update, render_hud_system);
        app.world_mut().spawn(DeliveryState {
            total_meals_delivered: 1,
            ..default()
        });
        app.update();
        let level = app.world().resource::<LevelState>();
        assert_eq!(level.time_remaining, 0.0);
        assert!(level.done);
        assert_eq!(level.score, 1);
    }
}
