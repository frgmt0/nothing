use nothing_core::exp::Exp;

pub type Path = Vec<usize>;

pub fn is_prefix(a: &[usize], b: &[usize]) -> bool {
    a.len() <= b.len() && b[..a.len()] == *a
}

pub fn nested(a: &[usize], b: &[usize]) -> bool {
    is_prefix(a, b) || is_prefix(b, a)
}

pub fn extend(base: &[usize], step: usize) -> Path {
    let mut out = base.to_vec();
    out.push(step);
    out
}

pub fn arity(exp: &Exp) -> usize {
    match exp {
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::Str(_)
        | Exp::Nil
        | Exp::Readline
        | Exp::EmptyHole(_) => 0,
        Exp::Lam(..)
        | Exp::Proj(..)
        | Exp::Field(..)
        | Exp::Print(..)
        | Exp::CmdPure(..)
        | Exp::NonEmptyHole(..) => 1,
        Exp::Ap(..)
        | Exp::BinOp(..)
        | Exp::Let(..)
        | Exp::Pair(..)
        | Exp::CmdBind(..)
        | Exp::Cons(..) => 2,
        Exp::If(..) | Exp::Fold(..) => 3,
        Exp::Record(fields) => fields.len(),
        Exp::Inj(..) => 1,
        Exp::Match(_, arms) => arms.len() + 1,
    }
}

pub fn child(exp: &Exp, n: usize) -> Option<&Exp> {
    match (exp, n) {
        (Exp::Lam(_, _, body), 0) => Some(body),
        (Exp::Proj(_, inner), 0) => Some(inner),
        (Exp::NonEmptyHole(_, inner), 0) => Some(inner),
        (Exp::Ap(f, _), 0) => Some(f),
        (Exp::Ap(_, a), 1) => Some(a),
        (Exp::BinOp(_, l, _), 0) => Some(l),
        (Exp::BinOp(_, _, r), 1) => Some(r),
        (Exp::Let(_, bound, _), 0) => Some(bound),
        (Exp::Let(_, _, body), 1) => Some(body),
        (Exp::Pair(l, _), 0) => Some(l),
        (Exp::Pair(_, r), 1) => Some(r),
        (Exp::If(c, _, _), 0) => Some(c),
        (Exp::If(_, t, _), 1) => Some(t),
        (Exp::If(_, _, e), 2) => Some(e),
        (Exp::Cons(h, _), 0) => Some(h),
        (Exp::Cons(_, t), 1) => Some(t),
        (Exp::Fold(l, _, _), 0) => Some(l),
        (Exp::Fold(_, i, _), 1) => Some(i),
        (Exp::Fold(_, _, s), 2) => Some(s),
        (Exp::Field(subject, _), 0) => Some(subject),
        (Exp::Record(fields), n) => fields.get(n).map(|(_, value)| value),
        (Exp::Inj(_, payload), 0) => Some(payload),
        (Exp::Print(text), 0) => Some(text),
        (Exp::CmdPure(value), 0) => Some(value),
        (Exp::CmdBind(command, _, _), 0) => Some(command),
        (Exp::CmdBind(_, _, body), 1) => Some(body),
        (Exp::Match(scrutinee, _), 0) => Some(scrutinee),
        (Exp::Match(_, arms), n) => arms.get(n - 1).map(|(_, _, body)| body),
        _ => None,
    }
}

pub fn with_child(exp: &Exp, n: usize, new: Exp) -> Option<Exp> {
    let out = match (exp, n) {
        (Exp::Lam(id, ty, _), 0) => Exp::Lam(*id, ty.clone(), Box::new(new)),
        (Exp::Proj(side, _), 0) => Exp::Proj(*side, Box::new(new)),
        (Exp::NonEmptyHole(h, _), 0) => Exp::NonEmptyHole(*h, Box::new(new)),
        (Exp::Ap(_, a), 0) => Exp::Ap(Box::new(new), a.clone()),
        (Exp::Ap(f, _), 1) => Exp::Ap(f.clone(), Box::new(new)),
        (Exp::BinOp(op, _, r), 0) => Exp::BinOp(*op, Box::new(new), r.clone()),
        (Exp::BinOp(op, l, _), 1) => Exp::BinOp(*op, l.clone(), Box::new(new)),
        (Exp::Let(id, _, body), 0) => Exp::Let(*id, Box::new(new), body.clone()),
        (Exp::Let(id, bound, _), 1) => Exp::Let(*id, bound.clone(), Box::new(new)),
        (Exp::Pair(_, r), 0) => Exp::Pair(Box::new(new), r.clone()),
        (Exp::Pair(l, _), 1) => Exp::Pair(l.clone(), Box::new(new)),
        (Exp::If(_, t, e), 0) => Exp::If(Box::new(new), t.clone(), e.clone()),
        (Exp::If(c, _, e), 1) => Exp::If(c.clone(), Box::new(new), e.clone()),
        (Exp::If(c, t, _), 2) => Exp::If(c.clone(), t.clone(), Box::new(new)),
        (Exp::Cons(_, t), 0) => Exp::Cons(Box::new(new), t.clone()),
        (Exp::Cons(h, _), 1) => Exp::Cons(h.clone(), Box::new(new)),
        (Exp::Fold(_, i, s), 0) => Exp::Fold(Box::new(new), i.clone(), s.clone()),
        (Exp::Fold(l, _, s), 1) => Exp::Fold(l.clone(), Box::new(new), s.clone()),
        (Exp::Fold(l, i, _), 2) => Exp::Fold(l.clone(), i.clone(), Box::new(new)),
        (Exp::Field(_, id), 0) => Exp::Field(Box::new(new), *id),
        (Exp::Record(fields), n) if n < fields.len() => {
            let mut fields = fields.clone();
            fields[n].1 = new;
            Exp::Record(fields)
        }
        (Exp::Inj(ctor, _), 0) => Exp::Inj(*ctor, Box::new(new)),
        (Exp::Print(_), 0) => Exp::Print(Box::new(new)),
        (Exp::CmdPure(_), 0) => Exp::CmdPure(Box::new(new)),
        (Exp::CmdBind(_, id, body), 0) => Exp::CmdBind(Box::new(new), *id, body.clone()),
        (Exp::CmdBind(command, id, _), 1) => Exp::CmdBind(command.clone(), *id, Box::new(new)),
        (Exp::Match(_, arms), 0) => Exp::Match(Box::new(new), arms.clone()),
        (Exp::Match(scrutinee, arms), n) if n <= arms.len() => {
            let mut arms = arms.clone();
            arms[n - 1].2 = new;
            Exp::Match(scrutinee.clone(), arms)
        }
        _ => return None,
    };
    Some(out)
}

