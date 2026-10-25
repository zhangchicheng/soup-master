//! UI placement helpers. All map/result artwork is loaded from PNG assets.
use bevy::prelude::*;

pub fn screen_image(
    parent: &mut ChildSpawnerCommands,
    image: Handle<Image>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Entity {
    parent
        .spawn((
            ImageNode::new(image),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x * 4.0),
                top: Val::Px(y * 4.0),
                width: Val::Px(w * 4.0),
                height: Val::Px(h * 4.0),
                ..default()
            },
        ))
        .id()
}
pub fn label(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    text: impl Into<String>,
    x: f32,
    y: f32,
    size: f32,
    color: Color,
) -> Entity {
    parent
        .spawn((
            Text::new(text),
            TextFont {
                font: font.clone(),
                font_size: size * 4.0,
                ..default()
            },
            TextColor(color),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x * 4.0),
                top: Val::Px(y * 4.0),
                ..default()
            },
        ))
        .id()
}
