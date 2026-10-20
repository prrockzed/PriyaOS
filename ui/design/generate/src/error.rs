//! Everything that can be wrong with a token file, said plainly enough to
//! fix it.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("{} can't be read: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the token file has a mistake: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the token file breaks tokens.schema.json:\n{}", .0.join("\n"))]
    Schema(Vec<String>),
    #[error("theme `{0}` is required")]
    MissingTheme(&'static str),
    #[error("theme `{theme}` inherits from `{inherits}`, which isn't a theme")]
    UnknownParent { theme: String, inherits: String },
    #[error("themes inherit from each other in a circle: {}", .0.join(" → "))]
    InheritanceCycle(Vec<String>),
    #[error("`{token}` has a value for `{theme}`, which isn't a theme")]
    ValueForUnknownTheme { token: String, theme: String },
    #[error("`{token}` has no value for theme `{theme}`")]
    NoValue { token: String, theme: String },
    #[error("`{token}` in `{theme}` is `{value}`, which isn't #RRGGBB, #RRGGBBAA or a reference")]
    BadColor {
        token: String,
        theme: String,
        value: String,
    },
    #[error("`{token}` in `{theme}` refers to `{reference}`, which doesn't exist")]
    UnknownReference {
        token: String,
        theme: String,
        reference: String,
    },
    #[error("references go round in a circle in `{theme}`: {}", .chain.join(" → "))]
    ReferenceCycle { theme: String, chain: Vec<String> },
    #[error("the default accent `{0}` isn't one of the accent options")]
    UnknownAccent(String),
    #[error("a contrast pair names `{name}`, which isn't a colour in `{theme}`")]
    UnknownPairColor { name: String, theme: String },
    #[error("bg.base is translucent in `{0}`; the page must be opaque")]
    TranslucentBase(String),
}
