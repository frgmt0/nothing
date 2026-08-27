//! Type grammar (Phase 1). Stubbed in Phase 0 so later agents do not need to
//! touch `lib.rs` concurrently.

use std::fmt;

/// The type grammar of `nothing`.
///
/// `Hole` is the unknown type — a gap in the type structure that has not
/// (yet) been determined. It participates in gradual typing's consistency
/// relation rather than ordinary equality.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ty {
    Num,
    Bool,
    Arrow(Box<Ty>, Box<Ty>),
    Prod(Box<Ty>, Box<Ty>),
    Hole,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_prec(self, 0, f)
    }
}

/// Render `ty` at the given minimum precedence context, parenthesising only
/// when necessary.
///
/// Precedence levels (higher binds tighter):
/// - 0: top level / right side of `->` (arrow is right-associative)
/// - 1: left side of `->`, or either side of `*`
/// - 2: atomic (Num, Bool, Hole, or anything parenthesised)
fn fmt_prec(ty: &Ty, min_prec: u8, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match ty {
        Ty::Num => write!(f, "Num"),
        Ty::Bool => write!(f, "Bool"),
        Ty::Hole => write!(f, "?"),
        Ty::Arrow(a, b) => {
            let needs_parens = min_prec > 0;
            if needs_parens {
                write!(f, "(")?;
            }
            // Left side of an arrow binds tighter than the arrow itself, so
            // a nested arrow there needs parens: (a -> b) -> c.
            fmt_prec(a, 1, f)?;
            write!(f, " -> ")?;
            // Arrow is right-associative, so a nested arrow on the right
            // does not need parens: a -> (b -> c) prints as a -> b -> c.
            fmt_prec(b, 0, f)?;
            if needs_parens {
                write!(f, ")")?;
            }
            Ok(())
        }
        Ty::Prod(a, b) => {
            let needs_parens = min_prec > 1;
            if needs_parens {
                write!(f, "(")?;
            }
            fmt_prec(a, 2, f)?;
            write!(f, " * ")?;
            fmt_prec(b, 2, f)?;
            if needs_parens {
                write!(f, ")")?;
            }
            Ok(())
        }
    }
}

/// Gradual typing's consistency relation, written `~`.
///
/// Two types are consistent if they are equal, or either is `Hole`, or they
/// share a constructor and their components are pairwise consistent.
///
/// This relation is reflexive and symmetric but **not transitive**:
/// `Num ~ ?` and `? ~ Bool` both hold, but `Num !~ Bool`.
pub fn is_consistent(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Hole, _) | (_, Ty::Hole) => true,
        (Ty::Num, Ty::Num) | (Ty::Bool, Ty::Bool) => true,
        (Ty::Arrow(a1, a2), Ty::Arrow(b1, b2)) => is_consistent(a1, b1) && is_consistent(a2, b2),
        (Ty::Prod(a1, a2), Ty::Prod(b1, b2)) => is_consistent(a1, b1) && is_consistent(a2, b2),
        _ => false,
    }
}

/// The matched-arrow judgment: extracts (or manufactures) an arrow shape
/// from a type, so that application can proceed even when the type is not
/// yet known.
///
/// - `Hole` matches as `(Hole, Hole)`.
/// - `Arrow(a, b)` matches as `(a, b)`.
/// - Anything else fails.
pub fn matched_arrow(ty: &Ty) -> Option<(Ty, Ty)> {
    match ty {
        Ty::Hole => Some((Ty::Hole, Ty::Hole)),
        Ty::Arrow(a, b) => Some((a.as_ref().clone(), b.as_ref().clone())),
        _ => None,
    }
}

