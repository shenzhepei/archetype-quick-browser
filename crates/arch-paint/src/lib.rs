use arch_layout::{LayoutTree, Rect};
pub use arch_style::TextDecoration;
use arch_style::{FontStyle, FontWeight, TextAlign, WhiteSpace};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DisplayCommand {
    Box {
        bounds: Rect,
        clip: Option<Rect>,
        background: Option<PaintColor>,
        border: Option<PaintColor>,
        border_width_px: f32,
        border_radius_px: f32,
        shadow: Option<PaintShadow>,
    },
    Text {
        bounds: Rect,
        clip: Option<Rect>,
        content: String,
        size_px: f32,
        font_family: Option<String>,
        link: Option<String>,
        color: Option<PaintColor>,
        line_height_px: f32,
        white_space: WhiteSpace,
        font_weight: FontWeight,
        font_style: FontStyle,
        text_align: TextAlign,
        text_decoration: TextDecoration,
    },
    Image {
        bounds: Rect,
        clip: Option<Rect>,
        source: String,
        alt: String,
        intrinsic_width: u32,
        intrinsic_height: u32,
        loaded: bool,
        opacity: f32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaintColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaintShadow {
    pub offset_x_px: f32,
    pub offset_y_px: f32,
    pub blur_px: f32,
    pub color: PaintColor,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayList {
    pub commands: Vec<DisplayCommand>,
    pub content_height: f32,
}

#[must_use]
pub fn paint(tree: &LayoutTree) -> DisplayList {
    let mut boxes = tree.boxes.iter().collect::<Vec<_>>();
    boxes.sort_by_key(|item| (item.z_index, item.paint_order));
    DisplayList {
        commands: boxes
            .into_iter()
            .flat_map(|item| {
                let background = item
                    .background_color
                    .as_deref()
                    .and_then(parse_color)
                    .map(|color| apply_opacity(color, item.opacity));
                let border = item
                    .border_color
                    .as_deref()
                    .and_then(parse_color)
                    .map(|color| apply_opacity(color, item.opacity));
                let shadow = item.box_shadow.as_ref().and_then(|shadow| {
                    Some(PaintShadow {
                        offset_x_px: shadow.offset_x_px,
                        offset_y_px: shadow.offset_y_px,
                        blur_px: shadow.blur_px,
                        color: apply_opacity(parse_color(&shadow.color)?, item.opacity),
                    })
                });
                let box_command =
                    (background.is_some() || item.border_width_px > 0.0 || shadow.is_some())
                        .then_some(DisplayCommand::Box {
                            bounds: item.bounds,
                            clip: item.clip,
                            background,
                            border,
                            border_width_px: item.border_width_px,
                            border_radius_px: item.border_radius_px,
                            shadow,
                        });
                let text = item.text.as_ref().map(|content| DisplayCommand::Text {
                    bounds: item.bounds,
                    clip: item.clip,
                    content: content.clone(),
                    size_px: item.font_size_px,
                    font_family: item.font_family.clone(),
                    link: item.link.clone(),
                    color: item
                        .color
                        .as_deref()
                        .and_then(parse_color)
                        .map(|color| apply_opacity(color, item.opacity)),
                    line_height_px: item.line_height_px,
                    white_space: item.white_space,
                    font_weight: item.font_weight,
                    font_style: item.font_style,
                    text_align: item.text_align,
                    text_decoration: item.text_decoration,
                });
                let image = item.image.as_ref().map(|image| DisplayCommand::Image {
                    bounds: item.bounds,
                    clip: item.clip,
                    source: image.source.clone(),
                    alt: image.alt.clone(),
                    intrinsic_width: image.intrinsic_width,
                    intrinsic_height: image.intrinsic_height,
                    loaded: image.loaded,
                    opacity: item.opacity,
                });
                box_command.into_iter().chain(text).chain(image)
            })
            .collect(),
        content_height: tree.content_height,
    }
}

fn parse_color(value: &str) -> Option<PaintColor> {
    let value = value.trim().to_ascii_lowercase();
    let (red, green, blue, alpha) = match value.as_str() {
        "black" => (0, 0, 0, 255),
        "white" => (255, 255, 255, 255),
        "red" => (255, 0, 0, 255),
        "green" => (0, 128, 0, 255),
        "blue" => (0, 0, 255, 255),
        "transparent" => (0, 0, 0, 0),
        value if value.starts_with('#') && value.len() == 4 => {
            let mut digits = value[1..].chars();
            let red = hex_digit(digits.next()?)?;
            let green = hex_digit(digits.next()?)?;
            let blue = hex_digit(digits.next()?)?;
            (red * 17, green * 17, blue * 17, 255)
        }
        value if value.starts_with('#') && value.len() == 5 => {
            let mut digits = value[1..].chars();
            let red = hex_digit(digits.next()?)?;
            let green = hex_digit(digits.next()?)?;
            let blue = hex_digit(digits.next()?)?;
            let alpha = hex_digit(digits.next()?)?;
            (red * 17, green * 17, blue * 17, alpha * 17)
        }
        value if value.starts_with('#') && value.len() == 7 => (
            u8::from_str_radix(&value[1..3], 16).ok()?,
            u8::from_str_radix(&value[3..5], 16).ok()?,
            u8::from_str_radix(&value[5..7], 16).ok()?,
            255,
        ),
        value if value.starts_with('#') && value.len() == 9 => (
            u8::from_str_radix(&value[1..3], 16).ok()?,
            u8::from_str_radix(&value[3..5], 16).ok()?,
            u8::from_str_radix(&value[5..7], 16).ok()?,
            u8::from_str_radix(&value[7..9], 16).ok()?,
        ),
        _ => return None,
    };
    Some(PaintColor {
        red,
        green,
        blue,
        alpha,
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn apply_opacity(mut color: PaintColor, opacity: f32) -> PaintColor {
    color.alpha = (f32::from(color.alpha) * opacity.clamp(0.0, 1.0)).round() as u8;
    color
}

fn hex_digit(value: char) -> Option<u8> {
    value
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
}

#[cfg(test)]
mod tests {
    use arch_dom::NodeId;
    use arch_layout::{LayoutBox, Rect};

    use super::*;

    #[test]
    fn preserves_computed_typography_in_text_command() {
        let tree = LayoutTree {
            boxes: vec![LayoutBox {
                node_id: NodeId(1),
                bounds: Rect::default(),
                clip: Some(Rect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                }),
                text: Some("large".to_owned()),
                image: None,
                link: None,
                font_size_px: 28.0,
                font_family: Some("Helvetica Neue".to_owned()),
                color: Some("#2468ac".to_owned()),
                line_height_px: 39.2,
                white_space: WhiteSpace::Normal,
                font_weight: FontWeight::Bold,
                font_style: FontStyle::Normal,
                background_color: Some("#fff".to_owned()),
                border_color: Some("blue".to_owned()),
                border_width_px: 1.0,
                border_radius_px: 0.0,
                opacity: 1.0,
                box_shadow: None,
                text_align: TextAlign::Start,
                text_decoration: TextDecoration::None,
                z_index: 0,
                paint_order: 0,
            }],
            content_height: 40.0,
        };
        assert!(matches!(
            paint(&tree).commands.as_slice(),
            [DisplayCommand::Box { .. }, DisplayCommand::Text { size_px, .. }]
                if (*size_px - 28.0).abs() < f32::EPSILON
        ));
        assert!(matches!(
            paint(&tree).commands.as_slice(),
            [DisplayCommand::Box { .. }, DisplayCommand::Text { font_family: Some(family), .. }]
                if family == "Helvetica Neue"
        ));
        assert!(paint(&tree).commands.iter().all(|command| matches!(
            command,
            DisplayCommand::Box { clip: Some(_), .. } | DisplayCommand::Text { clip: Some(_), .. }
        )));
        assert!(matches!(
            paint(&tree).commands.as_slice(),
            [
                DisplayCommand::Box { .. },
                DisplayCommand::Text {
                    color: Some(PaintColor {
                        red: 36,
                        green: 104,
                        blue: 172,
                        alpha: 255
                    }),
                    ..
                }
            ]
        ));
        assert!(matches!(
            paint(&tree).commands.as_slice(),
            [DisplayCommand::Box {
                background: Some(PaintColor { red: 255, green: 255, blue: 255, alpha: 255 }),
                border: Some(PaintColor { red: 0, green: 0, blue: 255, alpha: 255 }),
                border_width_px,
                ..
            }, _] if (*border_width_px - 1.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn orders_boxes_by_z_index_then_document_order() {
        let layout_box = |node, z_index, paint_order, color: &str| LayoutBox {
            node_id: NodeId(node),
            bounds: Rect::default(),
            clip: None,
            text: None,
            image: None,
            link: None,
            font_size_px: 16.0,
            font_family: None,
            color: None,
            line_height_px: 20.0,
            white_space: WhiteSpace::Normal,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            background_color: Some(color.to_owned()),
            border_color: None,
            border_width_px: 0.0,
            border_radius_px: 0.0,
            opacity: 1.0,
            box_shadow: None,
            text_align: TextAlign::Start,
            text_decoration: TextDecoration::None,
            z_index,
            paint_order,
        };
        let tree = LayoutTree {
            boxes: vec![
                layout_box(1, 2, 0, "red"),
                layout_box(2, -1, 1, "blue"),
                layout_box(3, 2, 2, "green"),
            ],
            content_height: 0.0,
        };
        let colors = paint(&tree)
            .commands
            .iter()
            .filter_map(|command| match command {
                DisplayCommand::Box { background, .. } => *background,
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(colors[0], parse_color("blue").unwrap());
        assert_eq!(colors[1], parse_color("red").unwrap());
        assert_eq!(colors[2], parse_color("green").unwrap());
    }
}
