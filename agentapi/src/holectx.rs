use nothing_action::act::{Action, EditState, ctx_and_expected_ty_at};
use nothing_action::cursor_render::render_with_cursor;
use nothing_action::script::action_name;
use nothing_action::zipper::{Frame, arity};
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::render::render;
use nothing_core::ty::{Ty, is_consistent};

use crate::encode::{action_json, exp_kind, holes, ty_json};
use crate::json::Json;

#[derive(Clone, PartialEq, Debug)]
pub struct Binding {
    pub id: Id,
    pub name: String,
    pub ty: Ty,
    pub consistent_with_expected: bool,
    pub shadowed: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Construction {
    pub action: Action,
    pub step: Option<String>,
    pub template: Option<String>,
    pub produces: String,
    pub cursor_after: Vec<usize>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct HoleContext {
    pub cursor_path: Vec<usize>,
    pub focus_kind: &'static str,
    pub focus_render: String,
    pub at_empty_hole: bool,
    pub expected_ty: Ty,
    pub bindings: Vec<Binding>,
    pub constructions: Vec<Construction>,
    pub movements: Vec<String>,
    pub other_actions: Vec<String>,
}

pub const NUM_TEMPLATE: &str = "construct-num <integer>";
pub const BOOL_TEMPLATE: &str = "construct-bool <true|false>";

fn cursor_path(state: &EditState) -> Vec<usize> {
    state.zipper.path.iter().map(Frame::child_index).collect()
}

fn path_of(state: &EditState) -> Vec<usize> {
    cursor_path(state)
}

pub fn in_scope(state: &EditState) -> Vec<Binding> {
    let (ctx, expected) = ctx_and_expected_ty_at(&state.zipper);
    let ids = state.zipper.binders();
    let mut out: Vec<Binding> = Vec::new();
    for id in &ids {
        let Some(ty) = ctx.lookup(id) else { continue };
        let name = state.names.display(*id);
        out.push(Binding {
            id: *id,
            name,
            consistent_with_expected: is_consistent(&ty, &expected),
            ty,
            shadowed: false,
        });
    }
    for i in 0..out.len() {
        let shadowed = out[(i + 1)..].iter().any(|later| later.name == out[i].name);
        out[i].shadowed = shadowed;
    }
    out
}

fn resolves_to(state: &EditState, name: &str, id: Id) -> bool {
    state
        .zipper
        .binders()
        .into_iter()
        .rev()
        .find(|other| state.names.get(*other) == Some(name))
        == Some(id)
}

fn candidate_actions(state: &EditState) -> Vec<Action> {
    let mut out = vec![
        Action::ConstructNum(0),
        Action::ConstructBool(true),
        Action::ConstructBool(false),
    ];
    for binding in in_scope(state) {
        out.push(Action::ConstructVar(binding.id));
    }
    out.extend([
        Action::ConstructLam,
        Action::ConstructAp,
        Action::ConstructBinOp(Op::Add),
        Action::ConstructBinOp(Op::Sub),
        Action::ConstructBinOp(Op::Mul),
        Action::ConstructBinOp(Op::Lt),
        Action::ConstructBinOp(Op::Eq),
        Action::ConstructIf,
        Action::ConstructLet,
        Action::ConstructPair,
        Action::ConstructProj(Side::L),
        Action::ConstructProj(Side::R),
    ]);
    out
}

fn step_for(state: &EditState, action: &Action) -> Option<String> {
    match action {
        Action::ConstructVar(id) => {
            let name = state.names.get(*id)?.to_string();
            if resolves_to(state, &name, *id) {
                Some(format!("construct-var {name}"))
            } else {
                None
            }
        }
        other => Some(action_name(other)),
    }
}

fn template_for(action: &Action) -> Option<String> {
    match action {
        Action::ConstructNum(_) => Some(NUM_TEMPLATE.to_string()),
        Action::ConstructBool(_) => Some(BOOL_TEMPLATE.to_string()),
        _ => None,
    }
}

pub fn well_typed_constructions(state: &EditState) -> Vec<Construction> {
    let before = holes(&state.exp()).1;
    let mut out = Vec::new();
    let mut seen_bool = false;
    for action in candidate_actions(state) {
        let Some(next) = state.apply(action.clone()) else {
            continue;
        };
        if holes(&next.exp()).1 > before {
            continue;
        }
        if matches!(action, Action::ConstructBool(_)) {
            if seen_bool {
                continue;
            }
            seen_bool = true;
        }
        out.push(Construction {
            step: step_for(state, &action),
            template: template_for(&action),
            produces: render_with_cursor(&next.zipper, &next.names),
            cursor_after: cursor_path(&next),
            action,
        });
    }
    out
}

fn movements(state: &EditState) -> Vec<String> {
    let mut out = Vec::new();
    for n in 0..arity(&state.zipper.focus) {
        out.push(format!("move-child {n}"));
    }
    for (action, name) in [
        (Action::MoveParent, "move-parent"),
        (Action::MoveNextSibling, "move-next-sibling"),
        (Action::MovePrevSibling, "move-prev-sibling"),
    ] {
        if state.apply(action).is_some() {
            out.push(name.to_string());
        }
    }
    out
}

fn other_actions(state: &EditState) -> Vec<String> {
    let mut out = Vec::new();
    if state.apply(Action::Delete).is_some() {
        out.push("delete".to_string());
    }
    if state.apply(Action::Finish).is_some() {
        out.push("finish".to_string());
    }
    if matches!(state.zipper.focus, Exp::Lam(..)) {
        out.push("set-ann <type>".to_string());
    }
    if state.zipper.binder_id().is_some() {
        out.push("rename <name>".to_string());
    }
    out
}

pub fn hole_context(state: &EditState) -> HoleContext {
    let (_, expected) = ctx_and_expected_ty_at(&state.zipper);
    HoleContext {
        cursor_path: path_of(state),
        focus_kind: exp_kind(&state.zipper.focus),
        focus_render: render(&state.zipper.focus, &state.names),
        at_empty_hole: matches!(state.zipper.focus, Exp::EmptyHole(_)),
        expected_ty: expected,
        bindings: in_scope(state),
        constructions: well_typed_constructions(state),
        movements: movements(state),
        other_actions: other_actions(state),
    }
}

impl Binding {
    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            ("id", Json::str(self.id.to_string())),
            ("name", Json::str(self.name.clone())),
            ("ty", ty_json(&self.ty)),
            ("ty_text", Json::str(self.ty.to_string())),
            (
                "consistent_with_expected",
                Json::Bool(self.consistent_with_expected),
            ),
            ("shadowed", Json::Bool(self.shadowed)),
        ])
    }
}

