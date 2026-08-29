use nothing_core::ctx::Ctx;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_core::render::render;

use crate::apply::apply_one;
use crate::diff::diff;
use crate::ops::{Operation, regions_overlap};
use crate::path::{at, label};
use crate::repair::{Repair, repair_in};
use crate::version::Version;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConflictKind {
    SameNodeDifferentValues,
    EditInsideRewrittenSubtree,
    CompetingRenames,
    OrderingAgainstRestructure,
    CompetingMoves,
    CompetingAnnotations,
}

impl ConflictKind {
    pub fn label(self) -> &'static str {
        match self {
            ConflictKind::SameNodeDifferentValues => "same node, different values",
            ConflictKind::EditInsideRewrittenSubtree => "edit inside a rewritten subtree",
            ConflictKind::CompetingRenames => "competing renames of one binder",
            ConflictKind::OrderingAgainstRestructure => {
                "reordering against a restructure of the same chain"
            }
            ConflictKind::CompetingMoves => "competing moves of overlapping subtrees",
            ConflictKind::CompetingAnnotations => "competing parameter annotations",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub site: String,
    pub ours: Operation,
    pub theirs: Operation,
    pub base_text: String,
    pub ours_text: String,
    pub theirs_text: String,
    pub why: String,
}

impl Conflict {
    pub fn report(&self) -> String {
        format!(
            "conflict ({}) at {}\n  why:    {}\n  base:   {}\n  ours:   {}\n  theirs: {}",
            self.kind.label(),
            self.site,
            self.why,
            self.base_text,
            self.ours_text,
            self.theirs_text
        )
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct MergeOutcome {
    pub merged: Version,
    pub conflicts: Vec<Conflict>,
    pub ours_ops: Vec<Operation>,
    pub theirs_ops: Vec<Operation>,
    pub applied: Vec<Operation>,
    pub commuted: Vec<Operation>,
    pub dropped: Vec<Operation>,
    pub repairs: Vec<Repair>,
}

impl MergeOutcome {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn report(&self) -> String {
        if self.conflicts.is_empty() {
            return format!("clean merge: {}", self.merged.render());
        }
        let bodies: Vec<String> = self.conflicts.iter().map(Conflict::report).collect();
        bodies.join("\n\n")
    }
}

const PHASE_PLAIN: u8 = 0;
const PHASE_MOVE: u8 = 1;
const PHASE_REBASED: u8 = 2;
const PHASE_ORDER: u8 = 3;

#[derive(Clone, PartialEq, Debug)]
struct Planned {
    original: Operation,
    effective: Operation,
    phase: u8,
    commuted_with: Option<usize>,
}

fn plan(ops: &[Operation]) -> Vec<Planned> {
    ops.iter()
        .map(|op| Planned {
            original: op.clone(),
            effective: op.clone(),
            phase: match op {
                Operation::Move { .. } => PHASE_MOVE,
                Operation::MoveBinding { .. } | Operation::ReorderFields { .. } => PHASE_ORDER,
                _ => PHASE_PLAIN,
            },
            commuted_with: None,
        })
        .collect()
}

fn rebase_against_moves(movers: &[Planned], riders: &mut [Planned]) {
    for (i, mover) in movers.iter().enumerate() {
        let (from, to) = match &mover.effective {
            Operation::Move { from, to, .. } => (from.clone(), to.clone()),
            _ => continue,
        };
        for rider in riders.iter_mut() {
            if rider.commuted_with.is_some() {
                continue;
            }
            if let Some(moved) = rider.original.rebased(&from, &to) {
                rider.effective = moved;
                rider.phase = PHASE_REBASED;
                rider.commuted_with = Some(i);
            }
        }
    }
}

fn undo_rebases(blocked: &[bool], riders: &mut [Planned]) {
    for rider in riders.iter_mut() {
        if let Some(index) = rider.commuted_with
            && blocked.get(index).copied().unwrap_or(false)
        {
            rider.effective = rider.original.clone();
            rider.phase = PHASE_PLAIN;
            rider.commuted_with = None;
        }
    }
}

pub fn merge(base: &Version, ours: &Version, theirs: &Version) -> MergeOutcome {
    merge_in(&Ctx::empty(), base, ours, theirs)
}

pub fn merge_in(ctx: &Ctx, base: &Version, ours: &Version, theirs: &Version) -> MergeOutcome {
    let ours_ops = diff(base, ours);
    let theirs_ops = diff(base, theirs);

    let mut mine = plan(&ours_ops);
    let mut yours = plan(&theirs_ops);
    let mine_snapshot = mine.clone();
    let yours_snapshot = yours.clone();
    rebase_against_moves(&mine_snapshot, &mut yours);
    rebase_against_moves(&yours_snapshot, &mut mine);

    let mut conflicts = Vec::new();
    let mut mine_blocked = vec![false; mine.len()];
    let mut yours_blocked = vec![false; yours.len()];

    for i in 0..mine.len() {
        for j in 0..yours.len() {
            if mine[i].original == yours[j].original {
                continue;
            }
            if yours[j].commuted_with == Some(i) || mine[i].commuted_with == Some(j) {
                continue;
            }
            if !ops_overlap(&mine[i].effective, &yours[j].effective) {
                continue;
            }
            mine_blocked[i] = true;
            yours_blocked[j] = true;
            conflicts.push(build_conflict(
                base,
                ours,
                theirs,
                &mine[i].original,
                &yours[j].original,
            ));
        }
    }

    undo_rebases(&mine_blocked, &mut yours);
    undo_rebases(&yours_blocked, &mut mine);

    let mut schedule: Vec<Planned> = Vec::new();
    for (i, item) in mine.iter().enumerate() {
        if !mine_blocked[i] {
            schedule.push(item.clone());
        }
    }
    for (j, item) in yours.iter().enumerate() {
        if yours_blocked[j] {
            continue;
        }
        if schedule
            .iter()
            .any(|other| other.effective == item.effective)
        {
            continue;
        }
        schedule.push(item.clone());
    }

    let mut names = base.names.clone();
    for item in &schedule {
        if let Operation::Rename { id, to, .. } = &item.effective {
            names.set(*id, to.clone());
        }
    }

    schedule.sort_by_key(|item| {
        (
            item.phase,
            std::cmp::Reverse(item.effective.site().map_or(0, Vec::len)),
        )
    });

    let mut exp = base.exp.clone();
    let mut dropped = Vec::new();
    for item in &schedule {
        match apply_one(&exp, &item.effective) {
            Some(next) => exp = next,
            None => dropped.push(item.effective.clone()),
        }
    }

    let repaired = repair_in(ctx, &exp, &names);
    let commuted: Vec<Operation> = schedule
        .iter()
        .filter(|item| item.phase == PHASE_REBASED)
        .map(|item| item.effective.clone())
        .collect();
    let applied: Vec<Operation> = schedule
        .into_iter()
        .map(|item| item.effective)
        .filter(|op| !dropped.contains(op))
        .collect();

    MergeOutcome {
        merged: Version::new(repaired.exp, names),
        conflicts,
        ours_ops,
        theirs_ops,
        applied,
        commuted,
        dropped,
        repairs: repaired.repairs,
    }
}

pub fn ops_overlap(a: &Operation, b: &Operation) -> bool {
    a.footprint()
        .iter()
        .any(|x| b.footprint().iter().any(|y| regions_overlap(x, y)))
}

fn classify(a: &Operation, b: &Operation) -> ConflictKind {
    match (a, b) {
        (Operation::Rename { .. }, Operation::Rename { .. }) => ConflictKind::CompetingRenames,
        (Operation::SetAnn { .. }, Operation::SetAnn { .. }) => ConflictKind::CompetingAnnotations,
        (Operation::MoveBinding { .. }, _)
        | (_, Operation::MoveBinding { .. })
        | (Operation::ReorderFields { .. }, _)
        | (_, Operation::ReorderFields { .. }) => ConflictKind::OrderingAgainstRestructure,
        (Operation::Move { .. }, _) | (_, Operation::Move { .. }) => ConflictKind::CompetingMoves,
        _ => {
            if a.site() == b.site() {
                ConflictKind::SameNodeDifferentValues
            } else {
                ConflictKind::EditInsideRewrittenSubtree
            }
        }
    }
}

fn base_text(base: &Version, op: &Operation) -> String {
    match op {
        Operation::Rename { from, id, .. } => match from {
            Some(name) => name.clone(),
            None => base.names.display(*id),
        },
        _ => match op.site().and_then(|path| at(&base.exp, path)) {
            Some(node) => render(node, &base.names),
            None => "<gone>".to_string(),
        },
    }
}

fn site_text(base: &Version, op: &Operation) -> String {
    match op {
        Operation::Rename { id, .. } => format!("the name of `{}`", base.names.display(*id)),
        _ => match op.site() {
            Some(path) => label(&base.exp, path),
            None => "the whole program".to_string(),
        },
    }
}

fn build_conflict(
    base: &Version,
    ours: &Version,
    theirs: &Version,
    mine: &Operation,
    yours: &Operation,
) -> Conflict {
    let kind = classify(mine, yours);
    let why = format!(
        "one branch {}; the other {}. Those two edits touch the same nodes and do not commute, so neither can be replayed on top of the other.",
        mine.describe(&base.exp, &ours.names),
        yours.describe(&base.exp, &theirs.names)
    );
    Conflict {
        kind,
        site: site_text(base, mine),
        base_text: base_text(base, mine),
        ours_text: mine.outcome(&ours.names),
        theirs_text: yours.outcome(&theirs.names),
        ours: mine.clone(),
        theirs: yours.clone(),
        why,
    }
}

pub fn merged_names(base: &NameTable, ops: &[Operation]) -> NameTable {
    crate::apply::apply_names(base, ops)
}

pub fn renders_as(exp: &Exp, names: &NameTable) -> String {
    render(exp, names)
}
