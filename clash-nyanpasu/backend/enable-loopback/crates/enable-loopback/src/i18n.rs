//! Embedded translations and supported language selection (§5); no application state singleton.

rust_i18n::i18n!("locales", fallback = "en");

pub const LANGUAGES: [&str; 6] = ["en", "zh-cn", "zh-tw", "ja", "ko", "ru"];

pub fn normalize(language: &str) -> Option<&'static str> {
    let language = language.trim().replace('_', "-").to_ascii_lowercase();
    let parts: Vec<_> = language.split('-').collect();
    match parts.first().copied()? {
        "zh" => Some(
            if parts
                .iter()
                .any(|part| matches!(*part, "hant" | "tw" | "hk" | "mo"))
            {
                "zh-tw"
            } else {
                "zh-cn"
            },
        ),
        base => LANGUAGES.iter().copied().find(|code| *code == base),
    }
}

pub fn choose(explicit: Option<&str>, system: Option<&str>) -> &'static str {
    explicit.or(system).and_then(normalize).unwrap_or("en")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, fs, path::Path};

    #[test]
    fn normalize_language_tags() {
        for (input, expected) in [
            ("zh-Hans-CN", "zh-cn"),
            ("zh-TW", "zh-tw"),
            ("zh-Hant", "zh-tw"),
            ("ZH_hk", "zh-tw"),
            ("zh", "zh-cn"),
            ("en-US", "en"),
            ("ja-JP", "ja"),
            ("ko-KR", "ko"),
            ("ru-RU", "ru"),
        ] {
            assert_eq!(normalize(input), Some(expected));
        }
        assert_eq!(normalize("fr"), None);
        assert_eq!(choose(Some("ja"), Some("ru")), "ja");
        assert_eq!(choose(None, Some("zh-Hant")), "zh-tw");
        assert_eq!(choose(Some("fr"), Some("ja")), "en");
        assert_eq!(choose(None, None), "en");
    }

    fn keys(value: &serde_json::Value, prefix: &str, result: &mut BTreeSet<String>) {
        if let Some(object) = value.as_object() {
            for (key, value) in object {
                keys(value, &format!("{prefix}.{key}"), result);
            }
        } else {
            assert!(value.as_str().is_some_and(|text| !text.is_empty()));
            result.insert(prefix.to_owned());
        }
    }

    #[test]
    fn all_six_locale_files_have_identical_nonempty_key_sets() {
        let mut expected = None;
        for language in LANGUAGES {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("locales")
                .join(format!("{language}.json"));
            let value: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            let mut actual = BTreeSet::new();
            keys(&value, "", &mut actual);
            assert!(!actual.is_empty());
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected, "locale {language}");
            } else {
                expected = Some(actual);
            }
        }
    }
}
