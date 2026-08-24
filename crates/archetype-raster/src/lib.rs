#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::path::Path;

use anyhow::{Context, Result};
use arch_layout::Rect;
use arch_paint::{DisplayCommand, PaintColor, PaintShadow, TextDecoration};
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use image::{Rgba, RgbaImage, imageops::FilterType};

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 800;
pub const VIEWPORT_WIDTH: f32 = 1280.0;

pub struct Rasterizer {
    font_system: FontSystem,
    glyph_cache: SwashCache,
}

impl Default for Rasterizer {
    fn default() -> Self {
        let mut database = fontdb::Database::new();
        database.load_font_data(
            include_bytes!("../../../assets/fonts/NotoSansSC/NotoSansSC-Regular.otf").to_vec(),
        );
        database.set_sans_serif_family("Noto Sans SC");
        Self {
            font_system: FontSystem::new_with_locale_and_db("en-US".to_owned(), database),
            glyph_cache: SwashCache::new(),
        }
    }
}

impl Rasterizer {
    #[must_use]
    pub fn render(
        &mut self,
        width_px: u32,
        height_px: u32,
        display_list: &arch_paint::DisplayList,
        image_resources: &std::collections::HashMap<String, Vec<u8>>,
    ) -> RgbaImage {
        let mut output = RgbaImage::from_pixel(width_px, height_px, Rgba([255, 255, 255, 255]));
        for command in &display_list.commands {
            match command {
                DisplayCommand::Box {
                    bounds,
                    clip,
                    background,
                    border,
                    border_width_px,
                    border_radius_px,
                    shadow,
                } => draw_box(
                    &mut output,
                    *bounds,
                    *clip,
                    &BoxPaintStyle {
                        background: *background,
                        border: *border,
                        border_width_px: *border_width_px,
                        border_radius_px: *border_radius_px,
                        shadow: *shadow,
                    },
                ),
                DisplayCommand::Text {
                    bounds,
                    clip,
                    content,
                    size_px,
                    color,
                    line_height_px,
                    text_decoration,
                    ..
                } => self.draw_text(
                    &mut output,
                    *bounds,
                    *clip,
                    content,
                    *size_px,
                    color.unwrap_or(PaintColor {
                        red: 32,
                        green: 33,
                        blue: 36,
                        alpha: 255,
                    }),
                    *line_height_px,
                    *text_decoration,
                ),
                DisplayCommand::Image {
                    bounds,
                    clip,
                    source,
                    loaded,
                    opacity,
                    ..
                } => {
                    if *loaded {
                        if let Some(bytes) = image_resources.get(source) {
                            draw_image(&mut output, *bounds, *clip, bytes, *opacity);
                        }
                    }
                }
            }
        }
        output
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &mut self,
        output: &mut RgbaImage,
        bounds: Rect,
        clip: Option<Rect>,
        content: &str,
        size_px: f32,
        color: PaintColor,
        line_height_px: f32,
        text_decoration: TextDecoration,
    ) {
        let metrics = Metrics::new(size_px.max(1.0), line_height_px.max(size_px).max(1.0));
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(Family::SansSerif);
        buffer.set_size(
            &mut self.font_system,
            Some(bounds.width.max(1.0)),
            Some(bounds.height.max(line_height_px).max(1.0)),
        );
        buffer.set_text(&mut self.font_system, content, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, true);
        let base = Color::rgba(color.red, color.green, color.blue, color.alpha);
        buffer.draw(
            &mut self.font_system,
            &mut self.glyph_cache,
            base,
            |x, y, width, height, glyph_color| {
                let origin_x = bounds.x.floor() as i32 + x;
                let origin_y = bounds.y.floor() as i32 + y;
                for offset_y in 0..height {
                    for offset_x in 0..width {
                        blend_pixel(
                            output,
                            origin_x + i32::try_from(offset_x).unwrap_or(i32::MAX),
                            origin_y + i32::try_from(offset_y).unwrap_or(i32::MAX),
                            glyph_color.as_rgba(),
                            clip,
                        );
                    }
                }
            },
        );
        let decoration_y = match text_decoration {
            TextDecoration::None => return,
            TextDecoration::Underline => bounds.y + size_px * 1.1,
            TextDecoration::LineThrough => bounds.y + size_px * 0.55,
        };
        fill_rect(
            output,
            Rect {
                x: bounds.x,
                y: decoration_y,
                width: bounds.width,
                height: (size_px / 14.0).max(1.0),
            },
            clip,
            paint_rgba(color),
        );
    }
}

