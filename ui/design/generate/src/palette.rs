//! Resolving the token file into concrete colours for each theme: values a
//! theme doesn't give come from the theme it inherits from, and
//! references (`{color.info}`, `{accent.marigold.on.accent}`) are followed
//! within the same theme.

use std::collections::BTreeMap;

use crate::color::Color;
use crate::error::TokenError;
use crate::model::{PerTheme, Tokens};

/// Every theme's colours by palette name: the colour tokens
/// (`text.primary`), `mode.<name>`, `focus`, and the elevation colours
/// (`elevation.<level>.shadow`, `elevation.<level>.border`).
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub themes: BTreeMap<String, BTreeMap<String, Color>>,
}

impl Palette {
    pub fn resolve(tokens: &Tokens) -> Result<Self, TokenError> {
        check_themes(tokens)?;
        if !tokens.accent.options.contains_key(&tokens.accent.default) {
            return Err(TokenError::UnknownAccent(tokens.accent.default.clone()));
        }
        let mut themes = BTreeMap::new();
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
            for (mode, value) in tokens.mode.all() {
                let name = format!("mode.{mode}");
                colors.insert(name.clone(), resolver.value(&name, value)?);
            }
            colors.insert(
                "focus".into(),
                resolver.value("focus", &tokens.focus.color)?,
            );
            for (level, per_theme) in &tokens.elevation {
                let token = format!("elevation.{level}");
                let elevation = pick(tokens, per_theme, theme, &token)?;
                if let Some(shadow) = &elevation.shadow {
                    let name = format!("{token}.shadow");
                    colors.insert(name.clone(), resolver.value(&name, &shadow.color)?);
                }
                if let Some(border) = &elevation.border {
                    let name = format!("{token}.border");
                    colors.insert(name.clone(), resolver.value(&name, border)?);
                }
            }
            themes.insert(theme.clone(), colors);
        }
        Ok(Self { themes })
    }

    /// The colour `name` in `theme`.
    pub fn get(&self, theme: &str, name: &str) -> Option<Color> {
        self.themes.get(theme)?.get(name).copied()
    }
}

/// light and dark exist; every parent exists; no theme inherits from
/// itself; values are only given for real themes.
fn check_themes(tokens: &Tokens) -> Result<(), TokenError> {
    for required in ["light", "dark"] {
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
        if self.stack.iter().any(|p| p == path) {
            let mut chain = self.stack.clone();
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

    /// A referable token's values, if it exists.
    fn values(&self, path: &str) -> Option<&'a PerTheme> {
        let tokens = self.tokens;
        if let Some(name) = path.strip_prefix("color.") {
            return tokens.color.get(name);
        }
        let (option, role) = path.strip_prefix("accent.")?.split_once('.')?;
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
