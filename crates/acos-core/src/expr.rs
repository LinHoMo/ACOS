//! Minimal condition expression language (`acos-expr`).
//!
//! Safe subset: identifier paths, literals, comparisons, existence checks,
//! and boolean combinators. **No arbitrary code execution** and **no fuzzy
//! reference resolution** — identifiers must resolve exactly in the env.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::AcosError;
use crate::types::TypedValue;

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Greater than.
    Gt,
    /// Less than.
    Lt,
    /// Greater than or equal.
    Ge,
    /// Less than or equal.
    Le,
}

/// An operand: literal or identifier path.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// Literal value.
    Literal(Value),
    /// Identifier path into the env.
    Path(Path),
}

/// Identifier path, e.g. `test.exit_code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    /// Path segments; first is the env binding name.
    pub segments: Vec<String>,
}

/// Parsed expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Logical and.
    And(Box<Expr>, Box<Expr>),
    /// Logical or.
    Or(Box<Expr>, Box<Expr>),
    /// Logical not.
    Not(Box<Expr>),
    /// Comparison.
    Cmp(Operand, CmpOp, Operand),
    /// Path resolves to a value.
    Exists(Path),
    /// Path does not resolve to a value.
    NotExists(Path),
    /// Boolean literal (`true` / `false`).
    Lit(bool),
}

/// Parses an expression string.
pub fn parse(input: &str) -> Result<Expr, AcosError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_or()?;
    if !matches!(parser.peek(), Some(Token::Eof)) {
        return Err(err("unexpected token after expression"));
    }
    Ok(expr)
}

/// Evaluates an expression against the environment (exact identifier lookup).
pub fn evaluate(expr: &Expr, env: &HashMap<String, TypedValue>) -> Result<bool, AcosError> {
    match expr {
        Expr::And(a, b) => Ok(evaluate(a, env)? && evaluate(b, env)?),
        Expr::Or(a, b) => Ok(evaluate(a, env)? || evaluate(b, env)?),
        Expr::Not(a) => Ok(!evaluate(a, env)?),
        Expr::Cmp(l, op, r) => {
            let lv = resolve_operand(l, env)?;
            let rv = resolve_operand(r, env)?;
            compare(&lv, *op, &rv)
        }
        Expr::Exists(p) => Ok(resolve_path(p, env)?.is_some()),
        Expr::NotExists(p) => Ok(resolve_path(p, env)?.is_none()),
        Expr::Lit(b) => Ok(*b),
    }
}

/// Collects the root identifier of every path in the expression (for
/// compile-time validation).
pub fn collect_identifiers(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::And(a, b) | Expr::Or(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Not(a) => walk(a, out),
            Expr::Cmp(l, _, r) => {
                for op in [l, r] {
                    if let Operand::Path(p) = op {
                        out.push(p.segments[0].clone());
                    }
                }
            }
            Expr::Exists(p) | Expr::NotExists(p) => out.push(p.segments[0].clone()),
            Expr::Lit(_) => {}
        }
    }
    walk(expr, &mut out);
    out
}

fn err(message: impl Into<String>) -> AcosError {
    AcosError::ValidationFailure { message: message.into() }
}

fn resolve_operand(op: &Operand, env: &HashMap<String, TypedValue>) -> Result<Value, AcosError> {
    match op {
        Operand::Literal(v) => Ok(v.clone()),
        Operand::Path(p) => resolve_path(p, env)?.ok_or_else(|| {
            err(format!(
                "condition referenced unknown binding '{}'",
                p.segments[0]
            ))
        }),
    }
}

