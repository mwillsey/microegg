/*
Common, mostly uninteresting utilities used across the codebase.
 */
use std::{collections::HashMap, ffi::OsStr, str::FromStr};

pub use symbol_table::GlobalSymbol as Symbol;

pub fn var_parse_or<T: FromStr>(key: impl AsRef<OsStr>, default: T) -> T
where
    T::Err: std::fmt::Debug,
{
    match std::env::var(key) {
        Ok(val) => val.parse().unwrap(),
        Err(_) => default,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct Id(u32);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Id {
    pub fn new(n: usize) -> Self {
        Id(n as u32)
    }

    pub fn usize(self) -> usize {
        self.0 as usize
    }
}

pub fn append<T, Container, I>(mut a: Container, b: I) -> Container
where
    Container: Extend<T>,
    I: IntoIterator<Item = T>,
{
    a.extend(b);
    a
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Subst(HashMap<Symbol, Id>);

impl Subst {
    pub fn with(&self, var: Symbol, id: Id) -> Self {
        let mut new = self.clone();
        new.0.insert(var, id);
        new
    }

    pub fn singleton(var: Symbol, id: Id) -> Self {
        Self::default().with(var, id)
    }

    pub fn join(&self, other: &Self) -> Option<Self> {
        let mut new = self.clone();
        for (&var, &id) in &other.0 {
            let old_id = new.0.insert(var, id);
            if let Some(old_id) = old_id
                && old_id != id
            {
                return None;
            }
        }
        Some(new)
    }

    pub fn add(&self, var: impl Into<Symbol>, id: Id) -> Self {
        self.join(&Subst::singleton(var.into(), id)).unwrap()
    }

    pub fn get(&self, name: Symbol) -> Option<Id> {
        self.0.get(&name).copied()
    }
}

impl std::ops::Add for Subst {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.join(&rhs).unwrap()
    }
}

impl std::ops::Index<Symbol> for Subst {
    type Output = Id;

    fn index(&self, index: Symbol) -> &Id {
        &self.0[&index]
    }
}

pub fn subst(var: impl Into<Symbol>, value: usize) -> Subst {
    Subst::singleton(var.into(), Id::new(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subst_add_merges_compatible_bindings() {
        let left = subst("x", 0);
        let added = left.add("y", Id::new(1));
        let expected = subst("x", 0) + subst("y", 1);

        assert_eq!(added, expected);
    }

    #[test]
    fn subst_join_rejects_conflict() {
        let left = subst("x", 0);
        let right = subst("x", 1);
        assert!(left.join(&right).is_none());
    }
}
