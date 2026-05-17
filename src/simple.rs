/*!

A simple e-graph in the style of egg's SymbolLang.

 */
use crate::util::*;

// The basics
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Node(Symbol, Vec<Id>);

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.1.is_empty() {
            write!(f, "{}", self.0)
        } else {
            write!(f, "({} {})", self.0, DisplayIter(&self.1, " "))
        }
    }
}

#[derive(Default)]
pub struct EGraph {
    nodes: IndexMap<Node, Id>,
    rev: IndexMap<Id, Vec<Node>>,
    uf: UnionFind,
}

impl EGraph {
    pub fn add_node(&mut self, node: Node) -> Id {
        let node = self.canonicalize_node(&node);
        let id = *self.nodes.entry(node).or_insert_with(|| self.uf.mkset());
        self.uf.find_mut(id)
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

    pub fn union(&mut self, a: Id, b: Id) -> bool {
        self.uf.union(a, b)
    }

    pub fn nodes_in_class(&self, class: Id) -> impl Iterator<Item = &Node> {
        let class = self.uf.find(class);
        match self.rev.get(&class) {
            Some(nodes) => nodes.iter(),
            None => [].iter(),
        }
    }

    fn is_node_canonical(&self, node: &Node) -> bool {
        node.1.iter().all(|id| self.uf.is_leader(*id))
    }

    pub fn canonicalize_node(&mut self, node: &Node) -> Node {
        Node(
            node.0.clone(),
            node.1.iter().map(|id| self.uf.find_mut(*id)).collect(),
        )
    }

    pub fn rebuild(&mut self) {
        let mut keep_going = true;
        while keep_going {
            keep_going = false;
            let nodes = std::mem::take(&mut self.nodes);
            for (node, id) in nodes {
                let node = self.canonicalize_node(&node);
                let id = self.uf.find_mut(id);
                let id2 = *self.nodes.entry(node).or_insert(id);
                if self.union(id, id2) {
                    keep_going = true;
                }
            }
        }

        self.rev.clear();
        for (node, id) in &self.nodes {
            self.rev.entry(*id).or_default().push(node.clone());
        }
        self.rev.sort_keys();

        if cfg!(debug_assertions) {
            // nodes in nodes are canonical
            for (node, id) in &self.nodes {
                assert!(self.uf.is_leader(*id));
                assert!(self.is_node_canonical(node));
            }

            // nodes in class map are canonical
            for (id, nodes) in &self.rev {
                assert!(self.uf.is_leader(*id));
                for node in nodes {
                    assert!(self.is_node_canonical(node));
                }
            }

            // class map has exactly the leaders
            for i in 0..self.uf.n_classes() {
                let id = Id::new(i);
                if self.uf.is_leader(id) {
                    assert!(self.rev.contains_key(&id));
                } else {
                    assert!(!self.rev.contains_key(&id));
                }
            }
        }
    }

    pub fn print_statistics(&self) {
        let n_classes = self.rev.len();
        let n_nodes = self.nodes.len();
        println!("classes: {n_classes}, nodes: {n_nodes}");
    }

    pub fn print(&self) {
        for (id, nodes) in &self.rev {
            println!("{id}: {}", DisplayIter(nodes, ", "));
        }
    }
}

// e-matching

impl EGraph {
    pub fn ematch(&self, pat: &Sexp, class: Id) -> Vec<Subst> {
        self.ematch_rec(0, pat, class, Default::default())
    }
    pub fn ematch_rec(&self, depth: usize, pat: &Sexp, class: Id, subst: Subst) -> Vec<Subst> {
        match pat {
            // all atoms in a pattern are considered variables
            Sexp::Atom(name) => subst.with(*name, class).into_iter().collect(),
            Sexp::List(items) => {
                let Some((Sexp::Atom(f), args)) = items.split_first() else {
                    panic!("expected atom at head of list");
                };
                let mut results = vec![];
                for node in self.nodes_in_class(class) {
                    let mut todo = vec![subst.clone()];
                    if node.0 == *f && node.1.len() == args.len() {
                        for (pa, na) in args.iter().zip(node.1.iter()) {
                            todo = todo
                                .into_iter()
                                .flat_map(|subst| self.ematch_rec(depth + 1, pa, *na, subst))
                                .collect();
                        }
                        results.extend(todo);
                    }
                }
                results
            }
        }
    }
}

pub type Rewrite = (Sexp, Sexp);

// rewriting, rebuilding
impl EGraph {
    pub fn instantiate(&mut self, pattern: &Sexp, subst: &Subst) -> Id {
        match pattern {
            Sexp::Atom(name) => subst[*name],
            Sexp::List(items) => {
                let Some((Sexp::Atom(f), args)) = items.split_first() else {
                    panic!("expected atom at head of list");
                };
                let args = args
                    .iter()
                    .map(|arg| self.instantiate(arg, subst))
                    .collect();
                self.add_node(Node(*f, args))
            }
        }
    }

    pub fn rewrite(&mut self, rewrites: &[Rewrite]) {
        let mut all_matches = vec![];
        for rw in rewrites {
            for &class in self.rev.keys() {
                let tup = (rw, class, self.ematch(&rw.0, class));
                all_matches.push(tup);
            }
        }

        for ((_lhs, rhs), class, matches) in all_matches {
            for subst in matches {
                let replacement = self.instantiate(rhs, &subst);
                self.union(class, replacement);
            }
        }
    }

    pub fn rewrite_to_fixed(&mut self, rewrites: &[Rewrite]) {
        self.rebuild();
        loop {
            let before = (self.nodes.len(), self.uf.n_classes());
            self.rewrite(rewrites);
            self.rebuild();
            let after = (self.nodes.len(), self.uf.n_classes());
            if before == after {
                break;
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
    assert!(!eg.uf.are_eq(f1, f2));

    eg.rebuild();
    assert!(eg.uf.are_eq(f1, f2));
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

    let fxx_matches = eg.ematch(&sexp("(f x x)"), f1);
    assert_eq!(fxx_matches.len(), 2);

    let fxy_matches = eg.ematch(&sexp("(f x y)"), f1);
    assert_eq!(fxy_matches.len(), 3);
}

#[test]
fn test_ac_rewriting() {
    let mut eg = EGraph::default();

    let n = var_parse_or("n", 5);
    let atoms = (0..n).map(|i| format!("x{}", i));
    let f = |acc, x| format!("(f {acc} {x})");
    let input = atoms.clone().reduce(f).unwrap();
    let goal = atoms.rev().reduce(f).unwrap();

    let input = eg.add(&input);
    let goal = eg.add(&goal);
    let rws = [
        (sexp("(f (f x y) z)"), sexp("(f x (f y z))")),
        (sexp("(f x y)"), sexp("(f y x)")),
    ];

    eg.rewrite_to_fixed(&rws);
    eg.print_statistics();

    assert!(eg.uf.are_eq(input, goal));

    assert_eq!(eg.uf.n_classes(), 2usize.pow(n as _) - 1);
}
