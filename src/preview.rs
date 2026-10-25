//! Render the actual UI to a texture without a desktop compositor.
use crate::{
    constants::{BASE_HEIGHT, BASE_WIDTH, WINDOW_SCALE},
    resources::*,
    states::AppState,
};
const PREVIEW_WIDTH: u32 = (BASE_WIDTH * WINDOW_SCALE) as u32;
const PREVIEW_HEIGHT: u32 = (BASE_HEIGHT * WINDOW_SCALE) as u32;
use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    camera::RenderTarget,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};

pub struct PreviewPlugin {
    pub score: bool,
}
#[derive(Resource)]
struct PreviewTarget {
    image: Handle<Image>,
    frames: u32,
    requested: bool,
    path: String,
}
impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PreviewMode(self.score))
            .add_plugins(ScheduleRunnerPlugin::run_loop(
                std::time::Duration::from_secs_f64(1.0 / 60.0),
            ))
            .add_systems(Startup, setup)
            .add_systems(Update, capture);
    }
}
#[derive(Resource)]
struct PreviewMode(bool);
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mode: Res<PreviewMode>,
    mut campaign: ResMut<Campaign>,
    mut pending: ResMut<PendingScore>,
    mut result: ResMut<RoundResult>,
    mut next: ResMut<NextState<AppState>>,
) {
    let mut image = Image::new_target_texture(
        PREVIEW_WIDTH,
        PREVIEW_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    std::fs::create_dir_all("previews").expect("preview directory must be writable");
    commands.insert_resource(PreviewTarget {
        image: images.add(image),
        frames: 0,
        requested: false,
        path: if mode.0 {
            "previews/results.png"
        } else {
            "previews/world-map.png"
        }
        .into(),
    });
    campaign.best = [6, 0, 0];
    if mode.0 {
        *result = RoundResult {
            level: 0,
            delivered: 6,
            burnt: 1,
        };
        pending.0 = Some(6);
        next.set(AppState::Score);
    } else {
        next.set(AppState::WorldMap);
    }
}
fn capture(
    mut commands: Commands,
    mut target: ResMut<PreviewTarget>,
    cameras: Query<Entity, Added<Camera>>,
    mut exit: MessageWriter<AppExit>,
) {
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(RenderTarget::Image(target.image.clone().into()));
    }
    target.frames += 1;
    if target.frames >= 200 && !target.requested {
        target.requested = true;
        commands
            .spawn(Screenshot::image(target.image.clone()))
            .observe(save_to_disk(target.path.clone()))
            .observe(
                |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                },
            );
    }
    if target.frames > 600 {
        exit.write(AppExit::error());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, state::app::StatesPlugin, time::TimeUpdateStrategy};
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    /// A CPU layout proof from the actual screen entities. ImageMagick substitutes
    /// for Bevy's GPU text renderer; this is intentionally not a gameplay screenshot.
    #[test]
    #[ignore = "development layout export requires ImageMagick"]
    fn export_cpu_layout_proofs() {
        for score in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
                .init_asset::<Image>()
                .init_asset::<Font>()
                .init_asset::<AudioSource>()
                .init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<HighScores>()
                .init_resource::<NameEntryState>()
                .init_state::<AppState>()
                .insert_resource(PendingScore(if score { Some(6) } else { None }))
                .insert_resource(Campaign {
                    selected: 0,
                    best: [6, 0, 0],
                })
                .insert_resource(RoundResult {
                    level: 0,
                    delivered: 6,
                    burnt: 1,
                })
                .insert_resource(TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_millis(50),
                ))
                .add_plugins((
                    crate::systems::audio::MenuMusicPlugin,
                    crate::plugins::world_map::WorldMapPlugin,
                    crate::plugins::score::ScorePlugin,
                ));
            // Load the exact PNG files through Bevy's decoder; a display and GPU
            // are unnecessary for this CPU proof of the actual screen entities.
            let art = app.world().resource::<crate::screen_art::ScreenArt>();
            let files = [
                (art.results.clone(), "assets/images/results/background.png"),
                (art.van.clone(), "assets/images/world_map/objects/van.png"),
                (
                    art.badge.clone(),
                    "assets/images/world_map/objects/stop.png",
                ),
                (art.star.clone(), "assets/images/results/star-gold.png"),
                (
                    art.star_empty.clone(),
                    "assets/images/results/star-empty.png",
                ),
                (
                    art.stop_flash.clone(),
                    "assets/images/world_map/objects/stop_flash.png",
                ),
                (
                    art.stop_locked.clone(),
                    "assets/images/world_map/objects/stop_locked.png",
                ),
            ];
            for (handle, path) in files {
                let image = Image::from_buffer(
                    &std::fs::read(path).unwrap(),
                    bevy::image::ImageType::Extension("png"),
                    bevy::image::CompressedImageFormats::NONE,
                    true,
                    bevy::image::ImageSampler::nearest(),
                    bevy::asset::RenderAssetUsages::MAIN_WORLD,
                )
                .unwrap();
                app.world_mut()
                    .resource_mut::<Assets<Image>>()
                    .insert(handle.id(), image)
                    .unwrap();
            }
            let map: crate::world_map_data::WorldMapData =
                serde_json::from_slice(&std::fs::read("assets/maps/overworld.tmj").unwrap())
                    .unwrap();
            map.runtime().unwrap();
            let mut proof_image_handles = Vec::new();
            for set in &map.tilesets {
                for tile in &set.tiles {
                    let path =
                        crate::world_map_data::WorldMapData::image_path(&tile.image).unwrap();
                    let handle = app
                        .world()
                        .resource::<AssetServer>()
                        .load::<Image>(path.clone());
                    proof_image_handles.push(handle.clone());
                    let image = Image::from_buffer(
                        &std::fs::read(format!("assets/{path}")).unwrap(),
                        bevy::image::ImageType::Extension("png"),
                        bevy::image::CompressedImageFormats::NONE,
                        true,
                        bevy::image::ImageSampler::nearest(),
                        bevy::asset::RenderAssetUsages::MAIN_WORLD,
                    )
                    .unwrap();
                    app.world_mut()
                        .resource_mut::<Assets<Image>>()
                        .insert(handle.id(), image)
                        .unwrap();
                }
            }
            let map_handle =
                app.world()
                    .resource::<AssetServer>()
                    .load::<crate::world_map_data::WorldMapData>(crate::world_map_data::MAP_PATH);
            app.world_mut()
                .resource_mut::<Assets<crate::world_map_data::WorldMapData>>()
                .insert(map_handle.id(), map)
                .unwrap();
            app.world_mut()
                .resource_mut::<NextState<AppState>>()
                .set(if score {
                    AppState::Score
                } else {
                    AppState::WorldMap
                });
            for _ in 0..85 {
                app.update();
            }
            let mut entities = app.world_mut().query::<EntityRef>();
            let world = app.world();
            let mut rgba: Vec<u8> = [218, 212, 201, 255]
                .into_iter()
                .cycle()
                .take((PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize)
                .collect();
            let mut text_commands: Vec<(f32, String, String, String)> = Vec::new();
            for entity in entities.iter(world) {
                if entity.get::<Visibility>() == Some(&Visibility::Hidden) {
                    continue;
                }
                let Some(node) = entity.get::<Node>() else {
                    continue;
                };
                let (Val::Px(mut x), Val::Px(mut y)) = (node.left, node.top) else {
                    continue;
                };
                let mut parent = entity.get::<ChildOf>().map(ChildOf::parent);
                while let Some(id) = parent {
                    if let Some(node) = world.get::<Node>(id) {
                        if let Val::Px(offset) = node.left {
                            x += offset;
                        }
                        if let Val::Px(offset) = node.top {
                            y += offset;
                        }
                    }
                    parent = world.get::<ChildOf>(id).map(ChildOf::parent);
                }
                if let Some(image_node) = entity.get::<ImageNode>() {
                    let images = world.resource::<Assets<Image>>();
                    let image = images
                        .get(&image_node.image)
                        .expect("Every preview image must be loaded");
                    let Some(data) = image.data.as_ref() else {
                        continue;
                    };
                    let (Val::Px(w), Val::Px(h)) = (node.width, node.height) else {
                        continue;
                    };
                    for dy in 0..h as usize {
                        for dx in 0..w as usize {
                            let px = x as i32 + dx as i32;
                            let py = y as i32 + dy as i32;
                            if px < 0
                                || py < 0
                                || px >= PREVIEW_WIDTH as i32
                                || py >= PREVIEW_HEIGHT as i32
                            {
                                continue;
                            }
                            let px = px as usize;
                            let py = py as usize;
                            let sample_x = if image_node.flip_x {
                                w as usize - 1 - dx
                            } else {
                                dx
                            };
                            let sample_y = if image_node.flip_y {
                                h as usize - 1 - dy
                            } else {
                                dy
                            };
                            let src = ((sample_y * image.height() as usize / h as usize)
                                * image.width() as usize
                                + sample_x * image.width() as usize / w as usize)
                                * 4;
                            let tint = image_node.color.to_srgba();
                            let alpha = data[src + 3] as f32 / 255.0 * tint.alpha;
                            let target = (py * PREVIEW_WIDTH as usize + px) * 4;
                            for channel in 0..3 {
                                let factor = [tint.red, tint.green, tint.blue][channel];
                                rgba[target + channel] =
                                    (data[src + channel] as f32 * factor * alpha
                                        + rgba[target + channel] as f32 * (1.0 - alpha))
                                        .round() as u8;
                            }
                            rgba[target + 3] = 255;
                        }
                    }
                }
                if let (Some(text), Some(font), Some(color)) = (
                    entity.get::<Text>(),
                    entity.get::<TextFont>(),
                    entity.get::<TextColor>(),
                ) {
                    if let Some(shadow) = entity.get::<TextShadow>() {
                        let rgb = shadow.color.to_srgba();
                        text_commands.push((
                            font.font_size,
                            format!(
                                "#{:02x}{:02x}{:02x}",
                                (rgb.red * 255.0) as u8,
                                (rgb.green * 255.0) as u8,
                                (rgb.blue * 255.0) as u8
                            ),
                            format!(
                                "+{}+{}",
                                (x + shadow.offset.x) as u32,
                                (y + shadow.offset.y + font.font_size * 0.85) as u32
                            ),
                            text.0.clone(),
                        ));
                    }
                    let rgb = color.0.to_srgba();
                    text_commands.push((
                        font.font_size,
                        format!(
                            "#{:02x}{:02x}{:02x}",
                            (rgb.red * 255.0) as u8,
                            (rgb.green * 255.0) as u8,
                            (rgb.blue * 255.0) as u8
                        ),
                        format!("+{}+{}", x as u32, (y + font.font_size * 0.85) as u32),
                        text.0.clone(),
                    ));
                }
            }
            std::fs::create_dir_all("previews").unwrap();
            let mut command = Command::new("magick");
            command.args([
                "-size",
                &format!("{PREVIEW_WIDTH}x{PREVIEW_HEIGHT}"),
                "-depth",
                "8",
                "rgba:-",
                "-font",
                "assets/fonts/gameboy.ttf",
            ]);
            for (size, color, position, text) in text_commands {
                command.args([
                    "-fill",
                    &color,
                    "-pointsize",
                    &size.to_string(),
                    "-annotate",
                    &position,
                    &text,
                ]);
            }
            command.arg(if score {
                "previews/results-painted-layout.png"
            } else {
                "previews/world-map-tiled-layout.png"
            });
            let mut child = command.stdin(Stdio::piped()).spawn().unwrap();
            child.stdin.take().unwrap().write_all(&rgba).unwrap();
            assert!(child.wait().unwrap().success());
        }
    }
}
