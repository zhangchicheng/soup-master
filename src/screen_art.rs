//! Editable, painted PNG artwork. The game loads these files directly.
use bevy::prelude::*;

#[derive(Resource)]
pub struct ScreenArt {
    pub results: Handle<Image>,
    pub van: Handle<Image>,
    pub badge: Handle<Image>,
    pub star: Handle<Image>,
    pub star_empty: Handle<Image>,
    pub stop_flash: Handle<Image>,
    pub stop_locked: Handle<Image>,
}
impl FromWorld for ScreenArt {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        Self {
            results: server.load("images/results/background.png"),
            van: server.load("images/world_map/objects/van.png"),
            badge: server.load("images/world_map/objects/stop.png"),
            star: server.load("images/results/star-gold.png"),
            star_empty: server.load("images/results/star-empty.png"),
            stop_flash: server.load("images/world_map/objects/stop_flash.png"),
            stop_locked: server.load("images/world_map/objects/stop_locked.png"),
        }
    }
}
