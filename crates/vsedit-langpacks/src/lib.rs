//! Language pack management – locale handling and translation registries.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during localization operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalizationError {
    /// The requested translation key was not found.
    MissingKey(String),
    /// No language pack registered for the requested locale.
    LocaleNotFound(String),
    /// The locale string could not be parsed.
    InvalidLocale(String),
}

impl fmt::Display for LocalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocalizationError::MissingKey(key) => write!(f, "missing translation key: {key}"),
            LocalizationError::LocaleNotFound(id) => {
                write!(f, "no language pack for locale: {id}")
            }
            LocalizationError::InvalidLocale(s) => write!(f, "invalid locale string: {s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locale {
    pub language: String,
    pub country: Option<String>,
}

impl Locale {
    pub fn parse(s: &str) -> Self {
        let mut parts = s.splitn(2, '-');
        let language = parts.next().unwrap_or("en").to_string();
        let country = parts.next().map(|c| c.to_string());
        Self { language, country }
    }

    /// Return a locale identifier string, e.g. `"en-US"` or `"fr"`.
    pub fn id(&self) -> String {
        match &self.country {
            Some(c) => format!("{}-{}", self.language, c),
            None => self.language.clone(),
        }
    }

    /// Check if this locale matches the given language, ignoring country.
    pub fn matches(&self, language: &str) -> bool {
        self.language == language
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id())
    }
}

#[derive(Debug, Clone)]
pub struct LocalizedString {
    pub key: String,
    pub default_value: String,
    /// (locale_id, translated_value)
    pub translations: Vec<(String, String)>,
}

impl LocalizedString {
    /// Get the translation for a specific locale id, or `None` if not present.
    pub fn get_for_locale(&self, locale_id: &str) -> Option<&str> {
        self.translations
            .iter()
            .find(|(id, _)| id == locale_id)
            .map(|(_, val)| val.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct LanguagePack {
    pub locale: Locale,
    pub translations: HashMap<String, String>,
}

impl fmt::Display for LanguagePack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} translations)", self.locale, self.translations.len())
    }
}

