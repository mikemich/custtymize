//! Starship-compatible format strings: `$var`, `${var}`, `[text](style)`, `(conditional)`
//! and backslash escapes.

use std::collections::HashMap;

use super::style::{Seg, Style, parse_style};

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Text(String),
    Var(String),
    Styled(Vec<Node>, String),
    /// Shown only if at least one variable inside it is non-empty.
    Cond(Vec<Node>),
}

/// A variable's value: plain text takes the style of where it is used; segments
/// (module output, symbols that are formats themselves) keep their own.
#[derive(Clone, Debug)]
pub enum Val {
    Text(String),
    Segs(Vec<Seg>),
}

impl Val {
    fn is_empty(&self) -> bool {
        match self {
            Val::Text(t) => t.is_empty(),
            Val::Segs(s) => s.iter().all(|s| s.text.is_empty()),
        }
    }

    fn as_text(&self) -> String {
        match self {
            Val::Text(t) => t.clone(),
            Val::Segs(s) => super::style::segs_text(s),
        }
    }
}

/// Variables for one format evaluation.
#[derive(Default)]
pub struct Vars {
    map: HashMap<String, Val>,
}

impl Vars {
    pub fn new() -> Vars {
        Vars::default()
    }

    pub fn text(&mut self, k: &str, v: impl Into<String>) -> &mut Self {
        self.map.insert(k.to_string(), Val::Text(v.into()));
        self
    }

    pub fn segs(&mut self, k: &str, v: Vec<Seg>) -> &mut Self {
        self.map.insert(k.to_string(), Val::Segs(v));
        self
    }

    pub fn get(&self, k: &str) -> Option<&Val> {
        self.map.get(k)
    }
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    src: &'a str,
}

impl Parser<'_> {
    fn err(&self, msg: &str) -> String {
        format!("{msg} at column {} in {:?}", self.pos + 1, self.src)
    }

    fn seq(&mut self, end: Option<char>) -> Result<Vec<Node>, String> {
        let mut nodes = Vec::new();
        let mut text = String::new();
        let flush = |text: &mut String, nodes: &mut Vec<Node>| {
            if !text.is_empty() {
                nodes.push(Node::Text(std::mem::take(text)));
            }
        };
        loop {
            let Some(&c) = self.chars.get(self.pos) else {
                return match end {
                    Some(e) => Err(self.err(&format!("missing closing `{e}`"))),
                    None => {
                        flush(&mut text, &mut nodes);
                        Ok(nodes)
                    }
                };
            };
            self.pos += 1;
            match c {
                '\\' => {
                    let next = self.chars.get(self.pos).copied().unwrap_or('\\');
                    self.pos += 1;
                    text.push(next);
                }
                '$' => {
                    flush(&mut text, &mut nodes);
                    let name = if self.chars.get(self.pos) == Some(&'{') {
                        self.pos += 1;
                        let start = self.pos;
                        while self.chars.get(self.pos).is_some_and(|&c| c != '}') {
                            self.pos += 1;
                        }
                        if self.pos >= self.chars.len() {
                            return Err(self.err("missing `}` after `${`"));
                        }
                        let n: String = self.chars[start..self.pos].iter().collect();
                        self.pos += 1;
                        n
                    } else {
                        let start = self.pos;
                        while self.chars.get(self.pos).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') {
                            self.pos += 1;
                        }
                        self.chars[start..self.pos].iter().collect()
                    };
                    if name.is_empty() {
                        return Err(self.err("expected a variable name after `$`"));
                    }
                    nodes.push(Node::Var(name));
                }
                '[' => {
                    flush(&mut text, &mut nodes);
                    let inner = self.seq(Some(']'))?;
                    if self.chars.get(self.pos) != Some(&'(') {
                        return Err(self.err("expected `(style)` after `[text]`"));
                    }
                    self.pos += 1;
                    let start = self.pos;
                    while self.chars.get(self.pos).is_some_and(|&c| c != ')') {
                        self.pos += 1;
                    }
                    if self.pos >= self.chars.len() {
                        return Err(self.err("missing `)` after style"));
                    }
                    let style: String = self.chars[start..self.pos].iter().collect();
                    self.pos += 1;
                    nodes.push(Node::Styled(inner, style));
                }
                '(' => {
                    flush(&mut text, &mut nodes);
                    let inner = self.seq(Some(')'))?;
                    nodes.push(Node::Cond(inner));
                }
                c if Some(c) == end => {
                    flush(&mut text, &mut nodes);
                    return Ok(nodes);
                }
                ']' | ')' => return Err(self.err(&format!("unexpected `{c}` (escape it as `\\{c}`)"))),
                c => text.push(c),
            }
        }
    }
}

pub fn parse(src: &str) -> Result<Vec<Node>, String> {
    Parser { chars: src.chars().collect(), pos: 0, src }.seq(None)
}

