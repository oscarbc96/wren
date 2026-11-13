use themes::{ThemeRegistry, parse_family};

#[test]
fn builtins_parse() {
    for json in themes::BUILTIN_FAMILIES {
        let fam = parse_family(json).unwrap();
        assert!(!fam.themes.is_empty());
    }
}

#[test]
fn unknown_theme_falls_back() {
    let reg = ThemeRegistry::from_builtins();
    let (theme, warning) = reg.resolve("Nope");
    assert_eq!(theme.name, themes::DEFAULT_LIGHT);
    assert!(warning.is_some());
}

#[test]
fn invalid_json_reports_error() {
    let err = parse_family("{").unwrap_err();
    assert!(!err.to_string().is_empty());
}
