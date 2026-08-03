use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

/// Vector-style icons rasterized procedurally at startup, no external image assets.
/// Each texture is a shape mask (alpha carries the silhouette, RGB carries only
/// grayscale shading for a subtle sense of volume) so a single texture per shape can be
/// recolored freely via `ImageNode::color` for any accent color.
pub(crate) const ICON_TEXTURE_SIZE: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum IconKind {
    Metal,
    Crystal,
    Fuel,
    Energy,
}

impl IconKind {
    pub(crate) const ALL: [Self; 4] = [Self::Metal, Self::Crystal, Self::Fuel, Self::Energy];
}

#[derive(Resource)]
pub(crate) struct IconAssets {
    textures: HashMap<IconKind, Handle<Image>>,
}

impl IconAssets {
    pub(crate) fn handle(&self, kind: IconKind) -> Handle<Image> {
        self.textures
            .get(&kind)
            .cloned()
            .expect("icon texture registered for every IconKind")
    }
}

impl FromWorld for IconAssets {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.resource_mut::<Assets<Image>>();
        let textures = IconKind::ALL
            .into_iter()
            .map(|kind| (kind, images.add(icon_texture(kind))))
            .collect();
        Self { textures }
    }
}

/// Spawns a small tinted `ImageNode` for `kind`, colored by `color` at display time.
pub(crate) fn spawn_icon(
    parent: &mut ChildSpawnerCommands<'_>,
    icon_assets: &IconAssets,
    kind: IconKind,
    size: f32,
    color: Color,
) {
    parent.spawn((
        ImageNode {
            image: icon_assets.handle(kind),
            color,
            ..default()
        },
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
    ));
}

