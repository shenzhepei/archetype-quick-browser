use arch_dom::{Document, ElementData, NodeId, NodeKind};
use html5ever::{ParseOpts, parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

#[must_use]
pub fn parse(source: &str) -> Document {
    let rcdom = parse_document(RcDom::default(), ParseOpts::default()).one(source);
    let mut document = Document::new();
    let root = document.root();
    copy_children(&rcdom.document, &mut document, root);
    document
}

fn copy_children(source: &Handle, target: &mut Document, parent: NodeId) {
    for child in source.children.borrow().iter() {
        let next_parent = match &child.data {
            NodeData::Document => parent,
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                if !text.is_empty() {
                    let _ = target.append(parent, NodeKind::Text(text));
                }
                continue;
            }
            NodeData::Element { name, attrs, .. } => {
                let attributes = attrs
                    .borrow()
                    .iter()
                    .map(|attribute| {
                        (
                            attribute.name.local.to_string(),
                            attribute.value.to_string(),
                        )
                    })
                    .collect();
                let Some(id) = target.append(
                    parent,
                    NodeKind::Element(ElementData {
                        name: name.local.to_string(),
                        attributes,
                    }),
                ) else {
                    return;
                };
                id
            }
            _ => continue,
        };
        copy_children(child, target, next_parent);
    }
}

#[must_use]
pub fn title(document: &Document) -> Option<String> {
    document.descendants(document.root()).find_map(|node| {
        if matches!(&node.kind, NodeKind::Element(element) if element.name == "title") {
            Some(document.text_content(node.id).trim().to_owned())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title_and_text() {
        let document =
            parse("<!doctype html><title>Archetype</title><p>Hello <strong>world</strong></p>");
        assert_eq!(title(&document).as_deref(), Some("Archetype"));
        assert!(
            document
                .text_content(document.root())
                .contains("Hello world")
        );
    }

    #[test]
    fn scripts_are_data_not_execution() {
        let document = parse("<script>window.bad = true</script><p>safe</p>");
        assert!(document.text_content(document.root()).contains("safe"));
    }
}
