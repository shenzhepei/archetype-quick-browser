use std::collections::{BTreeMap, BTreeSet};

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
    pub media: Option<MediaCondition>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaCondition {
    pub min_width_px: Option<f32>,
    pub max_width_px: Option<f32>,
}

impl MediaCondition {
    #[must_use]
    pub fn matches_width(self, viewport_width_px: f32) -> bool {
        viewport_width_px.is_finite()
            && self
                .min_width_px
                .is_none_or(|minimum| viewport_width_px >= minimum)
            && self
                .max_width_px
                .is_none_or(|maximum| viewport_width_px <= maximum)
    }

    fn merge(self, nested: Self) -> Self {
        Self {
            min_width_px: max_optional(self.min_width_px, nested.min_width_px),
            max_width_px: min_optional(self.max_width_px, nested.max_width_px),
        }
    }
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
    let mut source_order = 0;
    let mut unknown_properties = BTreeSet::new();
    let mut unsupported_at_rules = BTreeSet::new();
    let mut unsupported_media = BTreeSet::new();
    let mut parser = StylesheetAdapter {
        source_order: &mut source_order,
        unknown_properties: &mut unknown_properties,
        unsupported_at_rules: &mut unsupported_at_rules,
        unsupported_media: &mut unsupported_media,
        media: None,
        depth: 0,
    };
    let mut rules = Vec::new();
    let mut malformed_rules = 0;

    for result in StyleSheetParser::new(&mut input, &mut parser) {
        match result {
            Ok(parsed) => rules.extend(parsed),
            Err(_) => malformed_rules += 1,
        }
    }

    let mut diagnostics = vec!["ignored malformed rule".to_owned(); malformed_rules];
    diagnostics.extend(
        unknown_properties
            .into_iter()
            .map(|name| format!("ignored unsupported CSS property: {name}")),
    );
    diagnostics.extend(
        unsupported_at_rules
            .into_iter()
            .map(|name| format!("ignored unsupported CSS at-rule: {name}")),
    );
    diagnostics.extend(
        unsupported_media
            .into_iter()
            .map(|condition| format!("ignored unsupported media condition: {condition}")),
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

#[must_use]
pub fn resolve_variables(
    value: &str,
    custom_properties: &BTreeMap<String, String>,
) -> Option<String> {
    resolve_variable_value(value, custom_properties, &mut Vec::new(), 0)
}

fn resolve_variable_value(
    value: &str,
    custom_properties: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
    depth: u8,
) -> Option<String> {
    if depth >= 16 {
        return None;
    }
    let mut input = ParserInput::new(value);
    let mut input = Parser::new(&mut input);
    let mut output = String::new();
    resolve_remaining_components(&mut input, &mut output, custom_properties, stack, depth).ok()?;
    (output.len() <= 64 * 1024).then_some(output)
}

fn resolve_remaining_components<'i>(
    input: &mut Parser<'i, '_>,
    output: &mut String,
    custom_properties: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
    depth: u8,
) -> Result<(), ParseError<'i, ()>> {
    while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
        if let Token::Function(name) = &token
            && name.eq_ignore_ascii_case("var")
        {
            let resolved = input.parse_nested_block(|input| {
                input.skip_whitespace();
                let name = input.expect_ident_cloned()?.to_string();
                if !name.starts_with("--") {
                    return Err(input.new_custom_error(()));
                }
                input.skip_whitespace();
                let fallback = if input.try_parse(Parser::expect_comma).is_ok() {
                    let mut fallback = String::new();
                    serialize_remaining_components(input, &mut fallback)?;
                    Some(fallback.trim().to_owned())
                } else {
                    input.expect_exhausted()?;
                    None
                };
                if stack.contains(&name) {
                    return fallback
                        .as_deref()
                        .and_then(|fallback| {
                            resolve_variable_value(fallback, custom_properties, stack, depth + 1)
                        })
                        .ok_or_else(|| input.new_custom_error(()));
                }
                if let Some(value) = custom_properties.get(&name) {
                    stack.push(name);
                    let resolved =
                        resolve_variable_value(value, custom_properties, stack, depth + 1);
                    stack.pop();
                    if let Some(resolved) = resolved {
                        return Ok(resolved);
                    }
                }
                fallback
                    .as_deref()
                    .and_then(|fallback| {
                        resolve_variable_value(fallback, custom_properties, stack, depth + 1)
                    })
                    .ok_or_else(|| input.new_custom_error(()))
            })?;
            output.push_str(&resolved);
            continue;
        }
        if matches!(token, Token::Comment(_)) {
            continue;
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
            input.parse_nested_block(|input| {
                resolve_remaining_components(input, output, custom_properties, stack, depth)
            })?;
            output.push(closing);
        }
    }
    Ok(())
}