fn resolve_path(p: &Path, env: &HashMap<String, TypedValue>) -> Result<Option<Value>, AcosError> {
    let root = p
        .segments
        .first()
        .ok_or_else(|| err("empty path in condition"))?;
    let Some(tv) = env.get(root) else {
        return Ok(None);
    };
    let mut current = tv.payload.clone();
    for seg in &p.segments[1..] {
        match current {
            Value::Object(map) => {
                let Some(v) = map.get(seg) else {
                    return Ok(None);
                };
                current = v.clone();
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(current))
}

fn compare(l: &Value, op: CmpOp, r: &Value) -> Result<bool, AcosError> {
    match op {
        CmpOp::Eq => Ok(l == r),
        CmpOp::Ne => Ok(l != r),
        CmpOp::Gt | CmpOp::Lt | CmpOp::Ge | CmpOp::Le => {
            let (Some(ln), Some(rn)) = (l.as_f64(), r.as_f64()) else {
                return Err(err(format!(
                    "ordering comparison requires numbers, got {l} and {r}"
                )));
            };
            Ok(match op {
                CmpOp::Gt => ln > rn,
                CmpOp::Lt => ln < rn,
                CmpOp::Ge => ln >= rn,
                CmpOp::Le => ln <= rn,
                CmpOp::Eq | CmpOp::Ne => unreachable!(),
            })
        }
    }
}

// ── tokenizer ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    Str(String),
    True,
    False,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    And,
    Or,
    Not,
    LParen,
    RParen,
    Dot,
    Eof,
}

fn tokenize(input: &str) -> Result<Vec<Token>, AcosError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' => {
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            '.' => {
                tokens.push(Token::Dot);
                chars.next();
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Eq);
                } else {
                    return Err(err("expected '=='"));
                }
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ne);
                } else {
                    tokens.push(Token::Not);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ge);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Le);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::And);
                } else {
                    return Err(err("expected '&&'"));
                }
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::Or);
                } else {
                    return Err(err("expected '||'"));
                }
            }
            '\'' => {
                chars.next();
                let mut s = String::new();
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    s.push(c);
                }
                tokens.push(Token::Str(s));
            }
            c if c.is_ascii_digit() => {
                let mut s = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        s.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let n: f64 = s.parse().map_err(|_| err(format!("invalid number '{s}'")))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut s = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_alphanumeric() || d == '_' {
                        s.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(match s.as_str() {
                    "true" => Token::True,
                    "false" => Token::False,
                    _ => Token::Ident(s),
                });
            }
            other => return Err(err(format!("unexpected character '{other}'"))),
        }
    }
    tokens.push(Token::Eof);
    Ok(tokens)
}

