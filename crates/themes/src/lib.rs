//! Wren themes: own minimal theme model on GPUI-CE.

use std::collections::BTreeMap;

use gpui::{App, Global, Hsla};
use serde::Deserialize;

/// Light or dark appearance of a theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

/// The colour palette of a theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    pub background: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted_text: Hsla,
    pub accent: Hsla,
    pub accent_text: Hsla,
    pub danger: Hsla,
}

/// A single theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub appearance: Appearance,
    pub background: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted_text: Hsla,
    pub accent: Hsla,
    pub accent_text: Hsla,
    pub danger: Hsla,
    pub colors: ThemeColors,
}

impl Theme {
    pub fn colors(&self) -> ThemeColors {
        self.colors
    }
}

/// A family of themes loaded from one JSON file.
#[derive(Debug, Clone)]
pub struct ThemeFamily {
    pub name: String,
    pub author: String,
    pub themes: Vec<Theme>,
}

/// Why a theme name could not be honoured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeWarning {
    NotFound(String),
}

impl std::fmt::Display for ThemeWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(n) => write!(f, "theme not found: {n}"),
        }
    }
}

/// Error reported when theme JSON is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError {
    pub message: String,
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ThemeError {}

/// GPUI global holding the active theme.
#[derive(Clone)]
pub struct ActiveTheme {
    pub theme: Theme,
}

impl std::ops::Deref for ActiveTheme {
    type Target = Theme;
    fn deref(&self) -> &Self::Target {
        &self.theme
    }
}

impl Global for ActiveTheme {}

/// A registry of themes.
#[derive(Clone, Default)]
pub struct ThemeRegistry {
    themes: BTreeMap<String, Theme>,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_builtins() -> Self {
        let mut r = Self::new();
        for fam in BUILTIN_FAMILIES {
            let _ = r.load_family_str(fam);
        }
        r
    }

    pub fn load_family_str(&mut self, json: &str) -> Result<(), ThemeError> {
        let fam = parse_family(json)?;
        for t in fam.themes {
            self.themes.insert(t.name.clone(), t);
        }
        Ok(())
    }

    pub fn load_family(&mut self, family: &ThemeFamily) {
        for t in &family.themes {
            self.themes.insert(t.name.clone(), t.clone());
        }
    }

    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    pub fn resolve(&self, name: &str) -> (Theme, Option<ThemeWarning>) {
        resolve(self, name)
    }

    pub fn fallback(&self) -> Theme {
        default_theme()
    }
}

pub fn resolve(registry: &ThemeRegistry, name: &str) -> (Theme, Option<ThemeWarning>) {
    if let Some(t) = registry.get(name) {
        return (t.clone(), None);
    }
    let fb = registry
        .get(DEFAULT_LIGHT)
        .cloned()
        .unwrap_or_else(default_theme);
    (fb, Some(ThemeWarning::NotFound(name.to_string())))
}

/// Default light theme name.
pub const DEFAULT_LIGHT: &str = "Wren Light";
/// Default dark theme name.
pub const DEFAULT_DARK: &str = "Wren Dark";

/// Built-in family JSON strings.
pub const BUILTIN_LIGHT_JSON: &str = include_str!("../../../assets/themes/wren-light.json");
pub const BUILTIN_DARK_JSON: &str = include_str!("../../../assets/themes/wren-dark.json");
pub const BUILTIN_WREN_JSON: &str = include_str!("../../../assets/themes/wren.json");
pub const BUILTIN_FAMILIES: &[&str] = &[BUILTIN_WREN_JSON];

/// Parse a theme family from JSON text.
pub fn parse_family(json: &str) -> Result<ThemeFamily, ThemeError> {
    let raw: RawFamily = serde_json_lenient::from_str(json).map_err(|e| ThemeError {
        message: e.to_string(),
    })?;
    let mut themes = Vec::new();
    for rt in raw.themes {
        themes.push(convert_theme(rt)?);
    }
    Ok(ThemeFamily {
        name: raw.name,
        author: raw.author,
        themes,
    })
}

