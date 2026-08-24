use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasher,
};

use arch_dom::{Document, NodeId, NodeKind};
use arch_style::{
    BoxSizing, ComputedLength, Display, FlexAlign, FlexDirection, FlexJustify, FlexWrap, FontStyle,
    FontWeight, Overflow, StyledNode, TextAlign, WhiteSpace,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutBox {
    pub node_id: NodeId,
    pub bounds: Rect,
    pub clip: Option<Rect>,
    pub text: Option<String>,
    pub image: Option<ImageBox>,
    pub link: Option<String>,
    pub font_size_px: f32,
    pub font_family: Option<String>,
    pub color: Option<String>,
    pub line_height_px: f32,
    pub white_space: WhiteSpace,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub background_color: Option<String>,
    pub border_color: Option<String>,
    pub border_width_px: f32,
    pub text_align: TextAlign,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageBox {
    pub source: String,
    pub alt: String,
    pub intrinsic_width: u32,
    pub intrinsic_height: u32,
    pub loaded: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutTree {
    pub boxes: Vec<LayoutBox>,
    pub content_height: f32,
}

#[must_use]
pub fn layout<S: BuildHasher>(
    document: &Document,
    styled: &[StyledNode],
    viewport_width: f32,
    images: &HashMap<NodeId, ImageBox, S>,
    links: &HashMap<NodeId, String, S>,
) -> LayoutTree {
    let style_by_node: HashMap<_, _> = styled
        .iter()
        .map(|node| (node.node_id, &node.style))
        .collect();
    let mut context = LayoutContext {
        document,
        style_by_node,
        images,
        links,
        hidden: HashSet::new(),
        tree: LayoutTree::default(),
    };
    let mut cursor_y = 0.0;
    context.layout_node(document.root(), 0.0, viewport_width, None, &mut cursor_y);
    context.tree.content_height = cursor_y;
    context.tree
}

struct LayoutContext<'a, S> {
    document: &'a Document,
    style_by_node: HashMap<NodeId, &'a arch_style::ComputedStyle>,
    images: &'a HashMap<NodeId, ImageBox, S>,
    links: &'a HashMap<NodeId, String, S>,
    hidden: HashSet<NodeId>,
    tree: LayoutTree,
}

#[derive(Clone, Copy)]
struct FlexSeed {
    node_id: NodeId,
    basis: f32,
    grow: f32,
    shrink: f32,
}

struct FlexItem {
    start: usize,
    end: usize,
    x: f32,
    height: f32,
}

fn gap_count(item_count: usize) -> f32 {
    f32::from(u16::try_from(item_count.saturating_sub(1)).unwrap_or(u16::MAX))
}

fn flex_main_sizes(items: &[FlexSeed], available: f32, gap: f32) -> Vec<f32> {
    let mut sizes: Vec<_> = items.iter().map(|item| item.basis).collect();
    let occupied = sizes.iter().sum::<f32>() + gap * gap_count(items.len());
    let free = available - occupied;
    if free > 0.0 {
        let total_grow = items.iter().map(|item| item.grow).sum::<f32>();
        if total_grow > 0.0 {
            for (size, item) in sizes.iter_mut().zip(items) {
                *size += free * item.grow / total_grow;
            }
        }
    } else if free < 0.0 {
        let total_shrink = items
            .iter()
            .map(|item| item.shrink * item.basis)
            .sum::<f32>();
        if total_shrink > 0.0 {
            for (size, item) in sizes.iter_mut().zip(items) {
                *size = (*size + free * item.shrink * item.basis / total_shrink).max(0.0);
            }
        }
    }
    sizes
}

fn justify_offsets(justify: FlexJustify, free: f32, item_count: usize, gap: f32) -> (f32, f32) {
    let count = f32::from(u16::try_from(item_count).unwrap_or(u16::MAX));
    match justify {
        FlexJustify::Center => (free / 2.0, gap),
        FlexJustify::End => (free, gap),
        FlexJustify::SpaceBetween if item_count > 1 => (0.0, gap + free / (count - 1.0)),
        FlexJustify::SpaceAround if item_count > 0 => {
            let distributed = free / count;
            (distributed / 2.0, gap + distributed)
        }
        FlexJustify::SpaceEvenly if item_count > 0 => {
            let distributed = free / (count + 1.0);
            (distributed, gap + distributed)
        }
        FlexJustify::Start
        | FlexJustify::SpaceBetween
        | FlexJustify::SpaceAround
        | FlexJustify::SpaceEvenly => (0.0, gap),
    }
}

impl<S: BuildHasher> LayoutContext<'_, S> {
    fn layout_node(
        &mut self,
        node_id: NodeId,
        containing_x: f32,
        containing_width: f32,
        containing_height: Option<f32>,
        cursor_y: &mut f32,
    ) {
        let Some(node) = self.document.node(node_id) else {
            return;
        };
        let Some(style) = self.style_by_node.get(&node.id).copied() else {
            return;
        };
        let ancestor_hidden = node
            .parent
            .is_some_and(|parent| self.hidden.contains(&parent));
        if style.display == Display::None || ancestor_hidden {
            self.hidden.insert(node.id);
            return;
        }
        let text = match &node.kind {
            NodeKind::Text(text) if !text.trim().is_empty() => Some(match style.white_space {
                WhiteSpace::Pre | WhiteSpace::PreWrap => text.clone(),
                WhiteSpace::Normal | WhiteSpace::NoWrap => {
                    text.split_whitespace().collect::<Vec<_>>().join(" ")
                }
            }),
            _ => None,
        };
        let image = self.images.get(&node.id).cloned();
        let form_dimensions = form_control_dimensions(&node.kind);
        let creates_box = matches!(style.display, Display::Block | Display::Flex)
            || text.is_some()
            || image.is_some()
            || form_dimensions.is_some();
        if creates_box {
            let horizontal_edges = style.padding.left + style.padding.right + style.border_px * 2.0;
            let vertical_edges = style.padding.top + style.padding.bottom + style.border_px * 2.0;
            let width = resolve_box_width(style, containing_width);
            let content_width = (width - horizontal_edges).max(0.0);
            let own_content_height = form_dimensions.map_or_else(
                || intrinsic_content_height(style, image.as_ref(), text.as_deref(), content_width),
                |(_, height)| height,
            );
            let x = containing_x + style.margin.left;
            let y = *cursor_y + style.margin.top;
            let box_index = self.tree.boxes.len();
            let bounds = Rect {
                x,
                y,
                width,
                height: 0.0,
            };
            self.tree.boxes.push(LayoutBox {
                node_id: node.id,
                bounds,
                clip: None,
                text,
                image,
                link: self.links.get(&node.id).cloned(),
                font_size_px: style.font_size_px,
                font_family: style.font_family.clone(),
                color: style.color.clone(),
                line_height_px: style.line_height_px,
                white_space: style.white_space,
                font_weight: style.font_weight,
                font_style: style.font_style,
                background_color: style.background_color.clone(),
                border_color: style.border_color.clone(),
                border_width_px: style.border_px,
                text_align: style.text_align,
            });
            let content_x = x + style.padding.left + style.border_px;
            let content_y = y + style.padding.top + style.border_px;
            let mut child_cursor = content_y + own_content_height;
            let specified_height = style
                .height
                .and_then(|value| resolve_height(value, containing_height))
                .map(|value| box_height(style, value));
            let child_containing_height =
                specified_height.map(|height| (height - vertical_edges).max(0.0));
            self.layout_container_children(
                &node.children,
                content_x,
                content_width,
                child_containing_height,
                style,
                &mut child_cursor,
            );
            let descendants_height = child_cursor - content_y;
            let natural_height = descendants_height.max(own_content_height) + vertical_edges;
            let height = specified_height.unwrap_or(natural_height);
            self.tree.boxes[box_index].bounds.height = height;
            self.clip_descendants(box_index, style.overflow, style.border_px);
            *cursor_y = y + height + style.margin.bottom;
        } else {
            for child in &node.children {
                self.layout_node(
                    *child,
                    containing_x,
                    containing_width,
                    containing_height,
                    cursor_y,
                );
            }
        }
    }

    fn layout_container_children(
        &mut self,
        children: &[NodeId],
        content_x: f32,
        content_width: f32,
        content_height: Option<f32>,
        style: &arch_style::ComputedStyle,
        cursor_y: &mut f32,
    ) {
        if style.display == Display::Flex {
            self.layout_flex_children(
                children,
                content_x,
                content_width,
                content_height,
                style,
                cursor_y,
            );
        } else {
            self.layout_children(children, content_x, content_width, content_height, cursor_y);
        }
    }

    fn clip_descendants(&mut self, box_index: usize, overflow: Overflow, border_px: f32) {
        if overflow != Overflow::Hidden {
            return;
        }
        let clip = inset_rect(self.tree.boxes[box_index].bounds, border_px);
        for descendant in &mut self.tree.boxes[box_index + 1..] {
            descendant.clip = Some(
                descendant
                    .clip
                    .map_or(clip, |existing| intersect_rect(existing, clip)),
            );
        }
    }

    fn layout_children(
        &mut self,
        children: &[NodeId],
        content_x: f32,
        content_width: f32,
        content_height: Option<f32>,
        cursor_y: &mut f32,
    ) {
        let mut inline_x = 0.0_f32;
        let mut line_height = 0.0_f32;
        for child_id in children {
            let Some(style) = self.style_by_node.get(child_id).copied() else {
                continue;
            };
            if style.display == Display::None {
                self.hidden.insert(*child_id);
                continue;
            }
            if matches!(style.display, Display::Block | Display::Flex) {
                if line_height > 0.0 {
                    *cursor_y += line_height;
                    inline_x = 0.0;
                    line_height = 0.0;
                }
                self.layout_node(
                    *child_id,
                    content_x,
                    content_width,
                    content_height,
                    cursor_y,
                );
                continue;
            }
            self.layout_inline_subtree(
                *child_id,
                content_x,
                content_width,
                cursor_y,
                &mut inline_x,
                &mut line_height,
            );
        }
        if line_height > 0.0 {
            *cursor_y += line_height;
        }
    }

    fn layout_flex_children(
        &mut self,
        children: &[NodeId],
        content_x: f32,
        content_width: f32,
        content_height: Option<f32>,
        container_style: &arch_style::ComputedStyle,
        cursor_y: &mut f32,
    ) {
        if matches!(
            container_style.flex_direction,
            FlexDirection::Column | FlexDirection::ColumnReverse
        ) {
            self.layout_flex_column(
                children,
                content_x,
                content_width,
                content_height,
                container_style,
                cursor_y,
            );
            return;
        }

        let mut seeds: Vec<_> = children
            .iter()
            .filter_map(|node_id| {
                let style = self.style_by_node.get(node_id).copied()?;
                if style.display == Display::None {
                    self.hidden.insert(*node_id);
                    return None;
                }
                Some(FlexSeed {
                    node_id: *node_id,
                    basis: self.flex_basis(*node_id, style, content_width),
                    grow: style.flex_grow,
                    shrink: style.flex_shrink,
                })
            })
            .collect();
        if container_style.flex_direction == FlexDirection::RowReverse {
            seeds.reverse();
        }
        let mut lines = Vec::<Vec<FlexSeed>>::new();
        for seed in seeds {
            let line = lines.last();
            let wraps = container_style.flex_wrap == FlexWrap::Wrap
                && line.is_some_and(|line| {
                    let used: f32 = line.iter().map(|item| item.basis).sum::<f32>()
                        + container_style.column_gap * gap_count(line.len());
                    !line.is_empty()
                        && used + container_style.column_gap + seed.basis > content_width
                });
            if lines.is_empty() || wraps {
                lines.push(Vec::new());
            }
            lines.last_mut().expect("line exists").push(seed);
        }

        let has_lines = !lines.is_empty();
        for line in lines {
            let sizes = flex_main_sizes(&line, content_width, container_style.column_gap);
            let occupied =
                sizes.iter().sum::<f32>() + container_style.column_gap * gap_count(sizes.len());
            let (mut main_offset, between) = justify_offsets(
                container_style.justify_content,
                (content_width - occupied).max(0.0),
                line.len(),
                container_style.column_gap,
            );
            let line_y = *cursor_y;
            let mut items = Vec::new();
            for (seed, size) in line.iter().zip(sizes) {
                let start = self.tree.boxes.len();
                let mut temporary_y = 0.0;
                self.layout_node(seed.node_id, 0.0, size, content_height, &mut temporary_y);
                let end = self.tree.boxes.len();
                if start == end {
                    continue;
                }
                self.tree.boxes[start].bounds.width = size;
                let height = self.tree.boxes[start].bounds.height;
                items.push(FlexItem {
                    start,
                    end,
                    x: content_x + main_offset,
                    height,
                });
                main_offset += size + between;
            }
            let line_height = items.iter().map(|item| item.height).fold(0.0_f32, f32::max);
            for item in items {
                let align_offset = match container_style.align_items {
                    FlexAlign::Center => (line_height - item.height) / 2.0,
                    FlexAlign::End => line_height - item.height,
                    FlexAlign::Start | FlexAlign::Stretch => 0.0,
                };
                self.move_boxes(item.start, item.end, item.x, line_y + align_offset);
                if container_style.align_items == FlexAlign::Stretch {
                    self.tree.boxes[item.start].bounds.height = line_height;
                }
            }
            *cursor_y += line_height + container_style.row_gap;
        }
        if has_lines {
            *cursor_y -= container_style.row_gap;
        }
    }

    fn layout_flex_column(
        &mut self,
        children: &[NodeId],
        content_x: f32,
        content_width: f32,
        content_height: Option<f32>,
        container_style: &arch_style::ComputedStyle,
        cursor_y: &mut f32,
    ) {
        let mut ordered = children.to_vec();
        if container_style.flex_direction == FlexDirection::ColumnReverse {
            ordered.reverse();
        }
        let start_y = *cursor_y;
        let mut items = Vec::new();
        for node_id in ordered {
            let start = self.tree.boxes.len();
            self.layout_node(node_id, content_x, content_width, content_height, cursor_y);
            let end = self.tree.boxes.len();
            if start < end {
                items.push((start, end));
                *cursor_y += container_style.row_gap;
            }
        }
        if !items.is_empty() {
            *cursor_y -= container_style.row_gap;
        }
        let occupied = *cursor_y - start_y;
        let free = content_height.map_or(0.0, |height| (height - occupied).max(0.0));
        let (offset, extra_gap) = justify_offsets(
            container_style.justify_content,
            free,
            items.len(),
            container_style.row_gap,
        );
        let mut accumulated = offset;
        for (start, end) in items {
            let root = self.tree.boxes[start].bounds;
            let cross_offset = match container_style.align_items {
                FlexAlign::Center => (content_width - root.width).max(0.0) / 2.0,
                FlexAlign::End => (content_width - root.width).max(0.0),
                FlexAlign::Start | FlexAlign::Stretch => 0.0,
            };
            self.shift_boxes(start, end, cross_offset, accumulated);
            if extra_gap > container_style.row_gap {
                accumulated += extra_gap - container_style.row_gap;
            }
        }
        *cursor_y += offset;
    }

    fn flex_basis(
        &self,
        node_id: NodeId,
        style: &arch_style::ComputedStyle,
        content_width: f32,
    ) -> f32 {
        if style.width.is_some() {
            return resolve_box_width(style, content_width).min(content_width);
        }
        let text_width = self
            .document
            .node(node_id)
            .map(|_| self.document.text_content(node_id).chars().count())
            .and_then(|count| u16::try_from(count).ok())
            .map_or(0.0, |count| f32::from(count) * style.font_size_px * 0.55);
        form_control_dimensions(&self.document.node(node_id).expect("flex child exists").kind)
            .map_or(text_width.max(1.0), |(width, _)| width)
            .min(content_width)
    }

    fn move_boxes(&mut self, start: usize, end: usize, x: f32, y: f32) {
        let root = self.tree.boxes[start].bounds;
        self.shift_boxes(start, end, x - root.x, y - root.y);
    }

    fn shift_boxes(&mut self, start: usize, end: usize, dx: f32, dy: f32) {
        for layout_box in &mut self.tree.boxes[start..end] {
            layout_box.bounds.x += dx;
            layout_box.bounds.y += dy;
        }
    }

    fn layout_inline_subtree(
        &mut self,
        node_id: NodeId,
        content_x: f32,
        content_width: f32,
        cursor_y: &mut f32,
        inline_x: &mut f32,
        line_height: &mut f32,
    ) {
        let Some(node) = self.document.node(node_id) else {
            return;
        };
        let Some(style) = self.style_by_node.get(&node_id).copied() else {
            return;
        };
        if style.display == Display::None {
            self.hidden.insert(node_id);
            return;
        }
        if matches!(&node.kind, NodeKind::Element(element) if element.name == "br") {
            *cursor_y += (*line_height).max(style.line_height_px);
            *inline_x = 0.0;
            *line_height = 0.0;
            return;
        }
        let text = match &node.kind {
            NodeKind::Text(value) if !value.trim().is_empty() => Some(match style.white_space {
                WhiteSpace::Pre | WhiteSpace::PreWrap => value.clone(),
                WhiteSpace::Normal | WhiteSpace::NoWrap => {
                    value.split_whitespace().collect::<Vec<_>>().join(" ")
                }
            }),
            _ => None,
        };
        let image = self.images.get(&node_id).cloned();
        let form_dimensions = form_control_dimensions(&node.kind);
        if text.is_some() || image.is_some() || form_dimensions.is_some() {
            let intrinsic_width = if let Some(value) = &text {
                let count = u16::try_from(value.chars().count()).unwrap_or(u16::MAX);
                f32::from(count) * style.font_size_px * 0.55
            } else if let Some(image) = &image {
                pixel_dimension(image.intrinsic_width)
            } else if let Some((width, _)) = form_dimensions {
                width
            } else {
                0.0
            };
            let aligned_line = text.is_some() && style.text_align != TextAlign::Start;
            let run_width = if aligned_line {
                content_width
            } else {
                intrinsic_width.min(content_width).max(1.0)
            };
            if *inline_x > 0.0 && *inline_x + run_width > content_width {
                *cursor_y += (*line_height).max(1.0);
                *inline_x = 0.0;
                *line_height = 0.0;
            }
            let run_height = if let Some(value) = &text {
                let chars_per_line = (run_width / (style.font_size_px * 0.55)).max(1.0);
                text_line_count(value, chars_per_line, style.white_space) * style.line_height_px
            } else if let Some(image) = &image {
                let width = pixel_dimension(image.intrinsic_width.max(1));
                pixel_dimension(image.intrinsic_height) * (run_width / width).min(1.0)
            } else if let Some((_, height)) = form_dimensions {
                height
            } else {
                style.line_height_px
            };
            self.tree.boxes.push(LayoutBox {
                node_id,
                bounds: Rect {
                    x: content_x + *inline_x,
                    y: *cursor_y,
                    width: run_width,
                    height: run_height,
                },
                clip: None,
                text,
                image,
                link: self.links.get(&node_id).cloned(),
                font_size_px: style.font_size_px,
                font_family: style.font_family.clone(),
                color: style.color.clone(),
                line_height_px: style.line_height_px,
                white_space: style.white_space,
                font_weight: style.font_weight,
                font_style: style.font_style,
                background_color: style.background_color.clone(),
                border_color: style.border_color.clone(),
                border_width_px: style.border_px,
                text_align: style.text_align,
            });
            *inline_x += run_width;
            *line_height = (*line_height).max(run_height);
            return;
        }
        for child in &node.children {
            self.layout_inline_subtree(
                *child,
                content_x,
                content_width,
                cursor_y,
                inline_x,
                line_height,
            );
        }
    }
}

fn form_control_dimensions(kind: &NodeKind) -> Option<(f32, f32)> {
    let NodeKind::Element(element) = kind else {
        return None;
    };
    match element.name.as_str() {
        "select" | "button" => Some((
            if element.name == "button" {
                96.0
            } else {
                180.0
            },
            30.0,
        )),
        "input" => match element
            .attribute("type")
            .unwrap_or("text")
            .to_ascii_lowercase()
            .as_str()
        {
            "checkbox" | "radio" => Some((18.0, 18.0)),
            "text" | "password" => Some((180.0, 30.0)),
            "submit" | "button" => Some((96.0, 30.0)),
            _ => None,
        },
        _ => None,
    }
}

fn text_line_count(value: &str, chars_per_line: f32, white_space: WhiteSpace) -> f32 {
    value
        .lines()
        .map(|line| {
            if matches!(white_space, WhiteSpace::Pre | WhiteSpace::NoWrap) {
                1.0
            } else {
                let count = u16::try_from(line.chars().count()).unwrap_or(u16::MAX);
                (f32::from(count) / chars_per_line).ceil().max(1.0)
            }
        })
        .sum::<f32>()
        .max(1.0)
}

fn intersect_rect(first: Rect, second: Rect) -> Rect {
    let x = first.x.max(second.x);
    let y = first.y.max(second.y);
    let right = (first.x + first.width).min(second.x + second.width);
    let bottom = (first.y + first.height).min(second.y + second.height);
    Rect {
        x,
        y,
        width: (right - x).max(0.0),
        height: (bottom - y).max(0.0),
    }
}

fn inset_rect(rect: Rect, inset: f32) -> Rect {
    let inset = inset.max(0.0);
    let horizontal = inset.min(rect.width / 2.0);
    let vertical = inset.min(rect.height / 2.0);
    Rect {
        x: rect.x + horizontal,
        y: rect.y + vertical,
        width: rect.width - horizontal * 2.0,
        height: rect.height - vertical * 2.0,
    }
}

fn resolve(length: ComputedLength, containing: f32) -> f32 {
    match length {
        ComputedLength::Px(value) => value,
        ComputedLength::Percent(value) => containing * value,
    }
    .max(0.0)
}

fn box_dimension(style: &arch_style::ComputedStyle, value: f32) -> f32 {
    if style.box_sizing == BoxSizing::ContentBox {
        value + style.padding.left + style.padding.right + style.border_px * 2.0
    } else {
        value
    }
}

fn resolve_box_width(style: &arch_style::ComputedStyle, containing_width: f32) -> f32 {
    let available_width = (containing_width - style.margin.left - style.margin.right).max(0.0);
    let mut width = style
        .width
        .map_or(available_width, |value| resolve(value, containing_width));
    if style.box_sizing == BoxSizing::ContentBox && style.width.is_some() {
        width = box_dimension(style, width);
    }
    if let Some(min_width) = style.min_width {
        width = width.max(box_dimension(style, resolve(min_width, containing_width)));
    }
    if let Some(max_width) = style.max_width {
        width = width.min(box_dimension(style, resolve(max_width, containing_width)));
    }
    width.min(available_width).max(0.0)
}

fn intrinsic_content_height(
    style: &arch_style::ComputedStyle,
    image: Option<&ImageBox>,
    text: Option<&str>,
    content_width: f32,
) -> f32 {
    if let Some(image) = image {
        let intrinsic_width = pixel_dimension(image.intrinsic_width.max(1));
        let scale = (content_width / intrinsic_width).min(1.0);
        pixel_dimension(image.intrinsic_height) * scale
    } else if let Some(text) = text {
        let chars_per_line = (content_width / (style.font_size_px * 0.55)).max(1.0);
        text_line_count(text, chars_per_line, style.white_space) * style.line_height_px.max(1.0)
    } else {
        0.0
    }
}

fn box_height(style: &arch_style::ComputedStyle, value: f32) -> f32 {
    if style.box_sizing == BoxSizing::ContentBox {
        value + style.padding.top + style.padding.bottom + style.border_px * 2.0
    } else {
        value
    }
}

fn resolve_height(length: ComputedLength, containing: Option<f32>) -> Option<f32> {
    match length {
        ComputedLength::Px(value) => Some(value.max(0.0)),
        ComputedLength::Percent(value) => containing.map(|height| (height * value).max(0.0)),
    }
}

#[allow(clippy::cast_precision_loss)]
fn pixel_dimension(value: u32) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use arch_css::parse as parse_css;
    use arch_html::parse as parse_html;
    use arch_style::style_document;

    use super::*;

    #[test]
    fn lays_out_text_vertically() {
        let document = parse_html("<p>one</p><p>two</p>");
        let styled = style_document(&document, &parse_css("p { margin: 8px; padding: 4px }"));
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let text: Vec<_> = tree
            .boxes
            .iter()
            .filter(|item| item.text.is_some())
            .collect();
        assert_eq!(text.len(), 2);
        assert!(text[1].bounds.y > text[0].bounds.y);
    }

    #[test]
    fn preserves_computed_font_family_on_text_boxes() {
        let document = parse_html("<p>family</p>");
        let styled = style_document(
            &document,
            &parse_css(r#"p { font-family: "Helvetica Neue", sans-serif }"#),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let text = tree
            .boxes
            .iter()
            .find(|item| item.text.as_deref() == Some("family"))
            .unwrap();
        assert_eq!(text.font_family.as_deref(), Some("Helvetica Neue"));
    }

    #[test]
    fn intersects_nested_overflow_clips_for_descendants() {
        let document =
            parse_html("<section class='outer'><div class='inner'><p>clipped</p></div></section>");
        let styled = style_document(
            &document,
            &parse_css(
                ".outer { width: 200px; height: 80px; overflow: hidden; padding: 10px; border-width: 2px } \
                 .inner { width: 180px; height: 120px; overflow: hidden; margin-left: 30px }",
            ),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let outer = tree
            .boxes
            .iter()
            .find(|item| {
                document.node(item.node_id).is_some_and(
                    |node| matches!(&node.kind, NodeKind::Element(element) if element.attribute("class") == Some("outer")),
                )
            })
            .unwrap();
        let inner = tree
            .boxes
            .iter()
            .find(|item| {
                document.node(item.node_id).is_some_and(
                    |node| matches!(&node.kind, NodeKind::Element(element) if element.attribute("class") == Some("inner")),
                )
            })
            .unwrap();
        let text = tree
            .boxes
            .iter()
            .find(|item| item.text.as_deref() == Some("clipped"))
            .unwrap();
        let outer_clip = inset_rect(outer.bounds, outer.border_width_px);
        assert_eq!(text.clip, Some(intersect_rect(outer_clip, inner.bounds)));
        assert!((outer_clip.x - outer.bounds.x - 2.0).abs() < f32::EPSILON);
        assert!(text.bounds.x > outer.bounds.x);
    }

    #[test]
    fn applies_width_constraints_and_border_box_height() {
        let document = parse_html("<div id='box'>content</div>");
        let styled = style_document(
            &document,
            &parse_css(
                "#box { width: 50%; min-width: 300px; max-width: 320px; height: 80px; \
                 padding: 10px; border-width: 2px; box-sizing: border-box }",
            ),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let box_id = document
            .descendants(document.root())
            .find(|node| matches!(&node.kind, NodeKind::Element(element) if element.attribute("id") == Some("box")))
            .unwrap()
            .id;
        let item = tree
            .boxes
            .iter()
            .find(|item| item.node_id == box_id)
            .unwrap();
        assert!((item.bounds.width - 320.0).abs() < f32::EPSILON);
        assert!((item.bounds.height - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn resolves_percentage_height_against_definite_parent_content_height() {
        let document = parse_html("<section id='parent'><div id='child'></div></section>");
        let styled = style_document(
            &document,
            &parse_css("#parent { height: 200px } #child { height: 50% }"),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let child_id = document
            .descendants(document.root())
            .find(|node| matches!(&node.kind, NodeKind::Element(element) if element.attribute("id") == Some("child")))
            .unwrap()
            .id;
        let child = tree
            .boxes
            .iter()
            .find(|item| item.node_id == child_id)
            .unwrap();

        assert!((child.bounds.height - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn percentage_height_with_indefinite_parent_uses_natural_height() {
        let document = parse_html("<section id='parent'><div id='child'>content</div></section>");
        let styled = style_document(
            &document,
            &parse_css("#parent { width: 400px } #child { height: 50% }"),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let child_id = document
            .descendants(document.root())
            .find(|node| matches!(&node.kind, NodeKind::Element(element) if element.attribute("id") == Some("child")))
            .unwrap()
            .id;
        let child = tree
            .boxes
            .iter()
            .find(|item| item.node_id == child_id)
            .unwrap();

        assert!(child.bounds.height > 0.0);
        assert!(child.bounds.height < 100.0);
    }

    #[test]
    fn display_none_suppresses_the_entire_subtree() {
        let document = parse_html("<section class='hidden'><p>secret</p></section><p>visible</p>");
        let styled = style_document(&document, &parse_css(".hidden { display: none }"));
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let text: Vec<_> = tree
            .boxes
            .iter()
            .filter_map(|item| item.text.as_deref())
            .collect();
        assert_eq!(text, ["visible"]);
    }

    #[test]
    fn block_box_contains_nested_content_and_edges() {
        let document = parse_html("<section class='panel'><p>inside</p></section>");
        let styled = style_document(
            &document,
            &parse_css(
                ".panel { width: 300px; padding: 20px; border-width: 2px; \
                 box-sizing: border-box }",
            ),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let section_id = document
            .descendants(document.root())
            .find(|node| matches!(&node.kind, NodeKind::Element(element) if element.name == "section"))
            .unwrap()
            .id;
        let section = tree
            .boxes
            .iter()
            .find(|item| item.node_id == section_id)
            .unwrap();
        let text = tree
            .boxes
            .iter()
            .find(|item| item.text.as_deref() == Some("inside"))
            .unwrap();
        assert!((section.bounds.width - 300.0).abs() < f32::EPSILON);
        assert!(section.bounds.height >= text.bounds.height + 44.0);
        assert!(text.bounds.x >= section.bounds.x + 22.0);
        assert!(text.bounds.y >= section.bounds.y + 22.0);
        assert!(text.bounds.y + text.bounds.height <= section.bounds.y + section.bounds.height);
    }

    #[test]
    fn inline_runs_share_lines_and_wrap_or_break() {
        let document = parse_html(
            "<p class='wide'>alpha <strong>beta</strong><br>gamma</p>\
             <p class='narrow'><span>aaaa</span><span>bbbb</span><span>cccc</span></p>",
        );
        let styled = style_document(
            &document,
            &parse_css(".wide { width: 400px } .narrow { width: 70px }"),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let find = |content: &str| {
            tree.boxes
                .iter()
                .find(|item| item.text.as_deref() == Some(content))
                .unwrap()
        };
        let alpha = find("alpha");
        let beta = find("beta");
        let gamma = find("gamma");
        assert!((alpha.bounds.y - beta.bounds.y).abs() < f32::EPSILON);
        assert!(beta.bounds.x > alpha.bounds.x);
        assert!(gamma.bounds.y > beta.bounds.y);

        let narrow: Vec<_> = tree
            .boxes
            .iter()
            .filter(|item| matches!(item.text.as_deref(), Some("aaaa" | "bbbb" | "cccc")))
            .collect();
        assert_eq!(narrow.len(), 3);
        assert!(narrow[2].bounds.y > narrow[0].bounds.y);
    }

    #[test]
    fn inline_text_height_wraps_and_alignment_uses_full_line() {
        let document = parse_html(
            "<p class='wrap'>abcdefghijklmnopqrstuvwxyz</p>\
             <p class='center'>centered</p><pre>one\ntwo\nthree</pre>",
        );
        let styled = style_document(
            &document,
            &parse_css(".wrap { width: 80px } .center { width: 80px; text-align: center }"),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let find = |content: &str| {
            tree.boxes
                .iter()
                .find(|item| item.text.as_deref() == Some(content))
                .unwrap()
        };
        assert!(find("abcdefghijklmnopqrstuvwxyz").bounds.height > 16.0 * 1.4);
        assert!((find("centered").bounds.width - 80.0).abs() < f32::EPSILON);
        assert!((find("one\ntwo\nthree").bounds.height - 16.0 * 1.4 * 3.0).abs() < 0.01);
    }

    #[test]
    fn gives_supported_form_controls_stable_nonzero_boxes() {
        let document = parse_html(
            "<form><input id='text'><input id='password' type='password'>\
             <input id='checkbox' type='checkbox'><input id='radio' type='radio'>\
             <select id='select'><option>One</option></select>\
             <input id='submit' type='submit'><button id='button' type='button'>Button</button></form>",
        );
        let styled = style_document(&document, &parse_css(""));
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());

        let controls: Vec<_> = tree
            .boxes
            .iter()
            .filter(|layout_box| {
                document.node(layout_box.node_id).is_some_and(|node| {
                    matches!(&node.kind, NodeKind::Element(element) if
                        matches!(element.name.as_str(), "input" | "select" | "button"))
                })
            })
            .collect();
        assert_eq!(controls.len(), 7);
        assert!(
            controls
                .iter()
                .all(|control| { control.bounds.width > 0.0 && control.bounds.height > 0.0 })
        );
        assert!((controls[0].bounds.width - 180.0).abs() < f32::EPSILON);
        assert!((controls[2].bounds.width - 18.0).abs() < f32::EPSILON);
        assert!((controls[5].bounds.width - 96.0).abs() < f32::EPSILON);
    }

    fn element_box<'a>(document: &Document, tree: &'a LayoutTree, id: &str) -> &'a LayoutBox {
        let node_id = document
            .descendants(document.root())
            .find(|node| {
                matches!(&node.kind, NodeKind::Element(element) if element.attribute("id") == Some(id))
            })
            .expect("fixture element exists")
            .id;
        tree.boxes
            .iter()
            .find(|layout_box| layout_box.node_id == node_id)
            .expect("fixture element has a box")
    }

    #[test]
    fn flex_row_distributes_gap_growth_and_justification() {
        let document =
            parse_html("<main><div id='a'>A</div><div id='b'>B</div><div id='c'>C</div></main>");
        let styled = style_document(
            &document,
            &parse_css(
                "main { display: flex; width: 600px; gap: 20px; justify-content: center } \
                 main div { width: 100px; flex-grow: 1 }",
            ),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let a = element_box(&document, &tree, "a");
        let b = element_box(&document, &tree, "b");
        let c = element_box(&document, &tree, "c");
        assert!((a.bounds.width - 186.666_67).abs() < 0.01);
        assert!((b.bounds.x - a.bounds.x - a.bounds.width - 20.0).abs() < 0.01);
        assert!((c.bounds.x - b.bounds.x - b.bounds.width - 20.0).abs() < 0.01);
    }

    #[test]
    fn flex_wraps_and_aligns_each_line() {
        let document = parse_html(
            "<main><div id='a'>A</div><div id='b'>B<br>B</div><div id='c'>C</div></main>",
        );
        let styled = style_document(
            &document,
            &parse_css(
                "main { display: flex; width: 250px; flex-wrap: wrap; gap: 10px; align-items: center } \
                 main div { width: 120px }",
            ),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let a = element_box(&document, &tree, "a");
        let b = element_box(&document, &tree, "b");
        let c = element_box(&document, &tree, "c");
        assert!((b.bounds.x - a.bounds.x - 130.0).abs() < 0.01);
        assert!(a.bounds.y > b.bounds.y);
        assert!(c.bounds.y > a.bounds.y);
    }

    #[test]
    fn flex_shrinks_overflowing_items_and_reverses_rows() {
        let document = parse_html("<main><div id='a'>A</div><div id='b'>B</div></main>");
        let styled = style_document(
            &document,
            &parse_css(
                "main { display: flex; flex-direction: row-reverse; width: 300px } \
                 main div { width: 200px; flex-shrink: 1 }",
            ),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let a = element_box(&document, &tree, "a");
        let b = element_box(&document, &tree, "b");
        assert!((a.bounds.width - 150.0).abs() < 0.01);
        assert!(b.bounds.x < a.bounds.x);
    }

    #[test]
    fn flex_column_honors_reverse_gap_and_cross_axis_alignment() {
        let document = parse_html("<main><div id='a'>A</div><div id='b'>B</div></main>");
        let styled = style_document(
            &document,
            &parse_css(
                "main { display: flex; flex-direction: column-reverse; align-items: end; width: 300px; gap: 12px } \
                 main div { width: 80px }",
            ),
        );
        let tree = layout(&document, &styled, 800.0, &HashMap::new(), &HashMap::new());
        let a = element_box(&document, &tree, "a");
        let b = element_box(&document, &tree, "b");
        assert!(b.bounds.y < a.bounds.y);
        assert!((a.bounds.x - 220.0).abs() < 0.01);
        assert!((a.bounds.y - b.bounds.y - b.bounds.height - 12.0).abs() < 0.01);
    }
}
