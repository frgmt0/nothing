use nothing_core::exp::Exp;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct HoleCounts {
    pub empty: usize,
    pub non_empty: usize,
}

pub fn count_holes(exp: &Exp) -> HoleCounts {
    let mut counts = HoleCounts::default();
    walk(exp, &mut counts);
    counts
}

fn walk(exp: &Exp, counts: &mut HoleCounts) {
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::Readline => {}
        Exp::EmptyHole(_) => counts.empty += 1,
        Exp::NonEmptyHole(_, inner) => {
            counts.non_empty += 1;
            walk(inner, counts);
        }
        Exp::Lam(_, _, body) => walk(body, counts),
        Exp::Ap(f, a) => {
            walk(f, counts);
            walk(a, counts);
        }
        Exp::BinOp(_, l, r) => {
            walk(l, counts);
            walk(r, counts);
        }
        Exp::If(c, t, e) | Exp::Fold(c, t, e) => {
            walk(c, counts);
            walk(t, counts);
            walk(e, counts);
        }
        Exp::Let(_, bound, body) => {
            walk(bound, counts);
            walk(body, counts);
        }
        Exp::Pair(l, r) | Exp::Cons(l, r) => {
            walk(l, counts);
            walk(r, counts);
        }
        Exp::Proj(_, inner)
        | Exp::Field(inner, _)
        | Exp::Inj(_, inner)
        | Exp::Print(inner)
        | Exp::CmdPure(inner) => walk(inner, counts),
        Exp::CmdBind(command, _, body) => {
            walk(command, counts);
            walk(body, counts);
        }
        Exp::Match(scrutinee, arms) => {
            walk(scrutinee, counts);
            for (_, _, body) in arms {
                walk(body, counts);
            }
        }
        Exp::Record(fields) => {
            for (_, value) in fields {
                walk(value, counts);
            }
        }
    }
}
