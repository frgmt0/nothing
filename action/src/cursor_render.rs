//! Cursor-aware rendering (Phase 3).
//!
//! The plain-text projection lives in `core::render`; this module adds the
//! cursor to it, delimiting the focused subexpression distinctly so that
//! moving the cursor through a program produces visibly different output at
//! every position.
//!
//! Cursor markers: the focus is wrapped in [`CURSOR_OPEN`] / [`CURSOR_CLOSE`]
//! (`»focus«`) — chosen because neither glyph appears anywhere else in the
//! plain-text vocabulary (parens, `λ`, `⦇⦈`, operators, digits, `x`-names),
//! so a marker can never be confused with program syntax, and the
//! open/close pair reads left-to-right the same way parens do.
//!
//! Design: rather than re-deriving parenthesisation independently, this
//! module reuses `core::render`'s own precedence table and assembly rules —
//! `core::render` gained a small `pub` surface for exactly this
//! (`Prec`/`PREC_*`, `op_prec`, `op_str`, `render_id`, and a new
//! `render_prec` wrapper around its private `fmt_prec`; see that module's
//! doc comments). Every subtree *not* on the path to the cursor is rendered
//! wholesale via `render_prec`, so its formatting is provably identical to
//! the plain projection's. Only the spine of frames from the root down to
//! the cursor is walked by hand here, to splice the marked focus into its
//! ancestors' assembled text — and that walk's per-frame precedence
//! decisions (`min_prec_for`, `own_prec`) and syntax templates (`assemble`)
//! are a direct transcription of `core::render::fmt_prec`'s match arms,
//! keyed on [`Frame`] instead of [`Exp`] (each `Frame` variant already
//! pins down exactly one child position of exactly one `Exp` form, which is
//! what makes the transcription mechanical). The test
//! `stripping_markers_reproduces_the_plain_projection` pins this fidelity
//! down directly: removing the markers from this module's output at *every*
//! cursor position of *all ten* `core::examples` programs must reproduce
//! `core::render::render` byte for byte.

use nothing_core::exp::Side;
use nothing_core::render::{PREC_APP, PREC_ATOM, PREC_BINDER, PREC_CMP, Prec, op_prec, op_str, render_id, render_prec};

use crate::zipper::{Frame, Zipper};

/// Marks the start of the focused subexpression.
pub const CURSOR_OPEN: &str = "»";
/// Marks the end of the focused subexpression.
pub const CURSOR_CLOSE: &str = "«";

/// The minimum precedence at which the child sitting in `frame`'s own
/// position (`frame.child_index()`) must render — the same number
/// `core::render::fmt_prec` passes to its recursive call for that exact
/// child slot. Since a `Frame` variant is specific to one (parent form,
/// child index) pair, this one match covers every slot in the grammar.
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
        Frame::NonEmptyHoleBody(..) => PREC_BINDER,
    }
}

/// The rendering precedence of the node `frame` itself builds (i.e. of
/// `frame.clone().rebuild(anything)`) — needed to decide whether that
/// assembled node requires parentheses once it becomes a child one level
/// further out. Mirrors `core::render::prec_of`'s grouping by form family.
fn own_prec(frame: &Frame) -> Prec {
    match frame {
        Frame::LamBody(..)
        | Frame::IfCond(..)
        | Frame::IfThen(..)
        | Frame::IfElse(..)
        | Frame::LetBound(..)
        | Frame::LetBody(..) => PREC_BINDER,
        Frame::ApFun(..) | Frame::ApArg(..) | Frame::ProjBody(..) => PREC_APP,
        Frame::BinOpLeft(op, _) | Frame::BinOpRight(op, _) => op_prec(*op),
        Frame::PairFst(..) | Frame::PairSnd(..) | Frame::NonEmptyHoleBody(..) => PREC_ATOM,
    }
}

/// Assemble the full text of the node `frame` builds, given `child` already
/// rendered — and, if it is the marked focus itself, already
/// parenthesised — for `frame`'s own child slot. The sibling(s) stored in
/// `frame` are rendered here via `render_prec` at the same min-precedence
/// `min_prec_for` would report for *their* slot (cross-checked in the
/// doc-comment table above and pinned by the fidelity test).
fn assemble(frame: &Frame, child: &str) -> String {
    match frame {
        Frame::LamBody(id, ty) => format!("λ{}:{}. {child}", render_id(*id), ty),
        Frame::ApFun(arg) => format!("{child} {}", render_prec(arg, PREC_ATOM)),
        Frame::ApArg(fun) => format!("{} {child}", render_prec(fun, PREC_APP)),
        Frame::BinOpLeft(op, rhs) => {
            format!("{child} {} {}", op_str(*op), render_prec(rhs, op_prec(*op) + 1))
        }
        Frame::BinOpRight(op, lhs) => {
            format!("{} {} {child}", render_prec(lhs, op_prec(*op)), op_str(*op))
        }
        Frame::IfCond(then_, else_) => format!(
            "if {child} then {} else {}",
            render_prec(then_, PREC_CMP),
            render_prec(else_, PREC_BINDER)
        ),
        Frame::IfThen(cond, else_) => format!(
            "if {} then {child} else {}",
            render_prec(cond, PREC_CMP),
            render_prec(else_, PREC_BINDER)
        ),
        Frame::IfElse(cond, then_) => format!(
            "if {} then {} else {child}",
            render_prec(cond, PREC_CMP),
            render_prec(then_, PREC_CMP)
        ),
        Frame::LetBound(id, body) => format!(
            "let {} = {child} in {}",
            render_id(*id),
            render_prec(body, PREC_BINDER)
        ),
        Frame::LetBody(id, bound) => format!(
            "let {} = {} in {child}",
            render_id(*id),
            render_prec(bound, PREC_CMP)
        ),
        Frame::PairFst(snd) => format!("({child}, {})", render_prec(snd, PREC_BINDER)),
        Frame::PairSnd(fst) => format!("({}, {child})", render_prec(fst, PREC_BINDER)),
        Frame::ProjBody(side) => {
            let prefix = match side {
                Side::L => "fst ",
                Side::R => "snd ",
            };
            format!("{prefix}{child}")
        }
        Frame::NonEmptyHoleBody(_) => format!("⦇{child}⦈"),
    }
}

