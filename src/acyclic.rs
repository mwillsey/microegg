/*
An e-graph impl with acyclic union nodes, curried functions,
 and "variables" in the e-graph for native patterns.


*/

use indexmap::IndexSet;

use crate::sexp::{self, Sexp};
use crate::util::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Union(Id, Id),
    Pair(Id, Id),
    Var(Symbol),
    Const(Symbol),
}

#[derive(Default)]
pub struct Context {
    hashcons: IndexSet<Expr>,
    uf: UnionFind,
}

#[derive(Default)]
struct UnionFind {
    parent: Vec<Id>,
}

impl UnionFind {
    pub fn find(&self, mut a: Id) -> Id {
        while a != self.parent[a.usize()] {
            a = self.parent[a.usize()];
        }
        a
    }

    pub fn mkset(&mut self) -> Id {
        let id = Id::new(self.parent.len());
        self.parent.push(id);
        id
    }

    pub fn reparent(&mut self, a: Id, new_parent: Id) {
        let a = self.find(a);
        self.parent[a.usize()] = new_parent;
    }
}

impl Context {
    pub fn add_expr(&mut self, expr: Expr) -> Id {
        if let Some((id, _expr)) = self.hashcons.get_full(&expr) {
            Id::new(id)
        } else {
            let id = Id::new(self.hashcons.len());
            self.hashcons.insert(expr);
            assert_eq!(id, self.uf.mkset());
            id
        }
    }

    pub fn add_sexp(&mut self, sexp: &Sexp) -> Id {
        match sexp {
            Sexp::Atom(s) if s.as_str().starts_with('?') => self.add_expr(Expr::Var(*s).into()),
            Sexp::Atom(s) => self.add_expr(Expr::Const(*s)),
            Sexp::List(items) => {
                let (first, rest) = items.split_first().unwrap();
                let mut acc = self.add_sexp(first);
                for sexp in rest {
                    let id = self.add_sexp(sexp);
                    acc = self.add_expr(Expr::Pair(acc, id));
                }
                acc
            }
        }
    }

    pub fn add(&mut self, s: &str) -> Id {
        self.add_sexp(&s.parse().unwrap())
    }

    pub fn call(&mut self, f: impl Into<Symbol>, args: impl IntoIterator<Item = Id>) -> Id {
        let f = self.add_expr(Expr::Const(f.into()));
        args.into_iter()
            .fold(f, |a, b| self.add_expr(Expr::Pair(a, b)))
    }

    pub fn union(&mut self, target: Id, rewritten: Id) -> Id {
        let target = self.uf.find(target);
        let rewritten = self.uf.find(rewritten);
        if target == rewritten {
            target
        } else {
            let union = self.add_expr(Expr::Union(target, rewritten));
            self.uf.reparent(target, union);
            self.uf.reparent(rewritten, union);
            union
        }
    }

    pub fn get(&self, id: Id) -> &Expr {
        self.hashcons.get_index(id.usize()).unwrap()
    }

    pub fn matches(&self, pattern_id: Id, target_id: Id) -> Vec<Subst> {
        use Expr::*;
        let pattern = self.get(pattern_id);
        let target = self.get(target_id);
        match (pattern, target) {
            (Var(name), _) => vec![Subst::singleton(*name, target_id)],
            (Union(_, _), _) => panic!("can't match with union"),
            // this could be a nice optimization, but it messes up
            // when target includes variables
            // _ if pattern == target => vec![Subst::default()],
            (Const(a), Const(b)) if a == b => vec![Subst::default()],
            (_, Union(a, b)) => append(self.matches(pattern_id, *a), self.matches(pattern_id, *b)),
            (Pair(a1, b1), Pair(a2, b2)) => self
                .matches(*a1, *a2)
                .into_iter()
                .flat_map(|a_subst| self.matches_with(*b1, *b2, a_subst))
                .collect(),
            _ => vec![],
        }
    }

    pub fn matches_with(&self, pattern: Id, target: Id, subst: Subst) -> Vec<Subst> {
        self.matches(pattern, target)
            .into_iter()
            .filter_map(|s| s.join(&subst))
            .collect()
    }

    pub fn instantiate(&mut self, pattern_id: Id, subst: &Subst) -> Id {
        let pattern = self.get(pattern_id).clone();
        match pattern {
            Expr::Var(name) => subst[name],
            Expr::Union(_, _) => panic!("can't instantiate union"),
            Expr::Pair(a, b) => {
                let a = self.instantiate(a, subst);
                let b = self.instantiate(b, subst);
                self.add_expr(Expr::Pair(a, b))
            }
            Expr::Const(_) => pattern_id,
        }
    }

