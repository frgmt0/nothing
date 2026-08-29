use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::stack::on_deep_stack;
use nothing_core::ty::Ty;

pub type Env = im::HashMap<Id, Dyn>;

#[derive(Clone, PartialEq, Debug)]
pub enum Dyn {
    Var(Id),
    Lam(Id, Ty, Box<Dyn>),
    Ap(Box<Dyn>, Box<Dyn>),
    Num(i64),
    Bool(bool),
    Str(String),
    BinOp(Op, Box<Dyn>, Box<Dyn>),
    If(Box<Dyn>, Box<Dyn>, Box<Dyn>),
    Let(Id, Box<Dyn>, Box<Dyn>),
    Pair(Box<Dyn>, Box<Dyn>),
    Proj(Side, Box<Dyn>),
    Nil,
    Cons(Box<Dyn>, Box<Dyn>),
    Fold(Box<Dyn>, Box<Dyn>, Box<Dyn>),
    Record(Vec<(Id, Dyn)>),
    Field(Box<Dyn>, Id),
    Inj(Id, Box<Dyn>),
    Match(Box<Dyn>, Vec<(Id, Id, Dyn)>),
    Print(Box<Dyn>),
    Readline,
    CmdPure(Box<Dyn>),
    CmdBind(Box<Dyn>, Id, Box<Dyn>),

    EmptyHole(HoleId, Env),
    NonEmptyHole(HoleId, Env, Box<Dyn>),
}

enum Frame {
    Lam(Id, Ty),
    Ap(Dyn),
    BinOp(Op, Dyn),
    If(Dyn, Dyn),
    Let(Id, Dyn),
    Pair(Dyn),
    Proj(Side),
    Cons(Dyn),
    Fold(Dyn, Dyn),
    Field(Id),
    Inj(Id),
    Print,
    CmdPure,
    CmdBind(Dyn, Id),
    Hole(HoleId, Env),
}

fn close(frame: Frame, inner: Dyn) -> Dyn {
    match frame {
        Frame::Lam(id, ty) => Dyn::Lam(id, ty, Box::new(inner)),
        Frame::Ap(fun) => Dyn::Ap(Box::new(fun), Box::new(inner)),
        Frame::BinOp(op, lhs) => Dyn::BinOp(op, Box::new(lhs), Box::new(inner)),
        Frame::If(cond, then) => Dyn::If(Box::new(cond), Box::new(then), Box::new(inner)),
        Frame::Let(id, bound) => Dyn::Let(id, Box::new(bound), Box::new(inner)),
        Frame::Pair(fst) => Dyn::Pair(Box::new(fst), Box::new(inner)),
        Frame::Proj(side) => Dyn::Proj(side, Box::new(inner)),
        Frame::Cons(head) => Dyn::Cons(Box::new(head), Box::new(inner)),
        Frame::Fold(list, init) => Dyn::Fold(Box::new(list), Box::new(init), Box::new(inner)),
        Frame::Field(id) => Dyn::Field(Box::new(inner), id),
        Frame::Inj(ctor) => Dyn::Inj(ctor, Box::new(inner)),
        Frame::Print => Dyn::Print(Box::new(inner)),
        Frame::CmdPure => Dyn::CmdPure(Box::new(inner)),
        Frame::CmdBind(command, id) => Dyn::CmdBind(Box::new(command), id, Box::new(inner)),
        Frame::Hole(h, env) => Dyn::NonEmptyHole(h, env, Box::new(inner)),
    }
}

fn unwind(mut built: Dyn, mut frames: Vec<Frame>) -> Dyn {
    while let Some(frame) = frames.pop() {
        built = close(frame, built);
    }
    built
}

pub fn elaborate(exp: &Exp) -> Dyn {
    elaborate_in(exp, &Env::new())
}

pub fn elaborate_in(exp: &Exp, sigma: &Env) -> Dyn {
    on_deep_stack(|| elaborate_walk(exp, sigma))
}