/// Replaces `$name` / `${name}` in a style string with variable values.
fn expand_style(s: &str, vars: &dyn Fn(&str) -> Option<Val>) -> String {
    if !s.contains('$') {
        return s.to_string();
    }
    match parse(s) {
        Ok(nodes) => nodes
            .iter()
            .map(|n| match n {
                Node::Text(t) => t.clone(),
                Node::Var(v) => vars(v).map(|v| v.as_text()).unwrap_or_default(),
                _ => String::new(),
            })
            .collect(),
        Err(_) => s.to_string(),
    }
}

fn eval_into(
    nodes: &[Node],
    vars: &dyn Fn(&str) -> Option<Val>,
    style: Style,
    out: &mut Vec<Seg>,
    bad: &mut Vec<String>,
) -> bool {
    let mut any = false;
    for n in nodes {
        match n {
            Node::Text(t) => out.push(Seg { text: t.clone(), style }),
            Node::Var(name) => {
                if let Some(v) = vars(name) {
                    any |= !v.is_empty();
                    match v {
                        Val::Text(t) => out.push(Seg { text: t, style }),
                        Val::Segs(segs) => {
                            out.extend(segs.into_iter().map(|s| Seg { style: s.style.inherit(style), text: s.text }))
                        }
                    }
                }
            }
            Node::Styled(inner, st) => {
                let child = parse_style(&expand_style(st, vars), bad).inherit(style);
                any |= eval_into(inner, vars, child, out, bad);
            }
            Node::Cond(inner) => {
                let mut tmp = Vec::new();
                if eval_into(inner, vars, style, &mut tmp, bad) {
                    out.extend(tmp);
                    any = true;
                }
            }
        }
    }
    any
}

/// Evaluates parsed nodes. Unknown style words are appended to `bad`.
pub fn eval(nodes: &[Node], vars: &dyn Fn(&str) -> Option<Val>, bad: &mut Vec<String>) -> Vec<Seg> {
    let mut out = Vec::new();
    eval_into(nodes, vars, Style::default(), &mut out, bad);
    out
}

/// Parses and evaluates a format against a variable map.
pub fn render(src: &str, vars: &Vars) -> Result<Vec<Seg>, String> {
    let nodes = parse(src)?;
    let mut bad = Vec::new();
    Ok(eval(&nodes, &|k| vars.get(k).cloned(), &mut bad))
}

/// Every variable a format mentions, including inside groups.
pub fn variables(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        match n {
            Node::Var(v) => out.push(v.clone()),
            Node::Styled(inner, _) | Node::Cond(inner) => variables(inner, out),
            Node::Text(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::style::{Color, segs_text};

    fn vars(pairs: &[(&str, &str)]) -> Vars {
        let mut v = Vars::new();
        for (k, val) in pairs {
            v.text(k, *val);
        }
        v
    }

    #[test]
    fn parses_and_renders() {
        let v = vars(&[("branch", "main"), ("symbol", "@"), ("style", "bold purple")]);
        let out = render("on [$symbol$branch]($style) ", &v).expect("ok");
        assert_eq!(segs_text(&out), "on @main ");
        assert_eq!(out[1].style.fg, Some(Color::Ansi(5)));
        assert_eq!(out[0].style, Style::default());
    }

    #[test]
    fn conditional_groups_hide_when_empty() {
        let v = vars(&[("version", ""), ("symbol", "")]);
        assert_eq!(segs_text(&render("a($version )b", &v).expect("ok")), "ab");
        let v = vars(&[("version", "v1")]);
        assert_eq!(segs_text(&render("a($version )b", &v).expect("ok")), "av1 b");
        // A group with only text never shows.
        assert_eq!(segs_text(&render("(plain)", &v).expect("ok")), "");
        // Nested: the outer group shows because the inner one has a value.
        assert_eq!(segs_text(&render("([x($version)](red))", &v).expect("ok")), "xv1");
    }

    #[test]
    fn escapes_and_braced_vars() {
        let v = vars(&[("custom.vpn", "on"), ("virtualenv", "venv")]);
        assert_eq!(segs_text(&render(r"\[${custom.vpn}\] \$ (\($virtualenv\))", &v).expect("ok")), "[on] $ (venv)");
    }

    #[test]
    fn nested_styles_inherit() {
        let out = render("[a[b](red)](bold bg:blue)", &Vars::new()).expect("ok");
        assert_eq!(out[1].style.fg, Some(Color::Ansi(1)));
        assert_eq!(out[1].style.bg, Some(Color::Ansi(4)));
        assert_eq!(out[1].style.attrs, crate::engine::style::BOLD);
    }

    #[test]
    fn errors_are_reported() {
        assert!(parse("[$path(").is_err());
        assert!(parse("[x]").is_err());
        assert!(parse("$ alone").is_err());
        assert!(parse("oops)").is_err());
        assert!(parse("${unclosed").is_err());
    }
}
