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

// ---------------------------------------------------------------------------
// LanguagePackBundle – manage multiple translation bundles with fallback
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct LanguagePackBundle {
    packs: Vec<LanguagePack>,
    fallback_locale: Locale,
}

impl LanguagePackBundle {
    pub fn new(fallback_locale: Locale) -> Self {
        Self {
            packs: Vec::new(),
            fallback_locale,
        }
    }

    pub fn add_pack(&mut self, pack: LanguagePack) {
        self.packs.push(pack);
    }

    /// Translate `key` for `target_locale`, falling back to the bundle's
    /// fallback locale when the target does not contain the key.
    pub fn translate(&self, key: &str, target_locale: &Locale) -> Option<String> {
        // Try target locale first.
        if let Some(val) = self
            .packs
            .iter()
            .find(|p| &p.locale == target_locale)
            .and_then(|p| p.translations.get(key))
        {
            return Some(val.clone());
        }
        // Try fallback locale.
        self.packs
            .iter()
            .find(|p| p.locale == self.fallback_locale)
            .and_then(|p| p.translations.get(key).cloned())
    }

    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    pub fn supported_locales(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for p in &self.packs {
            let id = p.locale.id();
            if !seen.contains(&id) {
                seen.push(id);
            }
        }
        seen
    }

    /// Percentage of fallback-locale keys that `locale` covers (0.0–100.0).
    pub fn coverage_for(&self, locale: &Locale) -> f64 {
        let fallback_keys: Vec<&String> = self
            .packs
            .iter()
            .filter(|p| p.locale == self.fallback_locale)
            .flat_map(|p| p.translations.keys())
            .collect();
        if fallback_keys.is_empty() {
            return 0.0;
        }
        let target_pack = self.packs.iter().find(|p| &p.locale == locale);
        let covered = match target_pack {
            Some(tp) => fallback_keys
                .iter()
                .filter(|k| tp.translations.contains_key(**k))
                .count(),
            None => 0,
        };
        (covered as f64 / fallback_keys.len() as f64) * 100.0
    }