fn elaborate_walk(exp: &Exp, outer: &Env) -> Dyn {
    let mut frames: Vec<Frame> = Vec::new();
    let mut sigma = outer.clone();
    let mut cur = exp;
    let built = loop {
        match cur {
            Exp::Lam(id, ty, body) => {
                sigma = sigma.update(*id, Dyn::Var(*id));
                frames.push(Frame::Lam(*id, ty.clone()));
                cur = body;
            }
            Exp::Ap(fun, arg) => {
                frames.push(Frame::Ap(elaborate_walk(fun, &sigma)));
                cur = arg;
            }
            Exp::BinOp(op, lhs, rhs) => {
                frames.push(Frame::BinOp(*op, elaborate_walk(lhs, &sigma)));
                cur = rhs;
            }
            Exp::If(cond, then, else_) => {
                frames.push(Frame::If(
                    elaborate_walk(cond, &sigma),
                    elaborate_walk(then, &sigma),
                ));
                cur = else_;
            }
            Exp::Let(id, bound, body) => {
                let bound = elaborate_walk(bound, &sigma);
                sigma = sigma.update(*id, Dyn::Var(*id));
                frames.push(Frame::Let(*id, bound));
                cur = body;
            }
            Exp::Pair(fst, snd) => {
                frames.push(Frame::Pair(elaborate_walk(fst, &sigma)));
                cur = snd;
            }
            Exp::Proj(side, inner) => {
                frames.push(Frame::Proj(*side));
                cur = inner;
            }
            Exp::Cons(head, tail) => {
                frames.push(Frame::Cons(elaborate_walk(head, &sigma)));
                cur = tail;
            }
            Exp::Fold(list, init, step) => {
                frames.push(Frame::Fold(
                    elaborate_walk(list, &sigma),
                    elaborate_walk(init, &sigma),
                ));
                cur = step;
            }
            Exp::Field(subject, id) => {
                frames.push(Frame::Field(*id));
                cur = subject;
            }
            Exp::Inj(ctor, payload) => {
                frames.push(Frame::Inj(*ctor));
                cur = payload;
            }
            Exp::Print(text) => {
                frames.push(Frame::Print);
                cur = text;
            }
            Exp::CmdPure(value) => {
                frames.push(Frame::CmdPure);
                cur = value;
            }
            Exp::CmdBind(command, id, body) => {
                let command = elaborate_walk(command, &sigma);
                sigma = sigma.update(*id, Dyn::Var(*id));
                frames.push(Frame::CmdBind(command, *id));
                cur = body;
            }
            Exp::NonEmptyHole(h, inner) => {
                frames.push(Frame::Hole(*h, sigma.clone()));
                cur = inner;
            }
            Exp::Var(id) => break Dyn::Var(*id),
            Exp::Num(n) => break Dyn::Num(*n),
            Exp::Bool(b) => break Dyn::Bool(*b),
            Exp::Str(text) => break Dyn::Str(text.clone()),
            Exp::Nil => break Dyn::Nil,
            Exp::Readline => break Dyn::Readline,
            Exp::EmptyHole(h) => break Dyn::EmptyHole(*h, sigma.clone()),
            Exp::Record(fields) => {
                break Dyn::Record(
                    fields
                        .iter()
                        .map(|(id, value)| (*id, elaborate_walk(value, &sigma)))
                        .collect(),
                );
            }
            Exp::Match(scrutinee, arms) => {
                let scrutinee = elaborate_walk(scrutinee, &sigma);
                let arms = arms
                    .iter()
                    .map(|(ctor, binder, body)| {
                        let inner = sigma.update(*binder, Dyn::Var(*binder));
                        (*ctor, *binder, elaborate_walk(body, &inner))
                    })
                    .collect();
                break Dyn::Match(Box::new(scrutinee), arms);
            }
        }
    };
    unwind(built, frames)
}

pub fn subst(x: Id, v: &Dyn, d: &Dyn) -> Dyn {
    on_deep_stack(|| subst_walk(x, v, d))
}

