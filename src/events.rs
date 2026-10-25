use bevy::prelude::*;

/// Fired when a dish is delivered to a delivery station.
#[derive(Message, Debug)]
pub struct DishDelivered;

/// Fired when a clean dish is returned to the dish rack after washing.
#[derive(Message, Debug)]
pub struct CleanDishReturned;

/// Fired when a player attempts an illegal action.
#[derive(Message, Debug)]
pub struct IllegalAction;

/// Fired when a crate dispenses an item; identifies the source station.
#[derive(Message, Debug)]
pub struct ItemDispensed {
    #[allow(dead_code)]
    pub station_entity: Entity,
}
