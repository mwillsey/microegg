use crate::util::Symbol;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Sexp {
    Atom(Symbol),
    List(Vec<Sexp>),
}

impl std::fmt::Display for Sexp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sexp::Atom(s) => write!(f, "{}", s),
            Sexp::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }
        }
    }
}

pub fn atom(s: impl Into<Symbol>) -> Sexp {
    Sexp::Atom(s.into())
}

pub fn list(items: impl Into<Vec<Sexp>>) -> Sexp {
    Sexp::List(items.into())
}

fn skip_ws(input: &str, pos: &mut usize) {
    while let Some(&b) = input.as_bytes().get(*pos) {
        if b.is_ascii_whitespace() {
            *pos += 1;
        } else {
            break;
        }
    }
}

fn parse_atom(input: &str, pos: &mut usize) -> Result<Sexp, ()> {
    let start = *pos;
    while let Some(&b) = input.as_bytes().get(*pos) {
        if b.is_ascii_whitespace() || b == b'(' || b == b')' {
            break;
        }
        *pos += 1;
    }

    if start == *pos {
        return Err(());
    }

    Ok(atom(input[start..*pos].to_owned()))
}

fn parse_many(input: &str, pos: &mut usize) -> Result<Vec<Sexp>, ()> {
    let mut items = Vec::new();
    loop {
        skip_ws(input, pos);
        match input.as_bytes().get(*pos) {
            Some(b')') => return Ok(items),
            Some(_) => items.push(parse_sexp(input, pos)?),
            None => return Err(()),
        }
    }
}

fn parse_list(input: &str, pos: &mut usize) -> Result<Sexp, ()> {
    *pos += 1; // consume '('
    let items = parse_many(input, pos)?;
    match input.as_bytes().get(*pos) {
        Some(b')') => {
            *pos += 1;
            Ok(list(items))
        }
        _ => Err(()),
    }
}

fn parse_sexp(input: &str, pos: &mut usize) -> Result<Sexp, ()> {
    skip_ws(input, pos);
    match input.as_bytes().get(*pos) {
        Some(b'(') => parse_list(input, pos),
        Some(b')') => Err(()),
        Some(_) => parse_atom(input, pos),
        None => Err(()),
    }
}

impl FromStr for Sexp {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut pos = 0;
        let parsed = parse_sexp(s, &mut pos)?;
        skip_ws(s, &mut pos);
        if pos == s.len() { Ok(parsed) } else { Err(()) }
    }
}

pub fn sexp(s: &str) -> Sexp {
    s.parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use atom as a;

    #[test]
    fn parses_atom() {
        assert_eq!("foo".parse::<Sexp>(), Ok(a("foo")));
    }

    #[test]
    fn parses_nested_list() {
        let parsed = "(f (g x) y)".parse::<Sexp>();
        assert_eq!(parsed, Ok(list([a("f"), list([a("g"), a("x")]), a("y"),])));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!("".parse::<Sexp>().is_err());
        assert!("(a".parse::<Sexp>().is_err());
        assert!(")".parse::<Sexp>().is_err());
        assert!("a b".parse::<Sexp>().is_err());
        assert!("(foo bar) baz".parse::<Sexp>().is_err());
    }
}
