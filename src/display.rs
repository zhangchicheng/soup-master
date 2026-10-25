//! Fit the fixed game canvas into the actual window, including compositor/DPI changes.
use crate::constants::{BASE_HEIGHT, BASE_WIDTH, WINDOW_SCALE};
use bevy::{
    camera::{CameraUpdateSystems, RenderTarget, Viewport},
    prelude::*,
    window::{PrimaryWindow, WindowRef},
};

pub struct GameDisplayPlugin;
impl Plugin for GameDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiScale>()
            .add_systems(PostUpdate, fit_game_to_window.before(CameraUpdateSystems));
    }
}

fn fit_game_to_window(
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    mut cameras: Query<(&mut Camera, &RenderTarget)>,
    mut ui_scale: ResMut<UiScale>,
) {
    for (mut camera, target) in &mut cameras {
        let RenderTarget::Window(reference) = target else {
            // Texture previews keep their own fixed-size render target.
            continue;
        };
        let Some((_, window, _)) = windows.iter().find(|(entity, _, primary)| match reference {
            WindowRef::Primary => *primary,
            WindowRef::Entity(target) => entity == target,
        }) else {
            continue;
        };
        let physical = window.physical_size();
        if physical.x == 0 || physical.y == 0 {
            continue;
        }
        let factor = (physical.x as f32 / BASE_WIDTH).min(physical.y as f32 / BASE_HEIGHT);
        let size = (Vec2::new(BASE_WIDTH, BASE_HEIGHT) * factor)
            .floor()
            .as_uvec2()
            .max(UVec2::ONE);
        let position = (physical - size) / 2;
        let viewport = camera.viewport.as_ref();
        if !viewport.is_some_and(|v| v.physical_size == size && v.physical_position == position) {
            camera.viewport = Some(Viewport {
                physical_position: position,
                physical_size: size,
                ..default()
            });
        }
        // UI coordinates are authored at 4x. Bevy also multiplies by the
        // window's DPI scale, so cancel that factor exactly once here.
        let scale = (size.x as f32 / (BASE_WIDTH * WINDOW_SCALE))
            .min(size.y as f32 / (BASE_HEIGHT * WINDOW_SCALE))
            / window.scale_factor();
        if (ui_scale.0 - scale).abs() > f32::EPSILON {
            ui_scale.0 = scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::WindowResolution;

    fn app_with_window(width: u32, height: u32, dpi: f32) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameDisplayPlugin));
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(width, height)
                        .with_scale_factor_override(dpi),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((Camera::default(), RenderTarget::default()))
            .id();
        (app, window, camera)
    }

    fn assert_canvas_fits(app: &App, window: Entity, camera: Entity) {
        let window = app.world().get::<Window>(window).unwrap();
        let viewport = app
            .world()
            .get::<Camera>(camera)
            .unwrap()
            .viewport
            .as_ref()
            .unwrap();
        let bounds = viewport.physical_position + viewport.physical_size;
        assert!(bounds.cmple(window.physical_size()).all());
        assert_eq!(
            viewport.physical_position,
            (window.physical_size() - viewport.physical_size) / 2
        );
        let actual = Vec2::new(BASE_WIDTH, BASE_HEIGHT)
            * WINDOW_SCALE
            * app.world().resource::<UiScale>().0
            * window.scale_factor();
        assert!(actual.x <= viewport.physical_size.x as f32 + 0.001);
        assert!(actual.y <= viewport.physical_size.y as f32 + 0.001);
        // Fit the full canvas, not an unnecessarily tiny region of the window.
        assert!((actual.x - viewport.physical_size.x as f32).abs() <= 2.0);
        assert!((actual.y - viewport.physical_size.y as f32).abs() <= 2.0);
    }

    #[test]
    fn smaller_window_and_dpi_do_not_crop_the_canvas() {
        for (width, height, dpi) in [
            (595, 584, 1.0),
            (896, 640, 1.0),
            (896, 640, 2.0),
            (1792, 1280, 2.0),
        ] {
            let (mut app, window, camera) = app_with_window(width, height, dpi);
            app.update();
            assert_canvas_fits(&app, window, camera);
        }
    }

    #[test]
    fn resize_updates_the_viewport_and_ui_together() {
        let (mut app, window, camera) = app_with_window(896, 640, 1.0);
        app.update();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(600, 400);
        app.update();
        assert_canvas_fits(&app, window, camera);
        assert!(app.world().resource::<UiScale>().0 < 1.0);
    }

    #[test]
    fn texture_previews_are_not_resized_to_the_desktop_window() {
        let (mut app, _, camera) = app_with_window(595, 584, 2.0);
        app.world_mut()
            .entity_mut(camera)
            .insert(RenderTarget::Image(Handle::<Image>::default().into()));
        app.update();
        assert!(
            app.world()
                .get::<Camera>(camera)
                .unwrap()
                .viewport
                .is_none()
        );
        assert_eq!(app.world().resource::<UiScale>().0, 1.0);
    }
}
