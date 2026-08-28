use std::fmt;

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

            fmt_prec(a, 1, f)?;
            write!(f, " -> ")?;

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

pub fn is_consistent(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Hole, _) | (_, Ty::Hole) => true,
        (Ty::Num, Ty::Num) | (Ty::Bool, Ty::Bool) => true,
        (Ty::Arrow(a1, a2), Ty::Arrow(b1, b2)) => is_consistent(a1, b1) && is_consistent(a2, b2),
        (Ty::Prod(a1, a2), Ty::Prod(b1, b2)) => is_consistent(a1, b1) && is_consistent(a2, b2),
        _ => false,
    }
}

pub fn matched_arrow(ty: &Ty) -> Option<(Ty, Ty)> {
    match ty {
        Ty::Hole => Some((Ty::Hole, Ty::Hole)),
        Ty::Arrow(a, b) => Some((a.as_ref().clone(), b.as_ref().clone())),
        _ => None,
    }
}

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
        let ty = arrow(Ty::Num, arrow(Ty::Bool, Ty::Num));
        assert_eq!(ty.to_string(), "Num -> Bool -> Num");
    }

    #[test]
    fn display_arrow_left_nested_needs_parens() {
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
        let ty = arrow(prod(Ty::Num, Ty::Bool), Ty::Num);
        assert_eq!(ty.to_string(), "Num * Bool -> Num");
    }

    #[test]
    fn display_nested_hole() {
        let ty = arrow(Ty::Hole, Ty::Num);
        assert_eq!(ty.to_string(), "? -> Num");
    }

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
        assert!(is_consistent(
            &arrow(Ty::Hole, Ty::Bool),
            &arrow(Ty::Num, Ty::Bool)
        ));

        assert!(!is_consistent(
            &arrow(Ty::Num, Ty::Bool),
            &arrow(Ty::Bool, Ty::Bool)
        ));

        assert!(is_consistent(
            &prod(Ty::Hole, Ty::Bool),
            &prod(Ty::Num, Ty::Bool)
        ));

        assert!(!is_consistent(
            &prod(Ty::Num, Ty::Bool),
            &prod(Ty::Bool, Ty::Bool)
        ));
    }

    #[test]
    fn consistency_symmetric_on_compound_types() {
        let pairs = [
            (Ty::Num, Ty::Hole),
            (Ty::Hole, Ty::Bool),
            (arrow(Ty::Num, Ty::Bool), Ty::Hole),
            (arrow(Ty::Num, Ty::Bool), arrow(Ty::Hole, Ty::Bool)),
            (prod(Ty::Num, Ty::Bool), prod(Ty::Num, Ty::Hole)),
            (Ty::Num, Ty::Bool),
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
        assert!(is_consistent(&Ty::Num, &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &Ty::Bool));
        assert!(!is_consistent(&Ty::Num, &Ty::Bool));
    }

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
