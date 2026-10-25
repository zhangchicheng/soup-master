mod components;
mod constants;
mod display;
mod events;
mod level_loader;
mod plugins;
#[cfg(not(target_arch = "wasm32"))]
mod preview;
mod recipes;
mod resources;
mod score_store;
mod screen_art;
mod screen_ui;
mod states;
mod systems;
mod theme;
mod tileset;
mod world_map_data;

use bevy::prelude::*;
use bevy_tweening::TweeningPlugin;
use constants::*;
use level_loader::LevelLoaderPlugin;
use plugins::{
    gameplay::GameplayPlugin, name_entry::NameEntryPlugin, score::ScorePlugin, title::TitlePlugin,
};
use resources::*;
use states::*;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<_> = std::env::args().collect();
        if let Some(index) = args.iter().position(|arg| arg == "--validate-map") {
            let path = args
                .get(index + 1)
                .map(String::as_str)
                .unwrap_or("assets/maps/overworld.tmj");
            let checked = std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<world_map_data::WorldMapData>(&bytes)
                        .map_err(|e| e.to_string())
                })
                .and_then(|data| data.runtime());
            match checked {
                Ok(map) => println!(
                    "Map valid: {}x{} cells, {} road tiles, {} kitchens",
                    map.data.width,
                    map.data.height,
                    map.roads.len(),
                    map.levels.len()
                ),
                Err(error) => {
                    eprintln!("Invalid map {path}: {error}");
                    std::process::exit(1);
                }
            }
            return;
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let preview = std::env::args().find(|arg| arg == "--preview-map" || arg == "--preview-score");
    #[cfg(target_arch = "wasm32")]
    let preview: Option<String> = None;
    let mut defaults = DefaultPlugins
        .set(WindowPlugin {
            primary_window: if preview.is_some() {
                None
            } else {
                Some(Window {
                    title: "Soup Master".into(),
                    resolution: bevy::window::WindowResolution::new(
                        (BASE_WIDTH * WINDOW_SCALE) as u32,
                        (BASE_HEIGHT * WINDOW_SCALE) as u32,
                    )
                    .with_scale_factor_override(1.0),
                    resizable: true,
                    canvas: cfg!(target_arch = "wasm32").then(|| "#game-canvas".into()),
                    fit_canvas_to_parent: cfg!(target_arch = "wasm32"),
                    ..default()
                })
            },
            exit_condition: if preview.is_some() {
                bevy::window::ExitCondition::DontExit
            } else {
                bevy::window::ExitCondition::OnAllClosed
            },
            ..default()
        })
        .set(ImagePlugin::default_nearest())
        .build();
    if preview.is_some() {
        defaults = defaults.disable::<bevy::winit::WinitPlugin>();
    }
    let mut app = App::new();
    app.add_plugins(defaults)
        // States
        .init_state::<AppState>()
        .add_sub_state::<GameplayPhase>()
        // Global resources
        .init_resource::<HighScores>()
        .init_resource::<score_store::HighScoreStore>()
        .add_systems(Startup, score_store::load_high_scores)
        .init_resource::<SelectedPlayerCount>()
        .init_resource::<PendingScore>()
        .init_resource::<NameEntryState>()
        // Plugins
        .add_plugins((
            TweeningPlugin,
            display::GameDisplayPlugin,
            theme::KitchenThemePlugin,
            systems::audio::MenuMusicPlugin,
            TitlePlugin,
            plugins::world_map::WorldMapPlugin,
            GameplayPlugin,
            ScorePlugin,
            NameEntryPlugin,
            LevelLoaderPlugin,
        ));
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(mode) = preview {
        app.add_plugins(preview::PreviewPlugin {
            score: mode == "--preview-score",
        });
    }
    app.run();
}