/// The matched-product judgment: extracts (or manufactures) a product shape
/// from a type, so that projection can proceed even when the type is not
/// yet known.
///
/// - `Hole` matches as `(Hole, Hole)`.
/// - `Prod(a, b)` matches as `(a, b)`.
/// - Anything else fails.
pub fn matched_prod(ty: &Ty) -> Option<(Ty, Ty)> {
    match ty {
        Ty::Hole => Some((Ty::Hole, Ty::Hole)),
        Ty::Prod(a, b) => Some((a.as_ref().clone(), b.as_ref().clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrow(a: Ty, b: Ty) -> Ty {
        Ty::Arrow(Box::new(a), Box::new(b))
    }

    fn prod(a: Ty, b: Ty) -> Ty {
        Ty::Prod(Box::new(a), Box::new(b))
    }

    // --- Display ---

    #[test]
    fn display_atoms() {
        assert_eq!(Ty::Num.to_string(), "Num");
        assert_eq!(Ty::Bool.to_string(), "Bool");
        assert_eq!(Ty::Hole.to_string(), "?");
    }

    #[test]
    fn display_arrow_simple() {
        assert_eq!(arrow(Ty::Num, Ty::Bool).to_string(), "Num -> Bool");
    }

    #[test]
    fn display_arrow_right_associative_no_parens() {
        // Num -> (Bool -> Num) should print without parens on the right.
        let ty = arrow(Ty::Num, arrow(Ty::Bool, Ty::Num));
        assert_eq!(ty.to_string(), "Num -> Bool -> Num");
    }

    #[test]
    fn display_arrow_left_nested_needs_parens() {
        // (Num -> Bool) -> Num must parenthesise the left side.
        let ty = arrow(arrow(Ty::Num, Ty::Bool), Ty::Num);
        assert_eq!(ty.to_string(), "(Num -> Bool) -> Num");
    }

    #[test]
    fn display_prod_simple() {
        assert_eq!(prod(Ty::Num, Ty::Bool).to_string(), "Num * Bool");
    }

    #[test]
    fn display_prod_of_arrows_needs_parens() {
        let ty = prod(arrow(Ty::Num, Ty::Bool), Ty::Num);
        assert_eq!(ty.to_string(), "(Num -> Bool) * Num");
    }

    #[test]
    fn display_arrow_of_prod_needs_parens_on_left_only() {
        // A product on the left of an arrow does not need parens (it binds
        // tighter than arrow already), but let's confirm exact rendering.
        let ty = arrow(prod(Ty::Num, Ty::Bool), Ty::Num);
        assert_eq!(ty.to_string(), "Num * Bool -> Num");
    }

    #[test]
    fn display_nested_hole() {
        let ty = arrow(Ty::Hole, Ty::Num);
        assert_eq!(ty.to_string(), "? -> Num");
    }

    // --- is_consistent ---

    #[test]
    fn consistency_reflexive_atoms() {
        assert!(is_consistent(&Ty::Num, &Ty::Num));
        assert!(is_consistent(&Ty::Bool, &Ty::Bool));
        assert!(is_consistent(&Ty::Hole, &Ty::Hole));
    }

    #[test]
    fn consistency_reflexive_compound() {
        let a = arrow(Ty::Num, prod(Ty::Bool, Ty::Hole));
        assert!(is_consistent(&a, &a));
    }

    #[test]
    fn consistency_hole_with_anything() {
        assert!(is_consistent(&Ty::Hole, &Ty::Num));
        assert!(is_consistent(&Ty::Bool, &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &arrow(Ty::Num, Ty::Bool)));
        assert!(is_consistent(&prod(Ty::Num, Ty::Bool), &Ty::Hole));
    }

    #[test]
    fn consistency_unequal_atoms_fail() {
        assert!(!is_consistent(&Ty::Num, &Ty::Bool));
    }

    #[test]
    fn consistency_different_constructors_fail() {
        assert!(!is_consistent(&Ty::Num, &arrow(Ty::Num, Ty::Num)));
        assert!(!is_consistent(&prod(Ty::Num, Ty::Num), &Ty::Bool));
        assert!(!is_consistent(
            &arrow(Ty::Num, Ty::Num),
            &prod(Ty::Num, Ty::Num)
        ));
    }

    #[test]
    fn consistency_compound_components_must_be_consistent() {
        // Arrow: consistent components -> consistent whole (via Hole).
        assert!(is_consistent(
            &arrow(Ty::Hole, Ty::Bool),
            &arrow(Ty::Num, Ty::Bool)
        ));
        // Arrow: inconsistent component -> inconsistent whole.
        assert!(!is_consistent(
            &arrow(Ty::Num, Ty::Bool),
            &arrow(Ty::Bool, Ty::Bool)
        ));
        // Prod: consistent components -> consistent whole.
        assert!(is_consistent(&prod(Ty::Hole, Ty::Bool), &prod(Ty::Num, Ty::Bool)));
        // Prod: inconsistent component -> inconsistent whole.
        assert!(!is_consistent(&prod(Ty::Num, Ty::Bool), &prod(Ty::Bool, Ty::Bool)));
    }

    #[test]
    fn consistency_symmetric_on_compound_types() {
        let pairs = [
            (Ty::Num, Ty::Hole),
            (Ty::Hole, Ty::Bool),
            (arrow(Ty::Num, Ty::Bool), Ty::Hole),
            (arrow(Ty::Num, Ty::Bool), arrow(Ty::Hole, Ty::Bool)),
            (prod(Ty::Num, Ty::Bool), prod(Ty::Num, Ty::Hole)),
            (Ty::Num, Ty::Bool), // also check symmetry of a *failing* case
        ];
        for (a, b) in pairs {
            assert_eq!(
                is_consistent(&a, &b),
                is_consistent(&b, &a),
                "consistency not symmetric for {a:?} and {b:?}"
            );
        }
    }

    #[test]
    fn consistency_is_not_transitive() {
        // The load-bearing test: Num ~ ?, ? ~ Bool, but Num !~ Bool.
        assert!(is_consistent(&Ty::Num, &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &Ty::Bool));
        assert!(!is_consistent(&Ty::Num, &Ty::Bool));
    }

    // --- matched_arrow ---

    #[test]
    fn matched_arrow_hole() {
        assert_eq!(matched_arrow(&Ty::Hole), Some((Ty::Hole, Ty::Hole)));
    }

    #[test]
    fn matched_arrow_concrete() {
        let ty = arrow(Ty::Num, Ty::Bool);
        assert_eq!(matched_arrow(&ty), Some((Ty::Num, Ty::Bool)));
    }

    #[test]
    fn matched_arrow_failure() {
        assert_eq!(matched_arrow(&Ty::Num), None);
        assert_eq!(matched_arrow(&Ty::Bool), None);
        assert_eq!(matched_arrow(&prod(Ty::Num, Ty::Bool)), None);
    }

    // --- matched_prod ---

    #[test]
    fn matched_prod_hole() {
        assert_eq!(matched_prod(&Ty::Hole), Some((Ty::Hole, Ty::Hole)));
    }

    #[test]
    fn matched_prod_concrete() {
        let ty = prod(Ty::Num, Ty::Bool);
        assert_eq!(matched_prod(&ty), Some((Ty::Num, Ty::Bool)));
    }

    #[test]
    fn matched_prod_failure() {
        assert_eq!(matched_prod(&Ty::Num), None);
        assert_eq!(matched_prod(&Ty::Bool), None);
        assert_eq!(matched_prod(&arrow(Ty::Num, Ty::Bool)), None);
    }
}
