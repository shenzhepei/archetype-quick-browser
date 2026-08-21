use arch_layout::{LayoutTree, Rect};
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
    },
    Image {
        bounds: Rect,
        clip: Option<Rect>,
        source: String,
        alt: String,
        intrinsic_width: u32,
        intrinsic_height: u32,
        loaded: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaintColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayList {
    pub commands: Vec<DisplayCommand>,
    pub content_height: f32,
}

#[must_use]
pub fn paint(tree: &LayoutTree) -> DisplayList {
    DisplayList {
        commands: tree
            .boxes
            .iter()
            .flat_map(|item| {
                let background = item.background_color.as_deref().and_then(parse_color);
                let border = item.border_color.as_deref().and_then(parse_color);
                let box_command = (background.is_some() || item.border_width_px > 0.0).then_some(
                    DisplayCommand::Box {
                        bounds: item.bounds,
                        clip: item.clip,
                        background,
                        border,
                        border_width_px: item.border_width_px,
                    },
                );
                let text = item.text.as_ref().map(|content| DisplayCommand::Text {
                    bounds: item.bounds,
                    clip: item.clip,
                    content: content.clone(),
                    size_px: item.font_size_px,
                    font_family: item.font_family.clone(),
                    link: item.link.clone(),
                    color: item.color.as_deref().and_then(parse_color),
                    line_height_px: item.line_height_px,
                    white_space: item.white_space,
                    font_weight: item.font_weight,
                    font_style: item.font_style,
                    text_align: item.text_align,
                });
                let image = item.image.as_ref().map(|image| DisplayCommand::Image {
                    bounds: item.bounds,
                    clip: item.clip,
                    source: image.source.clone(),
                    alt: image.alt.clone(),
                    intrinsic_width: image.intrinsic_width,
                    intrinsic_height: image.intrinsic_height,
                    loaded: image.loaded,
                });
                box_command.into_iter().chain(text).chain(image)
            })
            .collect(),
        content_height: tree.content_height,
    }
}

fn parse_color(value: &str) -> Option<PaintColor> {
    let value = value.trim().to_ascii_lowercase();
    let (red, green, blue) = match value.as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        value if value.starts_with('#') && value.len() == 4 => {
            let mut digits = value[1..].chars();
            let red = hex_digit(digits.next()?)?;
            let green = hex_digit(digits.next()?)?;
            let blue = hex_digit(digits.next()?)?;
            (red * 17, green * 17, blue * 17)
        }
        value if value.starts_with('#') && value.len() == 7 => (
            u8::from_str_radix(&value[1..3], 16).ok()?,
            u8::from_str_radix(&value[3..5], 16).ok()?,
            u8::from_str_radix(&value[5..7], 16).ok()?,
        ),
        _ => return None,
    };
    Some(PaintColor {
        red,
        green,
        blue,
        alpha: 255,
    })
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
                text_align: TextAlign::Start,
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
}