fn subst_walk(x: Id, v: &Dyn, d: &Dyn) -> Dyn {
    let mut frames: Vec<Frame> = Vec::new();
    let mut cur = d;
    let built = loop {
        match cur {
            Dyn::Lam(id, ty, body) => {
                if *id == x {
                    break cur.clone();
                }
                frames.push(Frame::Lam(*id, ty.clone()));
                cur = body;
            }
            Dyn::Ap(fun, arg) => {
                frames.push(Frame::Ap(subst_walk(x, v, fun)));
                cur = arg;
            }
            Dyn::BinOp(op, lhs, rhs) => {
                frames.push(Frame::BinOp(*op, subst_walk(x, v, lhs)));
                cur = rhs;
            }
            Dyn::If(cond, then, else_) => {
                frames.push(Frame::If(subst_walk(x, v, cond), subst_walk(x, v, then)));
                cur = else_;
            }
            Dyn::Let(id, bound, body) => {
                let bound = subst_walk(x, v, bound);
                if *id == x {
                    break Dyn::Let(*id, Box::new(bound), body.clone());
                }
                frames.push(Frame::Let(*id, bound));
                cur = body;
            }
            Dyn::Pair(fst, snd) => {
                frames.push(Frame::Pair(subst_walk(x, v, fst)));
                cur = snd;
            }
            Dyn::Proj(side, inner) => {
                frames.push(Frame::Proj(*side));
                cur = inner;
            }
            Dyn::Cons(head, tail) => {
                frames.push(Frame::Cons(subst_walk(x, v, head)));
                cur = tail;
            }
            Dyn::Fold(list, init, step) => {
                frames.push(Frame::Fold(subst_walk(x, v, list), subst_walk(x, v, init)));
                cur = step;
            }
            Dyn::Field(subject, id) => {
                frames.push(Frame::Field(*id));
                cur = subject;
            }
            Dyn::Inj(ctor, payload) => {
                frames.push(Frame::Inj(*ctor));
                cur = payload;
            }
            Dyn::Print(text) => {
                frames.push(Frame::Print);
                cur = text;
            }
            Dyn::CmdPure(value) => {
                frames.push(Frame::CmdPure);
                cur = value;
            }
            Dyn::CmdBind(command, id, body) => {
                let command = subst_walk(x, v, command);
                if *id == x {
                    break Dyn::CmdBind(Box::new(command), *id, body.clone());
                }
                frames.push(Frame::CmdBind(command, *id));
                cur = body;
            }
            Dyn::NonEmptyHole(h, env, inner) => {
                frames.push(Frame::Hole(*h, subst_env(x, v, env)));
                cur = inner;
            }
            Dyn::Var(id) if *id == x => break v.clone(),
            Dyn::Var(id) => break Dyn::Var(*id),
            Dyn::Num(n) => break Dyn::Num(*n),
            Dyn::Bool(b) => break Dyn::Bool(*b),
            Dyn::Str(text) => break Dyn::Str(text.clone()),
            Dyn::Nil => break Dyn::Nil,
            Dyn::Readline => break Dyn::Readline,
            Dyn::EmptyHole(h, env) => break Dyn::EmptyHole(*h, subst_env(x, v, env)),
            Dyn::Record(fields) => {
                break Dyn::Record(
                    fields
                        .iter()
                        .map(|(id, value)| (*id, subst_walk(x, v, value)))
                        .collect(),
                );
            }
            Dyn::Match(scrutinee, arms) => {
                break Dyn::Match(
                    Box::new(subst_walk(x, v, scrutinee)),
                    arms.iter()
                        .map(|(ctor, binder, body)| {
                            if *binder == x {
                                (*ctor, *binder, body.clone())
                            } else {
                                (*ctor, *binder, subst_walk(x, v, body))
                            }
                        })
                        .collect(),
                );
            }
        }
    };
    unwind(built, frames)
}

fn subst_env(x: Id, v: &Dyn, env: &Env) -> Env {
    env.iter()
        .map(|(id, d)| (*id, subst_walk(x, v, d)))
        .collect()
}

