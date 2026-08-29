use nothing_core::exp::Side;
use nothing_core::names::NameTable;
use nothing_core::render::{
    CONS_STR, FIELD_STR, FOLD_STR, PREC_APP, PREC_ATOM, PREC_BINDER, PREC_CMP, PREC_CONS, Prec,
    op_prec, op_str, render_id, render_prec, render_ty,
};

use crate::zipper::{Frame, Zipper};

pub const CURSOR_OPEN: &str = "»";
pub const CURSOR_CLOSE: &str = "«";

fn min_prec_for(frame: &Frame) -> Prec {
    match frame {
        Frame::LamBody(..) => PREC_BINDER,
        Frame::ApFun(..) => PREC_APP,
        Frame::ApArg(..) => PREC_ATOM,
        Frame::BinOpLeft(op, _) => op_prec(*op),
        Frame::BinOpRight(op, _) => op_prec(*op) + 1,
        Frame::IfCond(..) => PREC_CMP,
        Frame::IfThen(..) => PREC_CMP,
        Frame::IfElse(..) => PREC_BINDER,
        Frame::LetBound(..) => PREC_CMP,
        Frame::LetBody(..) => PREC_BINDER,
        Frame::PairFst(..) => PREC_BINDER,
        Frame::PairSnd(..) => PREC_BINDER,
        Frame::ProjBody(..) => PREC_ATOM,
        Frame::ConsHead(..) => PREC_CONS + 1,
        Frame::ConsTail(..) => PREC_CONS,
        Frame::FoldList(..) | Frame::FoldInit(..) | Frame::FoldStep(..) => PREC_ATOM,
        Frame::RecordField(..) => PREC_BINDER,
        Frame::FieldSubject(..) => PREC_ATOM,
        Frame::NonEmptyHoleBody(..) => PREC_BINDER,
    }
}

fn own_prec(frame: &Frame) -> Prec {
    match frame {
        Frame::LamBody(..)
        | Frame::IfCond(..)
        | Frame::IfThen(..)
        | Frame::IfElse(..)
        | Frame::LetBound(..)
        | Frame::LetBody(..) => PREC_BINDER,
        Frame::ApFun(..)
        | Frame::ApArg(..)
        | Frame::ProjBody(..)
        | Frame::FoldList(..)
        | Frame::FoldInit(..)
        | Frame::FoldStep(..) => PREC_APP,
        Frame::BinOpLeft(op, _) | Frame::BinOpRight(op, _) => op_prec(*op),
        Frame::ConsHead(..) | Frame::ConsTail(..) => PREC_CONS,
        Frame::PairFst(..)
        | Frame::PairSnd(..)
        | Frame::RecordField(..)
        | Frame::FieldSubject(..)
        | Frame::NonEmptyHoleBody(..) => PREC_ATOM,
    }
}

fn assemble(frame: &Frame, child: &str, names: &NameTable) -> String {
    match frame {
        Frame::LamBody(id, ty) => format!(
            "λ{}:{}. {child}",
            render_id(*id, names),
            render_ty(ty, names)
        ),
        Frame::ApFun(arg) => format!("{child} {}", render_prec(arg, PREC_ATOM, names)),
        Frame::ApArg(fun) => format!("{} {child}", render_prec(fun, PREC_APP, names)),
        Frame::BinOpLeft(op, rhs) => {
            format!(
                "{child} {} {}",
                op_str(*op),
                render_prec(rhs, op_prec(*op) + 1, names)
            )
        }
        Frame::BinOpRight(op, lhs) => {
            format!(
                "{} {} {child}",
                render_prec(lhs, op_prec(*op), names),
                op_str(*op)
            )
        }
        Frame::IfCond(then_, else_) => format!(
            "if {child} then {} else {}",
            render_prec(then_, PREC_CMP, names),
            render_prec(else_, PREC_BINDER, names)
        ),
        Frame::IfThen(cond, else_) => format!(
            "if {} then {child} else {}",
            render_prec(cond, PREC_CMP, names),
            render_prec(else_, PREC_BINDER, names)
        ),
        Frame::IfElse(cond, then_) => format!(
            "if {} then {} else {child}",
            render_prec(cond, PREC_CMP, names),
            render_prec(then_, PREC_CMP, names)
        ),
        Frame::LetBound(id, body) => format!(
            "let {} = {child} in {}",
            render_id(*id, names),
            render_prec(body, PREC_BINDER, names)
        ),
        Frame::LetBody(id, bound) => format!(
            "let {} = {} in {child}",
            render_id(*id, names),
            render_prec(bound, PREC_CMP, names)
        ),
        Frame::PairFst(snd) => format!("({child}, {})", render_prec(snd, PREC_BINDER, names)),
        Frame::PairSnd(fst) => format!("({}, {child})", render_prec(fst, PREC_BINDER, names)),
        Frame::ProjBody(side) => {
            let prefix = match side {
                Side::L => "fst ",
                Side::R => "snd ",
            };
            format!("{prefix}{child}")
        }
        Frame::ConsHead(tail) => {
            format!("{child} {CONS_STR} {}", render_prec(tail, PREC_CONS, names))
        }
        Frame::ConsTail(head) => format!(
            "{} {CONS_STR} {child}",
            render_prec(head, PREC_CONS + 1, names)
        ),
        Frame::FoldList(init, step) => format!(
            "{FOLD_STR} {child} {} {}",
            render_prec(init, PREC_ATOM, names),
            render_prec(step, PREC_ATOM, names)
        ),
        Frame::FoldInit(list, step) => format!(
            "{FOLD_STR} {} {child} {}",
            render_prec(list, PREC_ATOM, names),
            render_prec(step, PREC_ATOM, names)
        ),
        Frame::FoldStep(list, init) => format!(
            "{FOLD_STR} {} {} {child}",
            render_prec(list, PREC_ATOM, names),
            render_prec(init, PREC_ATOM, names)
        ),
        Frame::RecordField(others, index, id) => {
            let mut fields: Vec<String> = Vec::with_capacity(others.len() + 1);
            for (i, (other, e)) in others.iter().enumerate() {
                if i == *index {
                    fields.push(format!("{} = {child}", render_id(*id, names)));
                }
                fields.push(format!(
                    "{} = {}",
                    render_id(*other, names),
                    render_prec(e, PREC_BINDER, names)
                ));
            }
            if *index >= others.len() {
                fields.push(format!("{} = {child}", render_id(*id, names)));
            }
            format!("{{{}}}", fields.join(", "))
        }
        Frame::FieldSubject(id) => format!("{child}{FIELD_STR}{}", render_id(*id, names)),
        Frame::NonEmptyHoleBody(_) => format!("⦇{child}⦈"),
    }
}