/// Saves a rendered reference image as PNG.
///
/// # Errors
/// Returns an error when the parent directory cannot be created or the PNG cannot be written.
pub fn save_snapshot(image: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    image
        .save(path)
        .with_context(|| format!("could not save {}", path.display()))
}

/// Loads a PNG reference image as RGBA pixels.
///
/// # Errors
/// Returns an error when the file cannot be opened or decoded as an image.
pub fn load_snapshot(path: &Path) -> Result<RgbaImage> {
    Ok(image::open(path)
        .with_context(|| format!("could not open {}", path.display()))?
        .to_rgba8())
}

#[must_use]
pub fn difference_ratio(actual: &RgbaImage, expected: &RgbaImage) -> f64 {
    if actual.dimensions() != expected.dimensions() {
        return 1.0;
    }
    let differing = actual
        .pixels()
        .zip(expected.pixels())
        .filter(|(actual, expected)| actual != expected)
        .count();
    differing as f64 / f64::from(actual.width() * actual.height())
}

struct BoxPaintStyle {
    background: Option<PaintColor>,
    border: Option<PaintColor>,
    border_width_px: f32,
    border_radius_px: f32,
    shadow: Option<PaintShadow>,
}

fn draw_box(output: &mut RgbaImage, bounds: Rect, clip: Option<Rect>, style: &BoxPaintStyle) {
    if let Some(shadow) = style.shadow {
        draw_shadow(output, bounds, clip, shadow, style.border_radius_px);
    }
    if let Some(background) = style.background {
        fill_rounded_rect(
            output,
            bounds,
            clip,
            paint_rgba(background),
            style.border_radius_px,
        );
    }
    if style.border_width_px > 0.0 {
        if let Some(border) = style.border {
            let width = style.border_width_px.ceil();
            let top = Rect {
                height: width,
                ..bounds
            };
            let bottom = Rect {
                y: bounds.y + bounds.height - width,
                height: width,
                ..bounds
            };
            let left = Rect { width, ..bounds };
            let right = Rect {
                x: bounds.x + bounds.width - width,
                width,
                ..bounds
            };
            for edge in [top, bottom, left, right] {
                fill_rect(output, edge, clip, paint_rgba(border));
            }
        }
    }
}

fn draw_image(
    output: &mut RgbaImage,
    bounds: Rect,
    clip: Option<Rect>,
    bytes: &[u8],
    opacity: f32,
) {
    let width = positive_dimension(bounds.width);
    let height = positive_dimension(bounds.height);
    let Ok(decoded) = image::load_from_memory(bytes) else {
        return;
    };
    let resized = decoded
        .resize_exact(width, height, FilterType::Nearest)
        .to_rgba8();
    let origin_x = bounds.x.floor() as i32;
    let origin_y = bounds.y.floor() as i32;
    for (x, y, pixel) in resized.enumerate_pixels() {
        blend_pixel(
            output,
            origin_x + i32::try_from(x).unwrap_or(i32::MAX),
            origin_y + i32::try_from(y).unwrap_or(i32::MAX),
            [
                pixel[0],
                pixel[1],
                pixel[2],
                (f32::from(pixel[3]) * opacity.clamp(0.0, 1.0)).round() as u8,
            ],
            clip,
        );
    }
}

fn draw_shadow(
    output: &mut RgbaImage,
    bounds: Rect,
    clip: Option<Rect>,
    shadow: PaintShadow,
    radius: f32,
) {
    let steps = shadow.blur_px.ceil().clamp(1.0, 64.0) as u32;
    let mut color = paint_rgba(shadow.color);
    color[3] = u8::try_from(u32::from(color[3]) / steps.max(1))
        .unwrap_or(1)
        .max(1);
    for step in (0..steps).rev() {
        let spread = step as f32;
        fill_rounded_rect(
            output,
            Rect {
                x: bounds.x + shadow.offset_x_px - spread,
                y: bounds.y + shadow.offset_y_px - spread,
                width: bounds.width + spread * 2.0,
                height: bounds.height + spread * 2.0,
            },
            clip,
            color,
            radius + spread,
        );
    }
}

