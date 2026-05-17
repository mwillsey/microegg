use crate::util::Id;

#[derive(Default)]
pub struct UnionFind {
    n_classes: usize,
    parents: Vec<Id>,
    next: Vec<Id>,
}
impl UnionFind {
    pub fn len(&self) -> usize {
        self.parents.len()
    }

    pub fn n_classes(&self) -> usize {
        self.n_classes
    }

    pub fn find(&self, mut a: Id) -> Id {
        while a != self.parents[a.usize()] {
            a = self.parents[a.usize()];
        }
        a
    }

    pub fn find_mut(&mut self, mut a: Id) -> Id {
        // First walk to the root.
        let mut root = a;
        while root != self.parents[root.usize()] {
            root = self.parents[root.usize()];
        }

        // Then compress the full traversed path to the root.
        while a != root {
            let next = self.parents[a.usize()];
            self.parents[a.usize()] = root;
            a = next;
        }

        root
    }

    pub fn mkset(&mut self) -> Id {
        let id = Id::new(self.parents.len());
        self.parents.push(id);
        self.next.push(id);
        self.n_classes += 1;
        id
    }

    pub fn reparent(&mut self, a: Id, new_parent: Id) {
        let a = self.find_mut(a);
        let new_parent = self.find_mut(new_parent);
        if a == new_parent {
            return;
        }

        // Splice the two class rings by swapping each root's successor.
        let a_next = self.next[a.usize()];
        let b_next = self.next[new_parent.usize()];
        self.next[a.usize()] = b_next;
        self.next[new_parent.usize()] = a_next;

        self.parents[a.usize()] = new_parent;
    }

    pub fn union(&mut self, a: Id, b: Id) -> bool {
        let a = self.find_mut(a);
        let b = self.find_mut(b);
        if a != b {
            self.reparent(a, b);
            self.n_classes -= 1;
            true
        } else {
            false
        }
    }

    pub fn are_eq(&self, a: Id, b: Id) -> bool {
        self.find(a) == self.find(b)
    }

    pub fn arg_eq_mut(&mut self, a: Id, b: Id) -> bool {
        self.find_mut(a) == self.find_mut(b)
    }

    pub fn class_of(&self, id: Id) -> ClassIter<'_> {
        let root = self.find(id);
        ClassIter {
            uf: self,
            start: root,
            curr: Some(root),
        }
    }

    pub fn is_leader(&self, id: Id) -> bool {
        self.find(id) == id
    }
}

pub struct ClassIter<'a> {
    uf: &'a UnionFind,
    start: Id,
    curr: Option<Id>,
}

impl<'a> Iterator for ClassIter<'a> {
    type Item = Id;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.curr?;
        let next = self.uf.next[id.usize()];
        self.curr = if next == self.start { None } else { Some(next) };
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_members(uf: &UnionFind, id: Id) -> Vec<usize> {
        let mut out: Vec<usize> = uf.class_of(id).map(|i| i.usize()).collect();
        out.sort_unstable();
        out
    }

    #[test]
    fn class_of_singleton() {
        let mut uf = UnionFind::default();
        let a = uf.mkset();
        assert_eq!(class_members(&uf, a), vec![a.usize()]);
    }

    #[test]
    fn class_of_after_unions() {
        let mut uf = UnionFind::default();
        let a = uf.mkset();
        let b = uf.mkset();
        let c = uf.mkset();
        let d = uf.mkset();

        uf.union(a, b);
        uf.union(c, d);
        uf.union(b, c);

        assert_eq!(class_members(&uf, a), vec![0, 1, 2, 3]);
        assert_eq!(class_members(&uf, d), vec![0, 1, 2, 3]);
    }

    #[test]
    fn reparent_same_class_is_noop() {
        let mut uf = UnionFind::default();
        let a = uf.mkset();
        let b = uf.mkset();

        uf.union(a, b);
        let before = class_members(&uf, a);
        uf.reparent(a, b);
        let after = class_members(&uf, a);
        assert_eq!(before, after);
    }
}
