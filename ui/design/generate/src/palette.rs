//! Resolving the token file into concrete colours for each theme: values a
//! theme doesn't give come from the theme it inherits from, and
//! references (`{color.info}`, `{accent.marigold.on.accent}`) are followed
//! within the same theme.

use std::collections::{BTreeMap, BTreeSet};

use crate::color::Color;
use crate::error::TokenError;
use crate::model::{ACCENT_ROLES, PerTheme, Tokens};

/// Every theme's colours and elevation.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    /// Colours by palette name: the colour tokens (`text.primary`), the
    /// default accent's `accent`, `accent.subtle` and `on.accent`,
    /// `mode.<name>` and `focus`. Every theme has every name.
    pub themes: BTreeMap<String, BTreeMap<String, Color>>,
    /// Elevation by theme, then level.
    pub elevation: BTreeMap<String, BTreeMap<String, Raised>>,
}

/// How one elevation level looks in one theme.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Raised {
    /// Layers, drawn together.
    pub shadows: Vec<DropShadow>,
    pub border: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropShadow {
    pub y: f64,
    pub blur: f64,
    pub color: Color,
}

impl Palette {
    pub fn resolve(tokens: &Tokens) -> Result<Self, TokenError> {
        check_themes(tokens)?;
        if !tokens.accent.options.contains_key(&tokens.accent.default) {
            return Err(TokenError::UnknownAccent(tokens.accent.default.clone()));
        }
        let mut themes = BTreeMap::new();
        let mut elevation = BTreeMap::new();
        for theme in tokens.themes.keys() {
            let mut resolver = Resolver {
                tokens,
                theme,
                done: BTreeMap::new(),
                stack: Vec::new(),
            };
            let mut colors = BTreeMap::new();
            for name in tokens.color.keys() {
                let path = format!("color.{name}");
                colors.insert(name.clone(), resolver.color(&path, &path)?);
            }
            for role in ACCENT_ROLES {
                let path = format!("color.{role}");
                colors.insert(role.to_owned(), resolver.color(&path, &path)?);
            }
            // The other accents aren't in the palette yet (5.11), but they
            // must already be whole and valid.
            for (option, accent) in &tokens.accent.options {
                for (role, _) in accent.roles() {
                    let path = format!("accent.{option}.{role}");
                    resolver.color(&path, &path)?;
                }
            }
            for (mode, value) in tokens.mode.all() {
                let name = format!("mode.{mode}");
                colors.insert(name.clone(), resolver.value(&name, value)?);
            }
            colors.insert(
                "focus".into(),
                resolver.value("focus", &tokens.focus.color)?,
            );

            let mut levels = BTreeMap::new();
            for (level, per_theme) in &tokens.elevation {
                let token = format!("elevation.{level}");
                let given = pick(tokens, per_theme, theme, &token)?;
                let mut raised = Raised::default();
                for shadow in &given.shadow {
                    raised.shadows.push(DropShadow {
                        y: shadow.y,
                        blur: shadow.blur,
                        color: resolver.value(&format!("{token}.shadow"), &shadow.color)?,
                    });
                }
                if let Some(border) = &given.border {
                    raised.border = Some(resolver.value(&format!("{token}.border"), border)?);
                }
                levels.insert(level.clone(), raised);
            }
            themes.insert(theme.clone(), colors);
            elevation.insert(theme.clone(), levels);
        }
        Ok(Self { themes, elevation })
    }

    /// Every palette name, sorted; each theme has them all.
    pub fn names(&self) -> BTreeSet<&str> {
        self.themes
            .values()
            .flat_map(|colors| colors.keys().map(String::as_str))
            .collect()
    }

    /// The colour `name` in `theme`.
    pub fn get(&self, theme: &str, name: &str) -> Option<Color> {
        self.themes.get(theme)?.get(name).copied()
    }
}

