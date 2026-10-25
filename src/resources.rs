use bevy::prelude::*;
use bevy::reflect::TypePath;
use serde::Deserialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Level data (deserialized from RON)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct PlayerSpawn {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DishSpawn {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub enum StationDef {
    Counter,
    Stove,
    Sink(String), // direction string
    Prep,
    Crate(String), // ingredient type
    DishRack,
    DishReturn,
    Delivery(String), // direction string
}

#[derive(Debug, Deserialize, Clone)]
pub struct TileProp {
    pub collideable: bool,
    pub station: Option<StationDef>,
}

#[derive(Debug, Deserialize, Resource, Clone, Asset, TypePath)]
pub struct LevelData {
    pub tile_width: f32,
    pub tile_height: f32,
    pub map_width: u32,
    // Part of the RON level format; not read at runtime (camera math is constant-based).
    #[allow(dead_code)]
    pub map_height: u32,
    pub spritesheet: String,
    #[serde(skip)]
    #[dependency]
    pub spritesheet_image: Handle<Image>,
    pub time_limit: f32,
    pub players: Vec<PlayerSpawn>,
    pub dishes: Vec<DishSpawn>,
    pub tile_props: HashMap<u32, TileProp>,
    pub tiles: Vec<u32>,
}

// ---------------------------------------------------------------------------
// Runtime level state
// ---------------------------------------------------------------------------

#[derive(Resource, Debug)]
pub struct LevelState {
    pub time_remaining: f32,
    pub score: u32,
    pub burnt_batches: u32,
    pub done: bool,
    pub paused: bool,
    pub startup_timer: f32,
    pub shutdown_timer: f32,
    #[allow(dead_code)]
    pub player_count: usize,
}

impl LevelState {
    pub fn new(time_limit: f32, player_count: usize) -> Self {
        Self {
            time_remaining: time_limit,
            score: 0,
            burnt_batches: 0,
            done: false,
            paused: false,
            startup_timer: crate::constants::STARTUP_TIMER,
            shutdown_timer: crate::constants::SHUTDOWN_TIMER,
            player_count,
        }
    }
}

// ---------------------------------------------------------------------------
// High scores
// ---------------------------------------------------------------------------

#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HighScores {
    pub entries: Vec<(String, u32)>,
}

// ---------------------------------------------------------------------------
// Shared menu state
// ---------------------------------------------------------------------------

#[derive(Resource, Debug)]
pub struct MenuSelection {
    pub index: usize,
    pub count: usize,
}

impl MenuSelection {
    pub fn new(count: usize) -> Self {
        Self { index: 0, count }
    }

    pub fn up(&mut self) {
        if self.index == 0 {
            self.index = self.count - 1;
        } else {
            self.index -= 1;
        }
    }

    pub fn down(&mut self) {
        self.index = (self.index + 1) % self.count;
    }
}

// ---------------------------------------------------------------------------
// Player count selection from title menu
// ---------------------------------------------------------------------------

#[derive(Resource, Debug, Default)]
pub struct SelectedPlayerCount(pub usize);

// ---------------------------------------------------------------------------
// Score to pass between states
// ---------------------------------------------------------------------------

#[derive(Resource, Debug, Default)]
pub struct PendingScore(pub Option<u32>);

// ---------------------------------------------------------------------------
// Name entry state
// ---------------------------------------------------------------------------

#[derive(Resource, Debug)]
pub struct NameEntryState {
    pub name: String,
    pub selected_row: usize,
    pub selected_col: usize,
    pub max_length: usize,
    pub score_index: Option<usize>,
}

impl Default for NameEntryState {
    fn default() -> Self {
        Self {
            name: String::new(),
            selected_row: 0,
            selected_col: 0,
            max_length: 10,
            score_index: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Audio handles
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct GameAudio {
    pub current_bgm: Option<Entity>,
}

// ---------------------------------------------------------------------------
// Spritesheet layout info (for texture atlas construction)
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct TilesetInfo {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    // Actual cell/grid dimensions taken from the loaded image.
    #[allow(dead_code)]
    pub tile_width: f32,
    #[allow(dead_code)]
    pub tile_height: f32,
    #[allow(dead_code)]
    pub columns: u32,
    #[allow(dead_code)]
    pub rows: u32,
}

// ---------------------------------------------------------------------------
// Level asset handle (used during async loading)
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct LevelLoadHandle(pub Handle<LevelData>);

/// Campaign progress survives screen changes for the current game session.
#[derive(Resource, Default, Debug)]
pub struct Campaign {
    pub selected: usize,
    pub best: [u32; 3],
}
impl Campaign {
    pub fn stars(score: u32) -> usize {
        match score {
            0..=1 => 0,
            2 => 1,
            3 => 2,
            _ => 3,
        }
    }
    pub fn unlocked(&self) -> usize {
        (0..2)
            .take_while(|&i| Self::stars(self.best[i]) > 0)
            .count()
    }
    pub fn record(&mut self, score: u32) {
        self.best[self.selected] = self.best[self.selected].max(score);
    }
}

#[derive(Resource, Debug, Default)]
pub struct RoundResult {
    pub level: usize,
    pub delivered: u32,
    pub burnt: u32,
}
impl RoundResult {
    pub fn total(&self) -> i32 {
        self.delivered as i32 * 12 - self.burnt as i32 * 10
    }
}
