use crate::exp::Id;
use crate::ty::Ty;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Ctx {
    bindings: im::HashMap<Id, Ty>,
}

impl Ctx {
    pub fn empty() -> Ctx {
        Ctx {
            bindings: im::HashMap::new(),
        }
    }

    pub fn extend(&self, id: Id, ty: Ty) -> Ctx {
        Ctx {
            bindings: self.bindings.update(id, ty),
        }
    }

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
        assert_eq!(ctx.lookup(&Id::from_u128(0)), None);
    }

    #[test]
    fn extend_finds_the_new_binding() {
        let ctx = Ctx::empty();
        let x = Id::from_u128(1);
        let extended = ctx.extend(x, Ty::Num);
        assert_eq!(extended.lookup(&x), Some(Ty::Num));
    }

    #[test]
    fn extend_does_not_mutate_the_original() {
        let original = Ctx::empty();
        let x = Id::from_u128(1);
        let extended = original.extend(x, Ty::Num);

        assert_eq!(original.lookup(&x), None);

        assert_eq!(extended.lookup(&x), Some(Ty::Num));
    }

    #[test]
    fn sibling_extensions_do_not_interfere() {
        let base = Ctx::empty().extend(Id::from_u128(1), Ty::Num);
        let left = base.extend(Id::from_u128(2), Ty::Bool);
        let right = base.extend(Id::from_u128(2), Ty::Hole);

        assert_eq!(left.lookup(&Id::from_u128(2)), Some(Ty::Bool));
        assert_eq!(right.lookup(&Id::from_u128(2)), Some(Ty::Hole));
        assert_eq!(base.lookup(&Id::from_u128(2)), None);
        assert_eq!(base.lookup(&Id::from_u128(1)), Some(Ty::Num));
    }

    #[test]
    fn re_extending_shadows_previous_binding_in_new_context_only() {
        let ctx = Ctx::empty().extend(Id::from_u128(1), Ty::Num);
        let shadowed = ctx.extend(Id::from_u128(1), Ty::Bool);

        assert_eq!(ctx.lookup(&Id::from_u128(1)), Some(Ty::Num));
        assert_eq!(shadowed.lookup(&Id::from_u128(1)), Some(Ty::Bool));
    }
}
