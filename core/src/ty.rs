use std::fmt;

use crate::exp::Id;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ty {
    Num,
    Bool,
    Str,
    Arrow(Box<Ty>, Box<Ty>),
    Prod(Box<Ty>, Box<Ty>),
    List(Box<Ty>),
    Record(Vec<(Id, Ty)>),
    Variant(Vec<(Id, Ty)>),
    Cmd(Box<Ty>),
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
        Ty::Str => write!(f, "Str"),
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
        Ty::List(elem) => {
            let needs_parens = min_prec > 2;
            if needs_parens {
                write!(f, "(")?;
            }
            write!(f, "List ")?;
            fmt_prec(elem, 3, f)?;
            if needs_parens {
                write!(f, ")")?;
            }
            Ok(())
        }
        Ty::Cmd(result) => {
            let needs_parens = min_prec > 2;
            if needs_parens {
                write!(f, "(")?;
            }
            write!(f, "Cmd ")?;
            fmt_prec(result, 3, f)?;
            if needs_parens {
                write!(f, ")")?;
            }
            Ok(())
        }
        Ty::Record(fields) => {
            write!(f, "{{")?;
            for (i, (id, ty)) in fields.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "#{}: ", id.short())?;
                fmt_prec(ty, 0, f)?;
            }
            write!(f, "}}")
        }
        Ty::Variant(ctors) => {
            write!(f, "[")?;
            for (i, (id, ty)) in ctors.iter().enumerate() {
                if i > 0 {
                    write!(f, " | ")?;
                }
                write!(f, "#{}: ", id.short())?;
                fmt_prec(ty, 0, f)?;
            }
            write!(f, "]")
        }
    }
}

pub fn is_consistent(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Hole, _) | (_, Ty::Hole) => true,
        (Ty::Num, Ty::Num) | (Ty::Bool, Ty::Bool) | (Ty::Str, Ty::Str) => true,
        (Ty::Arrow(a1, a2), Ty::Arrow(b1, b2)) => is_consistent(a1, b1) && is_consistent(a2, b2),
        (Ty::Prod(a1, a2), Ty::Prod(b1, b2)) => is_consistent(a1, b1) && is_consistent(a2, b2),
        (Ty::List(a), Ty::List(b)) => is_consistent(a, b),
        (Ty::Cmd(a), Ty::Cmd(b)) => is_consistent(a, b),
        (Ty::Record(a), Ty::Record(b)) => {
            a.len() == b.len()
                && a.iter().all(|(id, left)| {
                    b.iter()
                        .find(|(other, _)| other == id)
                        .is_some_and(|(_, right)| is_consistent(left, right))
                })
        }
        (Ty::Variant(a), Ty::Variant(b)) => a.iter().all(|(id, left)| {
            b.iter()
                .find(|(other, _)| other == id)
                .is_none_or(|(_, right)| is_consistent(left, right))
        }),
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

pub fn matched_list(ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::Hole => Some(Ty::Hole),
        Ty::List(elem) => Some(elem.as_ref().clone()),
        _ => None,
    }
}

pub fn matched_cmd(ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::Hole => Some(Ty::Hole),
        Ty::Cmd(result) => Some(result.as_ref().clone()),
        _ => None,
    }
}

pub fn matched_record(ty: &Ty, field: Id) -> Option<Ty> {
    match ty {
        Ty::Hole => Some(Ty::Hole),
        Ty::Record(fields) => fields
            .iter()
            .find(|(id, _)| *id == field)
            .map(|(_, ty)| ty.clone()),
        _ => None,
    }
}

pub fn matched_record_fields(ty: &Ty, fields: &[Id]) -> Option<Vec<Ty>> {
    match ty {
        Ty::Hole => Some(vec![Ty::Hole; fields.len()]),
        Ty::Record(known) if known.len() == fields.len() => fields
            .iter()
            .map(|id| matched_record(ty, *id))
            .collect::<Option<Vec<Ty>>>(),
        _ => None,
    }
}

pub fn matched_variant(ty: &Ty, ctor: Id) -> Option<Ty> {
    match ty {
        Ty::Hole => Some(Ty::Hole),
        Ty::Variant(ctors) => ctors
            .iter()
            .find(|(id, _)| *id == ctor)
            .map(|(_, ty)| ty.clone()),
        _ => None,
    }
}

pub fn variant_constructors(ty: &Ty) -> Option<Vec<Id>> {
    match ty {
        Ty::Hole => Some(Vec::new()),
        Ty::Variant(ctors) => Some(ctors.iter().map(|(id, _)| *id).collect()),
        _ => None,
    }
}