    /// Merge all packs that match the fallback locale into a single pack.
    pub fn merge_all(&self) -> LanguagePack {
        let mut merged = LanguagePack {
            locale: self.fallback_locale.clone(),
            translations: HashMap::new(),
        };
        for p in &self.packs {
            if p.locale == self.fallback_locale {
                for (k, v) in &p.translations {
                    merged.translations.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        merged
    }

    /// Remove the first pack whose locale matches. Returns `true` if removed.
    pub fn remove_pack(&mut self, locale: &Locale) -> bool {
        let before = self.packs.len();
        self.packs.retain(|p| &p.locale != locale);
        self.packs.len() < before
    }
}

// ---------------------------------------------------------------------------
// langpack_detect_system_locale – auto-detect locale from environment
// ---------------------------------------------------------------------------

/// Detect the system locale from environment variables (`LANG`, `LC_ALL`,
/// `LANGUAGE`). Falls back to `en` when nothing is found.
pub fn langpack_detect_system_locale() -> Locale {
    let raw = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LANGUAGE"))
        .unwrap_or_default();
    parse_posix_locale(&raw)
}

/// Parse a POSIX locale string like `"en_US.UTF-8"` or `"fr_FR"` into a
/// `Locale`.
fn parse_posix_locale(raw: &str) -> Locale {
    if raw.is_empty() {
        return Locale::parse("en");
    }
    // Strip encoding suffix (e.g. ".UTF-8").
    let without_encoding = raw.split('.').next().unwrap_or(raw);
    // Replace underscore with hyphen for Locale::parse.
    let normalised = without_encoding.replace('_', "-");
    Locale::parse(&normalised)
}

// ---------------------------------------------------------------------------
// LocaleNegotiator – pick best locale from a set of available locales
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct LocaleNegotiator {
    available: Vec<String>,
}

impl LocaleNegotiator {
    pub fn new(available: Vec<String>) -> Self {
        Self { available }
    }

    /// Find the best match from `requested` against the available locales.
    /// Returns the first exact match, then the first language-only match.
    /// Returns an empty string when nothing matches.
    pub fn negotiate(&self, requested: &[String]) -> String {
        // Exact match pass.
        for req in requested {
            if self.available.iter().any(|a| a == req) {
                return req.clone();
            }
        }
        // Language-only match pass.
        for req in requested {
            let lang = req.split('-').next().unwrap_or(req);
            if let Some(found) = self.available.iter().find(|a| {
                let a_lang = a.split('-').next().unwrap_or(a);
                a_lang == lang
            }) {
                return found.clone();
            }
        }
        String::new()
    }

    /// Like `negotiate`, but returns `fallback` when no match is found.
    pub fn negotiate_with_fallback(&self, requested: &[String], fallback: &str) -> String {
        let result = self.negotiate(requested);
        if result.is_empty() {
            fallback.to_string()
        } else {
            result
        }
    }
}

// ---------------------------------------------------------------------------
// TranslationValidator – validate translation packs
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TranslationValidator;

impl TranslationValidator {
    pub fn new() -> Self {
        Self
    }

    /// Return a list of warning strings for potential issues in `pack`.
    pub fn validate_pack(&self, pack: &LanguagePack) -> Vec<String> {
        let mut warnings = Vec::new();
        for (key, value) in &pack.translations {
            if key.is_empty() {
                warnings.push("empty translation key found".to_string());
            }
            if value.is_empty() {
                warnings.push(format!("empty value for key '{key}'"));
            }
        }
        warnings
    }

    /// Check that every placeholder (`{0}`, `{1}`, …) present in `template`
    /// also appears in `translation` and vice-versa.
    pub fn check_placeholders(&self, template: &str, translation: &str) -> bool {
        let extract = |s: &str| -> Vec<String> {
            let mut result = Vec::new();
            let mut i = 0;
            let bytes = s.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'{' {
                    if let Some(end) = s[i..].find('}') {
                        let token = &s[i..i + end + 1];
                        if !result.contains(&token.to_string()) {
                            result.push(token.to_string());
                        }
                        i += end + 1;
                        continue;
                    }
                }
                i += 1;
            }
            result.sort();
            result
        };
        extract(template) == extract(translation)
    }

    /// Return keys present in `reference` but missing from `target`.
    pub fn find_untranslated(
        &self,
        reference: &LanguagePack,
        target: &LanguagePack,
    ) -> Vec<String> {
        let mut missing: Vec<String> = reference
            .translations
            .keys()
            .filter(|k| !target.translations.contains_key(*k))
            .cloned()
            .collect();
        missing.sort();
        missing
    }
}

// ---------------------------------------------------------------------------
// localize_with_args – localize with argument substitution
// ---------------------------------------------------------------------------

/// Look up `key` in `pack`, substitute `{0}`, `{1}`, … with the provided
/// `args`, and fall back to `default` when the key is missing.
pub fn localize_with_args(pack: &LanguagePack, key: &str, args: &[&str], default: &str) -> String {
    let template = pack
        .translations
        .get(key)
        .map(|s| s.as_str())
        .unwrap_or(default);
    let mut result = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        let placeholder = format!("{{{i}}}");
        result = result.replace(&placeholder, arg);
    }
    result
}

// ---------------------------------------------------------------------------
// LocaleChain – ordered fallback chain for translation resolution
// ---------------------------------------------------------------------------

/// An ordered list of locales to try when resolving a translation.
///
/// For example, a chain `["pt-BR", "pt", "en"]` means: first try Brazilian
/// Portuguese, then generic Portuguese, then English.
#[derive(Debug, Clone)]
pub struct LocaleChain {
    locales: Vec<Locale>,
}

impl LocaleChain {
    /// Build a chain from an ordered slice of locale identifier strings.
    pub fn new(ids: &[&str]) -> Self {
        Self {
            locales: ids.iter().map(|s| Locale::parse(s)).collect(),
        }
    }

    /// Build a chain that automatically expands a locale with country into
    /// `[locale-with-country, language-only, fallback]`.
    pub fn from_locale_with_fallback(locale: &Locale, fallback: &str) -> Self {
        let mut locales = vec![locale.clone()];
        if locale.country.is_some() {
            let lang_only = Locale {
                language: locale.language.clone(),
                country: None,
            };
            if lang_only != *locale {
                locales.push(lang_only);
            }
        }
        let fb = Locale::parse(fallback);
        if !locales.iter().any(|l| l == &fb) {
            locales.push(fb);
        }
        Self { locales }
    }

