use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

// ---------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------

/// Marker + player index (0-based).
#[derive(Component, Debug)]
pub struct Player {
    pub index: usize,
}

/// Which direction the player is facing (for animations & station detection).
#[derive(Component, Debug, Default)]
pub struct Facing {
    pub direction: Direction,
}

/// Player movement velocity.
#[derive(Component, Debug, Default)]
pub struct Velocity {
    pub dx: f32,
    pub dy: f32,
    pub speed: f32,
}

/// Tracks whether the player moved this frame (for walk animation).
#[derive(Component, Debug, Default)]
pub struct DidMove(pub bool);

/// The station entity the player is currently facing (colliding with).
#[derive(Component, Debug, Default)]
pub struct FacingStation {
    pub entity: Option<Entity>,
}

/// The item entity the player is currently holding.
#[derive(Component, Debug, Default)]
pub struct HeldItem {
    pub entity: Option<Entity>,
}

/// Cooldown + action/transfer flags.
#[derive(Component, Debug)]
pub struct ActionState {
    pub action: bool,
    pub transfer: bool,
    pub cooldown: f32,
    pub cooldown_max: f32,
    /// The player's current activity state (for animation/audio).
    pub activity: Option<PlayerActivity>,
    pub did_change_state: bool,
    pub did_illegal_action: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerActivity {
    Washing,
    Prepping,
}

/// Keyboard mapping for a single player.
#[derive(Component, Debug)]
pub struct KeyMap {
    pub up: KeyCode,
    pub down: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub pause: KeyCode,
    pub action: KeyCode,
    pub transfer: KeyCode,
}

// ---------------------------------------------------------------------------
// Stations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StationKind {
    Counter,
    Stove,
    Sink,
    Prep,
    Crate,
    DishRack,
    DishReturn,
    Delivery,
}

/// Marker component for all station entities.
#[derive(Component, Debug)]
pub struct Station {
    pub kind: StationKind,
}

/// Tracks the item entity sitting on this station (if any).
#[derive(Component, Debug, Default)]
pub struct StationItem {
    pub entity: Option<Entity>,
}

/// For Sink / Delivery stations: which direction to search for the linked return station.
/// The linking itself happens at load time; the field is kept for debugging.
#[derive(Component, Debug)]
pub struct DishLocationDirection {
    #[allow(dead_code)]
    pub direction: Direction,
}

/// Link from a sink to a dish rack, or from a delivery station to a return station.
#[derive(Component, Debug)]
pub struct LinkedReturnStation {
    pub entity: Entity,
}

/// Crate station: which ingredient it dispenses.
#[derive(Component, Debug)]
pub struct CrateIngredient {
    pub food_type: FoodType,
}

/// Events flag: crate just dispensed an item (for animation reset).
#[derive(Component, Debug, Default)]
pub struct DidDispenseItem(pub bool);

/// Time since this crate last dispensed; None keeps its authored closed sprite.
#[derive(Component, Debug, Default)]
pub struct CrateLidAnimation {
    pub elapsed: Option<f32>,
}

/// Dish rack: stores a stack of dish entities.
#[derive(Component, Debug, Default)]
pub struct DishStack {
    pub dishes: Vec<Entity>,
}

/// Delivery station: tracks dish timers and total meals.
#[derive(Component, Debug, Default)]
pub struct DeliveryState {
    pub dish_timers: Vec<(Entity, f32)>,
    pub total_meals_delivered: u32,
    pub did_change_total_meals: bool,
}

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FoodType {
    Tomato,
    LotusRoot,
    Cabbage,
    Fish,
    Meat,
    Eggplant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CookState {
    Raw,
    Cooking,
    Cooked,
    WillBurn,
    Burnt,
}

/// Ingredient item component.
#[derive(Component, Debug)]
pub struct Ingredient {
    pub food_type: FoodType,
    pub prep_ratio: f32,
}

/// Pot item component.
#[derive(Component, Debug, Default)]
pub struct Pot {
    pub soup: Option<Entity>,
}

/// Dish item component.
#[derive(Component, Debug)]
pub struct Dish {
    pub soup: Option<Entity>,
    pub clean_ratio: f32,
}

/// Soup entity component (stored as a separate entity referenced by Pot/Dish).
#[derive(Component, Debug)]
pub struct Soup {
    pub ingredients: Vec<FoodType>,
    pub cook_time_elapsed: f32,
    pub max_ingredients: usize,
    pub cook_ratio: f32,
    pub state: CookState,
}

impl Soup {
    pub fn new(first_ingredient: FoodType) -> Self {
        Self {
            ingredients: vec![first_ingredient],
            cook_time_elapsed: 0.0,
            max_ingredients: crate::constants::MAX_INGREDIENTS,
            cook_ratio: 0.0,
            state: CookState::Raw,
        }
    }

    /// Refresh after both cooking and adding an ingredient: adding an ingredient
    /// increases the required time, including when the pot is off the stove.
    pub fn refresh_cook_state(&mut self) {
        if self.is_burnt() {
            return;
        }
        let required = self.ingredients.len() as f32 * crate::constants::PER_INGREDIENT_COOK_TIME;
        self.cook_ratio = if required > 0.0 {
            self.cook_time_elapsed / required
        } else {
            0.0
        };
        self.state = if self.cook_ratio > crate::constants::BURN_RATIO {
            CookState::Burnt
        } else if self.cook_ratio > crate::constants::BURN_WARN_RATIO {
            CookState::WillBurn
        } else if self.cook_ratio >= 1.0 {
            CookState::Cooked
        } else if self.cook_time_elapsed > 0.0 {
            CookState::Cooking
        } else {
            CookState::Raw
        };
    }

    pub fn is_cooked(&self) -> bool {
        matches!(self.state, CookState::Cooked | CookState::WillBurn)
    }

    pub fn is_burnt(&self) -> bool {
        self.state == CookState::Burnt
    }

    pub fn will_burn_soon(&self) -> bool {
        self.state == CookState::WillBurn
    }

    pub fn is_prepared(&self) -> bool {
        self.is_cooked() && self.ingredients.len() == self.max_ingredients
    }
}

/// Marker to distinguish item types quickly.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Ingredient,
    Pot,
    Dish,
}

