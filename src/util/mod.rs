/*
Common, mostly uninteresting utilities used across the codebase.
 */
use std::{ffi::OsStr, str::FromStr};

pub mod sexp;
pub use sexp::*;

pub mod unionfind;
pub use unionfind::UnionFind;

pub use symbol_table::GlobalSymbol as Symbol;

pub type IndexSet<K> = indexmap::IndexSet<K, rustc_hash::FxBuildHasher>;
pub type IndexMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;

pub fn var_parse_or<T: FromStr>(key: impl AsRef<OsStr>, default: T) -> T
where
    T::Err: std::fmt::Debug,
{
    match std::env::var(key) {
        Ok(val) => val.parse().unwrap(),
        Err(_) => default,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, PartialOrd, Ord)]
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

pub struct DisplayIter<I>(pub I, pub &'static str);

impl<I: Clone + IntoIterator> std::fmt::Display for DisplayIter<I>
where
    I::Item: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let DisplayIter(iter, sep) = self;
        let mut iter = iter.clone().into_iter();
        if let Some(item) = iter.next() {
            write!(f, "{}", item)?;
        }
        for item in iter {
            write!(f, "{sep}{}", item)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Subst(smallvec::SmallVec<[(Symbol, Id); 4]>);

impl PartialEq for Subst {
    fn eq(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        self.0
            .iter()
            .all(|(name, id)| other.get(*name) == Some(*id))
    }
}

impl Eq for Subst {}

impl Subst {
    fn get_ref(&self, name: Symbol) -> Option<&Id> {
        self.0
            .iter()
            .find_map(|(n, id)| if *n == name { Some(id) } else { None })
    }

    pub fn with(&self, var: Symbol, id: Id) -> Option<Self> {
        match self.get_ref(var) {
            Some(old) if *old == id => Some(self.clone()),
            Some(_) => None,
            None => {
                let mut new = self.clone();
                new.0.push((var, id));
                Some(new)
            }
        }
    }

    pub fn singleton(var: Symbol, id: Id) -> Self {
        Self::default().with(var, id).unwrap()
    }

    pub fn join(mut self, other: &Self) -> Option<Self> {
        for (var, id) in &other.0 {
            self = self.with(*var, *id)?;
        }
        Some(self)
    }

    pub fn get(&self, name: Symbol) -> Option<Id> {
        self.get_ref(name).copied()
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
        self.get_ref(index).unwrap()
    }
}

pub fn subst(var: impl Into<Symbol>, value: usize) -> Subst {
    Subst::singleton(var.into(), Id::new(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subst_join_rejects_conflict() {
        let left = subst("x", 0);
        let right = subst("x", 1);
        assert!(left.join(&right).is_none());
    }
}