    pub fn print(&self) {
        for (id, expr) in self.hashcons.iter().enumerate() {
            match expr {
                Expr::Union(a, b) => println!("{}: {} | {}", id, a, b),
                Expr::Pair(a, b) => println!("{}: ({} {})", id, a, b),
                Expr::Var(name) => println!("{}: {}", id, name),
                Expr::Const(name) => println!("{}: {}", id, name),
            }
        }
    }

    pub fn print_extract(&self, depth: usize) {
        for id in self.ids() {
            let extracted = self.extract(id, depth);
            let (first, rest) = extracted.split_first().unwrap();
            println!("{: >7}: {}", id, first);
            for sexp in rest {
                println!("         {}", sexp);
            }
        }
    }

    pub fn extract(&self, id: Id, depth: usize) -> Vec<Sexp> {
        if depth == 0 {
            return vec![sexp::atom(id.to_string())];
        }
        let expr = self.get(id);
        match expr {
            Expr::Union(a, b) => append(self.extract(*a, depth), self.extract(*b, depth)),
            Expr::Pair(a, b) => {
                let mut res = vec![];
                for a in self.extract(*a, depth - 1) {
                    for b in self.extract(*b, depth - 1) {
                        res.push(match &a {
                            Sexp::Atom(f) => sexp::list(vec![sexp::atom(*f), b]),
                            Sexp::List(items) => {
                                let mut items = items.clone();
                                items.push(b);
                                sexp::list(items)
                            }
                        })
                    }
                }
                res
            }
            Expr::Var(name) => vec![sexp::atom(*name)],
            Expr::Const(name) => vec![sexp::atom(*name)],
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = Id> + 'static {
        (0..self.hashcons.len()).map(Id::new)
    }

    pub fn rewrite(&mut self, mut target: Id, lhs: Id, rhs: Id) -> Id {
        for m in self.matches(lhs, target) {
            let rewritten = self.instantiate(rhs, &m);
            let union = self.union(target, rewritten);
            assert_eq!(union, self.uf.find(target));
            target = union;
        }
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_hashconses_equal_expressions() {
        let mut ctx = Context::default();
        let x1 = ctx.add("x");
        let x2 = ctx.add("x");
        assert_eq!(x1, x2);
    }

    #[test]
    fn insert_list_is_left_assoc() {
        let mut ctx = Context::default();
        let fab = ctx.add("(f a b)");
        let fab2 = ctx.add("((f a) b)");
        let fab3 = ctx.add("(f (a b))");
        assert_eq!(fab, fab2);
        assert_ne!(fab, fab3);
    }

    #[test]
    fn matches_repeated_var_enforces_same_target() {
        let mut ctx = Context::default();

        let pat = ctx.add("(?x ?x)");

        let a = ctx.add("a");
        let aa = ctx.add("(a a)");
        let ab = ctx.add("(a b)");

        let aa_matches = ctx.matches(pat, aa);
        assert_eq!(aa_matches, vec![subst("?x", a.usize())]);

        let ab_matches = ctx.matches(pat, ab);
        assert!(ab_matches.is_empty());
    }

    #[test]
    fn matches_exact_const_needs_equal_target() {
        let mut ctx = Context::default();
        let a = ctx.add("a");
        let b = ctx.add("b");

        assert_eq!(ctx.matches(a, a).len(), 1);
        assert!(ctx.matches(a, b).is_empty());
    }

    #[test]
    fn simple_rewriting() {
        let mut ctx = Context::default();

        // reassociate n things
        let n = var_parse_or("n", 5);
        let atoms: Vec<Id> = (0..n).map(|i| ctx.add(&format!("x{}", i))).collect();
        let mut f = |a, b| ctx.call("f", [a, b]);
        let iter = atoms.iter().cloned();
        let input = iter.clone().reduce(|a, b| f(a, b)).unwrap();
        let goal = iter.rev().reduce(|b, a| f(a, b)).unwrap();

        let lhs = ctx.add("(f (f ?x ?y) ?z)");
        let rhs = ctx.add("(f ?x (f ?y ?z))");

        let c_lhs = ctx.add("(f ?x ?y)");
        let c_rhs = ctx.add("(f ?y ?x)");

        for _ in 0..n {
            // rewrite everywhere
            for id in ctx.ids() {
                ctx.rewrite(id, lhs, rhs);
                ctx.rewrite(id, c_lhs, c_rhs);
            }
        }
        ctx.print_extract(9);

        assert_eq!(ctx.uf.find(input), ctx.uf.find(goal));
    }
}
