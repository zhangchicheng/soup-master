//! Kitchen atlas coordinates and validation. Artwork is never rewritten here.
use crate::components::{Direction, FoodType};
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Coordinates for the current tileset, including the inserted ingredient row.
// Zero-based row/column; one tile is 16x16.
// ---------------------------------------------------------------------------
pub const COLS: usize = 11;
pub const PLAYER_WALK_FRAMES: usize = 4;

pub const ATLAS_TOMATO: usize = 3 * COLS + 1;
pub const ATLAS_PREPPED_TOMATO: usize = 3 * COLS + 0;
pub const ATLAS_EMPTY_POT: usize = 2 * COLS + 6;
pub const ATLAS_FILLED_POT: usize = 2 * COLS + 2;
pub const ATLAS_BURNT_POT: usize = 2 * COLS + 10;
pub const ATLAS_BOILING_BASE: usize = 2 * COLS + 3;
/// Row 17, columns 8–11: four red pot frames for the burn warning.
pub const ATLAS_WARNING_POT_BASE: usize = 16 * COLS + 7;
pub const ATLAS_WARNING_POT_FRAMES: usize = 4;
pub const ATLAS_EMPTY_BOWL: usize = 2 * COLS + 7;
pub const ATLAS_FILLED_BOWL: usize = 2 * COLS + 8;
pub const ATLAS_DIRTY_BOWL: usize = 2 * COLS + 9;
pub const ATLAS_FILLED_SINK: usize = 0 * COLS + 7;
pub const ATLAS_PREP_READY: usize = 1 * COLS + 4;

pub const ATLAS_MISSING_TWO: usize = 6 * COLS + 8;
pub const ATLAS_MISSING_ONE: usize = 6 * COLS + 9;
pub const ATLAS_COOK_25: usize = 5 * COLS + 5;
pub const ATLAS_COOK_50: usize = 5 * COLS + 6;
pub const ATLAS_COOK_75: usize = 5 * COLS + 7;
pub const ATLAS_COOK_DONE: usize = 5 * COLS + 8;
pub const ATLAS_BURN_WARN_BASE: usize = 4 * COLS + 8;
pub const ATLAS_CHOPPING_BASE: usize = 8 * COLS + 0;
pub const ATLAS_CHOPPING_FRAMES: usize = 10;
pub const ATLAS_WASHING_BASE: usize = 6 * COLS + 5;
pub const ATLAS_WASHING_FRAMES: usize = 2;
/// Closed crate (first image in its five-cell strip); the next four animate.
pub fn crate_sprite_index(food: FoodType) -> usize {
    match food {
        FoodType::Tomato => 9 * COLS,
        FoodType::LotusRoot => 15 * COLS,
        FoodType::Meat => ingredient_sprite_index(food, false),
        FoodType::Cabbage => 9 * COLS + 6,
        FoodType::Fish => 10 * COLS,
        FoodType::Eggplant => 10 * COLS + 6,
    }
}

pub fn ingredient_sprite_index(food: FoodType, prepped: bool) -> usize {
    match (food, prepped) {
        (FoodType::LotusRoot, false) => 3 * COLS + 7,
        (FoodType::LotusRoot, true) => 3 * COLS + 6,
        (FoodType::Meat, false) => 3 * COLS + 10,
        (FoodType::Meat, true) => 3 * COLS + 9,
        (FoodType::Tomato, false) => ATLAS_TOMATO,
        (FoodType::Tomato, true) => ATLAS_PREPPED_TOMATO,
        (FoodType::Cabbage, false) => 3 * COLS + 3,
        (FoodType::Cabbage, true) => 3 * COLS + 2,
        (FoodType::Fish, false) => 2 * COLS + 1,
        (FoodType::Fish, true) => 2 * COLS,
        (FoodType::Eggplant, false) => 3 * COLS + 5,
        (FoodType::Eggplant, true) => 3 * COLS + 4,
    }
}
pub const ATLAS_CRATE_LID_FRAMES: usize = 4;

/// Read the real image size; the authored coordinates require 11 columns and
/// the 15 rows including the two new four-frame characters. Additional bottom rows work.
pub fn layout_for_size(size: UVec2, tile: UVec2) -> Result<TextureAtlasLayout, String> {
    if tile != UVec2::splat(16) || size.x % 16 != 0 || size.y % 16 != 0 {
        return Err(format!(
            "Tileset must use whole 16x16 cells; got image {size:?}, tile {tile:?}"
        ));
    }
    let columns = size.x / 16;
    let rows = size.y / 16;
    if columns != COLS as u32 || rows < 15 {
        return Err(format!(
            "Current atlas coordinates require 11 columns and at least 15 rows; got {columns}x{rows}"
        ));
    }
    Ok(TextureAtlasLayout::from_grid(
        tile, columns, rows, None, None,
    ))
}

