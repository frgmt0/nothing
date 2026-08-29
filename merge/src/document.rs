use std::collections::HashSet;

use nothing_core::ctx::Ctx;
use nothing_core::doc::{Def, Doc, references, vacate};
use nothing_core::exp::{Exp, HoleId, Id};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

use crate::merge3::{Conflict, ConflictKind, merge_in};
use crate::repair::{Repair, repair_in};
use crate::version::Version;

#[derive(Clone, PartialEq, Debug)]
pub struct DocVersion {
    pub doc: Doc,
    pub names: NameTable,
}

impl DocVersion {
    pub fn new(doc: Doc, names: NameTable) -> DocVersion {
        DocVersion { doc, names }
    }

    pub fn single(exp: Exp, names: NameTable) -> DocVersion {
        DocVersion {
            doc: Doc::single(exp),
            names,
        }
    }

    pub fn render(&self) -> String {
        self.doc.render(&self.names)
    }

    pub fn is_well_typed(&self) -> bool {
        self.doc.is_well_typed()
    }

    pub fn definition(&self, id: Id) -> Option<&Def> {
        self.doc.get(id)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocConflictKind {
    WithinDefinition(ConflictKind),
    DeletedAndEdited,
    CompetingAdditions,
    CompetingAnnotations,
    CompetingOrder,
    CompetingNames,
}

impl DocConflictKind {
    pub fn label(self) -> &'static str {
        match self {
            DocConflictKind::WithinDefinition(kind) => kind.label(),
            DocConflictKind::DeletedAndEdited => "deleted on one side, edited on the other",
            DocConflictKind::CompetingAdditions => "both sides added a different definition",
            DocConflictKind::CompetingAnnotations => "competing definition annotations",
            DocConflictKind::CompetingOrder => "competing definition order",
            DocConflictKind::CompetingNames => "competing definition names",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct DocConflict {
    pub kind: DocConflictKind,
    pub definition: Id,
    pub site: String,
    pub why: String,
    pub base_text: String,
    pub ours_text: String,
    pub theirs_text: String,
}

impl DocConflict {
    pub fn report(&self) -> String {
        format!(
            "conflict ({}) in definition {}\n  why:    {}\n  base:   {}\n  ours:   {}\n  theirs: {}",
            self.kind.label(),
            self.site,
            self.why,
            self.base_text,
            self.ours_text,
            self.theirs_text
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefChange {
    Untouched,
    Added,
    Removed,
    Edited,
    Reannotated,
    Renamed,
    Moved,
}

#[derive(Clone, PartialEq, Debug)]
pub struct DocMergeOutcome {
    pub merged: DocVersion,
    pub conflicts: Vec<DocConflict>,
    pub ours_changes: Vec<(Id, DefChange)>,
    pub theirs_changes: Vec<(Id, DefChange)>,
    pub repairs: Vec<Repair>,
}

impl DocMergeOutcome {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn report(&self) -> String {
        if self.conflicts.is_empty() {
            return format!("clean merge:\n{}", self.merged.render());
        }
        self.conflicts
            .iter()
            .map(DocConflict::report)
            .collect::<Vec<String>>()
            .join("\n\n")
    }
}

pub fn changes(base: &DocVersion, side: &DocVersion) -> Vec<(Id, DefChange)> {
    let mut out = Vec::new();
    for (index, def) in side.doc.defs().iter().enumerate() {
        let change = match base.doc.get(def.id) {
            None => DefChange::Added,
            Some(was) => {
                if was.body != def.body {
                    DefChange::Edited
                } else if was.ann != def.ann {
                    DefChange::Reannotated
                } else if base.names.get(def.id) != side.names.get(def.id) {
                    DefChange::Renamed
                } else if base.doc.index_of(def.id) != Some(index) {
                    DefChange::Moved
                } else {
                    DefChange::Untouched
                }
            }
        };
        out.push((def.id, change));
    }
    for def in base.doc.defs() {
        if side.doc.get(def.id).is_none() {
            out.push((def.id, DefChange::Removed));
        }
    }
    out
}

fn conflict(
    kind: DocConflictKind,
    id: Id,
    names: &NameTable,
    why: &str,
    base_text: String,
    ours_text: String,
    theirs_text: String,
) -> DocConflict {
    DocConflict {
        kind,
        definition: id,
        site: names.display(id),
        why: why.to_string(),
        base_text,
        ours_text,
        theirs_text,
    }
}

fn text(def: Option<&Def>, names: &NameTable) -> String {
    match def {
        Some(def) => nothing_core::doc::render_def(def, names),
        None => "(no definition)".to_string(),
    }
}

fn merged_names(
    base: &DocVersion,
    ours: &DocVersion,
    theirs: &DocVersion,
    conflicts: &mut Vec<DocConflict>,
) -> NameTable {
    let mut out = ours.names.flatten();
    let mut ids: Vec<Id> = base.names.ids();
    for id in theirs.names.ids() {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    for id in ours.names.ids() {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.sort_by_key(|id| id.as_u128());

    for id in ids {
        let was = base.names.get(id);
        let mine = ours.names.get(id);
        let yours = theirs.names.get(id);
        match (was == mine, was == yours) {
            (true, true) => {}
            (true, false) => {
                if let Some(name) = yours {
                    out.set(id, name);
                }
            }
            (false, true) => {}
            (false, false) => {
                if mine != yours {
                    conflicts.push(conflict(
                        DocConflictKind::CompetingNames,
                        id,
                        &ours.names,
                        "both sides renamed the same definition",
                        was.unwrap_or("(unnamed)").to_string(),
                        mine.unwrap_or("(unnamed)").to_string(),
                        yours.unwrap_or("(unnamed)").to_string(),
                    ));
                }
            }
        }
    }
    out
}

fn merged_ann(
    id: Id,
    base: &Ty,
    ours: &Ty,
    theirs: &Ty,
    names: &NameTable,
    conflicts: &mut Vec<DocConflict>,
) -> Ty {
    match (base == ours, base == theirs) {
        (true, true) => base.clone(),
        (true, false) => theirs.clone(),
        (false, true) => ours.clone(),
        (false, false) => {
            if ours == theirs {
                ours.clone()
            } else {
                conflicts.push(conflict(
                    DocConflictKind::CompetingAnnotations,
                    id,
                    names,
                    "both sides changed this definition's annotation",
                    base.to_string(),
                    ours.to_string(),
                    theirs.to_string(),
                ));
                ours.clone()
            }
        }
    }
}

fn merged_order(
    base: &DocVersion,
    ours: &DocVersion,
    theirs: &DocVersion,
    surviving: &[Id],
    names: &NameTable,
    conflicts: &mut Vec<DocConflict>,
) -> Vec<Id> {
    for id in surviving {
        let was = base.doc.index_of(*id);
        let mine = ours.doc.index_of(*id);
        let yours = theirs.doc.index_of(*id);
        if was.is_some()
            && mine != was
            && yours != was
            && mine != yours
            && mine.is_some()
            && yours.is_some()
        {
            conflicts.push(conflict(
                DocConflictKind::CompetingOrder,
                *id,
                names,
                "both sides moved this definition to a different place",
                was.map(|i| i.to_string()).unwrap_or_default(),
                mine.map(|i| i.to_string()).unwrap_or_default(),
                yours.map(|i| i.to_string()).unwrap_or_default(),
            ));
        }
    }

    let mut order: Vec<Id> = ours
        .doc
        .ids()
        .into_iter()
        .filter(|id| surviving.contains(id))
        .collect();

    for id in theirs.doc.ids() {
        if !surviving.contains(&id) || order.contains(&id) {
            continue;
        }
        let at = theirs
            .doc
            .index_of(id)
            .unwrap_or(order.len())
            .min(order.len());
        order.insert(at, id);
    }

    for id in surviving {
        if !order.contains(id) {
            order.push(*id);
        }
    }
    order
}

pub fn merge_documents(
    base: &DocVersion,
    ours: &DocVersion,
    theirs: &DocVersion,
) -> DocMergeOutcome {
    let mut conflicts: Vec<DocConflict> = Vec::new();
    let names = merged_names(base, ours, theirs, &mut conflicts);

    let mut ids: Vec<Id> = base.doc.ids();
    for id in ours.doc.ids().into_iter().chain(theirs.doc.ids()) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    let union_ctx = ids.iter().fold(Ctx::empty(), |ctx, id| {
        let ann = ours
            .doc
            .get(*id)
            .or_else(|| theirs.doc.get(*id))
            .or_else(|| base.doc.get(*id))
            .map(|def| def.ann.clone())
            .unwrap_or(Ty::Hole);
        ctx.extend(*id, ann)
    });

    let mut merged: Vec<Def> = Vec::new();
    let mut removed: Vec<Id> = Vec::new();

    for id in &ids {
        let was = base.doc.get(*id);
        let mine = ours.doc.get(*id);
        let yours = theirs.doc.get(*id);

        match (was, mine, yours) {
            (_, None, None) => removed.push(*id),

            (None, Some(def), None) | (None, None, Some(def)) => merged.push(def.clone()),

            (None, Some(a), Some(b)) => {
                if a == b {
                    merged.push(a.clone());
                } else {
                    conflicts.push(conflict(
                        DocConflictKind::CompetingAdditions,
                        *id,
                        &names,
                        "both sides added a definition with this id and different contents",
                        "(no definition)".to_string(),
                        text(Some(a), &names),
                        text(Some(b), &names),
                    ));
                    merged.push(a.clone());
                }
            }

            (Some(was), Some(def), None) | (Some(was), None, Some(def)) => {
                if def.body == was.body && def.ann == was.ann {
                    removed.push(*id);
                } else {
                    conflicts.push(conflict(
                        DocConflictKind::DeletedAndEdited,
                        *id,
                        &names,
                        "one side deleted this definition while the other edited it",
                        text(Some(was), &names),
                        text(ours.doc.get(*id), &names),
                        text(theirs.doc.get(*id), &names),
                    ));
                    merged.push(def.clone());
                }
            }

            (Some(was), Some(mine), Some(yours)) => {
                let outcome = merge_in(
                    &union_ctx,
                    &Version::new(was.body.clone(), base.names.flatten()),
                    &Version::new(mine.body.clone(), ours.names.flatten()),
                    &Version::new(yours.body.clone(), theirs.names.flatten()),
                );
                for c in &outcome.conflicts {
                    conflicts.push(within(*id, &names, c));
                }
                let ann = merged_ann(*id, &was.ann, &mine.ann, &yours.ann, &names, &mut conflicts);
                merged.push(Def::new(*id, ann, outcome.merged.exp.clone()));
            }
        }
    }

    if merged.is_empty() {
        let id = base
            .doc
            .defs()
            .first()
            .map(|def| def.id)
            .unwrap_or(nothing_core::doc::MAIN_ID);
        removed.retain(|r| *r != id);
        merged.push(Def::hole(id, HoleId::from_u128(0)));
    }

    let surviving: Vec<Id> = merged.iter().map(|def| def.id).collect();
    let order = merged_order(base, ours, theirs, &surviving, &names, &mut conflicts);
    merged.sort_by_key(|def| {
        order
            .iter()
            .position(|id| *id == def.id)
            .unwrap_or(usize::MAX)
    });

    let gone: HashSet<Id> = removed.into_iter().collect();
    let mut next_hole = 0u128;
    for def in &mut merged {
        for id in &gone {
            if references(&def.body, *id) {
                def.body = vacate(&def.body, *id, &mut || {
                    next_hole += 1;
                    HoleId::from_u128(0xdead_0000_0000_0000_0000_0000_0000_0000 | next_hole)
                });
            }
        }
    }

    let doc = Doc::new(merged).expect("the merged definitions have distinct ids");
    let ctx = doc.ctx();
    let mut repairs = Vec::new();
    let mut repaired: Vec<Def> = Vec::new();
    for def in doc.defs() {
        let out = repair_in(&ctx, &def.body, &names);
        repairs.extend(out.repairs);
        repaired.push(Def::new(def.id, def.ann.clone(), out.exp));
    }
    let doc = Doc::new(repaired).expect("repair preserves definition ids");
    let doc = settle_annotations(doc, &ctx);

    DocMergeOutcome {
        merged: DocVersion::new(doc, names),
        conflicts,
        ours_changes: changes(base, ours),
        theirs_changes: changes(base, theirs),
        repairs,
    }
}

fn settle_annotations(doc: Doc, ctx: &Ctx) -> Doc {
    let defs: Vec<Def> = doc
        .defs()
        .iter()
        .map(|def| {
            if nothing_core::doc::def_is_well_typed(ctx, def) {
                def.clone()
            } else {
                Def::new(def.id, Ty::Hole, def.body.clone())
            }
        })
        .collect();
    Doc::new(defs).expect("annotation settling preserves definition ids")
}

fn within(id: Id, names: &NameTable, c: &Conflict) -> DocConflict {
    DocConflict {
        kind: DocConflictKind::WithinDefinition(c.kind),
        definition: id,
        site: format!("{}: {}", names.display(id), c.site),
        why: c.why.clone(),
        base_text: c.base_text.clone(),
        ours_text: c.ours_text.clone(),
        theirs_text: c.theirs_text.clone(),
    }
}
