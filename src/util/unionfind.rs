use crate::util::Id;

#[derive(Default)]
pub struct UnionFind {
    parent: Vec<Id>,
}

impl UnionFind {
    pub fn find(&self, mut a: Id) -> Id {
        while a != self.parent[a.usize()] {
            a = self.parent[a.usize()];
        }
        a
    }

    pub fn find_mut(&mut self, mut a: Id) -> Id {
        // First walk to the root.
        let mut root = a;
        while root != self.parent[root.usize()] {
            root = self.parent[root.usize()];
        }

        // Then compress the full traversed path to the root.
        while a != root {
            let next = self.parent[a.usize()];
            self.parent[a.usize()] = root;
            a = next;
        }

        root
    }

    pub fn mkset(&mut self) -> Id {
        let id = Id::new(self.parent.len());
        self.parent.push(id);
        id
    }

    pub fn reparent(&mut self, a: Id, new_parent: Id) {
        let a = self.find_mut(a);
        self.parent[a.usize()] = new_parent;
    }
}
