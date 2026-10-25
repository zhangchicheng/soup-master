use bevy::prelude::*;

/// Top-level application state, drives which plugins/screens are active.
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Title,
    WorldMap,
    Loading,
    Gameplay,
    Score,
    NameEntry,
}

/// Sub-state for gameplay phase transitions (Ready? → Go! → Playing → Time's Up).
/// `Playing`/`Ending` are not yet driven as formal transitions — phases are
/// currently managed via `LevelState` fields (see ARCHITECTURE.md).
#[derive(SubStates, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::Gameplay)]
pub enum GameplayPhase {
    #[default]
    Starting,
    #[allow(dead_code)]
    Playing,
    #[allow(dead_code)]
    Ending,
}