impl Construction {
    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            (
                "step",
                match &self.step {
                    Some(text) => Json::str(text.clone()),
                    None => Json::Null,
                },
            ),
            (
                "template",
                match &self.template {
                    Some(text) => Json::str(text.clone()),
                    None => Json::Null,
                },
            ),
            ("action", action_json(&self.action)),
            ("produces", Json::str(self.produces.clone())),
            (
                "cursor_after",
                Json::arr(
                    self.cursor_after
                        .iter()
                        .map(|n| Json::Int(*n as i64))
                        .collect(),
                ),
            ),
        ])
    }
}

impl HoleContext {
    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            (
                "cursor_path",
                Json::arr(self.cursor_path.iter().map(|n| Json::Int(*n as i64)).collect()),
            ),
            ("focus_kind", Json::str(self.focus_kind)),
            ("focus_render", Json::str(self.focus_render.clone())),
            ("at_empty_hole", Json::Bool(self.at_empty_hole)),
            ("expected_ty", ty_json(&self.expected_ty)),
            ("expected_ty_text", Json::str(self.expected_ty.to_string())),
            (
                "bindings",
                Json::arr(self.bindings.iter().map(Binding::to_json).collect()),
            ),
            (
                "constructions",
                Json::arr(self.constructions.iter().map(Construction::to_json).collect()),
            ),
            (
                "movements",
                Json::arr(self.movements.iter().map(|m| Json::str(m.clone())).collect()),
            ),
            (
                "other_actions",
                Json::arr(
                    self.other_actions
                        .iter()
                        .map(|m| Json::str(m.clone()))
                        .collect(),
                ),
            ),
        ])
    }

    pub fn to_prompt_block(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("cursor path: {:?}\n", self.cursor_path));
        out.push_str(&format!(
            "focus: {} ({})\n",
            self.focus_render, self.focus_kind
        ));
        out.push_str(&format!("expected type at cursor: {}\n", self.expected_ty));
        if self.bindings.is_empty() {
            out.push_str("in scope: (nothing)\n");
        } else {
            out.push_str("in scope:\n");
            for b in &self.bindings {
                out.push_str(&format!(
                    "  {} : {}{}\n",
                    b.name,
                    b.ty,
                    if b.consistent_with_expected {
                        "   (fits the expected type)"
                    } else {
                        ""
                    }
                ));
            }
        }
        out.push_str("well-typed constructions here:\n");
        for c in &self.constructions {
            let label = c
                .template
                .clone()
                .or_else(|| c.step.clone())
                .unwrap_or_else(|| action_name(&c.action));
            out.push_str(&format!("  {label}   ->   {}\n", c.produces));
        }
        if !self.movements.is_empty() {
            out.push_str(&format!("movements: {}\n", self.movements.join(", ")));
        }
        if !self.other_actions.is_empty() {
            out.push_str(&format!("other: {}\n", self.other_actions.join(", ")));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::generate;
    use nothing_action::zipper::all_positions;
    use nothing_core::examples;
    use nothing_core::typing::is_well_typed;
    use proptest::prelude::*;

    fn state_from(script: &str) -> EditState {
        nothing_action::script::replay_script(script).expect("script replays")
    }

    fn non_empty_holes(state: &EditState) -> usize {
        holes(&state.exp()).1
    }

    fn assert_constructions_are_clean(state: &EditState) {
        let before = non_empty_holes(state);
        for construction in well_typed_constructions(state) {
            let next = state
                .apply(construction.action.clone())
                .unwrap_or_else(|| panic!("offered construction did not apply: {construction:?}"));
            assert!(
                is_well_typed(&next.exp()),
                "offered construction broke well-typedness: {construction:?}"
            );
            assert!(
                non_empty_holes(&next) <= before,
                "offered construction produced a non-empty hole: {construction:?} -> {}",
                next.render()
            );
        }
    }

    #[test]
    fn at_a_num_hole_no_offered_construction_produces_a_non_empty_hole() {
        let state = state_from("construct-num 1\nconstruct-binop add\n");
        assert_eq!(
            ctx_and_expected_ty_at(&state.zipper).1,
            Ty::Num,
            "the right operand of + expects Num"
        );
        assert_constructions_are_clean(&state);
    }

    #[test]
    fn a_num_hole_does_not_offer_a_boolean() {
        let state = state_from("construct-num 1\nconstruct-binop add\n");
        let steps: Vec<String> = well_typed_constructions(&state)
            .into_iter()
            .filter_map(|c| c.step)
            .collect();
        assert!(
            !steps.iter().any(|s| s.starts_with("construct-bool")),
            "a Num hole offered a boolean: {steps:?}"
        );
        assert!(
            steps.iter().any(|s| s == "construct-num 0"),
            "a Num hole must offer a number: {steps:?}"
        );
    }

    #[test]
    fn a_bool_hole_does_not_offer_a_number() {
        let state = state_from("construct-if\n");
        assert_eq!(ctx_and_expected_ty_at(&state.zipper).1, Ty::Bool);
        let steps: Vec<String> = well_typed_constructions(&state)
            .into_iter()
            .filter_map(|c| c.step)
            .collect();
        assert!(
            !steps.iter().any(|s| s.starts_with("construct-num")),
            "a Bool hole offered a number: {steps:?}"
        );
        assert!(steps.iter().any(|s| s.starts_with("construct-bool")), "{steps:?}");
        assert_constructions_are_clean(&state);
    }

    #[test]
    fn a_variable_of_the_wrong_type_is_not_offered() {
        let state = state_from(
            "construct-lam\nmove-parent\nrename b\nset-ann Bool\nmove-child 0\n\
             construct-num 1\nconstruct-binop add\n",
        );
        let steps: Vec<String> = well_typed_constructions(&state)
            .into_iter()
            .filter_map(|c| c.step)
            .collect();
        assert!(
            !steps.iter().any(|s| s == "construct-var b"),
            "a Bool variable was offered at a Num hole: {steps:?}"
        );
        assert_constructions_are_clean(&state);
    }

    #[test]
    fn a_variable_of_the_right_type_is_offered_with_its_display_name() {
        let state = state_from(
            "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\n\
             construct-num 1\nconstruct-binop add\n",
        );
        let ctx = hole_context(&state);
        assert!(ctx.at_empty_hole);
        assert_eq!(ctx.expected_ty, Ty::Num);
        assert_eq!(ctx.bindings.len(), 1);
        assert_eq!(ctx.bindings[0].name, "n");
        assert_eq!(ctx.bindings[0].ty, Ty::Num);
        assert!(ctx.bindings[0].consistent_with_expected);
        assert!(
            ctx.constructions
                .iter()
                .filter_map(|c| c.step.as_deref())
                .any(|s| s == "construct-var n")
        );
        assert_constructions_are_clean(&state);
    }

    #[test]
    fn the_query_is_clean_at_every_position_of_every_example() {
        let examples = [
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
        ];
        for exp in examples {
            for z in all_positions(&exp) {
                let state = EditState {
                    zipper: z,
                    fresh: nothing_action::act::Fresh::from_program(&exp),
                    names: examples::names(),
                };
                assert_constructions_are_clean(&state);
            }
        }
    }

    #[test]
    fn shadowed_bindings_are_flagged_and_only_the_visible_one_gets_a_step() {
        let state = state_from(
            "construct-lam\nmove-parent\nrename x\nset-ann Num\nmove-child 0\n\
             construct-lam\nmove-parent\nrename x\nset-ann Num\nmove-child 0\n",
        );
        let ctx = hole_context(&state);
        assert_eq!(ctx.bindings.len(), 2);
        assert!(ctx.bindings[0].shadowed, "the outer `x` is shadowed");
        assert!(!ctx.bindings[1].shadowed);
        let var_steps: Vec<&str> = ctx
            .constructions
            .iter()
            .filter(|c| matches!(c.action, Action::ConstructVar(_)))
            .filter_map(|c| c.step.as_deref())
            .collect();
        assert_eq!(var_steps, vec!["construct-var x"]);
        let var_actions = ctx
            .constructions
            .iter()
            .filter(|c| matches!(c.action, Action::ConstructVar(_)))
            .count();
        assert_eq!(var_actions, 2, "both bindings are reachable structurally");
    }

    #[test]
    fn the_json_shape_carries_names_types_and_constructions() {
        let state = state_from(
            "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\n",
        );
        let text = hole_context(&state).to_json().to_string();
        let parsed = crate::json::parse(&text).unwrap();
        assert_eq!(parsed.get("at_empty_hole").unwrap().as_bool(), Some(true));
        let bindings = parsed.get("bindings").unwrap().as_arr().unwrap();
        assert_eq!(bindings[0].get("name").unwrap().as_str(), Some("n"));
        assert_eq!(bindings[0].get("ty_text").unwrap().as_str(), Some("Num"));
        assert!(!parsed.get("constructions").unwrap().as_arr().unwrap().is_empty());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn no_offered_construction_ever_produces_a_non_empty_hole(
            seed in any::<u64>(),
            position in any::<u16>(),
        ) {
            let exp = generate::well_typed_exp(seed);
            let positions = all_positions(&exp);
            let z = positions[position as usize % positions.len()].clone();
            let state = EditState {
                zipper: z,
                fresh: nothing_action::act::Fresh::from_program(&exp),
                names: examples::names(),
            };
            let before = holes(&state.exp()).1;
            for construction in well_typed_constructions(&state) {
                let next = state.apply(construction.action.clone());
                prop_assert!(next.is_some(), "offered construction did not apply");
                let next = next.unwrap();
                prop_assert!(is_well_typed(&next.exp()));
                prop_assert!(
                    holes(&next.exp()).1 <= before,
                    "offered construction produced a non-empty hole: {}",
                    next.render()
                );
            }
        }

        #[test]
        fn the_offered_set_is_never_empty_at_an_empty_hole(seed in any::<u64>()) {
            let exp = generate::well_typed_exp(seed);
            for z in all_positions(&exp) {
                if !matches!(z.focus, Exp::EmptyHole(_)) {
                    continue;
                }
                let state = EditState {
                    zipper: z,
                    fresh: nothing_action::act::Fresh::from_program(&exp),
                    names: examples::names(),
                };
                prop_assert!(
                    !well_typed_constructions(&state).is_empty(),
                    "no construction was offered at an empty hole"
                );
            }
        }
    }
}
