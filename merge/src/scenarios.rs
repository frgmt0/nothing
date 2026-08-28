use nothing_core::exp::{Exp, HoleId, Id, Op};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

use crate::chain::{self, Binding, Chain};
use crate::text::{AIRY, CANONICAL, Style, WIDE};
use crate::version::Version;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Reordering,
    Renaming,
    Reformatting,
    Moving,
    Control,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Reordering => "reordering",
            Category::Renaming => "renaming",
            Category::Reformatting => "reformatting",
            Category::Moving => "moving",
            Category::Control => "control",
        }
    }

    pub const ALL: [Category; 5] = [
        Category::Reordering,
        Category::Renaming,
        Category::Reformatting,
        Category::Moving,
        Category::Control,
    ];
}

#[derive(Clone, PartialEq, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub category: Category,
    pub note: &'static str,
    pub base: Version,
    pub ours: Version,
    pub theirs: Version,
    pub base_style: Style,
    pub ours_style: Style,
    pub theirs_style: Style,
}

fn id(n: u128) -> Id {
    Id::from_u128(0x5CE7_0000_0000_0000_0000_0000_0000_0000 | n)
}

fn hole(n: u128) -> HoleId {
    HoleId::from_u128(0x5CE7_FFFF_0000_0000_0000_0000_0000_0000 | n)
}

const F: u128 = 1;
const G: u128 = 2;
const H: u128 = 3;
const A: u128 = 10;
const B: u128 = 11;
const C: u128 = 12;
const X: u128 = 13;

fn names() -> NameTable {
    let mut names = NameTable::new();
    names.set(id(F), "square");
    names.set(id(G), "bump");
    names.set(id(H), "drop2");
    names.set(id(A), "a");
    names.set(id(B), "b");
    names.set(id(C), "c");
    names.set(id(X), "x");
    names
}

fn square(offset: i64) -> Exp {
    let body = Exp::bin_op(Op::Mul, Exp::var(id(A)), Exp::var(id(A)));
    let body = if offset == 0 {
        body
    } else {
        Exp::bin_op(Op::Add, body, Exp::num(offset))
    };
    Exp::lam(id(A), Ty::Num, body)
}

fn bump(step: i64) -> Exp {
    Exp::lam(
        id(B),
        Ty::Num,
        Exp::bin_op(Op::Add, Exp::var(id(B)), Exp::num(step)),
    )
}

fn drop2(step: i64) -> Exp {
    Exp::lam(
        id(C),
        Ty::Num,
        Exp::bin_op(Op::Sub, Exp::var(id(C)), Exp::num(step)),
    )
}

fn adder(step: i64) -> Exp {
    Exp::lam(
        id(X),
        Ty::Num,
        Exp::bin_op(Op::Add, Exp::var(id(X)), Exp::num(step)),
    )
}

fn call_tail() -> Exp {
    Exp::bin_op(
        Op::Add,
        Exp::ap(Exp::var(id(F)), Exp::num(3)),
        Exp::ap(Exp::var(id(G)), Exp::num(4)),
    )
}

fn program(order: [u128; 3], bodies: [Exp; 3], tail: Exp) -> Exp {
    let slot = |which: u128| -> Exp {
        match which {
            F => bodies[0].clone(),
            G => bodies[1].clone(),
            _ => bodies[2].clone(),
        }
    };
    let chain = Chain {
        bindings: order
            .iter()
            .map(|which| Binding {
                id: id(*which),
                bound: slot(*which),
            })
            .collect(),
        tail,
    };
    chain::rebuild(&chain)
}

fn three_bindings(order: [u128; 3], bodies: [Exp; 3]) -> Version {
    Version::new(program(order, bodies, call_tail()), names())
}

fn stock() -> [Exp; 3] {
    [square(0), bump(1), drop2(2)]
}

fn pair_program(fst: Exp, snd: Exp) -> Version {
    Version::new(program([F, G, H], stock(), Exp::pair(fst, snd)), names())
}

fn renamed(version: &Version, target: u128, name: &str) -> Version {
    let mut names = version.names.clone();
    names.set(id(target), name);
    Version::new(version.exp.clone(), names)
}

