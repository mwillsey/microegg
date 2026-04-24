use crate::{sexp::Sexp, util::*};
use indexmap::{IndexMap, map::Entry};

// The basics
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Node(Symbol, Vec<Id>);

#[derive(Default)]
pub struct EGraph {
    nodes: IndexMap<Node, Id>,
}

impl EGraph {
    pub fn add_node(&mut self, node: Node) -> Id {
        let node = self.canonicalize_node(&node);
        let new_id = Id::new(self.nodes.len());
        println!("adding {:?}", node);
        let entry = match self.nodes.entry(node.clone()) {
            Entry::Vacant(e) => e.insert_entry(new_id),
            Entry::Occupied(e) => e,
        };
        let id = Id::new(entry.index());
        println!("added  {:?} -> {}", node, id);
        self.find(id)
    }

    pub fn add_sexp(&mut self, sexp: &Sexp) -> Id {
        match sexp {
            Sexp::Atom(s) => self.add_node(Node(*s, vec![])),
            Sexp::List(items) => {
                let Some((Sexp::Atom(f), rest)) = items.split_first() else {
                    panic!("expected atom at head of list");
                };
                let args = rest.iter().map(|s| self.add_sexp(s)).collect();
                self.add_node(Node(*f, args))
            }
        }
    }

    pub fn add(&mut self, s: &str) -> Id {
        self.add_sexp(&s.parse().unwrap())
    }

    pub fn union(&mut self, a: Id, b: Id) {
        let a = self.find(a);
        let b = self.find(b);
        if a != b {
            let (_node, id) = self.nodes.get_index_mut(a.usize()).unwrap();
            *id = b;
        }
    }

    pub fn find(&self, mut a: Id) -> Id {
        loop {
            let (_node, &id) = self.nodes.get_index(a.usize()).unwrap();
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
        Node(
            node.0.clone(),
            node.1.iter().map(|id| self.find(*id)).collect(),
        )
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

impl EGraph {
    pub fn ematch(&self, pat: &Sexp, class: Id) -> Vec<Subst> {
        self.ematch_rec(0, pat, class, Default::default())
    }
    pub fn ematch_rec(&self, depth: usize, pat: &Sexp, class: Id, subst: Subst) -> Vec<Subst> {
        println!("{:d$}subst: {subst:?}", "", d = depth * 2,);
        println!("{:d$}matching {pat:?} at {class:?}", "", d = depth * 2,);
        match pat {
            Sexp::Atom(name) => subst.with(*name, class).into_iter().collect(),
            Sexp::List(items) => {
                let Some((Sexp::Atom(f), args)) = items.split_first() else {
                    panic!("expected atom at head of list");
                };
                let mut results = vec![];
                for node in self.nodes_in_class(class) {
                    let mut todo = vec![subst.clone()];
                    println!(
                        "{:d$}matching {pat:?} at {class:?} - {node:?}",
                        " ",
                        d = depth * 2,
                    );
                    if node.0 == *f && node.1.len() == args.len() {
                        for (pa, na) in args.iter().zip(node.1.iter()) {
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
    let b = eg.add("b");
    let c = eg.add("c");
    let f1 = eg.add("(f a b)");
    let f2 = eg.add("(f a c)");

    eg.union(b, c);
    assert!(!eg.is_eq(f1, f2));

    eg.rebuild();
    assert!(eg.is_eq(f1, f2));
}

#[test]
fn test_match() {
    let mut eg = EGraph::default();
    let f1 = eg.add("(f a a)");
    let f2 = eg.add("(f b b)");
    let f3 = eg.add("(f a b)");

    eg.union(f1, f2);
    eg.union(f2, f3);
    eg.rebuild();

    let fxx_matches = eg.ematch(&"(f x x)".parse().unwrap(), f1);
    assert_eq!(fxx_matches.len(), 2);

    let fxy_matches = eg.ematch(&"(f x y)".parse().unwrap(), f1);
    assert_eq!(fxy_matches.len(), 3);
}
