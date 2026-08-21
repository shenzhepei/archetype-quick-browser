use std::collections::{BTreeMap, HashMap};

use arch_css::Stylesheet;
use arch_dom::{Document, ElementData, NodeId, NodeKind};
use serde::{Deserialize, Serialize};

type Specificity = (u16, u16, u16);
type CascadeWinner<'a> = (bool, Specificity, usize, &'a str);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Display {
    #[default]
    Inline,
    Block,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ComputedLength {
    Px(f32),
    Percent(f32),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Pre,
    PreWrap,
    NoWrap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeSizes {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: Option<String>,
    pub background_color: Option<String>,
    pub border_color: Option<String>,
    pub font_size_px: f32,
    pub font_family: Option<String>,
    pub line_height_px: f32,
    pub white_space: WhiteSpace,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub text_align: TextAlign,
    pub margin: EdgeSizes,
    pub padding: EdgeSizes,
    pub border_px: f32,
    pub width: Option<ComputedLength>,
    pub height: Option<ComputedLength>,
    pub min_width: Option<ComputedLength>,
    pub max_width: Option<ComputedLength>,
    pub box_sizing: BoxSizing,
    pub overflow: Overflow,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyledNode {
    pub node_id: NodeId,
    pub style: ComputedStyle,
}

#[must_use]
pub fn style_document(document: &Document, stylesheet: &Stylesheet) -> Vec<StyledNode> {
    let mut output = Vec::new();
    let mut computed_by_node = HashMap::new();
    for node in document.descendants(document.root()) {
        let inherited = node.parent.and_then(|parent| computed_by_node.get(&parent));
        let mut style = ua_style(&node.kind, inherited);
        if matches!(&node.kind, NodeKind::Element(_)) {
            let mut winners: BTreeMap<&str, CascadeWinner<'_>> = BTreeMap::new();
            for rule in &stylesheet.rules {
                if selector_matches(document, node.id, &rule.selector) {
                    let specificity = specificity(&rule.selector);
                    for declaration in &rule.declarations {
                        let candidate = (
                            declaration.important,
                            specificity,
                            rule.source_order,
                            declaration.value.as_str(),
                        );
                        if winners
                            .get(declaration.name.as_str())
                            .is_none_or(|existing| candidate > *existing)
                        {
                            winners.insert(declaration.name.as_str(), candidate);
                        }
                    }
                }
            }
            apply(&mut style, &winners);
        }
        computed_by_node.insert(node.id, style.clone());
        output.push(StyledNode {
            node_id: node.id,
            style,
        });
    }
    output
}

fn ua_style(kind: &NodeKind, inherited: Option<&ComputedStyle>) -> ComputedStyle {
    let hidden = matches!(
        kind,
        NodeKind::Element(ElementData { name, .. })
            if matches!(name.as_str(), "head" | "title" | "meta" | "link" | "style" | "script")
    );
    let block = match kind {
        NodeKind::Document => true,
        NodeKind::Element(ElementData { name, .. }) => matches!(
            name.as_str(),
            "html"
                | "body"
                | "main"
                | "article"
                | "section"
                | "header"
                | "footer"
                | "nav"
                | "div"
                | "p"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "ul"
                | "ol"
                | "li"
                | "pre"
        ),
        NodeKind::Text(_) => false,
    };
    let heading_font_size = match kind {
        NodeKind::Element(ElementData { name, .. }) => match name.as_str() {
            "h1" => Some(32.0),
            "h2" => Some(24.0),
            "h3" => Some(18.72),
            "h4" => Some(16.0),
            "h5" => Some(13.28),
            "h6" => Some(10.72),
            _ => None,
        },
        _ => None,
    };
    let default_font_size = heading_font_size.unwrap_or(16.0);
    let has_bold_default = matches!(
        kind,
        NodeKind::Element(ElementData { name, .. })
            if matches!(name.as_str(), "strong" | "b" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
    );
    let font_weight = match kind {
        _ if has_bold_default => FontWeight::Bold,
        _ => FontWeight::Normal,
    };
    let has_italic_default = matches!(kind, NodeKind::Element(ElementData { name, .. }) if matches!(name.as_str(), "em" | "i"));
    let font_style = match kind {
        _ if has_italic_default => FontStyle::Italic,
        _ => FontStyle::Normal,
    };
    let has_pre_default =
        matches!(kind, NodeKind::Element(ElementData { name, .. }) if name == "pre");
    let white_space = match kind {
        _ if has_pre_default => WhiteSpace::Pre,
        _ => WhiteSpace::Normal,
    };
    let mut style = ComputedStyle {
        display: if hidden {
            Display::None
        } else if block {
            Display::Block
        } else {
            Display::Inline
        },
        font_size_px: default_font_size,
        line_height_px: default_font_size * 1.4,
        font_weight,
        font_style,
        white_space,
        ..ComputedStyle::default()
    };
    if let Some(parent) = inherited {
        style.color.clone_from(&parent.color);
        style.font_family.clone_from(&parent.font_family);
        if heading_font_size.is_none() {
            style.font_size_px = parent.font_size_px;
            style.line_height_px = parent.line_height_px;
        }
        if !has_pre_default {
            style.white_space = parent.white_space;
        }
        if !has_bold_default {
            style.font_weight = parent.font_weight;
        }
        if !has_italic_default {
            style.font_style = parent.font_style;
        }
        style.text_align = parent.text_align;
    }
    style
}

fn selector_matches(document: &Document, node_id: NodeId, selector: &str) -> bool {
    if selector.contains([',', ':', '[']) {
        return false;
    }
    let tokens = selector
        .replace('>', " > ")
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    matches_tokens(document, node_id, &tokens)
}

fn matches_tokens(document: &Document, node_id: NodeId, tokens: &[String]) -> bool {
    let Some((last, preceding)) = tokens.split_last() else {
        return false;
    };
    if last == ">" || !matches_simple(document, node_id, last) {
        return false;
    }
    if preceding.is_empty() {
        return true;
    }
    let direct = preceding.last().is_some_and(|token| token == ">");
    let remaining = if direct {
        &preceding[..preceding.len() - 1]
    } else {
        preceding
    };
    let Some(parent) = document.node(node_id).and_then(|node| node.parent) else {
        return false;
    };
    if direct {
        return matches_tokens(document, parent, remaining);
    }
    let mut ancestor = Some(parent);
    while let Some(id) = ancestor {
        if matches_tokens(document, id, remaining) {
            return true;
        }
        ancestor = document.node(id).and_then(|node| node.parent);
    }
    false
}

fn matches_simple(document: &Document, node_id: NodeId, selector: &str) -> bool {
    let Some(NodeKind::Element(element)) = document.node(node_id).map(|node| &node.kind) else {
        return false;
    };
    let type_end = selector.find(['.', '#']).unwrap_or(selector.len());
    let element_type = &selector[..type_end];
    if !element_type.is_empty() && element_type != "*" && element.name != element_type {
        return false;
    }
    let mut rest = &selector[type_end..];
    while !rest.is_empty() {
        let marker = &rest[..1];
        rest = &rest[1..];
        let end = rest.find(['.', '#']).unwrap_or(rest.len());
        let value = &rest[..end];
        let matched = match marker {
            "#" => element.attribute("id") == Some(value),
            "." => element
                .attribute("class")
                .is_some_and(|classes| classes.split_whitespace().any(|item| item == value)),
            _ => false,
        };
        if !matched {
            return false;
        }
        rest = &rest[end..];
    }
    true
}

fn specificity(selector: &str) -> Specificity {
    let ids = selector.matches('#').count();
    let classes = selector.matches('.').count();
    let types = selector
        .replace('>', " ")
        .split_whitespace()
        .filter(|part| !part.starts_with(['.', '#', '*']))
        .count();
    (
        u16::try_from(ids).unwrap_or(u16::MAX),
        u16::try_from(classes).unwrap_or(u16::MAX),
        u16::try_from(types).unwrap_or(u16::MAX),
    )
}

fn apply(style: &mut ComputedStyle, declarations: &BTreeMap<&str, CascadeWinner<'_>>) {
    if let Some((_, _, _, value)) = declarations.get("font-size") {
        style.font_size_px =
            absolute_length(value, style.font_size_px).unwrap_or(style.font_size_px);
        style.line_height_px = style.font_size_px * 1.4;
    }
    if let Some((_, _, _, value)) = declarations.get("line-height") {
        style.line_height_px =
            parse_line_height(value, style.font_size_px).unwrap_or(style.line_height_px);
    }
    for (name, (_, _, _, value)) in declarations {
        if matches!(*name, "font-size" | "line-height") {
            continue;
        }
        match *name {
            "display" => {
                style.display = match *value {
                    "block" => Display::Block,
                    "none" => Display::None,
                    _ => Display::Inline,
                };
            }
            "color" => style.color = Some((*value).to_owned()),
            "background-color" => style.background_color = Some((*value).to_owned()),
            "border-color" => style.border_color = Some((*value).to_owned()),
            "font-family" => {
                if let Some(family) = arch_css::first_font_family(value) {
                    style.font_family = Some(family);
                }
            }
            "margin" => {
                if let Some(edges) = edge_sizes(value, style.font_size_px) {
                    style.margin = edges;
                }
            }
            "padding" => {
                if let Some(edges) = edge_sizes(value, style.font_size_px) {
                    style.padding = edges;
                }
            }
            "margin-top" => set_edge(&mut style.margin.top, value, style.font_size_px),
            "margin-right" => set_edge(&mut style.margin.right, value, style.font_size_px),
            "margin-bottom" => set_edge(&mut style.margin.bottom, value, style.font_size_px),
            "margin-left" => set_edge(&mut style.margin.left, value, style.font_size_px),
            "padding-top" => set_edge(&mut style.padding.top, value, style.font_size_px),
            "padding-right" => set_edge(&mut style.padding.right, value, style.font_size_px),
            "padding-bottom" => set_edge(&mut style.padding.bottom, value, style.font_size_px),
            "padding-left" => set_edge(&mut style.padding.left, value, style.font_size_px),
            "border-width" => {
                style.border_px =
                    absolute_length(value, style.font_size_px).unwrap_or(style.border_px);
            }
            "width" => style.width = length(value, style.font_size_px),
            "height" => style.height = length(value, style.font_size_px),
            "min-width" => style.min_width = length(value, style.font_size_px),
            "max-width" => style.max_width = length(value, style.font_size_px),
            "box-sizing" => {
                style.box_sizing = if *value == "border-box" {
                    BoxSizing::BorderBox
                } else {
                    BoxSizing::ContentBox
                };
            }
            "overflow" => match *value {
                "visible" => style.overflow = Overflow::Visible,
                "hidden" => style.overflow = Overflow::Hidden,
                _ => {}
            },
            "white-space" => {
                style.white_space = match *value {
                    "pre" => WhiteSpace::Pre,
                    "pre-wrap" => WhiteSpace::PreWrap,
                    "nowrap" => WhiteSpace::NoWrap,
                    _ => WhiteSpace::Normal,
                };
            }
            "font-weight" => {
                style.font_weight = if matches!(*value, "bold" | "700" | "800" | "900") {
                    FontWeight::Bold
                } else {
                    FontWeight::Normal
                };
            }
            "font-style" => {
                style.font_style = if matches!(*value, "italic" | "oblique") {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                };
            }
            "text-align" => {
                style.text_align = match *value {
                    "center" => TextAlign::Center,
                    "right" | "end" => TextAlign::End,
                    _ => TextAlign::Start,
                };
            }
            _ => {}
        }
    }
}

fn set_edge(edge: &mut f32, value: &str, em_px: f32) {
    if let Some(parsed) = absolute_length(value, em_px) {
        *edge = parsed;
    }
}

fn edge_sizes(value: &str, em_px: f32) -> Option<EdgeSizes> {
    let values = value
        .split_whitespace()
        .map(|part| absolute_length(part, em_px))
        .collect::<Option<Vec<_>>>()?;
    match values.as_slice() {
        [all] => Some(EdgeSizes {
            top: *all,
            right: *all,
            bottom: *all,
            left: *all,
        }),
        [vertical, horizontal] => Some(EdgeSizes {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        }),
        [top, horizontal, bottom] => Some(EdgeSizes {
            top: *top,
            right: *horizontal,
            bottom: *bottom,
            left: *horizontal,
        }),
        [top, right, bottom, left] => Some(EdgeSizes {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        }),
        _ => None,
    }
}

fn parse_line_height(value: &str, font_size_px: f32) -> Option<f32> {
    let value = value.trim();
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse::<f32>().ok().filter(|value| *value > 0.0);
    }
    value
        .parse::<f32>()
        .ok()
        .filter(|value| *value > 0.0)
        .map(|factor| factor * font_size_px)
}

fn absolute_length(value: &str, em_px: f32) -> Option<f32> {
    match length(value, em_px)? {
        ComputedLength::Px(value) => Some(value),
        ComputedLength::Percent(_) => None,
    }
}

fn length(value: &str, em_px: f32) -> Option<ComputedLength> {
    let value = value.trim();
    let parsed = if let Some(number) = value.strip_suffix("px") {
        ComputedLength::Px(number.trim().parse().ok()?)
    } else if let Some(number) = value.strip_suffix('%') {
        ComputedLength::Percent(number.trim().parse::<f32>().ok()? / 100.0)
    } else if let Some(number) = value.strip_suffix("rem") {
        ComputedLength::Px(number.trim().parse::<f32>().ok()? * 16.0)
    } else if let Some(number) = value.strip_suffix("em") {
        ComputedLength::Px(number.trim().parse::<f32>().ok()? * em_px)
    } else if value == "0" {
        ComputedLength::Px(0.0)
    } else {
        return None;
    };
    Some(parsed)
}

#[cfg(test)]
mod tests {
    use arch_css::parse;
    use arch_html::parse as parse_html;

    use super::*;

    #[test]
    fn id_rule_wins_over_type_rule() {
        let document = parse_html("<p id='lead'>hello</p>");
        let styled = style_document(&document, &parse("#lead { color: red } p { color: blue }"));
        assert!(
            styled
                .iter()
                .any(|node| node.style.color.as_deref() == Some("red"))
        );
    }

    #[test]
    fn descendant_child_and_compound_selectors_match() {
        let document =
            parse_html("<main><section><p class='notice primary'>hello</p></section></main>");
        let styled = style_document(
            &document,
            &parse("main p.notice { color: red } main > p { color: blue }"),
        );
        assert!(
            styled
                .iter()
                .any(|node| node.style.color.as_deref() == Some("red"))
        );
        assert!(
            !styled
                .iter()
                .any(|node| node.style.color.as_deref() == Some("blue"))
        );
    }

    #[test]
    fn color_and_font_size_inherit() {
        let document = parse_html("<section><span>hello</span></section>");
        let styled = style_document(
            &document,
            &parse("section { color: green; font-size: 20px }"),
        );
        let span_id = document
            .descendants(document.root())
            .find(|node| matches!(&node.kind, NodeKind::Element(element) if element.name == "span"))
            .unwrap()
            .id;
        let span = styled.iter().find(|node| node.node_id == span_id).unwrap();
        assert_eq!(span.style.color.as_deref(), Some("green"));
        assert!((span.style.font_size_px - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn font_family_parses_and_inherits_to_text() {
        let document = parse_html("<section><span>hello</span></section>");
        let styled = style_document(
            &document,
            &parse(
                r#"section { font-family: "Helvetica Neue", sans-serif }
                   span { font-family: var(--font) }"#,
            ),
        );
        let text_id = document
            .descendants(document.root())
            .find(|node| matches!(&node.kind, NodeKind::Text(value) if value == "hello"))
            .unwrap()
            .id;
        let text = styled.iter().find(|node| node.node_id == text_id).unwrap();
        assert_eq!(text.style.font_family.as_deref(), Some("Helvetica Neue"));
    }

    #[test]
    fn computes_supported_box_lengths() {
        let document = parse_html("<div id='box'></div>");
        let styled = style_document(
            &document,
            &parse(
                "#box { font-size: 20px; width: 50%; height: 2em; min-width: 10rem; \
                 border-width: 3px; box-sizing: border-box }",
            ),
        );
        let style = styled
            .iter()
            .find(|node| {
                document.node(node.node_id).is_some_and(
                    |dom| matches!(&dom.kind, NodeKind::Element(element) if element.attribute("id") == Some("box")),
                )
            })
            .unwrap();
        assert_eq!(style.style.width, Some(ComputedLength::Percent(0.5)));
        assert_eq!(style.style.height, Some(ComputedLength::Px(40.0)));
        assert_eq!(style.style.min_width, Some(ComputedLength::Px(160.0)));
        assert!((style.style.border_px - 3.0).abs() < f32::EPSILON);
        assert_eq!(style.style.box_sizing, BoxSizing::BorderBox);
    }

    #[test]
    fn computes_overflow_without_inheriting_it() {
        let document = parse_html("<section><div>child</div></section>");
        let styled = style_document(&document, &parse("section { overflow: hidden }"));
        let style_for = |name: &str| {
            let id = document
                .descendants(document.root())
                .find(
                    |node| matches!(&node.kind, NodeKind::Element(element) if element.name == name),
                )
                .unwrap()
                .id;
            styled
                .iter()
                .find(|node| node.node_id == id)
                .unwrap()
                .style
                .overflow
        };
        assert_eq!(style_for("section"), Overflow::Hidden);
        assert_eq!(style_for("div"), Overflow::Visible);
    }

    #[test]
    fn computes_box_shorthand_and_side_overrides() {
        let document = parse_html("<div id='box'></div>");
        let styled = style_document(
            &document,
            &parse(
                "#box { margin: 1px 2px 3px 4px; padding: 5px 6px; \
                 padding-left: 9px }",
            ),
        );
        let style = &styled
            .iter()
            .find(|node| {
                document.node(node.node_id).is_some_and(
                    |dom| matches!(&dom.kind, NodeKind::Element(element) if element.attribute("id") == Some("box")),
                )
            })
            .unwrap()
            .style;
        assert_eq!(
            style.margin,
            EdgeSizes {
                top: 1.0,
                right: 2.0,
                bottom: 3.0,
                left: 4.0
            }
        );
        assert_eq!(
            style.padding,
            EdgeSizes {
                top: 5.0,
                right: 6.0,
                bottom: 5.0,
                left: 9.0
            }
        );
    }

    #[test]
    fn hides_non_rendered_document_elements() {
        let document = parse_html(
            "<head><title>Hidden</title><style>p { color: red }</style></head>\
             <body><script>hiddenCall()</script><p>Visible</p></body>",
        );
        let styled = style_document(&document, &parse(""));
        for name in ["head", "title", "style", "script"] {
            let id = document
                .descendants(document.root())
                .find(
                    |node| matches!(&node.kind, NodeKind::Element(element) if element.name == name),
                )
                .unwrap()
                .id;
            assert_eq!(
                styled
                    .iter()
                    .find(|node| node.node_id == id)
                    .unwrap()
                    .style
                    .display,
                Display::None
            );
        }
    }

    #[test]
    fn ua_typography_defaults_survive_inheritance_and_reach_text() {
        let document = parse_html(
            "<body><h1>Heading</h1><p><strong>Bold</strong> <em>Italic</em></p>\
             <pre>  preserved</pre></body>",
        );
        let styled = style_document(&document, &parse("body { font-size: 12px }"));
        let text_style = |content: &str| {
            let id = document
                .descendants(document.root())
                .find(|node| matches!(&node.kind, NodeKind::Text(text) if text.contains(content)))
                .unwrap()
                .id;
            &styled.iter().find(|node| node.node_id == id).unwrap().style
        };
        assert!((text_style("Heading").font_size_px - 32.0).abs() < f32::EPSILON);
        assert_eq!(text_style("Bold").font_weight, FontWeight::Bold);
        assert_eq!(text_style("Italic").font_style, FontStyle::Italic);
        assert_eq!(text_style("preserved").white_space, WhiteSpace::Pre);
    }
}
