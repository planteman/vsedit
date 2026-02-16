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

// ---------------------------------------------------------------------------
// Plural rules
// ---------------------------------------------------------------------------

/// CLDR-style plural categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluralRule {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralRule {
    /// Simple English-style plural selection: 0 → Zero, 1 → One, else Other.
    pub fn select(n: u64) -> Self {
        match n {
            0 => PluralRule::Zero,
            1 => PluralRule::One,
            _ => PluralRule::Other,
        }
    }
}

impl fmt::Display for PluralRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            PluralRule::Zero => "zero",
            PluralRule::One => "one",
            PluralRule::Two => "two",
            PluralRule::Few => "few",
            PluralRule::Many => "many",
            PluralRule::Other => "other",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// PluralizedString
// ---------------------------------------------------------------------------

/// Holds translations for each plural form of a single key.
#[derive(Debug, Clone)]
pub struct PluralizedString {
    pub key: String,
    forms: HashMap<PluralRule, String>,
}

impl PluralizedString {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            forms: HashMap::new(),
        }
    }

    pub fn set_form(&mut self, rule: PluralRule, value: impl Into<String>) {
        self.forms.insert(rule, value.into());
    }

    /// Look up the form for `n`, falling back to `Other` then to `fallback`.
    pub fn get(&self, n: u64, fallback: &str) -> String {
        let rule = PluralRule::select(n);
        self.forms
            .get(&rule)
            .or_else(|| self.forms.get(&PluralRule::Other))
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn has_form(&self, rule: PluralRule) -> bool {
        self.forms.contains_key(&rule)
    }
}

// ---------------------------------------------------------------------------
// Extra LocalizedString methods
// ---------------------------------------------------------------------------

impl LocalizedString {
    /// Add or replace a translation for the given locale.
    pub fn add_translation(&mut self, locale_id: &str, value: &str) {
        if let Some(entry) = self.translations.iter_mut().find(|(id, _)| id == locale_id) {
            entry.1 = value.to_string();
        } else {
            self.translations
                .push((locale_id.to_string(), value.to_string()));
        }
    }

    /// Return the set of locale ids that have translations.
    pub fn available_locales(&self) -> Vec<&str> {
        self.translations.iter().map(|(id, _)| id.as_str()).collect()
    }

    /// Remove the translation for a locale. Returns `true` if one was removed.
    pub fn remove_translation(&mut self, locale_id: &str) -> bool {
        let before = self.translations.len();
        self.translations.retain(|(id, _)| id != locale_id);
        self.translations.len() < before
    }
}

// ---------------------------------------------------------------------------
// Extra Locale methods
// ---------------------------------------------------------------------------

impl Locale {
    /// Returns `true` for right-to-left languages (Arabic, Hebrew, Persian, Urdu).
    pub fn is_rtl(&self) -> bool {
        matches!(self.language.as_str(), "ar" | "he" | "fa" | "ur")
    }