    /// Return the number of locales in the chain.
    pub fn len(&self) -> usize {
        self.locales.len()
    }

    /// Return `true` if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.locales.is_empty()
    }

    /// Iterate over the locales in priority order.
    pub fn iter(&self) -> impl Iterator<Item = &Locale> {
        self.locales.iter()
    }

    /// Resolve a translation key by walking the chain and looking up packs
    /// in the provided registry.
    pub fn resolve(&self, registry: &LanguagePackRegistry, key: &str) -> Option<String> {
        for locale in &self.locales {
            if let Some(pack) = registry.get_pack(locale) {
                if let Some(val) = pack.translations.get(key) {
                    return Some(val.clone());
                }
            }
        }
        None
    }

    /// Like [`resolve`](Self::resolve) but returns `default` when no pack
    /// in the chain contains the key.
    pub fn resolve_or(&self, registry: &LanguagePackRegistry, key: &str, default: &str) -> String {
        self.resolve(registry, key)
            .unwrap_or_else(|| default.to_string())
    }
}

impl fmt::Display for LocaleChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ids: Vec<String> = self.locales.iter().map(|l| l.id()).collect();
        write!(f, "{}", ids.join(" -> "))
    }
}

impl From<&[&str]> for LocaleChain {
    fn from(ids: &[&str]) -> Self {
        Self::new(ids)
    }
}

// ---------------------------------------------------------------------------
// PluralRules – locale-aware plural category selection
// ---------------------------------------------------------------------------

/// Selects a [`PluralRule`] category based on a count and a locale's
/// pluralisation rules.
///
/// Supports a handful of common CLDR rule families:
/// - English-like (one / other)
/// - French / Portuguese (0-1 → one, rest → other)
/// - Arabic (zero, one, two, few, many, other)
/// - Polish / Russian-style Slavic (one, few, many, other)
#[derive(Debug, Clone)]
pub struct PluralRules {
    locale: Locale,
}

impl PluralRules {
    pub fn new(locale: &Locale) -> Self {
        Self {
            locale: locale.clone(),
        }
    }

    /// Select the plural category for `n`.
    pub fn select(&self, n: u64) -> PluralRule {
        match self.locale.language.as_str() {
            // French / Portuguese: 0 and 1 are singular
            "fr" | "pt" => match n {
                0 | 1 => PluralRule::One,
                _ => PluralRule::Other,
            },
            // Arabic: rich plural system
            "ar" => match n {
                0 => PluralRule::Zero,
                1 => PluralRule::One,
                2 => PluralRule::Two,
                3..=10 => PluralRule::Few,
                11..=99 => PluralRule::Many,
                _ => PluralRule::Other,
            },
            // Polish-style Slavic
            "pl" | "ru" | "uk" => {
                let mod10 = n % 10;
                let mod100 = n % 100;
                if n == 0 {
                    PluralRule::Zero
                } else if n == 1 {
                    PluralRule::One
                } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
                    PluralRule::Few
                } else {
                    PluralRule::Many
                }
            }
            // Default: English-like
            _ => PluralRule::select(n),
        }
    }

    /// Return the locale this rule set was built for.
    pub fn locale(&self) -> &Locale {
        &self.locale
    }
}

impl fmt::Display for PluralRules {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PluralRules({})", self.locale)
    }
}

// ---------------------------------------------------------------------------
// LocalizedFormatter – locale-aware number, date, and list formatting
// ---------------------------------------------------------------------------

/// Simple locale-sensitive formatter for numbers, dates, and lists.
#[derive(Debug, Clone)]
pub struct LocalizedFormatter {
    locale: Locale,
}

impl LocalizedFormatter {
    pub fn new(locale: &Locale) -> Self {
        Self {
            locale: locale.clone(),
        }
    }