/// light and dark exist; every parent exists; no theme inherits from
/// itself; values are only given for real themes.
fn check_themes(tokens: &Tokens) -> Result<(), TokenError> {
    for required in crate::model::REQUIRED_THEMES {
        if !tokens.themes.contains_key(required) {
            return Err(TokenError::MissingTheme(required));
        }
    }
    for (theme, info) in &tokens.themes {
        let mut chain = vec![theme.clone()];
        let mut parent = info.inherits.as_ref();
        while let Some(name) = parent {
            let Some(next) = tokens.themes.get(name) else {
                return Err(TokenError::UnknownParent {
                    theme: chain.last().cloned().unwrap_or_default(),
                    inherits: name.clone(),
                });
            };
            if chain.contains(name) {
                chain.push(name.clone());
                return Err(TokenError::InheritanceCycle(chain));
            }
            chain.push(name.clone());
            parent = next.inherits.as_ref();
        }
    }
    let mut per_theme: Vec<(String, Vec<&String>)> = tokens
        .color
        .iter()
        .map(|(name, values)| (format!("color.{name}"), values.keys().collect()))
        .collect();
    for (option, accent) in &tokens.accent.options {
        for (role, values) in accent.roles() {
            per_theme.push((format!("accent.{option}.{role}"), values.keys().collect()));
        }
    }
    for (level, values) in &tokens.elevation {
        per_theme.push((format!("elevation.{level}"), values.keys().collect()));
    }
    for (token, themes) in per_theme {
        if let Some(theme) = themes.into_iter().find(|t| !tokens.themes.contains_key(*t)) {
            return Err(TokenError::ValueForUnknownTheme {
                token,
                theme: theme.clone(),
            });
        }
    }
    Ok(())
}

/// The value `theme` has, or the nearest theme it inherits from.
fn pick<'t, T>(
    tokens: &Tokens,
    values: &'t PerTheme<T>,
    theme: &str,
    token: &str,
) -> Result<&'t T, TokenError> {
    let mut current = Some(theme);
    while let Some(name) = current {
        if let Some(value) = values.get(name) {
            return Ok(value);
        }
        current = tokens.themes.get(name).and_then(|t| t.inherits.as_deref());
    }
    Err(TokenError::NoValue {
        token: token.to_owned(),
        theme: theme.to_owned(),
    })
}

/// Follows references in one theme, remembering what it has resolved.
struct Resolver<'a> {
    tokens: &'a Tokens,
    theme: &'a str,
    done: BTreeMap<String, Color>,
    /// The references being followed, to catch circles.
    stack: Vec<String>,
}

impl<'a> Resolver<'a> {
    /// The colour of a referable token, `color.<name>` or
    /// `accent.<option>.<role>`, which `referrer` asked for.
    fn color(&mut self, path: &str, referrer: &str) -> Result<Color, TokenError> {
        if let Some(color) = self.done.get(path) {
            return Ok(*color);
        }
        if let Some(start) = self.stack.iter().position(|p| p == path) {
            // Only the circle itself, not the way into it.
            let mut chain = self.stack[start..].to_vec();
            chain.push(path.to_owned());
            return Err(TokenError::ReferenceCycle {
                theme: self.theme.to_owned(),
                chain,
            });
        }
        let Some(values) = self.values(path) else {
            return Err(TokenError::UnknownReference {
                token: referrer.to_owned(),
                theme: self.theme.to_owned(),
                reference: path.to_owned(),
            });
        };
        let raw = pick(self.tokens, values, self.theme, path)?;
        self.stack.push(path.to_owned());
        let color = self.value(path, raw);
        self.stack.pop();
        let color = color?;
        self.done.insert(path.to_owned(), color);
        Ok(color)
    }

    /// `value`, written for `token`, as a colour.
    fn value(&mut self, token: &str, value: &str) -> Result<Color, TokenError> {
        if let Some(reference) = value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) {
            return self.color(reference, token);
        }
        Color::parse(value).ok_or_else(|| TokenError::BadColor {
            token: token.to_owned(),
            theme: self.theme.to_owned(),
            value: value.to_owned(),
        })
    }

    /// A referable token's values, if it exists. `color.accent` and the
    /// other accent roles are the default accent's.
    fn values(&self, path: &str) -> Option<&'a PerTheme> {
        let tokens = self.tokens;
        let (option, role) = if let Some(name) = path.strip_prefix("color.") {
            if let Some(values) = tokens.color.get(name) {
                return Some(values);
            }
            (tokens.accent.default.as_str(), name)
        } else {
            path.strip_prefix("accent.")?.split_once('.')?
        };
        tokens
            .accent
            .options
            .get(option)?
            .roles()
            .into_iter()
            .find(|(name, _)| *name == role)
            .map(|(_, values)| values)
    }
}