pub fn record(fields: impl IntoIterator<Item = (Id, Ty)>) -> Ty {
    Ty::Record(fields.into_iter().collect())
}

pub fn variant(ctors: impl IntoIterator<Item = (Id, Ty)>) -> Ty {
    Ty::Variant(ctors.into_iter().collect())
}

pub fn unit() -> Ty {
    Ty::Record(Vec::new())
}

pub fn list(elem: Ty) -> Ty {
    Ty::List(Box::new(elem))
}

pub fn cmd(result: Ty) -> Ty {
    Ty::Cmd(Box::new(result))
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
        assert_eq!(Ty::Str.to_string(), "Str");
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
        assert!(!is_consistent(&Ty::Num, &Ty::Str));
        assert!(!is_consistent(&Ty::Bool, &Ty::Str));
    }

    #[test]
    fn consistency_str_with_itself_and_with_the_hole() {
        assert!(is_consistent(&Ty::Str, &Ty::Str));
        assert!(is_consistent(&Ty::Str, &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &Ty::Str));
        assert!(!is_consistent(&Ty::Str, &arrow(Ty::Str, Ty::Str)));
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

    #[test]
    fn display_list_binds_tighter_than_every_infix_type() {
        assert_eq!(list(Ty::Num).to_string(), "List Num");
        assert_eq!(list(Ty::Hole).to_string(), "List ?");
        assert_eq!(arrow(list(Ty::Num), Ty::Num).to_string(), "List Num -> Num");
        assert_eq!(prod(list(Ty::Num), Ty::Bool).to_string(), "List Num * Bool");
        assert_eq!(list(list(Ty::Num)).to_string(), "List (List Num)");
        assert_eq!(
            list(prod(Ty::Num, Ty::Bool)).to_string(),
            "List (Num * Bool)"
        );
        assert_eq!(
            list(arrow(Ty::Num, Ty::Num)).to_string(),
            "List (Num -> Num)"
        );
    }

    #[test]
    fn consistency_of_lists_is_consistency_of_their_elements() {
        assert!(is_consistent(&list(Ty::Num), &list(Ty::Num)));
        assert!(is_consistent(&list(Ty::Num), &list(Ty::Hole)));
        assert!(is_consistent(&list(Ty::Hole), &list(Ty::Num)));
        assert!(is_consistent(&list(Ty::Num), &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &list(Ty::Num)));

        assert!(!is_consistent(&list(Ty::Num), &list(Ty::Bool)));
        assert!(!is_consistent(&list(Ty::Num), &Ty::Num));
        assert!(!is_consistent(&list(Ty::Num), &prod(Ty::Num, Ty::Num)));
        assert!(!is_consistent(&list(list(Ty::Num)), &list(list(Ty::Bool))));
    }

    #[test]
    fn matched_list_hole_and_concrete() {
        assert_eq!(matched_list(&Ty::Hole), Some(Ty::Hole));
        assert_eq!(matched_list(&list(Ty::Num)), Some(Ty::Num));
        assert_eq!(matched_list(&list(list(Ty::Bool))), Some(list(Ty::Bool)));
    }

    fn f(n: u128) -> Id {
        Id::from_u128(0xf000 + n)
    }

    #[test]
    fn a_record_type_is_consistent_field_wise_and_ignores_field_order() {
        let a = record([(f(1), Ty::Num), (f(2), Ty::Bool)]);
        let reordered = record([(f(2), Ty::Bool), (f(1), Ty::Num)]);
        assert!(is_consistent(&a, &reordered));
        assert!(is_consistent(&reordered, &a));

        let with_hole = record([(f(1), Ty::Hole), (f(2), Ty::Bool)]);
        assert!(is_consistent(&a, &with_hole));

        let wrong_field_type = record([(f(1), Ty::Str), (f(2), Ty::Bool)]);
        assert!(!is_consistent(&a, &wrong_field_type));
    }

    #[test]
    fn record_consistency_is_exact_because_there_is_no_width_subtyping() {
        let wide = record([(f(1), Ty::Num), (f(2), Ty::Bool)]);
        let narrow = record([(f(1), Ty::Num)]);
        assert!(!is_consistent(&wide, &narrow));
        assert!(!is_consistent(&narrow, &wide));

        let renamed_field = record([(f(3), Ty::Num)]);
        assert!(
            !is_consistent(&narrow, &renamed_field),
            "a field is its id, so two records with different field ids differ"
        );

        assert!(is_consistent(&wide, &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &wide));
        assert!(!is_consistent(&wide, &Ty::Num));
        assert!(!is_consistent(&wide, &prod(Ty::Num, Ty::Bool)));
    }

    #[test]
    fn an_empty_record_is_a_type_of_its_own() {
        let unit = record([]);
        assert!(is_consistent(&unit, &unit));
        assert!(is_consistent(&unit, &Ty::Hole));
        assert!(!is_consistent(&unit, &record([(f(1), Ty::Num)])));
        assert_eq!(unit.to_string(), "{}");
    }

    #[test]
    fn matched_record_looks_a_field_up_and_fails_open_on_the_unknown_type() {
        let point = record([(f(1), Ty::Num), (f(2), Ty::Str)]);
        assert_eq!(matched_record(&point, f(1)), Some(Ty::Num));
        assert_eq!(matched_record(&point, f(2)), Some(Ty::Str));
        assert_eq!(matched_record(&point, f(9)), None);

        assert_eq!(matched_record(&Ty::Hole, f(9)), Some(Ty::Hole));
        assert_eq!(matched_record(&Ty::Num, f(1)), None);
        assert_eq!(matched_record(&list(Ty::Num), f(1)), None);
    }

    #[test]
    fn matched_record_fields_wants_the_whole_field_set() {
        let point = record([(f(1), Ty::Num), (f(2), Ty::Str)]);
        assert_eq!(
            matched_record_fields(&point, &[f(2), f(1)]),
            Some(vec![Ty::Str, Ty::Num]),
            "the answer follows the order asked for, not the order stored"
        );
        assert_eq!(matched_record_fields(&point, &[f(1)]), None);
        assert_eq!(matched_record_fields(&point, &[f(1), f(9)]), None);
        assert_eq!(
            matched_record_fields(&Ty::Hole, &[f(1), f(2)]),
            Some(vec![Ty::Hole, Ty::Hole])
        );
        assert_eq!(matched_record_fields(&Ty::Num, &[f(1)]), None);
    }

    #[test]
    fn a_record_type_displays_its_fields_by_identity_because_names_are_elsewhere() {
        let point = record([(f(1), Ty::Num)]);
        let shown = point.to_string();
        assert!(shown.starts_with("{#"), "{shown}");
        assert!(shown.ends_with(": Num}"), "{shown}");
        assert_eq!(
            record([(f(1), arrow(Ty::Num, Ty::Num))]).to_string(),
            format!("{{#{}: Num -> Num}}", f(1).short())
        );
    }

    #[test]
    fn a_variant_type_is_consistent_constructor_wise_and_ignores_their_order() {
        let a = variant([(f(1), Ty::Num), (f(2), unit())]);
        let reordered = variant([(f(2), unit()), (f(1), Ty::Num)]);
        assert!(is_consistent(&a, &reordered));
        assert!(is_consistent(&reordered, &a));

        let with_hole = variant([(f(1), Ty::Hole), (f(2), unit())]);
        assert!(is_consistent(&a, &with_hole));

        let wrong_payload = variant([(f(1), Ty::Str), (f(2), unit())]);
        assert!(!is_consistent(&a, &wrong_payload));
    }

    #[test]
    fn two_variants_are_consistent_unless_they_disagree_about_a_shared_constructor() {
        let wide = variant([(f(1), Ty::Num), (f(2), Ty::Bool)]);
        let narrow = variant([(f(1), Ty::Num)]);
        assert!(
            is_consistent(&wide, &narrow),
            "a value of one case is a value the two-case type also has"
        );
        assert!(is_consistent(&narrow, &wide));

        let disagreeing = variant([(f(1), Ty::Str)]);
        assert!(!is_consistent(&wide, &disagreeing));
        assert!(!is_consistent(&disagreeing, &wide));

        assert!(
            is_consistent(&narrow, &variant([(f(9), Ty::Num)])),
            "two variants with nothing in common contradict nothing"
        );
        assert!(is_consistent(&variant([]), &wide));

        assert!(is_consistent(&wide, &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &wide));
        assert!(
            !is_consistent(&narrow, &record([(f(1), Ty::Num)])),
            "a sum of one case is not a product of one field"
        );
        assert!(!is_consistent(&wide, &Ty::Num));
    }

    #[test]
    fn matched_variant_looks_a_constructor_up_and_fails_open_on_the_unknown_type() {
        let option = variant([(f(1), Ty::Num), (f(2), unit())]);
        assert_eq!(matched_variant(&option, f(1)), Some(Ty::Num));
        assert_eq!(matched_variant(&option, f(2)), Some(unit()));
        assert_eq!(matched_variant(&option, f(9)), None);

        assert_eq!(matched_variant(&Ty::Hole, f(9)), Some(Ty::Hole));
        assert_eq!(matched_variant(&Ty::Num, f(1)), None);
        assert_eq!(matched_variant(&record([(f(1), Ty::Num)]), f(1)), None);
    }

    #[test]
    fn the_unknown_type_requires_no_constructors_at_all() {
        assert_eq!(variant_constructors(&Ty::Hole), Some(Vec::new()));
        assert_eq!(
            variant_constructors(&variant([(f(1), Ty::Num), (f(2), Ty::Bool)])),
            Some(vec![f(1), f(2)])
        );
        assert_eq!(variant_constructors(&variant([])), Some(Vec::new()));
        assert_eq!(variant_constructors(&Ty::Num), None);
        assert_eq!(variant_constructors(&record([])), None);
    }

    #[test]
    fn a_nullary_constructor_carries_the_empty_record_the_language_already_had() {
        assert_eq!(unit(), record([]));
        assert_eq!(unit().to_string(), "{}");
        assert_eq!(
            variant([(f(1), unit())]).to_string(),
            format!("[#{}: {{}}]", f(1).short())
        );
        assert_eq!(variant([]).to_string(), "[]");
        assert_eq!(
            variant([(f(1), Ty::Num), (f(2), Ty::Str)]).to_string(),
            format!("[#{}: Num | #{}: Str]", f(1).short(), f(2).short())
        );
    }

    #[test]
    fn a_command_type_prints_like_a_list_because_it_is_the_same_shape() {
        assert_eq!(cmd(Ty::Str).to_string(), "Cmd Str");
        assert_eq!(cmd(unit()).to_string(), "Cmd {}");
        assert_eq!(cmd(Ty::Hole).to_string(), "Cmd ?");
        assert_eq!(cmd(cmd(Ty::Num)).to_string(), "Cmd (Cmd Num)");
        assert_eq!(cmd(list(Ty::Num)).to_string(), "Cmd (List Num)");
        assert_eq!(list(cmd(Ty::Num)).to_string(), "List (Cmd Num)");
        assert_eq!(arrow(Ty::Str, cmd(Ty::Num)).to_string(), "Str -> Cmd Num");
        assert_eq!(cmd(arrow(Ty::Num, Ty::Num)).to_string(), "Cmd (Num -> Num)");
    }

    #[test]
    fn consistency_of_commands_is_consistency_of_what_they_yield() {
        assert!(is_consistent(&cmd(Ty::Str), &cmd(Ty::Str)));
        assert!(is_consistent(&cmd(Ty::Str), &cmd(Ty::Hole)));
        assert!(is_consistent(&cmd(Ty::Hole), &cmd(Ty::Str)));
        assert!(is_consistent(&cmd(Ty::Str), &Ty::Hole));
        assert!(is_consistent(&Ty::Hole, &cmd(Ty::Str)));

        assert!(!is_consistent(&cmd(Ty::Str), &cmd(Ty::Num)));
        assert!(
            !is_consistent(&cmd(Ty::Str), &Ty::Str),
            "a command that yields text is not text"
        );
        assert!(!is_consistent(&cmd(Ty::Num), &list(Ty::Num)));
        assert!(!is_consistent(&cmd(unit()), &unit()));
    }

    #[test]
    fn matched_cmd_fails_open_on_the_unknown_type_like_every_other_matched_rule() {
        assert_eq!(matched_cmd(&Ty::Hole), Some(Ty::Hole));
        assert_eq!(matched_cmd(&cmd(Ty::Str)), Some(Ty::Str));
        assert_eq!(matched_cmd(&cmd(cmd(Ty::Num))), Some(cmd(Ty::Num)));
        assert_eq!(matched_cmd(&cmd(unit())), Some(unit()));

        assert_eq!(matched_cmd(&Ty::Str), None);
        assert_eq!(matched_cmd(&list(Ty::Str)), None);
        assert_eq!(matched_cmd(&arrow(Ty::Num, Ty::Num)), None);
        assert_eq!(matched_cmd(&record([])), None);
    }

    #[test]
    fn matched_list_failure() {
        assert_eq!(matched_list(&Ty::Num), None);
        assert_eq!(matched_list(&Ty::Str), None);
        assert_eq!(matched_list(&arrow(Ty::Num, Ty::Bool)), None);
        assert_eq!(matched_list(&prod(Ty::Num, Ty::Bool)), None);
    }
}
