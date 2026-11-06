//! Runtime locale switching via Project Fluent (fluent-rs — pure Rust, links
//! fine into the static musl armv7 target). Locale files are embedded with
//! `include_str!` so no runtime file IO is needed on the Kindle.

use std::cell::RefCell;
use std::collections::HashMap;

use fluent_bundle::{FluentBundle, FluentResource};

/// Locales we ship. `en` is the default and fallback.
pub const LOCALES: [&str; 2] = ["en", "de"];
pub const DEFAULT_LOCALE: &str = "en";

fn load_bundle(locale: &str) -> FluentBundle<FluentResource> {
    let source = match locale {
        "de" => include_str!("../i18n/de.ftl"),
        _ => include_str!("../i18n/en.ftl"),
    };
    let resource = FluentResource::try_new(source.to_string())
        .unwrap_or_else(|_| FluentResource::try_new(String::new()).unwrap());
    let mut bundle = FluentBundle::new(vec![locale.parse().unwrap_or_else(|_| "en".parse().unwrap())]);
    bundle.set_use_isolating(false);
    let _ = bundle.add_resource(resource);
    bundle
}

thread_local! {
    // FluentBundle is !Send/!Sync (intl memoizer uses RefCell), so it lives
    // in a thread_local — only the main/event-loop thread ever translates.
    static BUNDLES: RefCell<Option<HashMap<&'static str, FluentBundle<FluentResource>>>> =
        const { RefCell::new(None) };
}

fn with_bundle<R>(locale: &str, f: impl FnOnce(&FluentBundle<FluentResource>) -> R) -> R {
    BUNDLES.with(|b| {
        let mut map = b.borrow_mut();
        let map = map.get_or_insert_with(|| LOCALES.iter().map(|l| (*l, load_bundle(l))).collect());
        let bundle = map.get(locale).or_else(|| map.get(DEFAULT_LOCALE)).unwrap();
        f(bundle)
    })
}

/// Translate `key` in `locale`, falling back to English, then to the key
/// itself. Fluent variables come from `args`.
pub fn t(locale: &str, key: &str, args: &[(&str, &str)]) -> String {
    with_bundle(locale, |bundle| {
        let Some(message) = bundle.get_message(key) else {
            return key.to_string();
        };
        let Some(pattern) = message.value() else {
            return key.to_string();
        };
        let fluent_args: Option<fluent_bundle::FluentArgs> = if args.is_empty() {
            None
        } else {
            let mut fa = fluent_bundle::FluentArgs::new();
            for (k, v) in args {
                fa.set(*k, *v);
            }
            Some(fa)
        };
        let mut errors = vec![];
        bundle
            .format_pattern(pattern, fluent_args.as_ref(), &mut errors)
            .to_string()
    })
}

/// Convenience for calls without variables.
pub fn tr(locale: &str, key: &str) -> String {
    t(locale, key, &[])
}

/// The configured UI locale from the shared config store.
pub fn locale_of(config: &std::sync::RwLock<crate::utils::config::AppConfig>) -> String {
    config
        .read()
        .map(|c| c.language.clone())
        .unwrap_or_else(|_| DEFAULT_LOCALE.to_string())
}

/// Message keys defined in a locale file (simple `key = value` line scan —
/// enough for parity testing since our files use no multiline terms).
#[cfg(test)]
pub fn keys(locale: &str) -> Vec<String> {
    let source = match locale {
        "de" => include_str!("../i18n/de.ftl"),
        _ => include_str!("../i18n/en.ftl"),
    };
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            if line.starts_with('#') || line.is_empty() {
                return None;
            }
            line.split_once('=').map(|(k, _)| k.trim().to_string())
        })
        .collect()
}

#[cfg(test)]
mod i18n_tests {
    use super::*;

    #[test]
    fn en_and_de_have_identical_key_sets() {
        let mut en = keys("en");
        let mut de = keys("de");
        en.sort();
        de.sort();
        assert_eq!(en, de, "locale key parity violated");
    }

    #[test]
    fn german_strings_are_returned_for_de() {
        assert_eq!(tr("de", "home-play"), "Spielen");
        assert_eq!(tr("en", "home-play"), "Play");
    }

    #[test]
    fn unknown_key_falls_back_to_key_name() {
        assert_eq!(tr("en", "no-such-key"), "no-such-key");
    }

    #[test]
    fn fluent_args_are_interpolated() {
        assert_eq!(
            t("en", "play-opponent", &[("name", "Alice")]),
            "Opponent: Alice"
        );
        assert_eq!(
            t("de", "play-opponent", &[("name", "Alice")]),
            "Gegner: Alice"
        );
    }

    #[test]
    fn unknown_locale_falls_back_to_english() {
        assert_eq!(tr("fr", "home-play"), "Play");
    }
}