pub fn is_value(d: &Dyn) -> bool {
    let mut pending = vec![d];
    while let Some(cur) = pending.pop() {
        match cur {
            Dyn::Num(_) | Dyn::Bool(_) | Dyn::Str(_) | Dyn::Lam(..) | Dyn::Nil | Dyn::Readline => {}
            Dyn::Pair(fst, snd) | Dyn::Cons(fst, snd) => {
                pending.push(fst);
                pending.push(snd);
            }
            Dyn::Record(fields) => pending.extend(fields.iter().map(|(_, value)| value)),
            Dyn::Inj(_, payload) => pending.push(payload),
            Dyn::Print(text) => pending.push(text),
            Dyn::CmdPure(value) => pending.push(value),
            Dyn::CmdBind(command, _, _) => pending.push(command),
            _ => return false,
        }
    }
    true
}

enum ExpFrame {
    Lam(Id, Ty),
    Ap(Exp),
    BinOp(Op, Exp),
    If(Exp, Exp),
    Let(Id, Exp),
    Pair(Exp),
    Proj(Side),
    Cons(Exp),
    Fold(Exp, Exp),
    Field(Id),
    Inj(Id),
    Print,
    CmdPure,
    CmdBind(Exp, Id),
    Hole(HoleId),
}

fn close_exp(frame: ExpFrame, inner: Exp) -> Exp {
    match frame {
        ExpFrame::Lam(id, ty) => Exp::Lam(id, ty, Box::new(inner)),
        ExpFrame::Ap(fun) => Exp::Ap(Box::new(fun), Box::new(inner)),
        ExpFrame::BinOp(op, lhs) => Exp::BinOp(op, Box::new(lhs), Box::new(inner)),
        ExpFrame::If(cond, then) => Exp::If(Box::new(cond), Box::new(then), Box::new(inner)),
        ExpFrame::Let(id, bound) => Exp::Let(id, Box::new(bound), Box::new(inner)),
        ExpFrame::Pair(fst) => Exp::Pair(Box::new(fst), Box::new(inner)),
        ExpFrame::Proj(side) => Exp::Proj(side, Box::new(inner)),
        ExpFrame::Cons(head) => Exp::Cons(Box::new(head), Box::new(inner)),
        ExpFrame::Fold(list, init) => Exp::Fold(Box::new(list), Box::new(init), Box::new(inner)),
        ExpFrame::Field(id) => Exp::Field(Box::new(inner), id),
        ExpFrame::Inj(ctor) => Exp::Inj(ctor, Box::new(inner)),
        ExpFrame::Print => Exp::Print(Box::new(inner)),
        ExpFrame::CmdPure => Exp::CmdPure(Box::new(inner)),
        ExpFrame::CmdBind(command, id) => Exp::CmdBind(Box::new(command), id, Box::new(inner)),
        ExpFrame::Hole(h) => Exp::NonEmptyHole(h, Box::new(inner)),
    }
}

pub fn to_exp(d: &Dyn) -> Exp {
    on_deep_stack(|| to_exp_walk(d))
}