// ── parser ───────────────────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, t: &Token) -> Result<(), AcosError> {
        if self.peek() == Some(t) {
            self.pos += 1;
            Ok(())
        } else {
            Err(err(format!("expected {t:?}, found {:?}", self.peek())))
        }
    }

    fn parse_or(&mut self) -> Result<Expr, AcosError> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.pos += 1;
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, AcosError> {
        let mut left = self.parse_not()?;
        while self.peek() == Some(&Token::And) {
            self.pos += 1;
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, AcosError> {
        if self.peek() == Some(&Token::Not) {
            self.pos += 1;
            Ok(Expr::Not(Box::new(self.parse_not()?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, AcosError> {
        match self.peek() {
            Some(Token::LParen) => {
                self.pos += 1;
                let e = self.parse_or()?;
                self.expect(&Token::RParen)?;
                Ok(e)
            }
            Some(Token::Ident(name)) if name == "exists" || name == "not_exists" => {
                let not = name == "not_exists";
                self.pos += 1;
                self.expect(&Token::LParen)?;
                let path = self.parse_path()?;
                self.expect(&Token::RParen)?;
                if not {
                    Ok(Expr::NotExists(path))
                } else {
                    Ok(Expr::Exists(path))
                }
            }
            Some(Token::Ident(_)) => {
                let operand = self.parse_operand()?;
                self.finish_comparison(operand)
            }
            Some(Token::Number(_))
            | Some(Token::Str(_)) => {
                let operand = self.parse_operand()?;
                self.finish_comparison(operand)
            }
            Some(Token::True) => {
                self.pos += 1;
                Ok(Expr::Lit(true))
            }
            Some(Token::False) => {
                self.pos += 1;
                Ok(Expr::Lit(false))
            }
            other => Err(err(format!("unexpected token {other:?}"))),
        }
    }

    fn finish_comparison(&mut self, left: Operand) -> Result<Expr, AcosError> {
        let op = match self.peek() {
            Some(Token::Eq) => CmpOp::Eq,
            Some(Token::Ne) => CmpOp::Ne,
            Some(Token::Gt) => CmpOp::Gt,
            Some(Token::Lt) => CmpOp::Lt,
            Some(Token::Ge) => CmpOp::Ge,
            Some(Token::Le) => CmpOp::Le,
            Some(Token::Eof) | Some(Token::RParen) | Some(Token::And) | Some(Token::Or) => {
                return Err(err("bare operand in condition; use exists(...) or a comparison"));
            }
            _ => return Err(err("expected comparison operator")),
        };
        self.pos += 1;
        let right = self.parse_operand()?;
        Ok(Expr::Cmp(left, op, right))
    }

    fn parse_operand(&mut self) -> Result<Operand, AcosError> {
        match self.next() {
            Some(Token::Number(n)) => Ok(Operand::Literal(Value::from(n))),
            Some(Token::Str(s)) => Ok(Operand::Literal(Value::String(s))),
            Some(Token::True) => Ok(Operand::Literal(Value::Bool(true))),
            Some(Token::False) => Ok(Operand::Literal(Value::Bool(false))),
            Some(Token::Ident(name)) => {
                let mut segments = vec![name];
                while self.peek() == Some(&Token::Dot) {
                    self.pos += 1;
                    match self.next() {
                        Some(Token::Ident(seg)) => segments.push(seg),
                        _ => return Err(err("expected identifier after '.'")),
                    }
                }
                Ok(Operand::Path(Path { segments }))
            }
            other => Err(err(format!("expected operand, found {other:?}"))),
        }
    }

    fn parse_path(&mut self) -> Result<Path, AcosError> {
        match self.parse_operand()? {
            Operand::Path(p) => Ok(p),
            _ => Err(err("exists() requires an identifier path")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TypedValue, ValueType};

    fn env(pairs: &[(&str, Value)]) -> HashMap<String, TypedValue> {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    TypedValue {
                        value_type: ValueType::Scalar,
                        payload: v.clone(),
                    },
                )
            })
            .collect()
    }

    fn eval_str(input: &str, e: &HashMap<String, TypedValue>) -> bool {
        evaluate(&parse(input).unwrap(), e).unwrap()
    }

    #[test]
    fn evaluates_comparison_on_nested_field() {
        let e = env(&[("test", serde_json::json!({"exit_code": 1}))]);
        assert!(eval_str("test.exit_code != 0", &e));
        assert!(!eval_str("test.exit_code == 0", &e));
        assert!(eval_str("test.exit_code > 0 && test.exit_code <= 2", &e));
    }

    #[test]
    fn evaluates_exists_and_not_exists() {
        let e = env(&[("doc", serde_json::json!({"content": "x"}))]);
        assert!(eval_str("exists(doc)", &e));
        assert!(!eval_str("not_exists(doc)", &e));
        assert!(eval_str("not_exists(missing)", &e));
        assert!(eval_str("exists(doc.content)", &e));
    }

    #[test]
    fn literal_conditions_work() {
        let e = env(&[]);
        assert!(eval_str("1 == 1", &e));
        assert!(!eval_str("1 == 2", &e));
        assert!(eval_str("'ok' == 'ok'", &e));
        assert!(eval_str("true && !false", &e));
    }

    #[test]
    fn unknown_binding_is_an_error_not_false() {
        let e = env(&[]);
        let err = evaluate(&parse("undefined > 1").unwrap(), &e).unwrap_err();
        assert!(err.to_string().contains("undefined"));
    }

    #[test]
    fn collect_identifiers_returns_roots() {
        let expr = parse("exists(a) && b.x > 1 && !exists(c)").unwrap();
        let mut ids = collect_identifiers(&expr);
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn rejects_bare_identifier_condition() {
        assert!(parse("doc").is_err());
        assert!(parse("doc > ").is_err());
    }
}