    /// Return the likely script tag for common languages.
    pub fn script(&self) -> &'static str {
        match self.language.as_str() {
            "zh" => "Hans",
            "ja" => "Jpan",
            "ko" => "Kore",
            "ar" | "fa" | "ur" => "Arab",
            "he" => "Hebr",
            "hi" => "Deva",
            "th" => "Thai",
            _ => "Latn",
        }
    }

    /// Basic validation: language must be 2-3 lowercase ASCII letters,
    /// country (if present) must be 2 uppercase ASCII letters.
    pub fn validate(&self) -> Result<(), LocalizationError> {
        let lang = &self.language;
        if lang.len() < 2
            || lang.len() > 3
            || !lang.chars().all(|c| c.is_ascii_lowercase())
        {
            return Err(LocalizationError::InvalidLocale(self.id()));
        }
        if let Some(ref c) = self.country {
            if c.len() != 2 || !c.chars().all(|ch| ch.is_ascii_uppercase()) {
                return Err(LocalizationError::InvalidLocale(self.id()));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Extra LanguagePack methods + PartialEq
// ---------------------------------------------------------------------------

impl PartialEq for LanguagePack {
    fn eq(&self, other: &Self) -> bool {
        self.locale == other.locale && self.translations == other.translations
    }
}

impl Eq for LanguagePack {}

impl LanguagePack {
    /// Merge another pack's translations into this one. Existing keys are **not** overwritten.
    pub fn merge_with(&mut self, other: &LanguagePack) {
        for (k, v) in &other.translations {
            self.translations.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    /// Return a sorted list of all translation keys.
    pub fn keys(&self) -> Vec<&str> {
        let mut ks: Vec<&str> = self.translations.keys().map(|s| s.as_str()).collect();
        ks.sort();
        ks
    }

    /// Return the keys present in `reference` but missing from this pack.
    pub fn missing_keys_from<'a>(&self, reference: &'a LanguagePack) -> Vec<&'a str> {
        reference
            .translations
            .keys()
            .filter(|k| !self.translations.contains_key(k.as_str()))
            .map(|k| k.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Extra LanguagePackRegistry methods
// ---------------------------------------------------------------------------

impl LanguagePackRegistry {
    /// Translate with automatic fallback: first try the full locale (e.g. `fr-CA`),
    /// then try the language-only locale (e.g. `fr`), and finally return `default`.
    pub fn translate_with_fallback(&self, key: &str, default: &str) -> String {
        // Try exact locale
        if let Some(val) = self
            .packs
            .iter()
            .find(|p| p.locale == self.active_locale)
            .and_then(|p| p.translations.get(key))
        {
            return val.clone();
        }
        // Try language-only fallback
        let lang_only = Locale {
            language: self.active_locale.language.clone(),
            country: None,
        };
        if lang_only != self.active_locale {
            if let Some(val) = self
                .packs
                .iter()
                .find(|p| p.locale == lang_only)
                .and_then(|p| p.translations.get(key))
            {
                return val.clone();
            }
        }
        default.to_string()
    }

    /// Return a reference to the pack for the given locale, if registered.
    pub fn get_pack(&self, locale: &Locale) -> Option<&LanguagePack> {
        self.packs.iter().find(|p| &p.locale == locale)
    }

    /// Return the union of all translation keys across every registered pack, sorted.
    pub fn all_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .packs
            .iter()
            .flat_map(|p| p.translations.keys().cloned())
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

// ---------------------------------------------------------------------------
// format_message
// ---------------------------------------------------------------------------

/// Replace positional placeholders `{0}`, `{1}`, … with the corresponding arguments.
pub fn format_message(template: &str, args: &[&str]) -> String {
    let mut result = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        let placeholder = format!("{{{i}}}");
        result = result.replace(&placeholder, arg);
    }
    result
}

// ---------------------------------------------------------------------------
// TranslationCoverage
// ---------------------------------------------------------------------------

/// Computes how many keys from a reference pack are present in a target pack.
#[derive(Debug, Clone)]
pub struct TranslationCoverage {
    pub total: usize,
    pub translated: usize,
    pub missing: Vec<String>,
}

impl TranslationCoverage {
    /// Compare `target` against `reference` and compute coverage.
    pub fn compute(reference: &LanguagePack, target: &LanguagePack) -> Self {
        let total = reference.translations.len();
        let mut missing = Vec::new();
        for key in reference.translations.keys() {
            if !target.translations.contains_key(key) {
                missing.push(key.clone());
            }
        }
        missing.sort();
        let translated = total - missing.len();
        Self {
            total,
            translated,
            missing,
        }
    }

    /// Coverage as a percentage (0.0 – 100.0). Returns 100.0 when `total` is 0.
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.translated as f64 / self.total as f64) * 100.0
        }
    }

    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
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

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn plural_rule_select() {
        assert_eq!(PluralRule::select(0), PluralRule::Zero);
        assert_eq!(PluralRule::select(1), PluralRule::One);
        assert_eq!(PluralRule::select(2), PluralRule::Other);
        assert_eq!(PluralRule::select(100), PluralRule::Other);
    }

    #[test]
    fn plural_rule_display() {
        assert_eq!(PluralRule::Zero.to_string(), "zero");
        assert_eq!(PluralRule::One.to_string(), "one");
        assert_eq!(PluralRule::Two.to_string(), "two");
        assert_eq!(PluralRule::Few.to_string(), "few");
        assert_eq!(PluralRule::Many.to_string(), "many");
        assert_eq!(PluralRule::Other.to_string(), "other");
    }

    #[test]
    fn pluralized_string_basic() {
        let mut ps = PluralizedString::new("items");
        ps.set_form(PluralRule::Zero, "no items");
        ps.set_form(PluralRule::One, "1 item");
        ps.set_form(PluralRule::Other, "{0} items");

        assert_eq!(ps.get(0, "fallback"), "no items");
        assert_eq!(ps.get(1, "fallback"), "1 item");
        assert_eq!(ps.get(5, "fallback"), "{0} items");
        assert!(ps.has_form(PluralRule::Zero));
        assert!(!ps.has_form(PluralRule::Few));
    }

    #[test]
    fn pluralized_string_fallback_to_other() {
        let mut ps = PluralizedString::new("files");
        ps.set_form(PluralRule::Other, "some files");
        // n=0 selects Zero, which is missing, so falls back to Other
        assert_eq!(ps.get(0, "default"), "some files");
    }

    #[test]
    fn pluralized_string_fallback_to_default() {
        let ps = PluralizedString::new("empty");
        assert_eq!(ps.get(1, "default_val"), "default_val");
    }

    #[test]
    fn localized_string_add_translation() {
        let mut ls = LocalizedString {
            key: "ok".into(),
            default_value: "OK".into(),
            translations: vec![],
        };
        ls.add_translation("fr", "D'accord");
        assert_eq!(ls.get_for_locale("fr"), Some("D'accord"));

        // Overwrite existing
        ls.add_translation("fr", "OK (fr)");
        assert_eq!(ls.get_for_locale("fr"), Some("OK (fr)"));
    }

    #[test]
    fn localized_string_available_locales() {
        let ls = LocalizedString {
            key: "x".into(),
            default_value: "X".into(),
            translations: vec![
                ("en".into(), "X-en".into()),
                ("ja".into(), "X-ja".into()),
            ],
        };
        let locales = ls.available_locales();
        assert_eq!(locales, vec!["en", "ja"]);
    }

    #[test]
    fn localized_string_remove_translation() {
        let mut ls = LocalizedString {
            key: "y".into(),
            default_value: "Y".into(),
            translations: vec![("de".into(), "Y-de".into())],
        };
        assert!(ls.remove_translation("de"));
        assert!(!ls.remove_translation("de"));
        assert_eq!(ls.get_for_locale("de"), None);
    }

    #[test]
    fn locale_is_rtl() {
        assert!(Locale::parse("ar").is_rtl());
        assert!(Locale::parse("he").is_rtl());
        assert!(Locale::parse("fa").is_rtl());
        assert!(Locale::parse("ur").is_rtl());
        assert!(!Locale::parse("en").is_rtl());
        assert!(!Locale::parse("fr").is_rtl());
    }

    #[test]
    fn locale_script() {
        assert_eq!(Locale::parse("zh").script(), "Hans");
        assert_eq!(Locale::parse("ja").script(), "Jpan");
        assert_eq!(Locale::parse("ar").script(), "Arab");
        assert_eq!(Locale::parse("en").script(), "Latn");
        assert_eq!(Locale::parse("hi").script(), "Deva");
    }

    #[test]
    fn locale_validate_ok() {
        assert!(Locale::parse("en-US").validate().is_ok());
        assert!(Locale::parse("fr").validate().is_ok());
        assert!(Locale::parse("pt-BR").validate().is_ok());
    }

    #[test]
    fn locale_validate_bad_language() {
        let loc = Locale {
            language: "E".into(),
            country: None,
        };
        assert!(loc.validate().is_err());
    }

    #[test]
    fn locale_validate_bad_country() {
        let loc = Locale {
            language: "en".into(),
            country: Some("us".into()), // lowercase
        };
        assert!(loc.validate().is_err());
    }

    #[test]
    fn language_pack_merge_with() {
        let mut base = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A".into()),
                ("b".into(), "B".into()),
            ]),
        };
        let extra = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("b".into(), "B-new".into()),
                ("c".into(), "C".into()),
            ]),
        };
        base.merge_with(&extra);
        assert_eq!(base.translations.get("a").unwrap(), "A");
        assert_eq!(base.translations.get("b").unwrap(), "B"); // not overwritten
        assert_eq!(base.translations.get("c").unwrap(), "C");
    }

    #[test]
    fn language_pack_keys_sorted() {
        let pack = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("z".into(), "Z".into()),
                ("a".into(), "A".into()),
                ("m".into(), "M".into()),
            ]),
        };
        assert_eq!(pack.keys(), vec!["a", "m", "z"]);
    }

    #[test]
    fn language_pack_missing_keys_from() {
        let reference = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A".into()),
                ("b".into(), "B".into()),
                ("c".into(), "C".into()),
            ]),
        };
        let target = LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([("a".into(), "A-fr".into())]),
        };
        let mut missing = target.missing_keys_from(&reference);
        missing.sort();
        assert_eq!(missing, vec!["b", "c"]);
    }

    #[test]
    fn language_pack_partial_eq() {
        let a = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([("x".into(), "X".into())]),
        };
        let b = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([("x".into(), "X".into())]),
        };
        let c = LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([("x".into(), "X".into())]),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn translate_with_fallback_exact() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("fr-CA"));
        reg.register(LanguagePack {
            locale: Locale::parse("fr-CA"),
            translations: HashMap::from([("color".into(), "couleur (CA)".into())]),
        });
        assert_eq!(
            reg.translate_with_fallback("color", "color"),
            "couleur (CA)"
        );
    }

    #[test]
    fn translate_with_fallback_language_only() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("fr-CA"));
        reg.register(LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([("color".into(), "couleur".into())]),
        });
        // No fr-CA pack, should fall back to fr
        assert_eq!(reg.translate_with_fallback("color", "color"), "couleur");
    }

    #[test]
    fn translate_with_fallback_default() {
        let reg = LanguagePackRegistry::new(Locale::parse("fr-CA"));
        assert_eq!(
            reg.translate_with_fallback("missing", "default"),
            "default"
        );
    }

    #[test]
    fn registry_get_pack() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        let pack = LanguagePack {
            locale: Locale::parse("ja"),
            translations: HashMap::from([("hi".into(), "こんにちは".into())]),
        };
        reg.register(pack);
        assert!(reg.get_pack(&Locale::parse("ja")).is_some());
        assert!(reg.get_pack(&Locale::parse("ko")).is_none());
    }

    #[test]
    fn registry_all_keys() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        reg.register(LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A".into()),
                ("b".into(), "B".into()),
            ]),
        });
        reg.register(LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([
                ("b".into(), "B-fr".into()),
                ("c".into(), "C-fr".into()),
            ]),
        });
        assert_eq!(reg.all_keys(), vec!["a", "b", "c"]);
    }

    #[test]
    fn format_message_basic() {
        assert_eq!(
            format_message("Hello, {0}!", &["world"]),
            "Hello, world!"
        );
        assert_eq!(
            format_message("{0} has {1} items", &["Cart", "3"]),
            "Cart has 3 items"
        );
    }

    #[test]
    fn format_message_no_placeholders() {
        assert_eq!(format_message("plain text", &[]), "plain text");
    }

    #[test]
    fn format_message_extra_args_ignored() {
        assert_eq!(
            format_message("{0} only", &["used", "ignored"]),
            "used only"
        );
    }

    #[test]
    fn translation_coverage_full() {
        let reference = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A".into()),
                ("b".into(), "B".into()),
            ]),
        };
        let target = LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([
                ("a".into(), "A-fr".into()),
                ("b".into(), "B-fr".into()),
            ]),
        };
        let cov = TranslationCoverage::compute(&reference, &target);
        assert_eq!(cov.total, 2);
        assert_eq!(cov.translated, 2);
        assert!(cov.missing.is_empty());
        assert!(cov.is_complete());
        assert!((cov.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn translation_coverage_partial() {
        let reference = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A".into()),
                ("b".into(), "B".into()),
                ("c".into(), "C".into()),
            ]),
        };
        let target = LanguagePack {
            locale: Locale::parse("de"),
            translations: HashMap::from([("a".into(), "A-de".into())]),
        };
        let cov = TranslationCoverage::compute(&reference, &target);
        assert_eq!(cov.total, 3);
        assert_eq!(cov.translated, 1);
        assert_eq!(cov.missing.len(), 2);
        assert!(!cov.is_complete());
        assert!((cov.percentage() - 100.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn translation_coverage_empty_reference() {
        let reference = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::new(),
        };
        let target = LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::new(),
        };
        let cov = TranslationCoverage::compute(&reference, &target);
        assert!(cov.is_complete());
        assert!((cov.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn edge_case_empty_pack_keys() {
        let pack = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::new(),
        };
        assert!(pack.keys().is_empty());
    }

    #[test]
    fn edge_case_merge_empty_packs() {
        let mut a = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::new(),
        };
        let b = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::new(),
        };
        a.merge_with(&b);
        assert!(a.translations.is_empty());
    }
}