// ---------------------------------------------------------------------------
// Tile / Map rendering
// ---------------------------------------------------------------------------

/// Marks a tile entity, used for rendering. `tile_id` is the original 1-indexed
/// ID from the level data, kept for debugging.
#[derive(Component, Debug)]
pub struct MapTile {
    #[allow(dead_code)]
    pub tile_id: u32,
}

/// Marks an entity as part of the collision world.
#[derive(Component, Debug)]
pub struct Collideable;

// ---------------------------------------------------------------------------
// UI / HUD markers
// ---------------------------------------------------------------------------

/// Marker for the HUD time text.
#[derive(Component)]
pub struct HudTimeText;

/// Marker for the Ready/Go/Time's Up overlay text.
#[derive(Component)]
pub struct PhaseOverlayText;

// ---------------------------------------------------------------------------
// Title screen markers
// ---------------------------------------------------------------------------

/// Root marker for all title screen entities.
#[derive(Component)]
pub struct TitleScreenEntity;

#[derive(Component)]
pub struct TitleMenuItem;

// ---------------------------------------------------------------------------
// Score screen markers
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct ScoreScreenEntity;

// ---------------------------------------------------------------------------
// Name entry markers
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct NameEntryEntity;

#[derive(Component)]
pub struct NameEntryKey {
    pub row: usize,
    pub col: usize,
}

#[derive(Component)]
pub struct NameEntryDisplay;

// ---------------------------------------------------------------------------
// Gameplay entity cleanup marker
// ---------------------------------------------------------------------------

/// All entities spawned during gameplay get this so we can despawn them on exit.
#[derive(Component)]
pub struct GameplayEntity;

// ---------------------------------------------------------------------------
// Item visual sprites
// ---------------------------------------------------------------------------

/// Marks a sprite entity as the visual representation of a logical item.
/// Spawned/despawned by the render_items_system.
#[derive(Component)]
pub struct ItemVisual {
    /// The logical item entity this sprite represents.
    pub item_entity: Entity,
}

/// Marks a transient overlay sprite (cooking progress, ingredient counters, etc.)
/// that is recreated each frame. These are despawned and respawned every frame.
#[derive(Component)]
pub struct OverlayVisual;

/// Persistent overlay for player activity animations (chopping/washing).
/// Keyed by the player entity so it can be updated in place.
#[derive(Component)]
pub struct ActivityOverlay {
    pub player_entity: Entity,
}

#[cfg(test)]
mod soup_tests {
    use super::*;

    #[test]
    fn adding_third_tomato_requires_more_cooking() {
        let mut soup = Soup::new(FoodType::Tomato);
        soup.ingredients.push(FoodType::Tomato);
        soup.cook_time_elapsed = 16.0;
        soup.refresh_cook_state();
        assert!(soup.is_cooked());
        soup.ingredients.push(FoodType::Tomato);
        soup.refresh_cook_state();
        assert!(!soup.is_prepared());
        soup.cook_time_elapsed = 24.0;
        soup.refresh_cook_state();
        assert!(soup.is_prepared());
    }

    #[test]
    fn burnt_soup_cannot_be_restored_by_refresh() {
        let mut soup = Soup::new(FoodType::Tomato);
        soup.cook_time_elapsed = 14.0;
        soup.refresh_cook_state();
        assert!(soup.is_burnt());
        soup.ingredients.push(FoodType::Tomato);
        soup.refresh_cook_state();
        assert!(soup.is_burnt());
    }
}