/// Look up a key in the given language pack, falling back to `default`.
pub fn localize(pack: &LanguagePack, key: &str, default: &str) -> String {
    pack.translations
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

pub struct LanguagePackRegistry {
    packs: Vec<LanguagePack>,
    active_locale: Locale,
}

impl LanguagePackRegistry {
    pub fn new(default_locale: Locale) -> Self {
        Self {
            packs: Vec::new(),
            active_locale: default_locale,
        }
    }

    pub fn register(&mut self, pack: LanguagePack) {
        self.packs.push(pack);
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.active_locale = locale;
    }

    pub fn translate(&self, key: &str, default: &str) -> String {
        self.packs
            .iter()
            .find(|p| p.locale == self.active_locale)
            .and_then(|p| p.translations.get(key).cloned())
            .unwrap_or_else(|| default.to_string())
    }

    /// Like `translate`, but returns an error instead of falling back to a default.
    pub fn try_translate(&self, key: &str) -> Result<String, LocalizationError> {
        let pack = self
            .packs
            .iter()
            .find(|p| p.locale == self.active_locale)
            .ok_or_else(|| LocalizationError::LocaleNotFound(self.active_locale.id()))?;
        pack.translations
            .get(key)
            .cloned()
            .ok_or_else(|| LocalizationError::MissingKey(key.to_string()))
    }

    /// Return the list of all registered locale ids.
    pub fn available_locales(&self) -> Vec<String> {
        self.packs.iter().map(|p| p.locale.id()).collect()
    }

    /// Check whether a language pack is registered for the given locale.
    pub fn has_locale(&self, locale: &Locale) -> bool {
        self.packs.iter().any(|p| &p.locale == locale)
    }

    /// Return a reference to the currently active locale.
    pub fn active_locale(&self) -> &Locale {
        &self.active_locale
    }

    /// Remove the language pack for the given locale, returning `true` if one was removed.
    pub fn unregister(&mut self, locale: &Locale) -> bool {
        let before = self.packs.len();
        self.packs.retain(|p| &p.locale != locale);
        self.packs.len() < before
    }

    /// Return the total number of translation keys across all registered packs.
    pub fn key_count(&self) -> usize {
        self.packs.iter().map(|p| p.translations.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_parse_and_display() {
        let loc = Locale::parse("en-US");
        assert_eq!(loc.language, "en");
        assert_eq!(loc.country.as_deref(), Some("US"));
        assert_eq!(loc.to_string(), "en-US");

        let loc2 = Locale::parse("fr");
        assert_eq!(loc2.language, "fr");
        assert!(loc2.country.is_none());
        assert_eq!(loc2.to_string(), "fr");
    }

    #[test]
    fn localize_found() {
        let mut translations = HashMap::new();
        translations.insert("greeting".to_string(), "Bonjour".to_string());
        let pack = LanguagePack {
            locale: Locale::parse("fr"),
            translations,
        };
        assert_eq!(localize(&pack, "greeting", "Hello"), "Bonjour");
        assert_eq!(localize(&pack, "farewell", "Goodbye"), "Goodbye");
    }

    #[test]
    fn registry_translate() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        let mut fr_translations = HashMap::new();
        fr_translations.insert("save".to_string(), "Enregistrer".to_string());
        reg.register(LanguagePack {
            locale: Locale::parse("fr"),
            translations: fr_translations,
        });

        // English has no pack, falls back to default.
        assert_eq!(reg.translate("save", "Save"), "Save");

        reg.set_locale(Locale::parse("fr"));
        assert_eq!(reg.translate("save", "Save"), "Enregistrer");
    }

    #[test]
    fn registry_missing_key() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        reg.register(LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::new(),
        });
        assert_eq!(reg.translate("missing", "default_val"), "default_val");
    }

    #[test]
    fn localization_error_display() {
        let e1 = LocalizationError::MissingKey("btn.ok".into());
        assert_eq!(e1.to_string(), "missing translation key: btn.ok");

        let e2 = LocalizationError::LocaleNotFound("ja".into());
        assert_eq!(e2.to_string(), "no language pack for locale: ja");

        let e3 = LocalizationError::InvalidLocale("???".into());
        assert_eq!(e3.to_string(), "invalid locale string: ???");
    }

    #[test]
    fn try_translate_success() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("de"));
        let mut translations = HashMap::new();
        translations.insert("yes".to_string(), "Ja".to_string());
        reg.register(LanguagePack {
            locale: Locale::parse("de"),
            translations,
        });
        assert_eq!(reg.try_translate("yes").unwrap(), "Ja");
    }

    #[test]
    fn try_translate_missing_key() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("de"));
        reg.register(LanguagePack {
            locale: Locale::parse("de"),
            translations: HashMap::new(),
        });
        let err = reg.try_translate("nope").unwrap_err();
        assert_eq!(err, LocalizationError::MissingKey("nope".into()));
    }

    #[test]
    fn try_translate_locale_not_found() {
        let reg = LanguagePackRegistry::new(Locale::parse("zh"));
        let err = reg.try_translate("key").unwrap_err();
        assert_eq!(err, LocalizationError::LocaleNotFound("zh".into()));
    }

    #[test]
    fn available_locales_and_has_locale() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        reg.register(LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::new(),
        });
        reg.register(LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::new(),
        });
        let locales = reg.available_locales();
        assert_eq!(locales.len(), 2);
        assert!(locales.contains(&"en".to_string()));
        assert!(locales.contains(&"fr".to_string()));
        assert!(reg.has_locale(&Locale::parse("en")));
        assert!(!reg.has_locale(&Locale::parse("ja")));
    }

    #[test]
    fn active_locale_accessor() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en-US"));
        assert_eq!(reg.active_locale().id(), "en-US");
        reg.set_locale(Locale::parse("pt-BR"));
        assert_eq!(reg.active_locale().id(), "pt-BR");
    }

    #[test]
    fn unregister_pack() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        reg.register(LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::new(),
        });
        assert!(reg.has_locale(&Locale::parse("fr")));
        assert!(reg.unregister(&Locale::parse("fr")));
        assert!(!reg.has_locale(&Locale::parse("fr")));
        assert!(!reg.unregister(&Locale::parse("fr")));
    }

    #[test]
    fn key_count_across_packs() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        let mut en = HashMap::new();
        en.insert("a".into(), "A".into());
        en.insert("b".into(), "B".into());
        let mut fr = HashMap::new();
        fr.insert("a".into(), "X".into());
        reg.register(LanguagePack { locale: Locale::parse("en"), translations: en });
        reg.register(LanguagePack { locale: Locale::parse("fr"), translations: fr });
        assert_eq!(reg.key_count(), 3);
    }

    #[test]
    fn locale_matches_language() {
        let loc = Locale::parse("en-US");
        assert!(loc.matches("en"));
        assert!(!loc.matches("fr"));
        let loc2 = Locale::parse("fr");
        assert!(loc2.matches("fr"));
    }

    #[test]
    fn localized_string_get_for_locale() {
        let ls = LocalizedString {
            key: "hello".into(),
            default_value: "Hello".into(),
            translations: vec![
                ("fr".into(), "Bonjour".into()),
                ("de".into(), "Hallo".into()),
            ],
        };
        assert_eq!(ls.get_for_locale("fr"), Some("Bonjour"));
        assert_eq!(ls.get_for_locale("de"), Some("Hallo"));
        assert_eq!(ls.get_for_locale("ja"), None);
    }

    #[test]
    fn language_pack_display() {
        let mut translations = HashMap::new();
        translations.insert("a".into(), "A".into());
        translations.insert("b".into(), "B".into());
        let pack = LanguagePack {
            locale: Locale::parse("es"),
            translations,
        };
        assert_eq!(pack.to_string(), "es (2 translations)");
    }
}