pub fn default_theme() -> Theme {
    // default to light
    make_theme(
        "Wren Light",
        Appearance::Light,
        ThemeColors {
            background: parse_color("#f4efe6ff").unwrap(),
            surface: parse_color("#fbf8f2ff").unwrap(),
            border: parse_color("#d9d0c0ff").unwrap(),
            text: parse_color("#2b2620ff").unwrap(),
            muted_text: parse_color("#7a7060ff").unwrap(),
            accent: parse_color("#c96442ff").unwrap(),
            accent_text: parse_color("#fffaf2ff").unwrap(),
            danger: parse_color("#b33a2bff").unwrap(),
        },
    )
}

fn make_theme(name: &str, appearance: Appearance, colors: ThemeColors) -> Theme {
    Theme {
        name: name.to_string(),
        appearance,
        background: colors.background,
        surface: colors.surface,
        border: colors.border,
        text: colors.text,
        muted_text: colors.muted_text,
        accent: colors.accent,
        accent_text: colors.accent_text,
        danger: colors.danger,
        colors,
    }
}

fn convert_theme(rt: RawTheme) -> Result<Theme, ThemeError> {
    let colors = ThemeColors {
        background: parse_color_opt(&rt.style.background, "#000000ff"),
        surface: parse_color_opt(&rt.style.surface, "#000000ff"),
        border: parse_color_opt(&rt.style.border, "#000000ff"),
        text: parse_color_opt(&rt.style.text, "#ffffff"),
        muted_text: parse_color_opt(&rt.style.muted_text, "#888888"),
        accent: parse_color_opt(&rt.style.accent, "#4a90d9ff"),
        accent_text: parse_color_opt(&rt.style.accent_text, "#ffffff"),
        danger: parse_color_opt(&rt.style.danger, "#d94a4aff"),
    };
    Ok(make_theme(&rt.name, rt.appearance, colors))
}

fn parse_color_opt(v: &Option<String>, fallback: &str) -> Hsla {
    v.as_deref()
        .and_then(parse_color)
        .unwrap_or_else(|| parse_color(fallback).unwrap())
}

fn parse_color(s: &str) -> Option<Hsla> {
    let s = s.trim().trim_start_matches('#');
    let n = if s.len() == 6 {
        u32::from_str_radix(s, 16).ok()? << 8 | 0xff
    } else if s.len() == 8 {
        u32::from_str_radix(s, 16).ok()?
    } else {
        return None;
    };
    Some(gpui::rgb_to_hsla(gpui::rgba(n)))
}

/// Initialize themes: load builtins and set active theme global.
pub fn init(cx: &mut App) {
    let reg = ThemeRegistry::from_builtins();
    let active_name = DEFAULT_LIGHT;
    let active = reg.get(active_name).cloned().unwrap_or_else(default_theme);
    // store registry as global? add ThemeRegistry global
    cx.set_global(reg);
    cx.set_global(ActiveTheme { theme: active });
}

impl Global for ThemeRegistry {}

// serde raw types
#[derive(Deserialize)]
struct RawFamily {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    themes: Vec<RawTheme>,
}

#[derive(Deserialize)]
struct RawTheme {
    name: String,
    appearance: Appearance,
    #[serde(default)]
    style: RawStyle,
}

#[derive(Deserialize, Default)]
struct RawStyle {
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    surface: Option<String>,
    #[serde(default)]
    border: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    muted_text: Option<String>,
    #[serde(default)]
    accent: Option<String>,
    #[serde(default)]
    accent_text: Option<String>,
    #[serde(default)]
    danger: Option<String>,
}

pub fn load_user_themes(registry: &mut ThemeRegistry, dir: &std::path::Path) -> Vec<ThemeError> {
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return errors;
    };
    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                if let Err(e) = registry.load_family_str(&text) {
                    errors.push(e);
                }
            }
            Err(e) => errors.push(ThemeError {
                message: e.to_string(),
            }),
        }
    }
    errors
}