pub fn all() -> Vec<Scenario> {
    let base3 = three_bindings([F, G, H], stock());
    let moved_base = pair_program(adder(10), Exp::empty_hole(hole(1)));

    vec![
        Scenario {
            name: "swap adjacent bindings vs edit inside one of them",
            category: Category::Reordering,
            note: "one branch reorders `square` and `bump`; the other changes the body of `square`",
            base: base3.clone(),
            ours: three_bindings([G, F, H], stock()),
            theirs: three_bindings([F, G, H], [square(1), bump(1), drop2(2)]),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "move a binding past another vs edit the one it passes",
            category: Category::Reordering,
            note: "one branch moves `drop2` up one slot; the other changes the body of `bump`",
            base: base3.clone(),
            ours: three_bindings([F, H, G], stock()),
            theirs: three_bindings([F, G, H], [square(0), bump(7), drop2(2)]),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "reverse the whole chain vs edit the last binding",
            category: Category::Reordering,
            note: "one branch reverses all three bindings; the other changes the body of `drop2`",
            base: base3.clone(),
            ours: three_bindings([H, G, F], stock()),
            theirs: three_bindings([F, G, H], [square(0), bump(1), drop2(9)]),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "rename a parameter vs edit the line that uses it",
            category: Category::Renaming,
            note: "one branch renames the parameter `a`; the other changes the expression `a * a`",
            base: base3.clone(),
            ours: renamed(&base3, A, "value"),
            theirs: three_bindings([F, G, H], [square(1), bump(1), drop2(2)]),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "rename a function vs reorder the chain",
            category: Category::Renaming,
            note: "one branch renames `square` to `sq`; the other moves `drop2` to the front",
            base: base3.clone(),
            ours: renamed(&base3, F, "sq"),
            theirs: three_bindings([H, F, G], stock()),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "two branches rename two different functions",
            category: Category::Renaming,
            note: "both renames land on the call line `square 3 + bump 4`",
            base: base3.clone(),
            ours: renamed(&base3, F, "sq"),
            theirs: renamed(&base3, G, "inc"),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "two branches rename the same function differently",
            category: Category::Control,
            note: "a genuine conflict: both branches claim the display name of `square`",
            base: base3.clone(),
            ours: renamed(&base3, F, "sq"),
            theirs: renamed(&base3, F, "pow2"),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "reindent the whole file vs edit one literal",
            category: Category::Reformatting,
            note: "one branch reprints at four-space indent; the other changes `b + 1` to `b + 5`",
            base: base3.clone(),
            ours: base3.clone(),
            theirs: three_bindings([F, G, H], [square(0), bump(5), drop2(2)]),
            base_style: CANONICAL,
            ours_style: WIDE,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "reindent the whole file vs rename a function",
            category: Category::Reformatting,
            note: "one branch reprints at four-space indent; the other renames `bump`",
            base: base3.clone(),
            ours: base3.clone(),
            theirs: renamed(&base3, G, "inc"),
            base_style: CANONICAL,
            ours_style: WIDE,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "blank-line style vs reorder the chain",
            category: Category::Reformatting,
            note: "one branch adds a blank line after every binding; the other reorders bindings",
            base: base3.clone(),
            ours: base3.clone(),
            theirs: three_bindings([G, F, H], stock()),
            base_style: CANONICAL,
            ours_style: AIRY,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "reindent both branches, one of which also edits",
            category: Category::Reformatting,
            note: "both branches reprint; only one changes a literal",
            base: base3.clone(),
            ours: base3.clone(),
            theirs: three_bindings([F, G, H], [square(0), bump(1), drop2(6)]),
            base_style: CANONICAL,
            ours_style: WIDE,
            theirs_style: AIRY,
        },
        Scenario {
            name: "move a function into a hole vs edit inside the moved function",
            category: Category::Moving,
            note: "one branch moves the lambda to the other pair component; the other edits its body",
            base: moved_base.clone(),
            ours: pair_program(Exp::empty_hole(hole(2)), adder(10)),
            theirs: pair_program(adder(20), Exp::empty_hole(hole(1))),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "move a function into a hole vs edit a binding above it",
            category: Category::Moving,
            note: "one branch moves the lambda across the pair; the other edits `bump`",
            base: moved_base.clone(),
            ours: pair_program(Exp::empty_hole(hole(2)), adder(10)),
            theirs: Version::new(
                program(
                    [F, G, H],
                    [square(0), bump(8), drop2(2)],
                    Exp::pair(adder(10), Exp::empty_hole(hole(1))),
                ),
                names(),
            ),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "both branches move the same function to different places",
            category: Category::Control,
            note: "a genuine conflict: the same subtree is claimed by two destinations",
            base: pair_program(adder(10), Exp::empty_hole(hole(1))),
            ours: pair_program(Exp::empty_hole(hole(2)), adder(10)),
            theirs: Version::new(
                program(
                    [F, G, H],
                    [square(0), bump(1), adder(10)],
                    Exp::pair(Exp::empty_hole(hole(3)), Exp::empty_hole(hole(1))),
                ),
                names(),
            ),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "both branches change the same literal to different values",
            category: Category::Control,
            note: "a genuine conflict: line-based and structural merge should both refuse",
            base: base3.clone(),
            ours: three_bindings([F, G, H], [square(0), bump(2), drop2(2)]),
            theirs: three_bindings([F, G, H], [square(0), bump(3), drop2(2)]),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
        Scenario {
            name: "both branches make the identical edit",
            category: Category::Control,
            note: "convergent edits: both merges should be clean",
            base: base3.clone(),
            ours: three_bindings([F, G, H], [square(0), bump(4), drop2(2)]),
            theirs: three_bindings([F, G, H], [square(0), bump(4), drop2(2)]),
            base_style: CANONICAL,
            ours_style: CANONICAL,
            theirs_style: CANONICAL,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::typing::is_well_typed;

    #[test]
    fn every_scenario_version_is_well_typed() {
        for scenario in all() {
            for (which, version) in [
                ("base", &scenario.base),
                ("ours", &scenario.ours),
                ("theirs", &scenario.theirs),
            ] {
                assert!(
                    is_well_typed(&version.exp),
                    "{} / {which} is ill-typed: {}",
                    scenario.name,
                    version.render()
                );
            }
        }
    }

    #[test]
    fn every_scenario_actually_differs_from_its_base_on_at_least_one_side() {
        for scenario in all() {
            let structural = scenario.ours != scenario.base || scenario.theirs != scenario.base;
            let textual = scenario.ours_style != scenario.base_style
                || scenario.theirs_style != scenario.base_style;
            assert!(structural || textual, "{} changes nothing", scenario.name);
        }
    }

    #[test]
    fn every_category_has_at_least_one_scenario() {
        for category in Category::ALL {
            assert!(
                all().iter().any(|s| s.category == category),
                "no scenario for {}",
                category.label()
            );
        }
    }
}
