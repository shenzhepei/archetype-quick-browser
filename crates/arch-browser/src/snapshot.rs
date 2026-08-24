use std::path::Path;

use anyhow::Result;
use archetype_raster::Rasterizer;
use image::RgbaImage;

use crate::RenderedPage;

pub use archetype_raster::{HEIGHT, VIEWPORT_WIDTH, WIDTH, difference_ratio, load_snapshot};

#[derive(Default)]
pub struct SnapshotRenderer(Rasterizer);

impl SnapshotRenderer {
    #[must_use]
    pub fn render(&mut self, page: &RenderedPage) -> RgbaImage {
        self.0
            .render(WIDTH, HEIGHT, &page.display_list, &page.image_resources)
    }
}

/// Saves a rendered reference image as PNG.
///
/// # Errors
/// Returns an error when the parent directory cannot be created or the PNG cannot be written.
pub fn save_snapshot(image: &RgbaImage, path: &Path) -> Result<()> {
    archetype_raster::save_snapshot(image, path)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use arch_net::Loader;
    use image::{Rgba, RgbaImage};
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
        assert_eq!(directories.len(), 50);

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