pub(crate) fn icon_texture(kind: IconKind) -> Image {
    let mut pixels = Vec::with_capacity((ICON_TEXTURE_SIZE * ICON_TEXTURE_SIZE * 4) as usize);
    for y in 0..ICON_TEXTURE_SIZE {
        for x in 0..ICON_TEXTURE_SIZE {
            pixels.extend_from_slice(&icon_pixel(kind, x, y));
        }
    }

    Image::new_fill(
        Extent3d {
            width: ICON_TEXTURE_SIZE,
            height: ICON_TEXTURE_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn icon_pixel(kind: IconKind, x: u32, y: u32) -> [u8; 4] {
    let (shade, alpha) = icon_mask(kind, x as i32, y as i32);
    [shade, shade, shade, alpha]
}

/// Returns `(grayscale shade, alpha)` for a pixel of the given icon shape.
fn icon_mask(kind: IconKind, x: i32, y: i32) -> (u8, u8) {
    let size = ICON_TEXTURE_SIZE as i32;
    let cx = size / 2;
    let cy = size / 2;
    match kind {
        IconKind::Metal => metal_mask(x, y, size),
        IconKind::Crystal => crystal_mask(x, y, cx, cy, size),
        IconKind::Fuel => fuel_mask(x, y, cx, size),
        IconKind::Energy => energy_mask(x, y, size),
    }
}

/// A square with a lighter top-left face and a darker bottom-right face, suggesting a
/// simple cube/ingot volume without needing real 3D shading.
fn metal_mask(x: i32, y: i32, size: i32) -> (u8, u8) {
    let margin = size / 6;
    if x < margin || y < margin || x >= size - margin || y >= size - margin {
        return (0, 0);
    }
    let shade = if (x - y) >= 0 { 245 } else { 175 };
    (shade, 255)
}

/// A true diamond via a Manhattan-distance disk (`|dx| + |dy| <= radius`), which avoids
/// any need to rotate a `Node` — the rotation lives in the raster, not the UI layout.
fn crystal_mask(x: i32, y: i32, cx: i32, cy: i32, size: i32) -> (u8, u8) {
    let radius = size / 2 - size / 8;
    let dist = (x - cx).abs() + (y - cy).abs();
    if dist > radius {
        return (0, 0);
    }
    let shade = 255 - (dist * 60 / radius.max(1)) as u8;
    (shade, 255)
}

/// A droplet: a circular body in the lower half, narrowing linearly to a point at the top.
fn fuel_mask(x: i32, y: i32, cx: i32, size: i32) -> (u8, u8) {
    let apex_y = size / 6;
    let body_center_y = size * 2 / 3;
    let body_radius = size / 3;

    if y >= body_center_y {
        let dx = x - cx;
        let dy = y - body_center_y;
        if dx * dx + dy * dy <= body_radius * body_radius {
            return (235, 255);
        }
        return (0, 0);
    }

    if y < apex_y {
        return (0, 0);
    }
    let progress = (y - apex_y).max(0) as f32 / (body_center_y - apex_y).max(1) as f32;
    let half_width = (progress * body_radius as f32) as i32;
    if (x - cx).abs() <= half_width {
        (235, 255)
    } else {
        (0, 0)
    }
}

/// A lightning bolt built from two thick diagonal strokes (distance-to-segment test),
/// avoiding a full polygon rasterizer while still reading clearly as a zigzag.
fn energy_mask(x: i32, y: i32, size: i32) -> (u8, u8) {
    let half_width = (size as f32 * 0.09).max(1.0);
    let top = (size as f32 * 0.62, size as f32 * 0.06);
    let mid = (size as f32 * 0.32, size as f32 * 0.52);
    let bottom = (size as f32 * 0.60, size as f32 * 0.94);

    let point = (x as f32 + 0.5, y as f32 + 0.5);
    let dist = distance_to_segment(point, top, mid).min(distance_to_segment(point, mid, bottom));
    if dist <= half_width {
        (250, 255)
    } else {
        (0, 0)
    }
}

fn distance_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let ab = (b.0 - a.0, b.1 - a.1);
    let ap = (p.0 - a.0, p.1 - a.1);
    let ab_len_sq = ab.0 * ab.0 + ab.1 * ab.1;
    let t = if ab_len_sq > 0.0 {
        ((ap.0 * ab.0 + ap.1 * ab.1) / ab_len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest = (a.0 + ab.0 * t, a.1 + ab.1 * t);
    let dx = p.0 - closest.0;
    let dy = p.1 - closest.1;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_texture_has_the_expected_byte_length() {
        for kind in IconKind::ALL {
            let texture = icon_texture(kind);
            let expected = (ICON_TEXTURE_SIZE * ICON_TEXTURE_SIZE * 4) as usize;
            assert_eq!(texture.data.as_ref().map(|d| d.len()), Some(expected));
        }
    }

    #[test]
    fn crystal_mask_is_opaque_at_center_and_transparent_at_corners() {
        let size = ICON_TEXTURE_SIZE as i32;
        let (cx, cy) = (size / 2, size / 2);
        let (_, center_alpha) = crystal_mask(cx, cy, cx, cy, size);
        let (_, corner_alpha) = crystal_mask(0, 0, cx, cy, size);
        assert_eq!(center_alpha, 255);
        assert_eq!(corner_alpha, 0);
    }

    #[test]
    fn metal_mask_fills_a_square_with_margin() {
        let size = ICON_TEXTURE_SIZE as i32;
        let (_, edge_alpha) = metal_mask(0, 0, size);
        let (_, inside_alpha) = metal_mask(size / 2, size / 2, size);
        assert_eq!(edge_alpha, 0);
        assert_eq!(inside_alpha, 255);
    }

    #[test]
    fn fuel_mask_is_narrower_near_the_apex_than_at_the_body() {
        let size = ICON_TEXTURE_SIZE as i32;
        let cx = size / 2;
        let apex_row = size / 6 + 1;
        let body_row = size * 2 / 3;
        let apex_span = (0..size)
            .filter(|&x| fuel_mask(x, apex_row, cx, size).1 > 0)
            .count();
        let body_span = (0..size)
            .filter(|&x| fuel_mask(x, body_row, cx, size).1 > 0)
            .count();
        assert!(apex_span < body_span);
    }

    #[test]
    fn energy_mask_marks_pixels_along_the_bolt_and_not_far_from_it() {
        let size = ICON_TEXTURE_SIZE as i32;
        let (_, on_bolt) = energy_mask(size * 6 / 10, size / 20, size);
        let (_, far_from_bolt) = energy_mask(0, size - 1, size);
        assert_eq!(on_bolt, 255);
        assert_eq!(far_from_bolt, 0);
    }
}