fn to_exp_walk(d: &Dyn) -> Exp {
    let mut frames: Vec<ExpFrame> = Vec::new();
    let mut cur = d;
    let mut built = loop {
        match cur {
            Dyn::Lam(id, ty, body) => {
                frames.push(ExpFrame::Lam(*id, ty.clone()));
                cur = body;
            }
            Dyn::Ap(fun, arg) => {
                frames.push(ExpFrame::Ap(to_exp_walk(fun)));
                cur = arg;
            }
            Dyn::BinOp(op, lhs, rhs) => {
                frames.push(ExpFrame::BinOp(*op, to_exp_walk(lhs)));
                cur = rhs;
            }
            Dyn::If(cond, then, else_) => {
                frames.push(ExpFrame::If(to_exp_walk(cond), to_exp_walk(then)));
                cur = else_;
            }
            Dyn::Let(id, bound, body) => {
                frames.push(ExpFrame::Let(*id, to_exp_walk(bound)));
                cur = body;
            }
            Dyn::Pair(fst, snd) => {
                frames.push(ExpFrame::Pair(to_exp_walk(fst)));
                cur = snd;
            }
            Dyn::Proj(side, inner) => {
                frames.push(ExpFrame::Proj(*side));
                cur = inner;
            }
            Dyn::Cons(head, tail) => {
                frames.push(ExpFrame::Cons(to_exp_walk(head)));
                cur = tail;
            }
            Dyn::Fold(list, init, step) => {
                frames.push(ExpFrame::Fold(to_exp_walk(list), to_exp_walk(init)));
                cur = step;
            }
            Dyn::Field(subject, id) => {
                frames.push(ExpFrame::Field(*id));
                cur = subject;
            }
            Dyn::Inj(ctor, payload) => {
                frames.push(ExpFrame::Inj(*ctor));
                cur = payload;
            }
            Dyn::Print(text) => {
                frames.push(ExpFrame::Print);
                cur = text;
            }
            Dyn::CmdPure(value) => {
                frames.push(ExpFrame::CmdPure);
                cur = value;
            }
            Dyn::CmdBind(command, id, body) => {
                frames.push(ExpFrame::CmdBind(to_exp_walk(command), *id));
                cur = body;
            }
            Dyn::NonEmptyHole(h, _, inner) => {
                frames.push(ExpFrame::Hole(*h));
                cur = inner;
            }
            Dyn::Var(id) => break Exp::Var(*id),
            Dyn::Num(n) => break Exp::Num(*n),
            Dyn::Bool(b) => break Exp::Bool(*b),
            Dyn::Str(text) => break Exp::Str(text.clone()),
            Dyn::Nil => break Exp::Nil,
            Dyn::Readline => break Exp::Readline,
            Dyn::EmptyHole(h, _) => break Exp::EmptyHole(*h),
            Dyn::Record(fields) => {
                break Exp::Record(
                    fields
                        .iter()
                        .map(|(id, value)| (*id, to_exp_walk(value)))
                        .collect(),
                );
            }
            Dyn::Match(scrutinee, arms) => {
                break Exp::Match(
                    Box::new(to_exp_walk(scrutinee)),
                    arms.iter()
                        .map(|(ctor, binder, body)| (*ctor, *binder, to_exp_walk(body)))
                        .collect(),
                );
            }
        }
    };
    while let Some(frame) = frames.pop() {
        built = close_exp(frame, built);
    }
    built
}

pub fn render(d: &Dyn, names: &NameTable) -> String {
    nothing_core::render::render(&to_exp(d), names)
}

