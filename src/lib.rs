use indexmap::{IndexMap, map::Entry};
use std::rc::Rc;

pub type Id = usize;
pub type Name = Rc<str>;

// The basics
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Node {
    f: Name,
    args: Vec<Id>,
}

#[derive(Default)]
pub struct EGraph {
    nodes: IndexMap<Node, Id>,
}

impl EGraph {
    pub fn add_node(&mut self, node: Node) -> Id {
        let node = self.canonicalize_node(&node);
        let new_id = self.nodes.len();
        println!("adding {:?}", node);
        let entry = match self.nodes.entry(node.clone()) {
            Entry::Vacant(e) => e.insert_entry(new_id),
            Entry::Occupied(e) => e,
        };
        let id = entry.index();
        println!("added  {:?} -> {}", node, id);
        self.find(id)
    }

    pub fn add(&mut self, f: impl Into<Name>, args: impl Into<Vec<Id>>) -> Id {
        let node = Node {
            f: f.into(),
            args: args.into(),
        };
        self.add_node(node)
    }

    pub fn union(&mut self, a: Id, b: Id) {
        let a = self.find(a);
        let b = self.find(b);
        if a != b {
            let (_node, id) = self.nodes.get_index_mut(a).unwrap();
            *id = b;
        }
    }

    pub fn find(&self, mut a: Id) -> Id {
        loop {
            let (_node, &id) = self.nodes.get_index(a).unwrap();
            if id == a {
                return id;
            }
            a = id;
        }
    }

    pub fn is_eq(&self, a: Id, b: Id) -> bool {
        self.find(a) == self.find(b)
    }

    pub fn nodes_in_class(&self, class: Id) -> impl Iterator<Item = &Node> {
        self.nodes
            .iter()
            .filter(move |(_, id)| self.is_eq(**id, class))
            .map(|(node, _)| node)
    }

    pub fn canonicalize_node(&self, node: &Node) -> Node {
        Node {
            f: node.f.clone(),
            args: node.args.iter().map(|id| self.find(*id)).collect(),
        }
    }

    pub fn rebuild(&mut self) {
        println!("rebuilding...");
        // copy needed for borrowing
        let nodes_copy = self.nodes.clone();

        let mut keep_going = true;
        while keep_going {
            keep_going = false;
            for (node, id) in &nodes_copy {
                let new_node = self.canonicalize_node(node);
                let new_id = self.add_node(new_node);
                if !self.is_eq(new_id, *id) {
                    self.union(new_id, *id);
                    keep_going = true;
                }
            }
        }
        println!("rebuilt!")
    }
}

// e-matching

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum Pattern {
    Var(Name),
    App(Name, Vec<Pattern>),
}

pub type Subst = IndexMap<Name, Id>;

impl EGraph {
    pub fn ematch(&self, pat: &Pattern, class: Id) -> Vec<Subst> {
        self.ematch_rec(0, pat, class, Default::default())
    }
    pub fn ematch_rec(
        &self,
        depth: usize,
        pat: &Pattern,
        class: Id,
        mut subst: Subst,
    ) -> Vec<Subst> {
        println!("{:d$}subst: {subst:?}", "", d = depth * 2,);
        println!("{:d$}matching {pat:?} at {class:?}", "", d = depth * 2,);
        match pat {
            Pattern::Var(name) => {
                if let Some(&id) = subst.get(name) {
                    if self.is_eq(id, class) {
                        return vec![subst];
                    } else {
                        return vec![];
                    }
                } else {
                    subst.insert(name.clone(), class);
                    return vec![subst];
                }
            }
            Pattern::App(f, args) => {
                let mut results = vec![];
                for node in self.nodes_in_class(class) {
                    let mut todo = vec![subst.clone()];
                    println!(
                        "{:d$}matching {pat:?} at {class:?} - {node:?}",
                        " ",
                        d = depth * 2,
                    );
                    if node.f == *f && node.args.len() == args.len() {
                        for (pa, na) in args.iter().zip(node.args.iter()) {
                            todo = todo
                                .into_iter()
                                .flat_map(|subst| self.ematch_rec(depth + 1, pa, *na, subst))
                                .collect();
                        }
                    }
                    results.extend(todo);
                }
                results
            }
        }
    }
}

#[test]
fn test_rebuild() {
    let mut eg = EGraph::default();
    let a = eg.add("a", []);
    let b = eg.add("b", []);
    let c = eg.add("c", []);
    let f1 = eg.add("f", [a, b]);
    let f2 = eg.add("f", [a, c]);

    eg.union(b, c);
    assert!(!eg.is_eq(f1, f2));

    eg.rebuild();
    assert!(eg.is_eq(f1, f2));
}

#[test]
fn test_match() {
    let mut eg = EGraph::default();
    let a = eg.add("a", []);
    let b = eg.add("b", []);

    let f1 = eg.add("f", [a, a]);
    let f2 = eg.add("f", [b, b]);
    let f3 = eg.add("f", [a, b]);

    eg.union(f1, f2);
    eg.union(f2, f3);
    eg.rebuild();

    use Pattern::*;
    let pat_fxx = App("f".into(), vec![Var("x".into()), Var("x".into())]);
    let pat_fxy = App("f".into(), vec![Var("x".into()), Var("y".into())]);

    let fxx_matches = eg.ematch(&pat_fxx, f1);
    assert_eq!(fxx_matches.len(), 2);

    let fxy_matches = eg.ematch(&pat_fxy, f1);
    assert_eq!(fxy_matches.len(), 3);
}
