use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_core::render::render;

use crate::chain;
use crate::version::Version;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    pub indent: usize,
    pub blank_between_bindings: bool,
}

pub const CANONICAL: Style = Style {
    indent: 2,
    blank_between_bindings: false,
};

pub const WIDE: Style = Style {
    indent: 4,
    blank_between_bindings: false,
};

pub const AIRY: Style = Style {
    indent: 2,
    blank_between_bindings: true,
};

pub fn to_text(version: &Version, style: Style) -> String {
    let mut lines = Vec::new();
    emit(&version.exp, &version.names, style, 0, &mut lines);
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn pad(style: Style, depth: usize) -> String {
    " ".repeat(style.indent * depth)
}

fn breaks(exp: &Exp) -> bool {
    matches!(
        exp,
        Exp::Let(..) | Exp::Lam(..) | Exp::If(..) | Exp::Pair(..)
    )
}

fn emit(exp: &Exp, names: &NameTable, style: Style, depth: usize, out: &mut Vec<String>) {
    if !breaks(exp) {
        out.push(format!("{}{}", pad(style, depth), render(exp, names)));
        return;
    }
    match exp {
        Exp::Let(..) => {
            let flat = chain::chain_of(exp);
            for binding in &flat.bindings {
                out.push(format!(
                    "{}let {} =",
                    pad(style, depth),
                    names.display(binding.id)
                ));
                emit(&binding.bound, names, style, depth + 1, out);
                out.push(format!("{}in", pad(style, depth)));
                if style.blank_between_bindings {
                    out.push(String::new());
                }
            }
            emit(&flat.tail, names, style, depth, out);
        }
        Exp::Lam(id, ty, body) => {
            out.push(format!(
                "{}λ{}:{}.",
                pad(style, depth),
                names.display(*id),
                ty
            ));
            emit(body, names, style, depth + 1, out);
        }
        Exp::If(cond, then, else_) => {
            out.push(format!("{}if", pad(style, depth)));
            emit(cond, names, style, depth + 1, out);
            out.push(format!("{}then", pad(style, depth)));
            emit(then, names, style, depth + 1, out);
            out.push(format!("{}else", pad(style, depth)));
            emit(else_, names, style, depth + 1, out);
        }
        Exp::Pair(fst, snd) => {
            out.push(format!("{}(", pad(style, depth)));
            emit(fst, names, style, depth + 1, out);
            out.push(format!("{},", pad(style, depth)));
            emit(snd, names, style, depth + 1, out);
            out.push(format!("{})", pad(style, depth)));
        }
        _ => out.push(format!("{}{}", pad(style, depth), render(exp, names))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::examples;

    fn version() -> Version {
        Version::new(examples::square_and_compare(), examples::names())
    }

    #[test]
    fn the_projection_is_multi_line_and_deterministic() {
        let text = to_text(&version(), CANONICAL);
        assert!(text.lines().count() > 3, "{text}");
        assert_eq!(text, to_text(&version(), CANONICAL));
    }

    #[test]
    fn reformatting_changes_the_text_but_not_the_content() {
        let a = to_text(&version(), CANONICAL);
        let b = to_text(&version(), WIDE);
        assert_ne!(a, b);
        assert_eq!(normalise(&a), normalise(&b));
    }

    #[test]
    fn a_blank_line_style_also_normalises_to_the_same_content() {
        let a = to_text(&version(), CANONICAL);
        let c = to_text(&version(), AIRY);
        assert_ne!(a, c);
        assert_eq!(normalise(&a), normalise(&c));
    }
}
