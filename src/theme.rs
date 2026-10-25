//! Nostalgos 12 UI colors. Sprite colors live in editable PNGs, never runtime filters.
use bevy::prelude::*;

pub const INK: Color = Color::srgb(39.0 / 255.0, 42.0 / 255.0, 50.0 / 255.0);
pub const PAPER: Color = Color::srgb(218.0 / 255.0, 212.0 / 255.0, 201.0 / 255.0);
pub const RED: Color = Color::srgb(220.0 / 255.0, 98.0 / 255.0, 80.0 / 255.0);
pub const GREEN: Color = Color::srgb(85.0 / 255.0, 146.0 / 255.0, 127.0 / 255.0);

pub struct KitchenThemePlugin;
impl Plugin for KitchenThemePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(PAPER));
    }
}

/// Large quiet checks, like a printed Game Boy title screen.
pub fn checkerboard(parent: &mut ChildSpawnerCommands) {
    for row in 0..10 {
        for col in 0..14 {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(col as f32 * 100.0 / 14.0),
                    top: Val::Percent(row as f32 * 10.0),
                    width: Val::Percent(100.0 / 14.0),
                    height: Val::Percent(10.0),
                    ..default()
                },
                BackgroundColor(if (row + col) % 2 == 0 { PAPER } else { GREEN }),
            ));
        }
    }
}