pub fn size(d: &Dyn) -> usize {
    let mut total = 0;
    let mut pending = vec![d];
    while let Some(cur) = pending.pop() {
        total += 1;
        match cur {
            Dyn::Var(_)
            | Dyn::Num(_)
            | Dyn::Bool(_)
            | Dyn::Str(_)
            | Dyn::Nil
            | Dyn::Readline
            | Dyn::EmptyHole(..) => {}
            Dyn::Lam(_, _, b)
            | Dyn::Proj(_, b)
            | Dyn::Field(b, _)
            | Dyn::Inj(_, b)
            | Dyn::Print(b)
            | Dyn::CmdPure(b)
            | Dyn::NonEmptyHole(_, _, b) => pending.push(b),
            Dyn::Record(fields) => pending.extend(fields.iter().map(|(_, value)| value)),
            Dyn::Match(scrutinee, arms) => {
                pending.push(scrutinee);
                pending.extend(arms.iter().map(|(_, _, body)| body));
            }
            Dyn::Ap(a, b)
            | Dyn::BinOp(_, a, b)
            | Dyn::Let(_, a, b)
            | Dyn::Pair(a, b)
            | Dyn::CmdBind(a, _, b)
            | Dyn::Cons(a, b) => {
                pending.push(a);
                pending.push(b);
            }
            Dyn::If(c, t, e) | Dyn::Fold(c, t, e) => {
                pending.push(c);
                pending.push(t);
                pending.push(e);
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::examples;

    fn x() -> Id {
        Id::from_u128(1)
    }

    fn y() -> Id {
        Id::from_u128(2)
    }

    fn h(n: u128) -> HoleId {
        HoleId::from_u128(n)
    }

    #[test]
    fn elaboration_keeps_the_tree_and_only_adds_hole_environments() {
        for exp in [
            examples::let_identity(),
            examples::increment_applied(),
            examples::clamp_to_one(),
            examples::pair_and_project(),
            examples::pair_with_empty_hole(),
            examples::add_with_empty_hole(),
            examples::square_and_compare(),
            examples::identity_hole_annotated_applied(),
            examples::add_with_non_empty_hole(),
            examples::if_over_pairs_with_hole(),
        ] {
            assert_eq!(to_exp(&elaborate(&exp)), exp);
        }
    }

    #[test]
    fn a_hole_starts_with_the_identity_substitution_over_its_scope() {
        let e = Exp::lam(x(), Ty::Num, Exp::empty_hole(h(0)));
        match elaborate(&e) {
            Dyn::Lam(_, _, body) => match *body {
                Dyn::EmptyHole(_, env) => {
                    assert_eq!(env.len(), 1);
                    assert_eq!(env.get(&x()), Some(&Dyn::Var(x())));
                }
                other => panic!("expected a hole, got {other:?}"),
            },
            other => panic!("expected a lambda, got {other:?}"),
        }
    }

    #[test]
    fn a_hole_at_the_top_captures_nothing() {
        match elaborate(&Exp::empty_hole(h(0))) {
            Dyn::EmptyHole(_, env) => assert!(env.is_empty()),
            other => panic!("expected a hole, got {other:?}"),
        }
    }

    #[test]
    fn substitution_rewrites_a_holes_environment_rather_than_the_hole() {
        let d = elaborate(&Exp::lam(x(), Ty::Num, Exp::empty_hole(h(0))));
        let body = match d {
            Dyn::Lam(_, _, body) => *body,
            other => panic!("expected a lambda, got {other:?}"),
        };
        match subst(x(), &Dyn::Num(5), &body) {
            Dyn::EmptyHole(id, env) => {
                assert_eq!(id, h(0), "the hole keeps its identity");
                assert_eq!(env.get(&x()), Some(&Dyn::Num(5)));
            }
            other => panic!("expected a hole, got {other:?}"),
        }
    }

    #[test]
    fn substitution_stops_at_a_binder_that_shadows_it() {
        let inner = Dyn::Lam(x(), Ty::Num, Box::new(Dyn::Var(x())));
        assert_eq!(subst(x(), &Dyn::Num(9), &inner), inner);

        let outer = Dyn::Lam(y(), Ty::Num, Box::new(Dyn::Var(x())));
        assert_eq!(
            subst(x(), &Dyn::Num(9), &outer),
            Dyn::Lam(y(), Ty::Num, Box::new(Dyn::Num(9)))
        );
    }

    #[test]
    fn a_let_binds_its_body_but_not_its_bound_expression() {
        let d = Dyn::Let(x(), Box::new(Dyn::Var(x())), Box::new(Dyn::Var(x())));
        assert_eq!(
            subst(x(), &Dyn::Num(3), &d),
            Dyn::Let(x(), Box::new(Dyn::Num(3)), Box::new(Dyn::Var(x())))
        );
    }

    #[test]
    fn values_are_the_hole_free_normal_forms() {
        assert!(is_value(&Dyn::Num(1)));
        assert!(is_value(&Dyn::Bool(true)));
        assert!(is_value(&Dyn::Lam(x(), Ty::Num, Box::new(Dyn::Var(x())))));
        assert!(is_value(&Dyn::Pair(
            Box::new(Dyn::Num(1)),
            Box::new(Dyn::Bool(false))
        )));
        assert!(!is_value(&Dyn::EmptyHole(h(0), Env::new())));
        assert!(!is_value(&Dyn::Pair(
            Box::new(Dyn::Num(1)),
            Box::new(Dyn::EmptyHole(h(0), Env::new()))
        )));
        assert!(!is_value(&Dyn::Var(x())));
    }

    #[test]
    fn a_residual_renders_through_the_ordinary_projection() {
        let names = examples::names();
        let d = elaborate(&examples::add_with_empty_hole());
        assert_eq!(render(&d, &names), "1 + ⦇⦈");
    }
}
