use proc_macro2::{Ident, Span};

use crate::error::{Error, ErrorKind, Result};

const NAME: &str = "name an item of the bindings";

const KEYWORDS: [&str; 52] = [
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

const NOT_RAW: [&str; 4] = ["Self", "crate", "self", "super"];

/// Returns the Rust name of a field for a Slang name or parameter path: the
/// path in snake case, with `.` replaced by `_` and `_` inserted before an
/// uppercase letter that follows a lowercase letter or a digit.
pub(crate) fn snake_case(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut previous: Option<char> = None;
    for character in path.chars() {
        if character == '.' {
            out.push('_');
        } else if character.is_ascii_uppercase() {
            if previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
            {
                out.push('_');
            }
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
        previous = Some(character);
    }
    out
}

/// Returns the Rust name of a type for a Slang name: the name in upper camel
/// case, with each part between `_` and `.` starting with an uppercase
/// letter.
pub(crate) fn upper_camel_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for part in name.split(['_', '.']) {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(characters);
        }
    }
    out
}

/// Returns the identifier for a Rust name, raw when the name is a keyword.
///
/// `what` names the Slang item that the name comes from, such as
/// ``the kernel `main` ``, for the error.
pub(crate) fn ident(name: &str, what: &str) -> Result<Ident> {
    let mut characters = name.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && name != "_";
    if !valid || NOT_RAW.contains(&name) {
        return Err(
            Error::new(ErrorKind::NameConflict, NAME).with_detail(format!(
                "{what} becomes the Rust name `{name}`, which is not a Rust identifier"
            )),
        );
    }
    if KEYWORDS.contains(&name) {
        Ok(Ident::new_raw(name, Span::call_site()))
    } else {
        Ok(Ident::new(name, Span::call_site()))
    }
}

/// The Rust names already taken in one scope of the generated code, each
/// with the Slang item that took it, so that a second item with the same
/// name is reported with both.
#[derive(Debug, Default)]
pub(crate) struct Scope {
    taken: Vec<(String, String)>,
}

impl Scope {
    /// Returns a scope in which each of `reserved` is taken by `owner`.
    pub(crate) fn reserving(reserved: &[&str], owner: &str) -> Self {
        Self {
            taken: reserved
                .iter()
                .map(|name| ((*name).to_owned(), owner.to_owned()))
                .collect(),
        }
    }

    /// Takes `name` for `what`, the Slang item it comes from, and returns
    /// its identifier, or an error that names both items when another item
    /// took it first.
    pub(crate) fn take(&mut self, name: String, what: String) -> Result<Ident> {
        if let Some((_, owner)) = self.taken.iter().find(|(taken, _)| *taken == name) {
            return Err(
                Error::new(ErrorKind::NameConflict, NAME).with_detail(format!(
                    "{owner} and {what} both become the Rust name `{name}`"
                )),
            );
        }
        let ident = ident(&name, &what)?;
        self.taken.push((name, what));
        Ok(ident)
    }
}

#[cfg(test)]
mod tests {
    use super::{Scope, ident, snake_case, upper_camel_case};
    use crate::ErrorKind;

    #[test]
    fn snake_case_splits_at_an_uppercase_letter_after_a_lowercase_letter_or_a_digit() {
        assert_eq!(snake_case("count"), "count");
        assert_eq!(snake_case("thresholdValue"), "threshold_value");
        assert_eq!(snake_case("vector3Length"), "vector3_length");
        assert_eq!(snake_case("XYZ"), "xyz");
        assert_eq!(snake_case("_Tail"), "_tail");
        assert_eq!(snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn snake_case_replaces_each_dot_of_a_path() {
        assert_eq!(snake_case("params.offset"), "params_offset");
        assert_eq!(snake_case("Settings.Scale"), "settings_scale");
        assert_eq!(snake_case("cb.innerValue"), "cb_inner_value");
    }

    #[test]
    fn upper_camel_case_starts_each_part_with_an_uppercase_letter() {
        assert_eq!(upper_camel_case("saxpy"), "Saxpy");
        assert_eq!(upper_camel_case("computeMain"), "ComputeMain");
        assert_eq!(upper_camel_case("compute_main"), "ComputeMain");
        assert_eq!(upper_camel_case("__scale__twice"), "ScaleTwice");
        assert_eq!(upper_camel_case("CSMain"), "CSMain");
    }

    #[test]
    fn ident_makes_a_keyword_a_raw_identifier() -> crate::Result<()> {
        assert_eq!(ident("type", "the parameter `type`")?.to_string(), "r#type");
        assert_eq!(ident("gen", "the parameter `gen`")?.to_string(), "r#gen");
        assert_eq!(
            ident("scale", "the parameter `scale`")?.to_string(),
            "scale"
        );
        Ok(())
    }

    #[test]
    fn ident_rejects_a_name_that_is_no_identifier_even_when_raw() {
        for name in [
            "self",
            "Self",
            "super",
            "crate",
            "_",
            "",
            "1st",
            "ma\u{df}e",
        ] {
            let error = ident(name, "the kernel `k`").err();
            assert_eq!(
                error.as_ref().map(crate::Error::kind),
                Some(ErrorKind::NameConflict),
                "{name}"
            );
            assert_eq!(
                error.map(|error| error.to_string()),
                Some(format!(
                    "cannot name an item of the bindings: the kernel `k` becomes the Rust name `{name}`, which is not a Rust identifier"
                ))
            );
        }
    }

    #[test]
    fn scope_reports_a_second_item_with_the_same_name_with_both_items() -> crate::Result<()> {
        let mut scope = Scope::reserving(&["load", "module"], "a method of `Shader`");
        scope.take("saxpy".to_owned(), "the kernel `saxpy`".to_owned())?;
        let error = scope
            .take("saxpy".to_owned(), "the kernel `Saxpy`".to_owned())
            .err();
        assert_eq!(
            error.map(|error| (error.kind(), error.to_string())),
            Some((
                ErrorKind::NameConflict,
                "cannot name an item of the bindings: the kernel `saxpy` and the kernel `Saxpy` both become the Rust name `saxpy`".to_owned()
            ))
        );
        let reserved = scope
            .take("module".to_owned(), "the kernel `module`".to_owned())
            .err()
            .map(|error| error.to_string());
        assert_eq!(
            reserved.as_deref(),
            Some(
                "cannot name an item of the bindings: a method of `Shader` and the kernel `module` both become the Rust name `module`"
            )
        );
        Ok(())
    }
}