fn fill_rounded_rect(
    output: &mut RgbaImage,
    rect: Rect,
    clip: Option<Rect>,
    color: [u8; 4],
    radius: f32,
) {
    let radius = radius.max(0.0).min(rect.width / 2.0).min(rect.height / 2.0);
    if radius <= 0.0 {
        fill_rect(output, rect, clip, color);
        return;
    }
    let start_x = rect.x.floor().max(0.0) as u32;
    let start_y = rect.y.floor().max(0.0) as u32;
    let end_x = (rect.x + rect.width)
        .ceil()
        .clamp(0.0, output.width() as f32) as u32;
    let end_y = (rect.y + rect.height)
        .ceil()
        .clamp(0.0, output.height() as f32) as u32;
    for y in start_y..end_y {
        for x in start_x..end_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let nearest_x = px.clamp(rect.x + radius, rect.x + rect.width - radius);
            let nearest_y = py.clamp(rect.y + radius, rect.y + rect.height - radius);
            if (px - nearest_x).powi(2) + (py - nearest_y).powi(2) <= radius.powi(2) {
                blend_pixel(output, x as i32, y as i32, color, clip);
            }
        }
    }
}

fn fill_rect(output: &mut RgbaImage, rect: Rect, clip: Option<Rect>, color: [u8; 4]) {
    let start_x = rect.x.floor().max(0.0) as u32;
    let start_y = rect.y.floor().max(0.0) as u32;
    let end_x = (rect.x + rect.width)
        .ceil()
        .clamp(0.0, output.width() as f32) as u32;
    let end_y = (rect.y + rect.height)
        .ceil()
        .clamp(0.0, output.height() as f32) as u32;
    for y in start_y..end_y {
        for x in start_x..end_x {
            blend_pixel(
                output,
                i32::try_from(x).unwrap_or(i32::MAX),
                i32::try_from(y).unwrap_or(i32::MAX),
                color,
                clip,
            );
        }
    }
}

fn blend_pixel(output: &mut RgbaImage, x: i32, y: i32, source: [u8; 4], clip: Option<Rect>) {
    if x < 0 || y < 0 || x >= output.width() as i32 || y >= output.height() as i32 {
        return;
    }
    if clip.is_some_and(|clip| {
        (x as f32) < clip.x
            || (y as f32) < clip.y
            || (x as f32) >= clip.x + clip.width
            || (y as f32) >= clip.y + clip.height
    }) {
        return;
    }
    let pixel = output.get_pixel_mut(x as u32, y as u32);
    let alpha = u16::from(source[3]);
    for channel in 0..3 {
        let blended =
            (u16::from(source[channel]) * alpha + u16::from(pixel[channel]) * (255 - alpha)) / 255;
        pixel[channel] = u8::try_from(blended).unwrap_or(u8::MAX);
    }
    pixel[3] = 255;
}

fn paint_rgba(color: PaintColor) -> [u8; 4] {
    [color.red, color.green, color.blue, color.alpha]
}

fn positive_dimension(value: f32) -> u32 {
    value.round().clamp(1.0, u32::MAX as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difference_ratio_counts_changed_pixels() {
        let first = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]));
        let mut second = first.clone();
        second.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        assert!((difference_ratio(&first, &second) - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn rounded_translucent_box_preserves_corner_and_blends_center() {
        let mut image = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
        draw_box(
            &mut image,
            Rect {
                x: 2.0,
                y: 2.0,
                width: 16.0,
                height: 16.0,
            },
            None,
            &BoxPaintStyle {
                background: Some(PaintColor {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 128,
                }),
                border: None,
                border_width_px: 0.0,
                border_radius_px: 6.0,
                shadow: None,
            },
        );

        assert_eq!(image.get_pixel(2, 2), &Rgba([255, 255, 255, 255]));
        assert_eq!(image.get_pixel(10, 10), &Rgba([255, 127, 127, 255]));
    }
}