struct StylesheetAdapter<'a> {
    source_order: &'a mut usize,
    unknown_properties: &'a mut BTreeSet<String>,
    unsupported_at_rules: &'a mut BTreeSet<String>,
    unsupported_media: &'a mut BTreeSet<String>,
    media: Option<MediaCondition>,
    depth: u8,
}

impl<'i> QualifiedRuleParser<'i> for StylesheetAdapter<'_> {
    type Prelude = String;
    type QualifiedRule = Vec<Rule>;
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
            unknown_properties: self.unknown_properties,
        };
        let declarations = RuleBodyParser::new(input, &mut declaration_parser)
            .filter_map(Result::ok)
            .collect();
        let rule = Rule {
            selector,
            declarations,
            source_order: *self.source_order,
            media: self.media,
        };
        *self.source_order += 1;
        Ok(vec![rule])
    }
}

impl<'i> AtRuleParser<'i> for StylesheetAdapter<'_> {
    type Prelude = MediaCondition;
    type AtRule = Vec<Rule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        if !name.eq_ignore_ascii_case("media") {
            self.unsupported_at_rules.insert(name.to_string());
            return Err(input.new_custom_error(()));
        }
        if self.depth >= 8 {
            self.unsupported_media
                .insert("nesting depth exceeded".to_owned());
            return Err(input.new_custom_error(()));
        }
        let start = input.position();
        parse_media_condition(input).inspect_err(|_| {
            self.unsupported_media
                .insert(input.slice_from(start).trim().to_owned());
        })
    }

    fn parse_block<'t>(
        &mut self,
        condition: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        let media = self
            .media
            .map_or(condition, |parent| parent.merge(condition));
        let mut nested = StylesheetAdapter {
            source_order: self.source_order,
            unknown_properties: self.unknown_properties,
            unsupported_at_rules: self.unsupported_at_rules,
            unsupported_media: self.unsupported_media,
            media: Some(media),
            depth: self.depth + 1,
        };
        let mut rules = Vec::new();
        for rule in StyleSheetParser::new(input, &mut nested) {
            rules.extend(rule.map_err(|(error, _)| error)?);
        }
        Ok(rules)
    }
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
        let name = if name.starts_with("--") {
            name.to_string()
        } else {
            name.to_ascii_lowercase()
        };
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
    name.starts_with("--")
        || matches!(
            name,
            "display"
                | "color"
                | "background-color"
                | "background"
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
                | "border"
                | "border-width"
                | "border-color"
                | "border-radius"
                | "box-shadow"
                | "opacity"
                | "text-decoration"
                | "width"
                | "height"
                | "min-width"
                | "max-width"
                | "box-sizing"
                | "overflow"
                | "flex-direction"
                | "flex-wrap"
                | "justify-content"
                | "align-items"
                | "gap"
                | "row-gap"
                | "column-gap"
                | "grid-template-columns"
                | "flex-grow"
                | "flex-shrink"
                | "flex-basis"
                | "order"
                | "position"
                | "top"
                | "right"
                | "bottom"
                | "left"
                | "z-index"
        )
}

fn parse_media_condition<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaCondition, ParseError<'i, ()>> {
    input.skip_whitespace();
    let mut condition = MediaCondition::default();
    let mut has_term = false;
    if let Ok(medium) = input.try_parse(Parser::expect_ident_cloned) {
        if !medium.eq_ignore_ascii_case("all") && !medium.eq_ignore_ascii_case("screen") {
            return Err(input.new_custom_error(()));
        }
        has_term = true;
    }
    loop {
        input.skip_whitespace();
        if input.is_exhausted() {
            return has_term
                .then_some(condition)
                .ok_or_else(|| input.new_custom_error(()));
        }
        if has_term {
            input.expect_ident_matching("and")?;
            input.skip_whitespace();
        }
        input.expect_parenthesis_block()?;
        let (name, value) = input.parse_nested_block(|input| {
            input.skip_whitespace();
            let name = input.expect_ident_cloned()?.to_ascii_lowercase();
            input.skip_whitespace();
            input.expect_colon()?;
            input.skip_whitespace();
            let value = media_length(input)?;
            input.skip_whitespace();
            input.expect_exhausted()?;
            Ok((name, value))
        })?;
        match name.as_str() {
            "min-width" => {
                condition.min_width_px = max_optional(condition.min_width_px, Some(value));
            }
            "max-width" => {
                condition.max_width_px = min_optional(condition.max_width_px, Some(value));
            }
            _ => return Err(input.new_custom_error(())),
        }
        has_term = true;
    }
}

