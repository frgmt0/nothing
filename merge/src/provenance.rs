use nothing_action::act::EditState;
use nothing_action::log::{AuthorId, LogEntry};

use crate::diff::diff;
use crate::ops::{Operation, regions_overlap};
use crate::version::Version;

#[derive(Clone, PartialEq, Debug)]
pub struct AttributedOp {
    pub op: Operation,
    pub author: Option<AuthorId>,
    pub timestamp: Option<u64>,
    pub entry: Option<usize>,
}

impl AttributedOp {
    pub fn by(&self, author: AuthorId) -> bool {
        self.author == Some(author)
    }

    pub fn unattributed(&self) -> bool {
        self.author.is_none()
    }

    pub fn describe(&self, base: &Version) -> String {
        let who = match self.author {
            Some(author) => format!("author {}", author.0),
            None => "an unattributed edit".to_string(),
        };
        format!("{who} {}", self.op.describe(&base.exp, &base.names))
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Attributed {
    pub base: Version,
    pub head: Version,
    pub ops: Vec<AttributedOp>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Filter {
    All,
    Only(Vec<AuthorId>),
    Excluding(Vec<AuthorId>),
    Unattributed,
}

impl Filter {
    pub fn only(author: AuthorId) -> Filter {
        Filter::Only(vec![author])
    }

    pub fn excluding(author: AuthorId) -> Filter {
        Filter::Excluding(vec![author])
    }

    pub fn admits(&self, op: &AttributedOp) -> bool {
        match self {
            Filter::All => true,
            Filter::Only(authors) => op.author.is_some_and(|a| authors.contains(&a)),
            Filter::Excluding(authors) => !op.author.is_some_and(|a| authors.contains(&a)),
            Filter::Unattributed => op.author.is_none(),
        }
    }
}

impl Attributed {
    pub fn filter(&self, filter: &Filter) -> Vec<Operation> {
        self.ops
            .iter()
            .filter(|op| filter.admits(op))
            .map(|op| op.op.clone())
            .collect()
    }

    pub fn filter_attributed(&self, filter: &Filter) -> Vec<AttributedOp> {
        self.ops
            .iter()
            .filter(|op| filter.admits(op))
            .cloned()
            .collect()
    }

    pub fn by(&self, author: AuthorId) -> Vec<Operation> {
        self.filter(&Filter::only(author))
    }

    pub fn not_by(&self, author: AuthorId) -> Vec<Operation> {
        self.filter(&Filter::excluding(author))
    }

    pub fn authors(&self) -> Vec<AuthorId> {
        let mut out: Vec<AuthorId> = Vec::new();
        for author in self.ops.iter().filter_map(|op| op.author) {
            if !out.contains(&author) {
                out.push(author);
            }
        }
        out.sort();
        out
    }

    pub fn ops(&self) -> Vec<Operation> {
        self.ops.iter().map(|op| op.op.clone()).collect()
    }
}

struct Step {
    author: AuthorId,
    timestamp: u64,
    index: usize,
    ops: Vec<Operation>,
}

fn overlaps(a: &Operation, b: &Operation) -> bool {
    a.footprint().iter().any(|left| {
        b.footprint()
            .iter()
            .any(|right| regions_overlap(left, right))
    })
}

pub fn attribute_from(base: &EditState, entries: &[LogEntry]) -> Attributed {
    let base_version = Version::from_state(base);
    let mut state = base.clone();
    let mut steps: Vec<Step> = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let before = Version::from_state(&state);
        if !state.apply_mut(entry.action.clone()) {
            continue;
        }
        let after = Version::from_state(&state);
        steps.push(Step {
            author: entry.author,
            timestamp: entry.timestamp,
            index,
            ops: diff(&before, &after),
        });
    }

    let head = Version::from_state(&state);
    let ops = diff(&base_version, &head)
        .into_iter()
        .map(|op| {
            let source = steps
                .iter()
                .rev()
                .find(|step| step.ops.iter().any(|candidate| overlaps(candidate, &op)));
            AttributedOp {
                author: source.map(|s| s.author),
                timestamp: source.map(|s| s.timestamp),
                entry: source.map(|s| s.index),
                op,
            }
        })
        .collect();

    Attributed {
        base: base_version,
        head,
        ops,
    }
}

pub fn attribute(base: &Version, entries: &[LogEntry]) -> Attributed {
    attribute_from(
        &EditState::with_names(base.exp.clone(), base.names.clone()),
        entries,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::act::Action;
    use nothing_action::log::{ActionLog, EditSession};
    use nothing_action::script::parse_step;

    const HUMAN: AuthorId = AuthorId::new(1);
    const AGENT: AuthorId = AuthorId::new(2);

    struct Recorder {
        session: EditSession,
        clock: u64,
    }

    impl Recorder {
        fn new() -> Recorder {
            Recorder {
                session: EditSession::new(),
                clock: 0,
            }
        }

        fn step(&mut self, text: &str, author: AuthorId) {
            let step = parse_step(text).unwrap_or_else(|e| panic!("`{text}`: {e}"));
            let action = step
                .resolve(self.session.state())
                .unwrap_or_else(|e| panic!("`{text}`: {e}"));
            self.clock += 1;
            assert!(
                self.session.apply(action, self.clock, author),
                "`{text}` did not apply to `{}`",
                self.session.state().render()
            );
        }

        fn steps(&mut self, script: &str, author: AuthorId) {
            for line in script.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    self.step(line, author);
                }
            }
        }

        fn snapshot(&self) -> (EditState, usize) {
            let at = self.session.log().len();
            (self.session.log().replay_prefix(at), at)
        }

        fn since(&self, at: usize) -> Vec<LogEntry> {
            self.session.log().entries()[at..].to_vec()
        }

        fn render(&self) -> String {
            self.session.state().render()
        }
    }

    fn mixed() -> (EditState, Vec<LogEntry>, String) {
        let mut r = Recorder::new();
        r.steps(
            "construct-lam
             move-parent
             rename n
             set-ann Num
             move-child 0
             construct-var n
             construct-binop add
             construct-num 1
             move-parent
             construct-pair
             construct-var n
             construct-binop mul
             construct-num 2
             move-parent
             move-parent",
            HUMAN,
        );
        assert_eq!(r.render(), "λn:Num. (n + 1, n * 2)");
        let (base, at) = r.snapshot();

        r.steps(
            "move-child 0
             move-child 1
             delete
             construct-num 10
             move-parent
             move-parent",
            HUMAN,
        );
        r.steps(
            "move-child 1
             move-child 1
             delete
             construct-num 3
             move-parent
             move-parent
             move-parent
             rename k",
            AGENT,
        );
        assert_eq!(r.render(), "λk:Num. (k + 10, k * 3)");
        (base, r.since(at), r.render())
    }

    #[test]
    fn a_mixed_authorship_diff_attributes_every_operation() {
        let (base, entries, head) = mixed();
        let attributed = attribute_from(&base, &entries);
        assert_eq!(attributed.head.render(), head);
        assert!(!attributed.ops.is_empty());
        for op in &attributed.ops {
            assert!(op.author.is_some(), "{:?} was not attributed", op.op.kind());
        }
        assert_eq!(attributed.authors(), vec![HUMAN, AGENT]);
    }

    #[test]
    fn the_human_filter_keeps_only_the_human_edit() {
        let (base, entries, _) = mixed();
        let attributed = attribute_from(&base, &entries);
        let human = attributed.by(HUMAN);
        assert_eq!(human.len(), 1, "{human:?}");
        assert!(
            matches!(&human[0], Operation::Replace { to, .. } if *to == nothing_core::exp::Exp::num(10)),
            "{human:?}"
        );
    }

    #[test]
    fn the_agent_filter_keeps_only_the_agent_edits() {
        let (base, entries, _) = mixed();
        let attributed = attribute_from(&base, &entries);
        let agent = attributed.by(AGENT);
        assert_eq!(agent.len(), 2, "{agent:?}");
        assert!(
            agent
                .iter()
                .any(|op| matches!(op, Operation::Rename { to, .. } if to == "k")),
            "{agent:?}"
        );
        assert!(
            agent.iter().any(
                |op| matches!(op, Operation::Replace { to, .. } if *to == nothing_core::exp::Exp::num(3))
            ),
            "{agent:?}"
        );
    }

    #[test]
    fn the_two_filters_partition_the_diff() {
        let (base, entries, _) = mixed();
        let attributed = attribute_from(&base, &entries);
        assert_eq!(
            attributed.by(HUMAN).len() + attributed.by(AGENT).len(),
            attributed.ops.len()
        );
        assert_eq!(
            attributed.not_by(AGENT),
            attributed.by(HUMAN),
            "excluding the agent is the same as keeping the human here"
        );
        assert_eq!(attributed.filter(&Filter::All), attributed.ops());
        assert!(attributed.filter(&Filter::Unattributed).is_empty());
    }

    #[test]
    fn a_filtered_operation_set_still_applies_to_the_base() {
        use nothing_core::typing::is_well_typed;
        let (base, entries, _) = mixed();
        let attributed = attribute_from(&base, &entries);
        for filter in [Filter::only(HUMAN), Filter::only(AGENT), Filter::All] {
            let ops = attributed.filter(&filter);
            let applied = crate::apply::apply_all(&attributed.base, &ops);
            assert!(
                applied.dropped.is_empty(),
                "{filter:?} dropped {:?}",
                applied.dropped
            );
            assert!(
                is_well_typed(&applied.version.exp),
                "{filter:?} produced an ill-typed program: {}",
                applied.version.render()
            );
        }
    }

    #[test]
    fn applying_the_human_half_reaches_the_program_without_the_agent_edits() {
        let (base, entries, _) = mixed();
        let attributed = attribute_from(&base, &entries);
        let applied = crate::apply::apply_all(&attributed.base, &attributed.by(HUMAN));
        assert_eq!(applied.version.render(), "λn:Num. (n + 10, n * 2)");
    }

    #[test]
    fn applying_the_agent_half_reaches_the_program_without_the_human_edits() {
        let (base, entries, _) = mixed();
        let attributed = attribute_from(&base, &entries);
        let applied = crate::apply::apply_all(&attributed.base, &attributed.by(AGENT));
        assert_eq!(applied.version.render(), "λk:Num. (k + 1, k * 3)");
    }

    #[test]
    fn a_single_author_log_attributes_everything_to_that_author() {
        let mut r = Recorder::new();
        let (base, at) = r.snapshot();
        r.steps(
            "construct-num 1\nconstruct-binop add\nconstruct-num 2",
            AGENT,
        );
        let attributed = attribute_from(&base, &r.since(at));
        assert!(!attributed.ops.is_empty());
        assert!(attributed.ops.iter().all(|op| op.by(AGENT)));
        assert!(attributed.by(HUMAN).is_empty());
    }

    #[test]
    fn an_empty_log_produces_an_empty_diff() {
        let base = Version::new(
            nothing_core::examples::square_and_compare(),
            nothing_core::examples::names(),
        );
        let attributed = attribute(&base, &[]);
        assert!(attributed.ops.is_empty());
        assert_eq!(attributed.head.exp, base.exp);
    }

    #[test]
    fn a_log_entry_that_no_longer_applies_is_skipped_rather_than_panicking() {
        let base = Version::new(
            nothing_core::exp::Exp::num(1),
            nothing_core::names::NameTable::new(),
        );
        let mut log = ActionLog::new();
        log.append(Action::MoveParent, 1, AGENT);
        log.append(Action::ConstructBinOp(nothing_core::exp::Op::Add), 2, AGENT);
        let attributed = attribute(&base, log.entries());
        assert_eq!(attributed.head.render(), "1 + ⦇⦈");
        assert!(attributed.ops.iter().all(|op| op.by(AGENT)));
    }

    #[test]
    fn attribution_carries_the_timestamp_and_the_entry_index() {
        let (base, entries, _) = mixed();
        let attributed = attribute_from(&base, &entries);
        for op in &attributed.ops {
            let index = op.entry.expect("an attributed op names its entry");
            assert_eq!(op.timestamp, Some(entries[index].timestamp));
            assert_eq!(op.author, Some(entries[index].author));
        }
    }
}
