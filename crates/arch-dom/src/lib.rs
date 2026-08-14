use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct NodeId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Document,
    Element(ElementData),
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ElementData {
    pub name: String,
    pub attributes: Vec<(String, String)>,
}

impl ElementData {
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value.as_str()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Document {
    nodes: Vec<Node>,
    root: NodeId,
}

impl Document {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![Node {
                id: NodeId(0),
                parent: None,
                children: Vec::new(),
                kind: NodeKind::Document,
            }],
            root: NodeId(0),
        }
    }

    #[must_use]
    pub const fn root(&self) -> NodeId {
        self.root
    }

    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0 as usize)
    }

    #[must_use]
    pub fn append(&mut self, parent: NodeId, kind: NodeKind) -> Option<NodeId> {
        let id = NodeId(u32::try_from(self.nodes.len()).ok()?);
        let parent_node = self.nodes.get_mut(parent.0 as usize)?;
        parent_node.children.push(id);
        self.nodes.push(Node {
            id,
            parent: Some(parent),
            children: Vec::new(),
            kind,
        });
        Some(id)
    }

    #[must_use]
    pub fn descendants(&self, root: NodeId) -> Descendants<'_> {
        Descendants {
            document: self,
            stack: vec![root],
        }
    }

    #[must_use]
    pub fn text_content(&self, root: NodeId) -> String {
        self.descendants(root)
            .filter_map(|node| match &node.kind {
                NodeKind::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Descendants<'a> {
    document: &'a Document,
    stack: Vec<NodeId>,
}

impl<'a> Iterator for Descendants<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = self.document.node(id)?;
        self.stack.extend(node.children.iter().rev().copied());
        Some(node)
    }
}
