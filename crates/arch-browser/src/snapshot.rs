#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::path::Path;

use anyhow::{Context, Result};
use arch_layout::Rect;
use arch_paint::{DisplayCommand, PaintColor};
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use image::{Rgba, RgbaImage, imageops::FilterType};

use crate::RenderedPage;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 800;
pub const VIEWPORT_WIDTH: f32 = 1280.0;

pub struct SnapshotRenderer {
    font_system: FontSystem,
    glyph_cache: SwashCache,
}

impl Default for SnapshotRenderer {
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

impl SnapshotRenderer {
    #[must_use]
    pub fn render(&mut self, page: &RenderedPage) -> RgbaImage {
        let mut output = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([255, 255, 255, 255]));
        for command in &page.display_list.commands {
            match command {
                DisplayCommand::Box {
                    bounds,
                    clip,
                    background,
                    border,
                    border_width_px,
                } => draw_box(
                    &mut output,
                    *bounds,
                    *clip,
                    *background,
                    *border,
                    *border_width_px,
                ),
                DisplayCommand::Text {
                    bounds,
                    clip,
                    content,
                    size_px,
                    color,
                    line_height_px,
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
                ),
                DisplayCommand::Image {
                    bounds,
                    clip,
                    source,
                    loaded,
                    ..
                } => {
                    if *loaded {
                        if let Some(bytes) = page.image_resources.get(source) {
                            draw_image(&mut output, *bounds, *clip, bytes);
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

fn draw_box(
    output: &mut RgbaImage,
    bounds: Rect,
    clip: Option<Rect>,
    background: Option<PaintColor>,
    border: Option<PaintColor>,
    border_width_px: f32,
) {
    if let Some(background) = background {
        fill_rect(output, bounds, clip, paint_rgba(background));
    }
    if border_width_px > 0.0 {
        if let Some(border) = border {
            let width = border_width_px.ceil();
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

fn draw_image(output: &mut RgbaImage, bounds: Rect, clip: Option<Rect>, bytes: &[u8]) {
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
            pixel.0,
            clip,
        );
    }
}

fn fill_rect(output: &mut RgbaImage, rect: Rect, clip: Option<Rect>, color: [u8; 4]) {
    let start_x = rect.x.floor().max(0.0) as u32;
    let start_y = rect.y.floor().max(0.0) as u32;
    let end_x = (rect.x + rect.width).ceil().clamp(0.0, WIDTH as f32) as u32;
    let end_y = (rect.y + rect.height).ceil().clamp(0.0, HEIGHT as f32) as u32;
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
    if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
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
    use std::fs;

    use arch_net::Loader;
    use url::Url;

    use super::*;

    #[test]
    fn difference_ratio_counts_changed_pixels() {
        let first = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]));
        let mut second = first.clone();
        second.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        assert!((difference_ratio(&first, &second) - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn renders_text_boxes_and_images() {
        let page = crate::render_html(
            &url::Url::parse("file:///snapshot.html").unwrap(),
            "<style>body { background-color: #eef4ff } h1 { color: #2457c5 }</style><h1>Snapshot</h1>",
            VIEWPORT_WIDTH,
        );
        let snapshot = SnapshotRenderer::default().render(&page);
        assert_eq!(snapshot.dimensions(), (WIDTH, HEIGHT));
        assert!(
            snapshot
                .pixels()
                .any(|pixel| pixel.0 != [255, 255, 255, 255])
        );
    }

    #[test]
    fn fixed_font_shapes_english_and_chinese_glyphs() {
        let url = url::Url::parse("file:///font-shaping.html").unwrap();
        for text in ["English shaping", "中文塑形"] {
            let page = crate::render_html(&url, &format!("<p>{text}</p>"), VIEWPORT_WIDTH);
            let snapshot = SnapshotRenderer::default().render(&page);
            assert!(
                snapshot
                    .pixels()
                    .any(|pixel| pixel.0 != [255, 255, 255, 255]),
                "fixed snapshot font did not rasterize {text}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn fixture_snapshots_match_references() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pages = root.join("fixtures/pages");
        let references = root.join("fixtures/screenshots");
        let mut directories = fs::read_dir(&pages)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        directories.sort();
        assert_eq!(directories.len(), 30);

        let loader = Loader::default();
        let mut renderer = SnapshotRenderer::default();
        for directory in directories {
            let name = directory.file_name().unwrap().to_str().unwrap();
            let source = directory.join("index.html");
            let page = crate::render_url(
                &loader,
                &Url::from_file_path(&source).unwrap(),
                VIEWPORT_WIDTH,
            )
            .unwrap();
            let actual = renderer.render(&page);
            let expected = load_snapshot(&references.join(format!("{name}.png"))).unwrap();
            let difference = difference_ratio(&actual, &expected);
            assert!(
                difference <= 0.005,
                "{name} snapshot difference {:.4}% exceeds 0.5%",
                difference * 100.0
            );
        }
    }
}
