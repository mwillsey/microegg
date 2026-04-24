/*
An e-graph impl with acyclic union nodes, curried functions,
 and "variables" in the e-graph for native patterns.


*/

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

    fn find_mut(&mut self, mut a: Id) -> Id {
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

impl Context {
    pub fn add_expr(&mut self, expr: Expr) -> Id {
        let expr = match expr {
            Expr::Pair(a, b) => Expr::Pair(self.uf.find_mut(a), self.uf.find_mut(b)),
            Expr::Union(a, b) => Expr::Union(self.uf.find_mut(a), self.uf.find_mut(b)),
            x => x,
        };
        let id = if let Some((id, _expr)) = self.hashcons.get_full(&expr) {
            Id::new(id)
        } else {
            let id = Id::new(self.hashcons.len());
            self.hashcons.insert(expr);
            assert_eq!(id, self.uf.mkset());
            id
        };
        self.uf.find_mut(id)
    }

    pub fn add_sexp(&mut self, sexp: &Sexp) -> Id {
        let id = match sexp {
            Sexp::Atom(s) if s.as_str().starts_with('?') => self.add_expr(Expr::Var(*s).into()),
            Sexp::Atom(s) => self.add_expr(Expr::Const(*s)),
            Sexp::List(items) => {
                let (first, rest) = items.split_first().unwrap();
                let mut acc = self.add_sexp(first);
                for sexp in rest {
                    let id = self.add_sexp(sexp);
                    let id = self.uf.find_mut(id);
                    acc = self.add_expr(Expr::Pair(acc, id));
                }
                acc
            }
        };
        self.uf.find_mut(id)
    }

    /// Parses and adds an s-expression
    pub fn add(&mut self, s: &str) -> Id {
        self.add_sexp(&s.parse().unwrap())
    }

    pub fn call(&mut self, f: impl Into<Symbol>, args: impl IntoIterator<Item = Id>) -> Id {
        let f = self.add_expr(Expr::Const(f.into()));
        args.into_iter()
            .fold(f, |a, b| self.add_expr(Expr::Pair(a, b)))
    }

    pub fn union(&mut self, target: Id, rewritten: Id) -> Id {
        let target = self.uf.find_mut(target);
        let rewritten = self.uf.find_mut(rewritten);
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
        let mut out = vec![];
        self.matches_with(pattern_id, target_id, &[Subst::default()], &mut out);
        out
    }

    pub fn matches_with(
        &self,
        pattern_id: Id,
        target_id: Id,
        substs: &[Subst],
        out: &mut Vec<Subst>,
    ) {
        use Expr::*;
        let pattern = self.get(pattern_id);
        let mut todo = vec![target_id];
        let mut seen = IndexSet::default();
        while let Some(target_id) = todo.pop() {
            if !seen.insert(target_id) {
                continue;
            }
            let target = self.get(target_id);
            match (pattern, target) {
                (Var(name), _) => out.extend(substs.iter().flat_map(|s| s.with(*name, target_id))),
                (Union(_, _), _) => panic!("can't match with union"),
                // this could be a nice optimization, but it messes up
                // when target includes variables
                // _ if pattern == target => vec![Subst::default()],
                (Const(a), Const(b)) if a == b => out.extend(substs.iter().cloned()),
                (_, Union(a, b)) => {
                    todo.extend([*a, *b]);
                }
                (Pair(a1, b1), Pair(a2, b2)) => {
                    let mut left = vec![];
                    self.matches_with(*a1, *a2, substs, &mut left);
                    self.matches_with(*b1, *b2, &left, out);
                }
                _ => {}
            }
        }
    }

    pub fn instantiate(&mut self, pattern_id: Id, subst: &Subst) -> Id {
        let pattern = self.get(pattern_id).clone();
        match pattern {
            Expr::Var(name) => self.uf.find_mut(subst[name]),
            Expr::Union(_, _) => panic!("can't instantiate union"),
            Expr::Pair(a, b) => {
                let a = self.instantiate(a, subst);
                let b = self.instantiate(b, subst);
                self.add_expr(Expr::Pair(a, b))
            }
            Expr::Const(_) => pattern_id,
        }
    }

    pub fn rewrite(&mut self, mut target: Id, lhs: Id, rhs: Id) -> Id {
        let mut seen_rewritten = IndexSet::default();
        for m in self.matches(lhs, target) {
            let rewritten = self.instantiate(rhs, &m);
            let rewritten = self.uf.find_mut(rewritten);
            if !seen_rewritten.insert(rewritten) {
                continue;
            }
            let union = self.union(target, rewritten);
            assert_eq!(union, self.uf.find(target));
            target = union;
        }
        target
    }

    pub fn rewrite2(&mut self, target: Id, rws: &[(Id, Id)]) -> Id {
        let mut done = IndexSet::default();
        let mut todo = vec![target];
        while let Some(target) = todo.pop() {
            if !done.insert(target) {
                continue;
            }
            for &(lhs, rhs) in rws {
                for m in self.matches(lhs, target) {
                    todo.push(self.instantiate(rhs, &m));
                }
            }
        }

        done.iter().cloned().fold(target, |t, r| self.union(t, r))
    }

    pub fn to_fixed(&mut self, mut f: impl FnMut(&mut Self, usize)) {
        let mut i = 0;
        loop {
            f(self, i);
            let after = self.hashcons.len();
            if after == i {
                break;
            }
            i = after;
        }
    }

    pub fn rewrite_all_to_fixed(&mut self, rewrites: &[(Id, Id)]) {
        let mut rewrite_start = 0;
        loop {
            // Saturate rewrites on the frontier of newly-added nodes.
            loop {
                let rewrite_end = self.hashcons.len();
                if rewrite_start == rewrite_end {
                    break;
                }
                for id in (rewrite_start..rewrite_end).map(Id::new) {
                    for &(lhs, rhs) in rewrites {
                        self.rewrite(id, lhs, rhs);
                    }
                }
                rewrite_start = rewrite_end;
            }

            let before_rebuild = self.hashcons.len();
            self.rebuild_pairs();
            let after_rebuild = self.hashcons.len();
            if after_rebuild == before_rebuild {
                break;
            }

            // Rebuild only appends new nodes; rewrite that tail next round.
            rewrite_start = before_rebuild;
        }
    }

    /// Rewrites only the subgraph reachable from `target`, visiting children
    /// before parents. This does not perform a global rebuild pass.
    pub fn rewrite_from_target_bottom_up(&mut self, target: Id, rewrites: &[(Id, Id)]) -> Id {
        let mut memo: IndexMap<Id, Id> = IndexMap::default();
        self.rewrite_from_target_bottom_up_impl(target, rewrites, &mut memo)
    }

    fn rewrite_from_target_bottom_up_impl(
        &mut self,
        target: Id,
        rewrites: &[(Id, Id)],
        memo: &mut IndexMap<Id, Id>,
    ) -> Id {
        let target = self.uf.find_mut(target);
        if let Some(done) = memo.get(&target) {
            return self.uf.find_mut(*done);
        }

        let expr = self.get(target).clone();
        let mut current = match expr {
            Expr::Var(_) | Expr::Const(_) => target,
            Expr::Pair(a, b) => {
                let a = self.rewrite_from_target_bottom_up_impl(a, rewrites, memo);
                let b = self.rewrite_from_target_bottom_up_impl(b, rewrites, memo);
                let rebuilt = self.add_expr(Expr::Pair(a, b));
                self.union(target, rebuilt)
            }
            Expr::Union(a, b) => {
                let a = self.rewrite_from_target_bottom_up_impl(a, rewrites, memo);
                let b = self.rewrite_from_target_bottom_up_impl(b, rewrites, memo);
                let rebuilt = self.add_expr(Expr::Union(a, b));
                self.union(target, rebuilt)
            }
        };

        loop {
            let before = self.uf.find_mut(current);
            current = self.rewrite2(current, rewrites);
            let after = self.uf.find_mut(current);
            current = after;
            if after == before {
                break;
            }
        }

        memo.insert(target, current);
        current
    }

    /// Rebuild pair nodes using canonical UF representatives for children.
    /// This gives a lightweight congruence-closure step after unions.
    #[inline(never)]
    fn rebuild_pairs(&mut self) {
        for id in self.ids(0) {
            if let Expr::Pair(a, b) = self.get(id).clone() {
                let a = self.uf.find_mut(a);
                let b = self.uf.find_mut(b);
                let Expr::Pair(x, y) = self.get(id) else {
                    unreachable!()
                };
                if (a, b) == (*x, *y) {
                    continue;
                }
                let rebuilt = self.add_expr(Expr::Pair(a, b));
                self.union(id, rebuilt);
            }
        }
    }

    pub fn ids(&self, start: usize) -> impl Iterator<Item = Id> + 'static {
        (start..self.hashcons.len()).map(Id::new)
    }
}