fn media_length<'i>(input: &mut Parser<'i, '_>) -> Result<f32, ParseError<'i, ()>> {
    let token = input.next()?.clone();
    match token {
        Token::Dimension { value, unit, .. } if value.is_finite() && value >= 0.0 => {
            if unit.eq_ignore_ascii_case("px") {
                Ok(value)
            } else if unit.eq_ignore_ascii_case("em") || unit.eq_ignore_ascii_case("rem") {
                Ok(value * 16.0)
            } else {
                Err(input.new_custom_error(()))
            }
        }
        Token::Number { value: 0.0, .. } => Ok(0.0),
        _ => Err(input.new_custom_error(())),
    }
}

fn max_optional(first: Option<f32>, second: Option<f32>) -> Option<f32> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.max(second)),
        (first, second) => first.or(second),
    }
}

fn min_optional(first: Option<f32>, second: Option<f32>) -> Option<f32> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.min(second)),
        (first, second) => first.or(second),
    }
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
    fn accepts_v7_grid_and_visual_properties() {
        let sheet = parse(
            "main { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; \
             border-radius: 8px; opacity: .8; box-shadow: 0 2px 8px #000; \
             text-decoration: underline; background: #fff }",
        );

        assert!(sheet.diagnostics.is_empty());
        assert_eq!(sheet.rules[0].declarations.len(), 8);
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

    #[test]
    fn accepts_border_shorthand() {
        let sheet = parse("section { border: 3px solid #2457c5 }");
        assert_eq!(sheet.rules[0].declarations[0].name, "border");
        assert_eq!(sheet.rules[0].declarations[0].value, "3px solid #2457c5");
        assert!(sheet.diagnostics.is_empty());
    }

    #[test]
    fn parses_width_media_queries_and_custom_properties() {
        let sheet = parse(
            ":root { --BrandColor: #2468ac } \
             @media screen and (min-width: 40em) and (max-width: 1200px) { \
               main { color: var(--BrandColor) } \
             }",
        );
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[0].declarations[0].name, "--BrandColor");
        assert_eq!(sheet.rules[1].declarations[0].value, "var(--BrandColor)");
        let media = sheet.rules[1].media.unwrap();
        assert_eq!(media.min_width_px, Some(640.0));
        assert_eq!(media.max_width_px, Some(1200.0));
        assert!(!media.matches_width(639.0));
        assert!(media.matches_width(768.0));
        assert!(!media.matches_width(1201.0));
        assert!(sheet.diagnostics.is_empty());
    }

    #[test]
    fn merges_nested_media_and_diagnoses_unsupported_at_rules() {
        let sheet = parse(
            "@supports (display: grid) { main { display: grid } } \
             @media (min-width: 320px) { \
               @media (max-width: 800px) { main { display: flex } } \
             }",
        );
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(
            sheet.rules[0].media,
            Some(MediaCondition {
                min_width_px: Some(320.0),
                max_width_px: Some(800.0),
            })
        );
        assert!(
            sheet
                .diagnostics
                .contains(&"ignored unsupported CSS at-rule: supports".to_owned())
        );
    }

    #[test]
    fn rejects_unsupported_media_features_without_losing_following_rules() {
        let sheet = parse(
            "@media (prefers-color-scheme: dark) { p { color: white } } \
             p { color: black }",
        );
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].declarations[0].value, "black");
        assert!(
            sheet
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.starts_with("ignored unsupported media condition:"))
        );
    }

    #[test]
    fn resolves_nested_variables_fallbacks_and_cycles() {
        let properties = BTreeMap::from([
            ("--space".to_owned(), "8px".to_owned()),
            ("--double".to_owned(), "calc(var(--space) * 2)".to_owned()),
            ("--cycle".to_owned(), "var(--cycle)".to_owned()),
        ]);
        assert_eq!(
            resolve_variables("var(--double)", &properties).as_deref(),
            Some("calc(8px * 2)")
        );
        assert_eq!(
            resolve_variables("var(--missing, rgb(1, 2, 3))", &properties).as_deref(),
            Some("rgb(1, 2, 3)")
        );
        assert_eq!(
            resolve_variables("var(--cycle, blue)", &properties).as_deref(),
            Some("blue")
        );
        assert!(resolve_variables("var(--missing)", &properties).is_none());
    }
}