    /// Format an integer with locale-appropriate thousands separators.
    pub fn format_integer(&self, n: i64) -> String {
        let (sep, group_size) = self.thousands_separator();
        let negative = n < 0;
        let abs = if negative {
            (n as i128).unsigned_abs() as u64
        } else {
            n as u64
        };
        let digits = abs.to_string();
        let mut result = String::new();
        for (i, ch) in digits.chars().rev().enumerate() {
            if i > 0 && i % group_size == 0 {
                result.push(sep);
            }
            result.push(ch);
        }
        let formatted: String = result.chars().rev().collect();
        if negative {
            format!("-{formatted}")
        } else {
            formatted
        }
    }

    /// Format a decimal number with locale-appropriate decimal and thousands
    /// separators (fixed to `precision` decimal places).
    pub fn format_decimal(&self, value: f64, precision: usize) -> String {
        let (thousands_sep, group_size) = self.thousands_separator();
        let decimal_sep = self.decimal_separator();

        let rounded = format!("{value:.prec$}", prec = precision);
        let parts: Vec<&str> = rounded.splitn(2, '.').collect();

        let int_part = parts[0];
        let negative = int_part.starts_with('-');
        let int_digits = if negative { &int_part[1..] } else { int_part };

        let mut grouped = String::new();
        for (i, ch) in int_digits.chars().rev().enumerate() {
            if i > 0 && i % group_size == 0 {
                grouped.push(thousands_sep);
            }
            grouped.push(ch);
        }
        let int_formatted: String = grouped.chars().rev().collect();

        let mut out = String::new();
        if negative {
            out.push('-');
        }
        out.push_str(&int_formatted);

        if precision > 0 {
            out.push(decimal_sep);
            if parts.len() > 1 {
                out.push_str(parts[1]);
            } else {
                for _ in 0..precision {
                    out.push('0');
                }
            }
        }
        out
    }

    /// Format a simple date given as `(year, month, day)` according to locale
    /// conventions (not a full i18n date formatter).
    pub fn format_date(&self, year: i32, month: u32, day: u32) -> String {
        match self.locale.language.as_str() {
            // US-style: MM/DD/YYYY
            "en" if self.locale.country.as_deref() == Some("US") => {
                format!("{month:02}/{day:02}/{year:04}")
            }
            // ISO / East-Asian: YYYY-MM-DD
            "ja" | "ko" | "zh" => {
                format!("{year:04}-{month:02}-{day:02}")
            }
            // Most of Europe & Latin America: DD/MM/YYYY
            _ => {
                format!("{day:02}/{month:02}/{year:04}")
            }
        }
    }

    /// Join a list of items with locale-appropriate separators and a
    /// conjunction before the last element.
    pub fn format_list(&self, items: &[&str]) -> String {
        match items.len() {
            0 => String::new(),
            1 => items[0].to_string(),
            2 => {
                let conj = self.conjunction();
                format!("{} {} {}", items[0], conj, items[1])
            }
            _ => {
                let (sep, conj) = self.list_separators();
                let init = items[..items.len() - 1].join(sep);
                format!("{}{} {} {}", init, sep.trim_end(), conj, items[items.len() - 1])
            }
        }
    }

    // -- private helpers ----------------------------------------------------

    fn thousands_separator(&self) -> (char, usize) {
        match self.locale.language.as_str() {
            "de" | "fr" | "pt" | "es" | "it" | "pl" | "ru" => ('.', 3),
            "hi" => {
                // Indian numbering uses groups of 2 after the first 3
                // We simplify to groups of 3 for this basic formatter.
                (',', 3)
            }
            _ => (',', 3),
        }
    }

    fn decimal_separator(&self) -> char {
        match self.locale.language.as_str() {
            "de" | "fr" | "pt" | "es" | "it" | "pl" | "ru" => ',',
            _ => '.',
        }
    }

    fn conjunction(&self) -> &'static str {
        match self.locale.language.as_str() {
            "fr" => "et",
            "de" => "und",
            "es" => "y",
            "pt" => "e",
            "it" => "e",
            "ja" => "と",
            _ => "and",
        }
    }

    fn list_separators(&self) -> (&'static str, &'static str) {
        match self.locale.language.as_str() {
            "fr" => (", ", "et"),
            "de" => (", ", "und"),
            "es" => (", ", "y"),
            "pt" => (", ", "e"),
            "ja" => ("、", "と"),
            _ => (", ", "and"),
        }
    }
}

impl fmt::Display for LocalizedFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LocalizedFormatter({})", self.locale)
    }
}

