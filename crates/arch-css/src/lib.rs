use std::collections::BTreeSet;

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser, ToCss, Token,
    parse_important,
};
use serde::{Deserialize, Serialize};

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
    let mut input = ParserInput::new(source);
    let mut input = Parser::new(&mut input);
    let mut parser = StylesheetAdapter::default();
    let mut rules = Vec::new();
    let mut malformed_rules = 0;

    for result in StyleSheetParser::new(&mut input, &mut parser) {
        match result {
            Ok(rule) => rules.push(rule),
            Err(_) => malformed_rules += 1,
        }
    }

    let mut diagnostics = vec!["ignored malformed rule".to_owned(); malformed_rules];
    diagnostics.extend(
        parser
            .unknown_properties
            .into_iter()
            .map(|name| format!("ignored unsupported CSS property: {name}")),
    );
    Stylesheet { rules, diagnostics }
}

#[must_use]
pub fn first_font_family(value: &str) -> Option<String> {
    let mut input = ParserInput::new(value);
    let mut input = Parser::new(&mut input);
    let mut identifiers = Vec::new();
    let mut quoted = None;

    while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
        match token {
            Token::WhiteSpace(_) | Token::Comment(_) => {}
            Token::Comma => break,
            Token::QuotedString(value) if identifiers.is_empty() && quoted.is_none() => {
                quoted = Some(value.to_string());
            }
            Token::Ident(value) if quoted.is_none() => identifiers.push(value.to_string()),
            _ => return None,
        }
    }

    quoted
        .or_else(|| (!identifiers.is_empty()).then(|| identifiers.join(" ")))
        .filter(|family| !family.is_empty())
}

#[derive(Default)]
struct StylesheetAdapter {
    source_order: usize,
    unknown_properties: BTreeSet<String>,
}

impl<'i> QualifiedRuleParser<'i> for StylesheetAdapter {
    type Prelude = String;
    type QualifiedRule = Rule;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let mut selector = String::new();
        serialize_remaining_components(input, &mut selector)?;
        let selector = selector.trim();
        if selector.is_empty() {
            return Err(input.new_custom_error(()));
        }
        Ok(selector.to_owned())
    }

    fn parse_block<'t>(
        &mut self,
        selector: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let mut declaration_parser = DeclarationAdapter {
            unknown_properties: &mut self.unknown_properties,
        };
        let declarations = RuleBodyParser::new(input, &mut declaration_parser)
            .filter_map(Result::ok)
            .collect();
        let rule = Rule {
            selector,
            declarations,
            source_order: self.source_order,
        };
        self.source_order += 1;
        Ok(rule)
    }
}

impl AtRuleParser<'_> for StylesheetAdapter {
    type Prelude = ();
    type AtRule = Rule;
    type Error = ();
}

struct DeclarationAdapter<'a> {
    unknown_properties: &'a mut BTreeSet<String>,
}

impl<'i> DeclarationParser<'i> for DeclarationAdapter<'_> {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let name = name.to_ascii_lowercase();
        if !supported_property(&name) {
            self.unknown_properties.insert(name);
            return Err(input.new_custom_error(()));
        }

        let mut value = String::new();
        let mut important = false;
        loop {
            let token_start = input.state();
            let Ok(token) = input.next_including_whitespace_and_comments().cloned() else {
                break;
            };
            if token == Token::Delim('!') {
                input.reset(&token_start);
                if input.try_parse(parse_important).is_ok() && input.is_exhausted() {
                    important = true;
                    break;
                }
                input.reset(&token_start);
                let _ = input.next_including_whitespace_and_comments();
            }
            serialize_component(&token, input, &mut value)?;
        }

        let value = value.trim().to_owned();
        if value.is_empty() {
            return Err(input.new_custom_error(()));
        }
        Ok(Declaration {
            name,
            value,
            important,
        })
    }
}

impl AtRuleParser<'_> for DeclarationAdapter<'_> {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl QualifiedRuleParser<'_> for DeclarationAdapter<'_> {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

impl RuleBodyItemParser<'_, Declaration, ()> for DeclarationAdapter<'_> {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

fn serialize_component<'i>(
    token: &Token<'i>,
    input: &mut Parser<'i, '_>,
    output: &mut String,
) -> Result<(), ParseError<'i, ()>> {
    if matches!(token, Token::Comment(_)) {
        return Ok(());
    }
    token
        .to_css(output)
        .map_err(|_| input.new_custom_error(()))?;
    let closing = match token {
        Token::Function(_) | Token::ParenthesisBlock => Some(')'),
        Token::SquareBracketBlock => Some(']'),
        Token::CurlyBracketBlock => Some('}'),
        _ => None,
    };
    if let Some(closing) = closing {
        input.parse_nested_block(|input| serialize_remaining_components(input, output))?;
        output.push(closing);
    }
    Ok(())
}

fn serialize_remaining_components<'i>(
    input: &mut Parser<'i, '_>,
    output: &mut String,
) -> Result<(), ParseError<'i, ()>> {
    while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
        serialize_component(&token, input, output)?;
    }
    Ok(())
}

fn supported_property(name: &str) -> bool {
    matches!(
        name,
        "display"
            | "color"
            | "background-color"
            | "font-size"
            | "font-family"
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
            | "overflow"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules_and_important() {
        let sheet = parse("p { color: red; margin: 8px ! important; }");
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].declarations[1].name, "margin");
        assert_eq!(sheet.rules[0].declarations[1].value, "8px");
        assert!(sheet.rules[0].declarations[1].important);
    }

    #[test]
    fn preserves_nested_functions_strings_and_selector_tokens() {
        let sheet =
            parse(r#"a[href="x;y"] { color: rgb(10, 20, 30); background-color: "red; }"; }"#);
        assert_eq!(sheet.rules[0].selector, r#"a[href="x;y"]"#);
        assert_eq!(sheet.rules[0].declarations[0].value, "rgb(10, 20, 30)");
        assert_eq!(sheet.rules[0].declarations[1].value, r#""red; }""#);
    }

    #[test]
    fn comments_do_not_change_rule_boundaries() {
        let sheet = parse("p/* { ; } */ { color: red /* ; } */; margin: 2px; }");
        assert_eq!(sheet.rules[0].selector, "p");
        assert_eq!(sheet.rules[0].declarations.len(), 2);
        assert_eq!(sheet.rules[0].declarations[0].value, "red");
    }

    #[test]
    fn recovers_after_a_malformed_declaration() {
        let sheet = parse("p { color red; margin: 2px; }");
        assert_eq!(sheet.rules[0].declarations.len(), 1);
        assert_eq!(sheet.rules[0].declarations[0].name, "margin");
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

    #[test]
    fn parses_first_quoted_or_unquoted_font_family() {
        assert_eq!(
            first_font_family(r#""Helvetica Neue", sans-serif"#).as_deref(),
            Some("Helvetica Neue")
        );
        assert_eq!(
            first_font_family("Helvetica Neue, sans-serif").as_deref(),
            Some("Helvetica Neue")
        );
    }

    #[test]
    fn rejects_invalid_first_font_family() {
        assert_eq!(first_font_family("var(--font), sans-serif"), None);
        assert_eq!(first_font_family(r#""Helvetica" Neue, sans-serif"#), None);
        assert_eq!(first_font_family("  "), None);
    }

    #[test]
    fn accepts_overflow_declarations() {
        let sheet = parse("section { overflow: hidden }");
        assert_eq!(sheet.rules[0].declarations[0].name, "overflow");
        assert_eq!(sheet.rules[0].declarations[0].value, "hidden");
        assert!(sheet.diagnostics.is_empty());
    }
}
