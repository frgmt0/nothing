//! Typing context (Phase 1).
//!
//! `Ctx` is a persistent map from [`crate::exp::Id`] to [`crate::ty::Ty`].
//! It backs both `syn` and `ana`: every recursive call into a binder's body
//! extends the context with a fresh binding and passes the *new* context
//! down, leaving the caller's context untouched. Because traversal clones
//! the context constantly, it is built on `im::HashMap`, which shares
//! structure between versions instead of copying on every extend.

use crate::exp::Id;
use crate::ty::Ty;

/// A persistent typing context: `Id -> Ty`. Cheap to clone (it's a
/// structure-sharing persistent map under the hood), so every recursive
/// typing rule that needs to extend the context for a subterm can just
/// call [`Ctx::extend`] and pass the result down without disturbing its
/// own copy.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Ctx {
    bindings: im::HashMap<Id, Ty>,
}

impl Ctx {
    /// The empty context.
    pub fn empty() -> Ctx {
        Ctx {
            bindings: im::HashMap::new(),
        }
    }

    /// Returns a *new* context with `id` bound to `ty`, leaving `self`
    /// unmodified. This is the only way to add a binding — there is no
    /// mutating `insert`.
    pub fn extend(&self, id: Id, ty: Ty) -> Ctx {
        Ctx {
            bindings: self.bindings.update(id, ty),
        }
    }

    /// Looks up the type bound to `id`, if any.
    pub fn lookup(&self, id: &Id) -> Option<Ty> {
        self.bindings.get(id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exp::Id;

    #[test]
    fn empty_context_has_no_bindings() {
        let ctx = Ctx::empty();
        assert_eq!(ctx.lookup(&Id::new(0)), None);
    }

    #[test]
    fn extend_finds_the_new_binding() {
        let ctx = Ctx::empty();
        let x = Id::new(1);
        let extended = ctx.extend(x, Ty::Num);
        assert_eq!(extended.lookup(&x), Some(Ty::Num));
    }

    /// `extend` returns a new context without mutating the original —
    /// verified by looking the id up in the *old* context after extending
    /// and asserting it is still absent.
    #[test]
    fn extend_does_not_mutate_the_original() {
        let original = Ctx::empty();
        let x = Id::new(1);
        let extended = original.extend(x, Ty::Num);

        // The original must not see the new binding...
        assert_eq!(original.lookup(&x), None);
        // ...while the extended context does.
        assert_eq!(extended.lookup(&x), Some(Ty::Num));
    }

    /// Extending an already-bound id in a derived context must not affect
    /// a sibling context derived from the same parent (no shared mutable
    /// state leaking between branches of a persistent structure).
    #[test]
    fn sibling_extensions_do_not_interfere() {
        let base = Ctx::empty().extend(Id::new(1), Ty::Num);
        let left = base.extend(Id::new(2), Ty::Bool);
        let right = base.extend(Id::new(2), Ty::Hole);

        assert_eq!(left.lookup(&Id::new(2)), Some(Ty::Bool));
        assert_eq!(right.lookup(&Id::new(2)), Some(Ty::Hole));
        assert_eq!(base.lookup(&Id::new(2)), None);
        assert_eq!(base.lookup(&Id::new(1)), Some(Ty::Num));
    }

    #[test]
    fn re_extending_shadows_previous_binding_in_new_context_only() {
        let ctx = Ctx::empty().extend(Id::new(1), Ty::Num);
        let shadowed = ctx.extend(Id::new(1), Ty::Bool);

        assert_eq!(ctx.lookup(&Id::new(1)), Some(Ty::Num));
        assert_eq!(shadowed.lookup(&Id::new(1)), Some(Ty::Bool));
    }
}