/// P1/P2 use the appended characters: columns 0-3 / 4-7, four frames each.
/// The artwork's new direction order is down, right, up, left (rows 11-14).
pub fn player_sprite_index(player_index: usize, direction: Direction) -> usize {
    let col = player_index * PLAYER_WALK_FRAMES;
    let row = match direction {
        Direction::Down => 11,
        Direction::Right => 12,
        Direction::Up => 13,
        Direction::Left => 14,
    };
    row * COLS + col
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_new_characters_use_the_authored_direction_rows() {
        for (direction, row) in [
            (Direction::Down, 11),
            (Direction::Right, 12),
            (Direction::Left, 14),
            (Direction::Up, 13),
        ] {
            assert_eq!(player_sprite_index(0, direction), row * 11);
            assert_eq!(player_sprite_index(1, direction), row * 11 + 4);
        }
    }
    #[test]
    fn real_png_includes_every_animation_frame_after_the_inserted_row() {
        let image = Image::from_buffer(
            include_bytes!("../assets/images/kitchen/tileset.png"),
            bevy::image::ImageType::Extension("png"),
            bevy::image::CompressedImageFormats::NONE,
            true,
            bevy::image::ImageSampler::nearest(),
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        )
        .unwrap();
        let layout = layout_for_size(image.size(), UVec2::splat(16)).unwrap();
        assert_eq!(
            layout.textures.len(),
            (image.width() / 16 * (image.height() / 16)) as usize
        );
        assert_eq!(layout.textures[ATLAS_TOMATO].min, UVec2::new(16, 48));
        assert_eq!(layout.textures[ATLAS_CHOPPING_BASE].min, UVec2::new(0, 128));
        assert_eq!(layout.textures[ATLAS_WASHING_BASE].min, UVec2::new(80, 96));
        for index in [
            ATLAS_CHOPPING_BASE + ATLAS_CHOPPING_FRAMES - 1,
            ATLAS_WASHING_BASE + ATLAS_WASHING_FRAMES - 1,
            crate_sprite_index(FoodType::Eggplant) + ATLAS_CRATE_LID_FRAMES,
            ATLAS_BURN_WARN_BASE + 1,
            ATLAS_WARNING_POT_BASE + ATLAS_WARNING_POT_FRAMES - 1,
            ATLAS_COOK_DONE,
            crate_sprite_index(FoodType::LotusRoot) + ATLAS_CRATE_LID_FRAMES,
            player_sprite_index(1, Direction::Left) + PLAYER_WALK_FRAMES - 1,
        ] {
            assert!(layout.textures[index].max.cmple(image.size()).all());
        }
        assert!(layout_for_size(UVec2::new(176, 144), UVec2::splat(16)).is_err());
        assert!(layout_for_size(UVec2::new(192, 160), UVec2::splat(16)).is_err());
        assert!(layout_for_size(UVec2::new(176, 176), UVec2::splat(16)).is_err());
        assert_eq!(
            layout_for_size(UVec2::new(176, 256), UVec2::splat(16))
                .unwrap()
                .textures
                .len(),
            176
        );
    }
}

/// Row 8, columns 6-11: the small colored pot-content markers.
pub fn ingredient_icon_index(food: FoodType) -> usize {
    7 * COLS
        + 5
        + match food {
            FoodType::Tomato => 0,
            FoodType::LotusRoot => 1,
            FoodType::Cabbage => 2,
            FoodType::Fish => 3,
            FoodType::Meat => 4,
            FoodType::Eggplant => 5,
        }
}

/// Order thumbnails authored in row 17. Crop the centered 8×8 pixels at native size.
/// Meat has no thumbnail yet; fit its existing whole sprite into the same slot.
pub fn order_ingredient_rect(food: FoodType) -> Rect {
    let column = match food {
        FoodType::Fish => 0,
        FoodType::LotusRoot => 1,
        FoodType::Cabbage => 2,
        FoodType::Eggplant => 3,
        FoodType::Tomato => 4,
        FoodType::Meat => {
            let index = ingredient_sprite_index(food, false);
            let min = Vec2::new((index % COLS) as f32 * 16.0, (index / COLS) as f32 * 16.0);
            return Rect::from_corners(min, min + Vec2::splat(16.0));
        }
    };
    let min = Vec2::new(column as f32 * 16.0 + 4.0, 16.0 * 16.0 + 4.0);
    Rect::from_corners(min, min + Vec2::splat(8.0))
}
