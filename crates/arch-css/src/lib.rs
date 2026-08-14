use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub selector: String,
    pub declarations: Vec<Declaration>,
    pub source_order: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Declaration {
    pub name: String,
    pub value: String,
    pub important: bool,
}

#[must_use]
pub fn parse(source: &str) -> Stylesheet {
    let without_comments = strip_comments(source);
    let mut rules = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unknown_properties = BTreeSet::new();
    for (source_order, part) in without_comments.split('}').enumerate() {
        let Some((selector, body)) = part.split_once('{') else {
            if !part.trim().is_empty() {
                diagnostics.push("ignored malformed rule".to_owned());
            }
            continue;
        };
        let selector = selector.trim();
        if selector.is_empty() {
            diagnostics.push("ignored empty selector".to_owned());
            continue;
        }
        let declarations = body
            .split(';')
            .filter_map(|item| {
                let (name, value) = item.split_once(':')?;
                let name = name.trim().to_ascii_lowercase();
                let mut value = value.trim().to_owned();
                if name.is_empty() || value.is_empty() {
                    return None;
                }
                let important = value.ends_with("!important");
                if important {
                    value.truncate(value.len() - "!important".len());
                    value = value.trim().to_owned();
                }
                if !supported_property(&name) {
                    unknown_properties.insert(name.clone());
                    return None;
                }
                Some(Declaration {
                    name,
                    value,
                    important,
                })
            })
            .collect();
        rules.push(Rule {
            selector: selector.to_owned(),
            declarations,
            source_order,
        });
    }
    diagnostics.extend(
        unknown_properties
            .into_iter()
            .map(|name| format!("ignored unsupported CSS property: {name}")),
    );
    Stylesheet { rules, diagnostics }
}

fn supported_property(name: &str) -> bool {
    matches!(
        name,
        "display"
            | "color"
            | "background-color"
            | "font-size"
            | "line-height"
            | "white-space"
            | "font-weight"
            | "font-style"
            | "text-align"
            | "margin"
            | "margin-top"
            | "margin-right"
            | "margin-bottom"
            | "margin-left"
            | "padding"
            | "padding-top"
            | "padding-right"
            | "padding-bottom"
            | "padding-left"
            | "border-width"
            | "border-color"
            | "width"
            | "height"
            | "min-width"
            | "max-width"
            | "box-sizing"
    )
}

fn strip_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("/*") {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start + 2..].find("*/") else {
            return output;
        };
        rest = &rest[start + end + 4..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules_and_important() {
        let sheet = parse("p { color: red; margin: 8px !important; }");
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].declarations[1].name, "margin");
        assert!(sheet.rules[0].declarations[1].important);
    }

    #[test]
    fn aggregates_unsupported_properties() {
        let sheet = parse("p { transform: rotate(2deg); unknown: yes } div { transform: none }");
        assert_eq!(sheet.rules[0].declarations.len(), 0);
        assert_eq!(
            sheet.diagnostics,
            [
                "ignored unsupported CSS property: transform",
                "ignored unsupported CSS property: unknown"
            ]
        );
    }
}