pub fn at<'a>(exp: &'a Exp, path: &[usize]) -> Option<&'a Exp> {
    let mut cursor = exp;
    for step in path {
        cursor = child(cursor, *step)?;
    }
    Some(cursor)
}

pub fn replace_at(exp: &Exp, path: &[usize], new: Exp) -> Option<Exp> {
    match path.split_first() {
        None => Some(new),
        Some((step, rest)) => {
            let old_child = child(exp, *step)?;
            let rebuilt = replace_at(old_child, rest, new)?;
            with_child(exp, *step, rebuilt)
        }
    }
}

pub fn describe(path: &[usize]) -> String {
    if path.is_empty() {
        "the whole program".to_string()
    } else {
        let steps: Vec<String> = path.iter().map(|s| s.to_string()).collect();
        format!("node {}", steps.join("."))
    }
}

pub fn label(exp: &Exp, path: &[usize]) -> String {
    let mut cursor = exp;
    let mut words: Vec<&'static str> = Vec::new();
    for step in path {
        let word = match (cursor, step) {
            (Exp::Lam(..), _) => "the body of a function",
            (Exp::Proj(..), _) => "the operand of a projection",
            (Exp::NonEmptyHole(..), _) => "the contents of a non-empty hole",
            (Exp::Ap(..), 0) => "the function of an application",
            (Exp::Ap(..), _) => "the argument of an application",
            (Exp::BinOp(..), 0) => "the left operand",
            (Exp::BinOp(..), _) => "the right operand",
            (Exp::Let(..), 0) => "a let binding's value",
            (Exp::Let(..), _) => "a let body",
            (Exp::Pair(..), 0) => "the first component",
            (Exp::Pair(..), _) => "the second component",
            (Exp::If(..), 0) => "an if condition",
            (Exp::If(..), 1) => "an if then-branch",
            (Exp::If(..), _) => "an if else-branch",
            (Exp::Record(..), _) => "the value of a record field",
            (Exp::Field(..), _) => "the subject of a projection",
            (Exp::Inj(..), _) => "the payload of an injection",
            (Exp::Match(..), 0) => "the subject of a match",
            (Exp::Match(..), _) => "the body of a match arm",
            (Exp::Print(..), _) => "the text a command prints",
            (Exp::CmdPure(..), _) => "the value a command yields",
            (Exp::CmdBind(..), 0) => "the command a bind performs first",
            (Exp::CmdBind(..), _) => "what a bind does with the result",
            _ => "an unreachable position",
        };
        words.push(word);
        cursor = match child(cursor, *step) {
            Some(c) => c,
            None => break,
        };
    }
    match words.last() {
        None => "the whole program".to_string(),
        Some(word) => format!("{word} ({})", describe(path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::exp::{Id, Op};

    #[test]
    fn prefixes_and_nesting() {
        assert!(is_prefix(&[], &[0, 1]));
        assert!(is_prefix(&[0], &[0, 1]));
        assert!(!is_prefix(&[1], &[0, 1]));
        assert!(nested(&[0], &[0, 1, 2]));
        assert!(nested(&[0, 1, 2], &[0]));
        assert!(!nested(&[0], &[1]));
    }

    #[test]
    fn child_indices_match_the_zipper() {
        let e = Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2));
        assert_eq!(child(&e, 0), Some(&Exp::num(1)));
        assert_eq!(child(&e, 1), Some(&Exp::num(2)));
        assert_eq!(child(&e, 2), None);
        assert_eq!(arity(&e), 2);
    }

    #[test]
    fn replace_at_rebuilds_the_spine() {
        let x = Id::from_u128(1);
        let e = Exp::let_(
            x,
            Exp::num(1),
            Exp::bin_op(Op::Add, Exp::var(x), Exp::num(2)),
        );
        let out = replace_at(&e, &[1, 1], Exp::num(9)).unwrap();
        assert_eq!(
            out,
            Exp::let_(
                x,
                Exp::num(1),
                Exp::bin_op(Op::Add, Exp::var(x), Exp::num(9))
            )
        );
        assert_eq!(at(&out, &[1, 1]), Some(&Exp::num(9)));
    }

    #[test]
    fn a_zipper_walk_agrees_with_the_path_walk() {
        use nothing_action::zipper::Zipper;
        let x = Id::from_u128(1);
        let e = Exp::let_(
            x,
            Exp::num(1),
            Exp::bin_op(Op::Add, Exp::var(x), Exp::num(2)),
        );
        for path in [vec![0], vec![1], vec![1, 0], vec![1, 1]] {
            let mut z = Zipper::new(e.clone());
            for step in &path {
                z = z.move_child(*step).unwrap();
            }
            assert_eq!(Some(&z.focus), at(&e, &path), "path {path:?}");
        }
    }
}