// Display, extraction, and statistics
impl Context {
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
        for id in self.ids(0) {
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

    pub fn print_statistics(&self) {
        println!("nodes: {}", self.hashcons.len());
        println!(
            "unions: {}",
            self.hashcons
                .iter()
                .filter(|e| matches!(e, Expr::Union(_, _)))
                .count()
        );
        let mut pairs: IndexMap<Id, Vec<Id>> = IndexMap::default();
        for expr in &self.hashcons {
            if let Expr::Pair(a, b) = expr {
                pairs.entry(*a).or_default().push(*b);
            }
        }
        println!("pairs: {}", pairs.values().map(|v| v.len()).sum::<usize>());
        pairs.sort_by_key(|_k, v| -(v.len() as isize));

        println!("top 10 most common first elements of pairs:");
        for (a, bs) in pairs.iter().take(10) {
            println!("{}: {} pairs", a, bs.len());
            for sexp in self.extract(*a, 5).into_iter().take(10) {
                println!("  {}", sexp);
            }
        }
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

        // reassociate/commute n things
        let n = var_parse_or("n", 4);
        let atoms: Vec<Id> = (0..n).map(|i| ctx.add(&format!("x{}", i))).collect();
        let mut f = |a, b| ctx.call("f", [a, b]);
        let iter = atoms.iter().cloned();
        let input = iter.clone().reduce(&mut f).unwrap();
        let goal = iter.rev().reduce(&mut f).unwrap();

        let rws = &[
            (ctx.add("(f (f ?x ?y) ?z)"), ctx.add("(f ?x (f ?y ?z))")),
            (ctx.add("(f ?x ?y)"), ctx.add("(f ?y ?x)")),
        ];
        ctx.rewrite_all_to_fixed(rws);

        // ctx.print_extract(5);
        ctx.print_statistics();

        assert_eq!(ctx.uf.find(input), ctx.uf.find(goal));
    }

    #[test]
    fn simple_rewriting_bottom_up_target() {
        let mut ctx = Context::default();

        let n = var_parse_or("n", 4);
        let atoms: Vec<Id> = (0..n).map(|i| ctx.add(&format!("x{}", i))).collect();
        let mut f = |a, b| ctx.call("f", [a, b]);
        let iter = atoms.iter().cloned();
        let input = iter.clone().reduce(&mut f).unwrap();
        let goal = iter.rev().reduce(&mut f).unwrap();

        let rws = &[
            (ctx.add("(f (f ?x ?y) ?z)"), ctx.add("(f ?x (f ?y ?z))")),
            (ctx.add("(f ?x ?y)"), ctx.add("(f ?y ?x)")),
        ];

        ctx.rewrite_from_target_bottom_up(input, rws);
        ctx.print_statistics();

        assert_eq!(ctx.uf.find(input), ctx.uf.find(goal));
    }
}