impl From<&Locale> for LocalizedFormatter {
    fn from(locale: &Locale) -> Self {
        Self::new(locale)
    }
}

// ---------------------------------------------------------------------------
// TranslationCoverage – additional helpers
// ---------------------------------------------------------------------------

impl TranslationCoverage {
    /// Produce a one-line summary string, e.g. `"75.0% (3/4, missing 1)"`.
    pub fn summary(&self) -> String {
        let pct = self.percentage();
        if self.is_complete() {
            format!("{pct:.1}% ({}/{})", self.translated, self.total)
        } else {
            format!(
                "{pct:.1}% ({}/{}, missing {})",
                self.translated,
                self.total,
                self.missing.len()
            )
        }
    }
}

impl fmt::Display for TranslationCoverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
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

    // --- LanguagePackBundle tests ---

    #[test]
    fn bundle_translate_target_locale() {
        let mut bundle = LanguagePackBundle::new(Locale::parse("en"));
        let mut translations = HashMap::new();
        translations.insert("hello".to_string(), "Bonjour".to_string());
        bundle.add_pack(LanguagePack {
            locale: Locale::parse("fr"),
            translations,
        });
        let fr = Locale::parse("fr");
        assert_eq!(
            bundle.translate("hello", &fr),
            Some("Bonjour".to_string())
        );
    }

    #[test]
    fn bundle_translate_falls_back() {
        let mut bundle = LanguagePackBundle::new(Locale::parse("en"));
        let mut en_trans = HashMap::new();
        en_trans.insert("hello".to_string(), "Hello".to_string());
        bundle.add_pack(LanguagePack {
            locale: Locale::parse("en"),
            translations: en_trans,
        });
        bundle.add_pack(LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::new(),
        });
        let fr = Locale::parse("fr");
        assert_eq!(
            bundle.translate("hello", &fr),
            Some("Hello".to_string())
        );
    }

    #[test]
    fn bundle_coverage_for_calculation() {
        let mut bundle = LanguagePackBundle::new(Locale::parse("en"));
        let mut en_trans = HashMap::new();
        en_trans.insert("a".to_string(), "A".to_string());
        en_trans.insert("b".to_string(), "B".to_string());
        bundle.add_pack(LanguagePack {
            locale: Locale::parse("en"),
            translations: en_trans,
        });
        let mut fr_trans = HashMap::new();
        fr_trans.insert("a".to_string(), "A-fr".to_string());
        bundle.add_pack(LanguagePack {
            locale: Locale::parse("fr"),
            translations: fr_trans,
        });
        let fr = Locale::parse("fr");
        let cov = bundle.coverage_for(&fr);
        assert!((cov - 50.0).abs() < f64::EPSILON);
    }

    // --- langpack_detect_system_locale tests ---

    #[test]
    fn detect_system_locale_from_env() {
        // Save originals so we can restore them.
        let orig = std::env::var("LANG").ok();
        // SAFETY: test is single-threaded for this env var.
        unsafe { std::env::set_var("LANG", "fr_FR.UTF-8") };
        let locale = langpack_detect_system_locale();
        // Restore.
        match orig {
            Some(v) => unsafe { std::env::set_var("LANG", v) },
            None => unsafe { std::env::remove_var("LANG") },
        }
        assert_eq!(locale.language, "fr");
        assert_eq!(locale.country.as_deref(), Some("FR"));
    }

    // --- LocaleNegotiator tests ---

    #[test]
    fn negotiator_exact_match() {
        let neg = LocaleNegotiator::new(vec![
            "en-US".to_string(),
            "fr-FR".to_string(),
            "de".to_string(),
        ]);
        let result = neg.negotiate(&["fr-FR".to_string()]);
        assert_eq!(result, "fr-FR");
    }

    #[test]
    fn negotiator_language_only_match() {
        let neg = LocaleNegotiator::new(vec![
            "en-US".to_string(),
            "fr-FR".to_string(),
        ]);
        let result = neg.negotiate(&["fr".to_string()]);
        assert_eq!(result, "fr-FR");
    }

    // --- TranslationValidator tests ---

    #[test]
    fn validator_check_placeholders_match() {
        let v = TranslationValidator::new();
        assert!(v.check_placeholders("Hello {0}, you have {1} items", "Bonjour {0}, vous avez {1} articles"));
    }

    #[test]
    fn validator_check_placeholders_mismatch() {
        let v = TranslationValidator::new();
        assert!(!v.check_placeholders("Hello {0} {1}", "Bonjour {0}"));
    }

    // --- localize_with_args tests ---

    #[test]
    fn localize_with_args_substitutes() {
        let mut translations = HashMap::new();
        translations.insert(
            "greet".to_string(),
            "Hello {0}, welcome to {1}!".to_string(),
        );
        let pack = LanguagePack {
            locale: Locale::parse("en"),
            translations,
        };
        let result = localize_with_args(&pack, "greet", &["Alice", "Rust"], "fallback");
        assert_eq!(result, "Hello Alice, welcome to Rust!");
    }

    // -----------------------------------------------------------------------
    // LocaleChain tests
    // -----------------------------------------------------------------------

    #[test]
    fn locale_chain_resolve_walks_chain() {
        let mut reg = LanguagePackRegistry::new(Locale::parse("en"));
        reg.register(LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("greeting".into(), "Hello".into()),
                ("farewell".into(), "Goodbye".into()),
            ]),
        });
        reg.register(LanguagePack {
            locale: Locale::parse("pt"),
            translations: HashMap::from([("greeting".into(), "Olá".into())]),
        });
        reg.register(LanguagePack {
            locale: Locale::parse("pt-BR"),
            translations: HashMap::from([("greeting".into(), "Oi".into())]),
        });

        let chain = LocaleChain::new(&["pt-BR", "pt", "en"]);

        // "greeting" found in pt-BR (first in chain)
        assert_eq!(chain.resolve(&reg, "greeting"), Some("Oi".to_string()));
        // "farewell" missing from pt-BR and pt, found in en
        assert_eq!(chain.resolve(&reg, "farewell"), Some("Goodbye".to_string()));
        // completely missing key
        assert_eq!(chain.resolve(&reg, "unknown"), None);
        assert_eq!(chain.resolve_or(&reg, "unknown", "???"), "???");
    }

    #[test]
    fn locale_chain_from_locale_with_fallback() {
        let locale = Locale::parse("pt-BR");
        let chain = LocaleChain::from_locale_with_fallback(&locale, "en");
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.to_string(), "pt-BR -> pt -> en");
    }

    #[test]
    fn locale_chain_from_locale_no_duplicate_fallback() {
        let locale = Locale::parse("en");
        let chain = LocaleChain::from_locale_with_fallback(&locale, "en");
        // "en" should appear only once
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.to_string(), "en");
    }

    #[test]
    fn locale_chain_display_and_from() {
        let chain = LocaleChain::from(["fr-CA", "fr", "en"].as_slice());
        assert_eq!(chain.to_string(), "fr-CA -> fr -> en");
        assert!(!chain.is_empty());
    }

    // -----------------------------------------------------------------------
    // PluralRules tests
    // -----------------------------------------------------------------------

    #[test]
    fn plural_rules_english() {
        let rules = PluralRules::new(&Locale::parse("en"));
        assert_eq!(rules.select(0), PluralRule::Zero);
        assert_eq!(rules.select(1), PluralRule::One);
        assert_eq!(rules.select(2), PluralRule::Other);
        assert_eq!(rules.select(42), PluralRule::Other);
        assert_eq!(rules.to_string(), "PluralRules(en)");
    }

    #[test]
    fn plural_rules_french() {
        let rules = PluralRules::new(&Locale::parse("fr"));
        // French: 0 and 1 are "one"
        assert_eq!(rules.select(0), PluralRule::One);
        assert_eq!(rules.select(1), PluralRule::One);
        assert_eq!(rules.select(2), PluralRule::Other);
    }

    #[test]
    fn plural_rules_arabic() {
        let rules = PluralRules::new(&Locale::parse("ar"));
        assert_eq!(rules.select(0), PluralRule::Zero);
        assert_eq!(rules.select(1), PluralRule::One);
        assert_eq!(rules.select(2), PluralRule::Two);
        assert_eq!(rules.select(5), PluralRule::Few);
        assert_eq!(rules.select(11), PluralRule::Many);
        assert_eq!(rules.select(99), PluralRule::Many);
        assert_eq!(rules.select(100), PluralRule::Other);
    }

    #[test]
    fn plural_rules_polish() {
        let rules = PluralRules::new(&Locale::parse("pl"));
        assert_eq!(rules.select(0), PluralRule::Zero);
        assert_eq!(rules.select(1), PluralRule::One);
        assert_eq!(rules.select(2), PluralRule::Few);
        assert_eq!(rules.select(4), PluralRule::Few);
        assert_eq!(rules.select(5), PluralRule::Many);
        assert_eq!(rules.select(12), PluralRule::Many); // 12 is in 12..=14 range
        assert_eq!(rules.select(22), PluralRule::Few);
    }

    // -----------------------------------------------------------------------
    // LocalizedFormatter tests
    // -----------------------------------------------------------------------

    #[test]
    fn formatter_format_integer_english() {
        let fmt = LocalizedFormatter::new(&Locale::parse("en"));
        assert_eq!(fmt.format_integer(0), "0");
        assert_eq!(fmt.format_integer(999), "999");
        assert_eq!(fmt.format_integer(1000), "1,000");
        assert_eq!(fmt.format_integer(1_234_567), "1,234,567");
        assert_eq!(fmt.format_integer(-42_000), "-42,000");
    }

    #[test]
    fn formatter_format_integer_german() {
        let fmt = LocalizedFormatter::new(&Locale::parse("de"));
        assert_eq!(fmt.format_integer(1_234_567), "1.234.567");
    }

    #[test]
    fn formatter_format_decimal() {
        let en = LocalizedFormatter::new(&Locale::parse("en"));
        assert_eq!(en.format_decimal(1234.5, 2), "1,234.50");

        let de = LocalizedFormatter::new(&Locale::parse("de"));
        assert_eq!(de.format_decimal(1234.5, 2), "1.234,50");
    }

    #[test]
    fn formatter_format_date() {
        let en_us = LocalizedFormatter::new(&Locale::parse("en-US"));
        assert_eq!(en_us.format_date(2025, 3, 7), "03/07/2025");

        let fr = LocalizedFormatter::new(&Locale::parse("fr"));
        assert_eq!(fr.format_date(2025, 3, 7), "07/03/2025");

        let ja = LocalizedFormatter::new(&Locale::parse("ja"));
        assert_eq!(ja.format_date(2025, 3, 7), "2025-03-07");
    }

    #[test]
    fn formatter_format_list() {
        let en = LocalizedFormatter::new(&Locale::parse("en"));
        assert_eq!(en.format_list(&[]), "");
        assert_eq!(en.format_list(&["apple"]), "apple");
        assert_eq!(en.format_list(&["apple", "banana"]), "apple and banana");
        assert_eq!(
            en.format_list(&["apple", "banana", "cherry"]),
            "apple, banana, and cherry"
        );

        let fr = LocalizedFormatter::new(&Locale::parse("fr"));
        assert_eq!(fr.format_list(&["pomme", "banane"]), "pomme et banane");
    }

    #[test]
    fn formatter_display_and_from() {
        let locale = Locale::parse("es");
        let fmt = LocalizedFormatter::from(&locale);
        assert_eq!(fmt.to_string(), "LocalizedFormatter(es)");
    }

    // -----------------------------------------------------------------------
    // TranslationCoverage summary / Display tests
    // -----------------------------------------------------------------------

    #[test]
    fn translation_coverage_summary_complete() {
        let reference = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([("a".into(), "A".into())]),
        };
        let target = LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([("a".into(), "A-fr".into())]),
        };
        let cov = TranslationCoverage::compute(&reference, &target);
        assert_eq!(cov.to_string(), "100.0% (1/1)");
    }

    #[test]
    fn translation_coverage_summary_partial() {
        let reference = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A".into()),
                ("b".into(), "B".into()),
            ]),
        };
        let target = LanguagePack {
            locale: Locale::parse("de"),
            translations: HashMap::from([("a".into(), "A-de".into())]),
        };
        let cov = TranslationCoverage::compute(&reference, &target);
        assert_eq!(cov.to_string(), "50.0% (1/2, missing 1)");
    }
}
