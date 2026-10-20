//! Token names as each output spells them.

/// `bg.surface1` → `bgSurface1`, `mode.direct_raw` → `modeDirectRaw`.
pub fn camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper = false;
    for c in name.chars() {
        if matches!(c, '.' | '_' | '-') {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// A palette name as a QML property. QML reads `onX` as a handler for a
/// signal `x`, so `on.accent` becomes `textOnAccent`.
pub fn qml_property(name: &str) -> String {
    match name.strip_prefix("on.") {
        Some(rest) => camel(&format!("text.on.{rest}")),
        None => camel(name),
    }
}

/// `bg.surface1` → `bg-surface1`, `mode.direct_raw` → `mode-direct-raw`,
/// `bodyLarge` → `body-large`.
pub fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        match c {
            '.' | '_' => out.push('-'),
            c if c.is_ascii_uppercase() => {
                out.push('-');
                out.push(c.to_ascii_lowercase());
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: DS-01
    // suite: unit
    #[test]
    fn names_follow_each_language() {
        assert_eq!(camel("bg.surface1"), "bgSurface1");
        assert_eq!(camel("accent.subtle"), "accentSubtle");
        assert_eq!(camel("mode.direct_raw"), "modeDirectRaw");
        assert_eq!(camel("focus"), "focus");
        assert_eq!(qml_property("on.accent"), "textOnAccent");
        assert_eq!(qml_property("bg.base"), "bgBase");
        assert_eq!(kebab("bg.surface1"), "bg-surface1");
        assert_eq!(kebab("mode.direct_raw"), "mode-direct-raw");
        assert_eq!(kebab("bodyLarge"), "body-large");
    }
}
