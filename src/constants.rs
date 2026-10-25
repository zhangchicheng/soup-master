// Game-wide constants ported from the Love2D version

pub const WINDOW_SCALE: f32 = 4.0;
pub const BASE_WIDTH: f32 = 224.0;
pub const BASE_HEIGHT: f32 = 160.0;
pub const TILE_SIZE: f32 = 16.0;

// Shared title and score screen music.
pub const MENU_BGM: &str = "sounds/music/After-Breakfast.wav";
pub const GAME_BGM: &str = "sounds/music/Simple_8-bit.wav";

// Player
pub const PLAYER_SPEED: f32 = 90.0;
pub const ACTION_COOLDOWN: f32 = 0.5;
pub const PLAYER_WIDTH: f32 = 16.0;
pub const PLAYER_HEIGHT: f32 = 16.0;

// Action timings
pub const ACTION_TIME: f32 = 2.0; // seconds for washing / prepping
pub const EATING_TIME: f32 = 8.0; // seconds customers take to eat

// Cooking
pub const PER_INGREDIENT_COOK_TIME: f32 = 8.0; // seconds per ingredient
pub const BURN_WARN_RATIO: f32 = 1.2;
pub const BURN_RATIO: f32 = 1.6;
pub const MAX_INGREDIENTS: usize = 3;

// Level
pub const STARTUP_TIMER: f32 = 4.0;
pub const SHUTDOWN_TIMER: f32 = 4.0;

// HUD
pub const HUD_FONT_SIZE: f32 = 8.0;

// Title screen
pub const MENU_X: f32 = 4.0;
pub const MENU_Y: f32 = 132.0;

// Rendering: Z-order layers (the full scheme is documented even where a layer
// is not referenced in code yet)
#[allow(dead_code)]
pub const Z_BACKGROUND: f32 = 0.0;
pub const Z_FLOOR: f32 = 1.0;
#[allow(dead_code)]
pub const Z_STATION: f32 = 2.0;
pub const Z_ITEM: f32 = 3.0;
pub const Z_PLAYER: f32 = 4.0;
pub const Z_OVERLAY: f32 = 5.0;
#[allow(dead_code)]
pub const Z_HUD: f32 = 10.0;