pub fn render_with_cursor(z: &Zipper, names: &NameTable) -> String {
    let path = &z.path;

    let focus_min_prec = path.last().map(min_prec_for).unwrap_or(PREC_BINDER);
    let mut content = format!(
        "{CURSOR_OPEN}{}{CURSOR_CLOSE}",
        render_prec(&z.focus, focus_min_prec, names)
    );

    for i in (0..path.len()).rev() {
        let frame = &path[i];
        let assembled = assemble(frame, &content, names);
        let needed = if i == 0 {
            PREC_BINDER
        } else {
            min_prec_for(&path[i - 1])
        };
        content = if own_prec(frame) < needed {
            format!("({assembled})")
        } else {
            assembled
        };
    }

    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zipper;
    use nothing_core::examples;
    use nothing_core::render;

    fn names() -> NameTable {
        examples::names()
    }

    fn all_examples() -> Vec<nothing_core::exp::Exp> {
        vec![
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
        ]
    }

    #[test]
    fn root_position_matches_the_plain_projection_once_markers_are_stripped() {
        let e = examples::square_and_compare();
        let z = zipper::unzip(e.clone());
        let marked = render_with_cursor(&z, &names());
        assert_eq!(
            marked,
            format!(
                "{CURSOR_OPEN}{}{CURSOR_CLOSE}",
                render::render(&e, &names())
            )
        );
    }

    #[test]
    fn a_leaf_deep_inside_the_program_is_delimited_in_place() {
        let e = examples::pair_and_project();
        let z = zipper::unzip(e)
            .move_child(0)
            .unwrap()
            .move_child(0)
            .unwrap();
        assert_eq!(
            render_with_cursor(&z, &names()),
            format!("let x0 = ({CURSOR_OPEN}1{CURSOR_CLOSE}, true) in fst x0")
        );
    }

    #[test]
    fn cursor_moves_produce_visibly_distinct_output_at_every_position() {
        let e = examples::square_and_compare();
        let positions = zipper::all_positions(&e);

        assert!(
            positions.len() >= 10,
            "expected a nontrivial example, got {} positions",
            positions.len()
        );

        let rendered: Vec<String> = positions
            .iter()
            .map(|z| render_with_cursor(z, &names()))
            .collect();

        for (z, s) in positions.iter().zip(&rendered) {
            assert_eq!(
                s.matches(CURSOR_OPEN).count(),
                1,
                "expected exactly one opening marker at depth {} (child_index {:?}), got: {s}",
                z.depth(),
                z.child_index()
            );
            assert_eq!(
                s.matches(CURSOR_CLOSE).count(),
                1,
                "expected exactly one closing marker at depth {} (child_index {:?}), got: {s}",
                z.depth(),
                z.child_index()
            );
        }

        for i in 0..rendered.len() {
            for j in (i + 1)..rendered.len() {
                assert_ne!(
                    rendered[i],
                    rendered[j],
                    "positions {i} and {j} (depths {}, {}) rendered identically: {}",
                    positions[i].depth(),
                    positions[j].depth(),
                    rendered[i]
                );
            }
        }
    }

    #[test]
    fn stripping_markers_reproduces_the_plain_projection() {
        for e in all_examples() {
            let expected = render::render(&e, &names());
            for z in zipper::all_positions(&e) {
                let marked = render_with_cursor(&z, &names());
                let stripped = marked.replace(CURSOR_OPEN, "").replace(CURSOR_CLOSE, "");
                assert_eq!(
                    stripped,
                    expected,
                    "mismatch at depth {} (child_index {:?}) of {expected:?}",
                    z.depth(),
                    z.child_index()
                );
            }
        }
    }

    #[test]
    fn stripping_markers_reproduces_the_plain_projection_on_generated_programs() {
        use crate::generate;
        for seed in 0..200u64 {
            let e = generate::well_typed_exp(seed);
            let expected = render::render(&e, &names());
            for z in zipper::all_positions(&e) {
                let marked = render_with_cursor(&z, &names());
                let stripped = marked.replace(CURSOR_OPEN, "").replace(CURSOR_CLOSE, "");
                assert_eq!(
                    stripped,
                    expected,
                    "mismatch for seed {seed} at depth {}",
                    z.depth()
                );
            }
        }
    }
}