/// Render the whole program `z` belongs to, with the focused subexpression
/// delimited by [`CURSOR_OPEN`] / [`CURSOR_CLOSE`].
///
/// The focus is rendered at the minimum precedence its position demands
/// (root: [`PREC_BINDER`], same as [`nothing_core::render::render`]'s own
/// top-level call), then marked; each ancestor frame from the immediate
/// parent up to the root is assembled in turn via [`assemble`], gaining its
/// own parentheses exactly when [`own_prec`] falls below what the next
/// frame out requires — the same rule [`nothing_core::render::render_prec`]
/// applies internally.
pub fn render_with_cursor(z: &Zipper) -> String {
    let path = &z.path;

    let focus_min_prec = path.last().map(min_prec_for).unwrap_or(PREC_BINDER);
    let mut content = format!(
        "{CURSOR_OPEN}{}{CURSOR_CLOSE}",
        render_prec(&z.focus, focus_min_prec)
    );

    for i in (0..path.len()).rev() {
        let frame = &path[i];
        let assembled = assemble(frame, &content);
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
        let marked = render_with_cursor(&z);
        assert_eq!(marked, format!("{CURSOR_OPEN}{}{CURSOR_CLOSE}", render::render(&e)));
    }

    #[test]
    fn a_leaf_deep_inside_the_program_is_delimited_in_place() {
        // let x0 = (1, true) in fst x0 -- put the cursor on the `1`.
        let e = examples::pair_and_project();
        let z = zipper::unzip(e)
            .move_child(0) // the pair
            .unwrap()
            .move_child(0) // the 1
            .unwrap();
        assert_eq!(
            render_with_cursor(&z),
            format!("let x0 = ({CURSOR_OPEN}1{CURSOR_CLOSE}, true) in fst x0")
        );
    }

    /// The literal Done-when criterion: walking every cursor position of a
    /// nontrivial example produces visibly different, uniquely-delimited
    /// output at each one.
    #[test]
    fn cursor_moves_produce_visibly_distinct_output_at_every_position() {
        let e = examples::square_and_compare();
        let positions = zipper::all_positions(&e);
        // "Nontrivial": several distinct frame kinds nested (let, lambda,
        // binop, application, comparison), enough positions to make
        // pairwise distinctness a real assertion rather than a vacuous one.
        assert!(
            positions.len() >= 10,
            "expected a nontrivial example, got {} positions",
            positions.len()
        );

        let rendered: Vec<String> = positions.iter().map(render_with_cursor).collect();

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
                    rendered[i], rendered[j],
                    "positions {i} and {j} (depths {}, {}) rendered identically: {}",
                    positions[i].depth(),
                    positions[j].depth(),
                    rendered[i]
                );
            }
        }
    }

    /// Stripping the markers must always reproduce the plain projection
    /// exactly, at every position of every one of the ten example programs
    /// — the fidelity guarantee this module exists to provide.
    #[test]
    fn stripping_markers_reproduces_the_plain_projection() {
        for e in all_examples() {
            let expected = render::render(&e);
            for z in zipper::all_positions(&e) {
                let marked = render_with_cursor(&z);
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

    /// Also true, and load-bearing for the criterion above at scale: over
    /// arbitrary generated well-typed programs, not just the ten fixed
    /// examples.
    #[test]
    fn stripping_markers_reproduces_the_plain_projection_on_generated_programs() {
        use crate::generate;
        for seed in 0..200u64 {
            let e = generate::well_typed_exp(seed);
            let expected = render::render(&e);
            for z in zipper::all_positions(&e) {
                let marked = render_with_cursor(&z);
                let stripped = marked.replace(CURSOR_OPEN, "").replace(CURSOR_CLOSE, "");
                assert_eq!(stripped, expected, "mismatch for seed {seed} at depth {}", z.depth());
            }
        }
    }
}
