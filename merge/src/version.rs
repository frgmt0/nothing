use nothing_action::act::EditState;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_core::render::render;
use nothing_core::typing::is_well_typed;

#[derive(Clone, PartialEq, Debug)]
pub struct Version {
    pub exp: Exp,
    pub names: NameTable,
}

impl Version {
    pub fn new(exp: Exp, names: NameTable) -> Version {
        Version { exp, names }
    }

    pub fn from_state(state: &EditState) -> Version {
        Version {
            exp: state.exp(),
            names: state.names().clone(),
        }
    }

    pub fn render(&self) -> String {
        render(&self.exp, &self.names)
    }

    pub fn is_well_typed(&self) -> bool {
        is_well_typed(&self.exp)
    }
}
