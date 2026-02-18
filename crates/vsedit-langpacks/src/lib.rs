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

// ---------------------------------------------------------------------------
// LangPackResolver – find the best locale match from available locales
// ---------------------------------------------------------------------------

/// Resolves a requested locale against a set of available locales using
/// a two-tier strategy: exact match first, then language-only fallback.
#[derive(Debug, Clone)]
pub struct LangPackResolver {
    available: Vec<Locale>,
}

impl LangPackResolver {
    /// Create a resolver with the given set of available locales.
    pub fn new(available: Vec<Locale>) -> Self {
        Self { available }
    }

    /// Resolve a single requested locale.
    ///
    /// 1. Exact match (language + country).
    /// 2. Language-only match (first locale whose language matches).
    /// 3. `None` if nothing matches.
    pub fn resolve(&self, requested: &Locale) -> Option<&Locale> {
        // Exact match
        if let Some(loc) = self.available.iter().find(|l| {
            l.language == requested.language && l.country == requested.country
        }) {
            return Some(loc);
        }
        // Language-only fallback
        self.available
            .iter()
            .find(|l| l.language == requested.language)
    }

    /// Try each locale in `preferred` order, returning the first match.
    pub fn resolve_chain(&self, preferred: &[Locale]) -> Option<&Locale> {
        for req in preferred {
            if let Some(loc) = self.resolve(req) {
                return Some(loc);
            }
        }
        None
    }

    /// Return the unique set of language codes available.
    pub fn available_languages(&self) -> Vec<&str> {
        let mut langs: Vec<&str> = self
            .available
            .iter()
            .map(|l| l.language.as_str())
            .collect();
        langs.sort_unstable();
        langs.dedup();
        langs
    }
}

// ---------------------------------------------------------------------------
// LangPackMerger – combine translations from multiple packs
// ---------------------------------------------------------------------------

/// Merges translations from several [`LanguagePack`]s.  Packs added later
/// override keys from earlier packs.
#[derive(Debug, Clone)]
pub struct LangPackMerger {
    packs: Vec<(String, HashMap<String, String>)>,
}

impl LangPackMerger {
    pub fn new() -> Self {
        Self { packs: Vec::new() }
    }

    /// Record a pack's translations for merging.
    pub fn add_pack(&mut self, pack: &LanguagePack) {
        self.packs
            .push((pack.locale.id(), pack.translations.clone()));
    }

    /// Produce the merged translation map (later packs win).
    pub fn merge(&self) -> HashMap<String, String> {
        let mut merged = HashMap::new();
        for (_id, translations) in &self.packs {
            for (k, v) in translations {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged
    }

    /// Total number of unique keys across all packs.
    pub fn key_count(&self) -> usize {
        self.merge().len()
    }

    /// Keys that appear in more than one pack with *different* values.
    pub fn conflicts(&self) -> Vec<String> {
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut conflicts: Vec<String> = Vec::new();
        for (_id, translations) in &self.packs {
            for (k, v) in translations {
                if let Some(prev) = seen.get(k) {
                    if prev != v && !conflicts.contains(k) {
                        conflicts.push(k.clone());
                    }
                } else {
                    seen.insert(k.clone(), v.clone());
                }
            }
        }
        conflicts.sort();
        conflicts
    }
}

impl Default for LangPackMerger {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LangPackValidator / ValidationReport – validate against a reference
// ---------------------------------------------------------------------------

/// Report produced by [`LangPackValidator::validate`].
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Keys present in the reference but missing from the pack.
    pub missing_keys: Vec<String>,
    /// Keys present in the pack but absent from the reference.
    pub extra_keys: Vec<String>,
    /// Keys whose values are empty strings.
    pub empty_values: Vec<String>,
}

impl ValidationReport {
    /// A pack is valid when there are no missing keys and no empty values.
    pub fn is_valid(&self) -> bool {
        self.missing_keys.is_empty() && self.empty_values.is_empty()
    }

    /// Human-readable summary line.
    pub fn summary(&self) -> String {
        if self.is_valid() && self.extra_keys.is_empty() {
            return "validation passed".to_string();
        }
        let mut parts: Vec<String> = Vec::new();
        if !self.missing_keys.is_empty() {
            parts.push(format!("{} missing", self.missing_keys.len()));
        }
        if !self.extra_keys.is_empty() {
            parts.push(format!("{} extra", self.extra_keys.len()));
        }
        if !self.empty_values.is_empty() {
            parts.push(format!("{} empty", self.empty_values.len()));
        }
        parts.join(", ")
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Validates a [`LanguagePack`] against a reference set of keys.
#[derive(Debug, Clone)]
pub struct LangPackValidator {
    reference_keys: Vec<String>,
}

impl LangPackValidator {
    pub fn new(reference_keys: Vec<String>) -> Self {
        Self { reference_keys }
    }

    /// Check `pack` against the reference keys and return a report.
    pub fn validate(&self, pack: &LanguagePack) -> ValidationReport {
        let mut missing_keys: Vec<String> = self
            .reference_keys
            .iter()
            .filter(|k| !pack.translations.contains_key(k.as_str()))
            .cloned()
            .collect();
        missing_keys.sort();

        let ref_set: std::collections::HashSet<&str> =
            self.reference_keys.iter().map(|s| s.as_str()).collect();
        let mut extra_keys: Vec<String> = pack
            .translations
            .keys()
            .filter(|k| !ref_set.contains(k.as_str()))
            .cloned()
            .collect();
        extra_keys.sort();

        let mut empty_values: Vec<String> = pack
            .translations
            .iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(k, _)| k.clone())
            .collect();
        empty_values.sort();

        ValidationReport {
            missing_keys,
            extra_keys,
            empty_values,
        }
    }
}

// ---------------------------------------------------------------------------
// RuntimeLocalizer – runtime string lookup with fallback
// ---------------------------------------------------------------------------

/// Provides runtime translation lookups with optional fallback pack.
#[derive(Debug, Clone)]
pub struct RuntimeLocalizer {
    primary: LanguagePack,
    fallback: Option<LanguagePack>,
}

impl RuntimeLocalizer {
    pub fn new(primary: LanguagePack) -> Self {
        Self {
            primary,
            fallback: None,
        }
    }

    /// Set a fallback pack (builder pattern).
    pub fn with_fallback(mut self, fb: LanguagePack) -> Self {
        self.fallback = Some(fb);
        self
    }

    /// Look up `key` in the primary pack, then fallback, then return the key
    /// itself as a last resort.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(v) = self.primary.translations.get(key) {
            return v.as_str();
        }
        if let Some(fb) = &self.fallback {
            if let Some(v) = fb.translations.get(key) {
                return v.as_str();
            }
        }
        key
    }

    /// Look up `key` and replace `{name}` placeholders with supplied args.
    pub fn get_formatted(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self.get(key);
        let mut result = template.to_string();
        for (name, value) in args {
            let placeholder = format!("{{{}}}", name);
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Check whether `key` exists in primary or fallback.
    pub fn has_key(&self, key: &str) -> bool {
        self.primary.translations.contains_key(key)
            || self
                .fallback
                .as_ref()
                .map_or(false, |fb| fb.translations.contains_key(key))
    }
}

// ---------------------------------------------------------------------------
// LangPackDownloadProgress – tracks download progress with bytes/percentage
// ---------------------------------------------------------------------------

/// State of a language pack download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPhase {
    /// Waiting to start.
    Pending,
    /// Actively downloading.
    Downloading,
    /// Verifying integrity after download.
    Verifying,
    /// Download completed successfully.
    Completed,
    /// Download failed.
    Failed,
}

/// Tracks the progress of downloading a language pack.
#[derive(Debug, Clone)]
pub struct LangPackDownloadProgress {
    pack_id: String,
    locale_id: String,
    total_bytes: u64,
    downloaded_bytes: u64,
    phase: DownloadPhase,
    error_message: Option<String>,
    started_at: Option<u64>,
    completed_at: Option<u64>,
}

impl LangPackDownloadProgress {
    /// Create a new download progress tracker.
    pub fn new(pack_id: impl Into<String>, locale_id: impl Into<String>, total_bytes: u64) -> Self {
        Self {
            pack_id: pack_id.into(),
            locale_id: locale_id.into(),
            total_bytes,
            downloaded_bytes: 0,
            phase: DownloadPhase::Pending,
            error_message: None,
            started_at: None,
            completed_at: None,
        }
    }

    /// Begin the download, recording the start timestamp.
    pub fn start(&mut self, timestamp: u64) {
        self.phase = DownloadPhase::Downloading;
        self.started_at = Some(timestamp);
    }

    /// Record additional downloaded bytes.  Clamps to `total_bytes`.
    pub fn advance(&mut self, bytes: u64) {
        self.downloaded_bytes = (self.downloaded_bytes + bytes).min(self.total_bytes);
    }

    /// Percentage complete (0–100).
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.downloaded_bytes as f64 / self.total_bytes as f64) * 100.0
    }

    /// Remaining bytes to download.
    pub fn remaining_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.downloaded_bytes)
    }

    /// Transition to verification phase.
    pub fn begin_verify(&mut self) {
        self.phase = DownloadPhase::Verifying;
    }

    /// Mark the download as completed.
    pub fn complete(&mut self, timestamp: u64) {
        self.downloaded_bytes = self.total_bytes;
        self.phase = DownloadPhase::Completed;
        self.completed_at = Some(timestamp);
    }

    /// Mark the download as failed with an error message.
    pub fn fail(&mut self, message: impl Into<String>) {
        self.phase = DownloadPhase::Failed;
        self.error_message = Some(message.into());
    }

    /// Whether the download finished (success or failure).
    pub fn is_finished(&self) -> bool {
        matches!(self.phase, DownloadPhase::Completed | DownloadPhase::Failed)
    }

    /// Duration in seconds if both start and end are known.
    pub fn elapsed_secs(&self) -> Option<u64> {
        match (self.started_at, self.completed_at) {
            (Some(s), Some(e)) => Some(e.saturating_sub(s)),
            _ => None,
        }
    }

    pub fn pack_id(&self) -> &str { &self.pack_id }
    pub fn locale_id(&self) -> &str { &self.locale_id }
    pub fn total_bytes(&self) -> u64 { self.total_bytes }
    pub fn downloaded_bytes(&self) -> u64 { self.downloaded_bytes }
    pub fn phase(&self) -> DownloadPhase { self.phase }
    pub fn error_message(&self) -> Option<&str> { self.error_message.as_deref() }

    /// Render a simple text progress bar of given width.
    pub fn render_bar(&self, width: usize) -> String {
        let filled = if self.total_bytes == 0 {
            width
        } else {
            ((self.downloaded_bytes as f64 / self.total_bytes as f64) * width as f64) as usize
        };
        let empty = width.saturating_sub(filled);
        format!("[{}{}]", "#".repeat(filled), "-".repeat(empty))
    }
}

impl fmt::Display for LangPackDownloadProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) {:.1}% {:?}",
            self.pack_id,
            self.locale_id,
            self.percentage(),
            self.phase,
        )
    }
}

// ---------------------------------------------------------------------------
// LangPackCompatibilityChecker – checks pack compatibility with editor versions
// ---------------------------------------------------------------------------

/// Result of a compatibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityVerdict {
    /// The pack is fully compatible.
    Compatible,
    /// The pack works but may have minor issues.
    Degraded(String),
    /// The pack is not compatible.
    Incompatible(String),
}

/// A simple semantic version triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Parse "major.minor.patch". Returns `None` on bad input.
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self { major, minor, patch })
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Checks whether a language pack is compatible with a given editor version.
#[derive(Debug)]
pub struct LangPackCompatibilityChecker {
    editor_version: SemVer,
    min_pack_api: SemVer,
}

impl LangPackCompatibilityChecker {
    pub fn new(editor_version: SemVer, min_pack_api: SemVer) -> Self {
        Self { editor_version, min_pack_api }
    }

    /// Check a pack that declares its own required API version and target editor version.
    pub fn check(&self, pack_api_version: SemVer, pack_target_editor: SemVer) -> CompatibilityVerdict {
        if pack_api_version < self.min_pack_api {
            return CompatibilityVerdict::Incompatible(format!(
                "pack API {pack_api_version} < minimum {}", self.min_pack_api
            ));
        }
        if pack_target_editor.major != self.editor_version.major {
            return CompatibilityVerdict::Incompatible(format!(
                "major version mismatch: pack targets {}, editor is {}",
                pack_target_editor.major, self.editor_version.major
            ));
        }
        if pack_target_editor.minor > self.editor_version.minor {
            return CompatibilityVerdict::Degraded(format!(
                "pack targets newer minor {}, editor is {}",
                pack_target_editor.minor, self.editor_version.minor
            ));
        }
        CompatibilityVerdict::Compatible
    }

    /// Batch-check multiple packs, returning only compatible/degraded ones.
    pub fn filter_compatible(&self, packs: &[(SemVer, SemVer)]) -> Vec<(usize, CompatibilityVerdict)> {
        packs
            .iter()
            .enumerate()
            .filter_map(|(i, (api, target))| {
                let v = self.check(*api, *target);
                if matches!(v, CompatibilityVerdict::Incompatible(_)) {
                    None
                } else {
                    Some((i, v))
                }
            })
            .collect()
    }

    pub fn editor_version(&self) -> SemVer { self.editor_version }
}



/// Language pack configuration manager.
#[derive(Debug, Clone)]
pub struct LangpacksConfig {
    entries: Vec<LangpacksEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single language pack entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LangpacksEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl LangpacksEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl LangpacksConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: LangpacksEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&LangpacksEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LangpacksEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&LangpacksEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&LangpacksEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&LangpacksEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<LangpacksEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for langpacks
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaLangpacksRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaLangpacksRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaLangpacksCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaLangpacksCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaLangpacksCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 107
// ---------------------------------------------------------------------------

/// Generic object pool `Xc107Pool<T>`.
pub struct Xc107Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc107Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc107PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc107Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc107PoolStats {
        Xc107PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc107Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc107Scheduler`.
pub struct Xc107Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc107Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc107Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_107 hash for the given byte slice.
pub fn xc_107_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_107 convention.
pub fn xc_107_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_55 deepening: state machine + event bus ---

/// States for the Xd55 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd55State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd55State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd55Transition {
    pub from: Xd55State,
    pub to: Xd55State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd55StateMachine {
    current: Xd55State,
    history: Vec<Xd55Transition>,
    step_counter: usize,
}

impl Xd55StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd55State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd55State {
        self.current
    }

    pub fn history(&self) -> &[Xd55Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd55State) -> Result<Xd55State, String> {
        let allowed = match (self.current, target) {
            (Xd55State::Idle, Xd55State::Running) => true,
            (Xd55State::Running, Xd55State::Paused) => true,
            (Xd55State::Running, Xd55State::Done) => true,
            (Xd55State::Paused, Xd55State::Running) => true,
            (Xd55State::Paused, Xd55State::Done) => true,
            (Xd55State::Done, Xd55State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_55: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd55Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd55SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd55State> {
        let prefix = "Xd55SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd55State::Idle),
            "Running" => Some(Xd55State::Running),
            "Paused" => Some(Xd55State::Paused),
            "Done" => Some(Xd55State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd55State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd55 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd55Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd55Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd55HandlerFn = Box<dyn Fn(&Xd55Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd55EventBus {
    handlers: Vec<(usize, Option<String>, Xd55HandlerFn)>,
    next_id: usize,
    published: Vec<Xd55Event>,
}

impl Xd55EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd55Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd55Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd55Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd55Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #53
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf53Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf53TrieNode {
    children: std::collections::HashMap<char, Xf53TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf53Trie {
    root: Xf53TrieNode,
    count: usize,
}

impl Xf53Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf53TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf53TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf53TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf53BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf53BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
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

    // -----------------------------------------------------------------------
    // LangPackResolver tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolver_exact_match() {
        let resolver = LangPackResolver::new(vec![
            Locale::parse("en-US"),
            Locale::parse("en-GB"),
            Locale::parse("fr"),
        ]);
        let req = Locale::parse("en-GB");
        let resolved = resolver.resolve(&req).unwrap();
        assert_eq!(resolved.to_string(), "en-GB");
    }

    #[test]
    fn resolver_language_fallback() {
        let resolver = LangPackResolver::new(vec![
            Locale::parse("en-US"),
            Locale::parse("fr-FR"),
        ]);
        let req = Locale::parse("fr-CA");
        let resolved = resolver.resolve(&req).unwrap();
        assert_eq!(resolved.language, "fr");
    }

    #[test]
    fn resolver_no_match() {
        let resolver = LangPackResolver::new(vec![Locale::parse("en-US")]);
        assert!(resolver.resolve(&Locale::parse("de")).is_none());
    }

    #[test]
    fn resolver_chain() {
        let resolver = LangPackResolver::new(vec![
            Locale::parse("en-US"),
            Locale::parse("de"),
        ]);
        let prefs = vec![Locale::parse("ja"), Locale::parse("de")];
        let resolved = resolver.resolve_chain(&prefs).unwrap();
        assert_eq!(resolved.to_string(), "de");
    }

    #[test]
    fn resolver_available_languages() {
        let resolver = LangPackResolver::new(vec![
            Locale::parse("en-US"),
            Locale::parse("en-GB"),
            Locale::parse("fr"),
        ]);
        let langs = resolver.available_languages();
        assert_eq!(langs, vec!["en", "fr"]);
    }

    // -----------------------------------------------------------------------
    // LangPackMerger tests
    // -----------------------------------------------------------------------

    #[test]
    fn merger_later_overrides() {
        let pack1 = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("greet".into(), "Hello".into()),
                ("bye".into(), "Goodbye".into()),
            ]),
        };
        let pack2 = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([("greet".into(), "Hi".into())]),
        };
        let mut merger = LangPackMerger::new();
        merger.add_pack(&pack1);
        merger.add_pack(&pack2);
        let merged = merger.merge();
        assert_eq!(merged.get("greet").unwrap(), "Hi");
        assert_eq!(merged.get("bye").unwrap(), "Goodbye");
        assert_eq!(merger.key_count(), 2);
    }

    #[test]
    fn merger_conflicts() {
        let pack1 = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A1".into()),
                ("b".into(), "B".into()),
            ]),
        };
        let pack2 = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A2".into()),
                ("b".into(), "B".into()),
            ]),
        };
        let mut merger = LangPackMerger::new();
        merger.add_pack(&pack1);
        merger.add_pack(&pack2);
        let conflicts = merger.conflicts();
        assert_eq!(conflicts, vec!["a"]);
    }

    // -----------------------------------------------------------------------
    // LangPackValidator tests
    // -----------------------------------------------------------------------

    #[test]
    fn validator_all_present() {
        let validator =
            LangPackValidator::new(vec!["greet".into(), "bye".into()]);
        let pack = LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([
                ("greet".into(), "Bonjour".into()),
                ("bye".into(), "Au revoir".into()),
            ]),
        };
        let report = validator.validate(&pack);
        assert!(report.is_valid());
        assert_eq!(report.to_string(), "validation passed");
    }

    #[test]
    fn validator_missing_and_extra() {
        let validator =
            LangPackValidator::new(vec!["a".into(), "b".into(), "c".into()]);
        let pack = LanguagePack {
            locale: Locale::parse("de"),
            translations: HashMap::from([
                ("a".into(), "A".into()),
                ("d".into(), "D".into()),
            ]),
        };
        let report = validator.validate(&pack);
        assert!(!report.is_valid());
        assert_eq!(report.missing_keys, vec!["b", "c"]);
        assert_eq!(report.extra_keys, vec!["d"]);
    }

    #[test]
    fn validator_empty_values() {
        let validator = LangPackValidator::new(vec!["x".into()]);
        let pack = LanguagePack {
            locale: Locale::parse("ja"),
            translations: HashMap::from([("x".into(), "".into())]),
        };
        let report = validator.validate(&pack);
        assert!(!report.is_valid());
        assert_eq!(report.empty_values, vec!["x"]);
    }

    // -----------------------------------------------------------------------
    // RuntimeLocalizer tests
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_localizer_primary_lookup() {
        let pack = LanguagePack {
            locale: Locale::parse("es"),
            translations: HashMap::from([("hello".into(), "Hola".into())]),
        };
        let loc = RuntimeLocalizer::new(pack);
        assert_eq!(loc.get("hello"), "Hola");
        assert_eq!(loc.get("missing"), "missing");
    }

    #[test]
    fn runtime_localizer_fallback() {
        let primary = LanguagePack {
            locale: Locale::parse("fr"),
            translations: HashMap::from([("a".into(), "A-fr".into())]),
        };
        let fallback = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([
                ("a".into(), "A-en".into()),
                ("b".into(), "B-en".into()),
            ]),
        };
        let loc = RuntimeLocalizer::new(primary).with_fallback(fallback);
        assert_eq!(loc.get("a"), "A-fr");
        assert_eq!(loc.get("b"), "B-en");
        assert!(loc.has_key("b"));
        assert!(!loc.has_key("z"));
    }

    #[test]
    fn runtime_localizer_formatted() {
        let pack = LanguagePack {
            locale: Locale::parse("en"),
            translations: HashMap::from([(
                "welcome".into(),
                "Hello, {user}! You have {count} messages.".into(),
            )]),
        };
        let loc = RuntimeLocalizer::new(pack);
        let msg = loc.get_formatted("welcome", &[("user", "Alice"), ("count", "5")]);
        assert_eq!(msg, "Hello, Alice! You have 5 messages.");
    }

    #[test]
    fn download_progress_new() {
        let p = LangPackDownloadProgress::new("pack1", "en-US", 1000);
        assert_eq!(p.pack_id(), "pack1");
        assert_eq!(p.locale_id(), "en-US");
        assert_eq!(p.total_bytes(), 1000);
        assert_eq!(p.downloaded_bytes(), 0);
        assert_eq!(p.phase(), DownloadPhase::Pending);
    }

    #[test]
    fn download_progress_advance() {
        let mut p = LangPackDownloadProgress::new("p", "en", 500);
        p.start(100);
        p.advance(200);
        assert_eq!(p.downloaded_bytes(), 200);
        assert!((p.percentage() - 40.0).abs() < 0.01);
        assert_eq!(p.remaining_bytes(), 300);
    }

    #[test]
    fn download_progress_clamp() {
        let mut p = LangPackDownloadProgress::new("p", "en", 100);
        p.advance(9999);
        assert_eq!(p.downloaded_bytes(), 100);
        assert!((p.percentage() - 100.0).abs() < 0.01);
    }

    #[test]
    fn download_progress_complete_and_elapsed() {
        let mut p = LangPackDownloadProgress::new("p", "en", 100);
        p.start(10);
        p.advance(50);
        p.complete(20);
        assert!(p.is_finished());
        assert_eq!(p.elapsed_secs(), Some(10));
        assert_eq!(p.downloaded_bytes(), 100);
    }

    #[test]
    fn download_progress_fail() {
        let mut p = LangPackDownloadProgress::new("p", "en", 100);
        p.fail("network error");
        assert!(p.is_finished());
        assert_eq!(p.error_message(), Some("network error"));
    }

    #[test]
    fn download_progress_render_bar() {
        let mut p = LangPackDownloadProgress::new("p", "en", 200);
        p.advance(100);
        let bar = p.render_bar(10);
        assert_eq!(bar, "[#####-----]");
    }

    #[test]
    fn download_progress_zero_total() {
        let p = LangPackDownloadProgress::new("p", "en", 0);
        assert!((p.percentage() - 100.0).abs() < 0.01);
        assert_eq!(p.render_bar(4), "[####]");
    }

    #[test]
    fn download_progress_display() {
        let p = LangPackDownloadProgress::new("pack1", "fr", 1000);
        let s = format!("{p}");
        assert!(s.contains("pack1"));
        assert!(s.contains("fr"));
    }

    #[test]
    fn semver_parse_valid() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v, SemVer::new(1, 2, 3));
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn semver_parse_invalid() {
        assert!(SemVer::parse("1.2").is_none());
        assert!(SemVer::parse("a.b.c").is_none());
        assert!(SemVer::parse("1.2.3.4").is_none());
    }

    #[test]
    fn compatibility_checker_compatible() {
        let checker = LangPackCompatibilityChecker::new(
            SemVer::new(2, 5, 0),
            SemVer::new(1, 0, 0),
        );
        let result = checker.check(SemVer::new(1, 0, 0), SemVer::new(2, 3, 0));
        assert_eq!(result, CompatibilityVerdict::Compatible);
    }

    #[test]
    fn compatibility_checker_incompatible_api() {
        let checker = LangPackCompatibilityChecker::new(
            SemVer::new(2, 5, 0),
            SemVer::new(2, 0, 0),
        );
        let result = checker.check(SemVer::new(1, 0, 0), SemVer::new(2, 5, 0));
        assert!(matches!(result, CompatibilityVerdict::Incompatible(_)));
    }

    #[test]
    fn compatibility_checker_major_mismatch() {
        let checker = LangPackCompatibilityChecker::new(
            SemVer::new(2, 5, 0),
            SemVer::new(1, 0, 0),
        );
        let result = checker.check(SemVer::new(1, 0, 0), SemVer::new(3, 0, 0));
        assert!(matches!(result, CompatibilityVerdict::Incompatible(_)));
    }

    #[test]
    fn compatibility_checker_degraded() {
        let checker = LangPackCompatibilityChecker::new(
            SemVer::new(2, 5, 0),
            SemVer::new(1, 0, 0),
        );
        let result = checker.check(SemVer::new(1, 0, 0), SemVer::new(2, 8, 0));
        assert!(matches!(result, CompatibilityVerdict::Degraded(_)));
    }

    #[test]
    fn compatibility_filter_compatible() {
        let checker = LangPackCompatibilityChecker::new(
            SemVer::new(2, 5, 0),
            SemVer::new(1, 0, 0),
        );
        let packs = vec![
            (SemVer::new(1, 0, 0), SemVer::new(2, 3, 0)), // compatible
            (SemVer::new(0, 5, 0), SemVer::new(2, 5, 0)), // incompatible (api too old)
            (SemVer::new(1, 0, 0), SemVer::new(2, 7, 0)), // degraded
        ];
        let result = checker.filter_compatible(&packs);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, 0);
        assert_eq!(result[1].0, 2);
    }


    #[test]
    fn langpacks_entry_creation() {
        let e = LangpacksEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn langpacks_entry_with_priority() {
        let e = LangpacksEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn langpacks_entry_metadata() {
        let e = LangpacksEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn langpacks_entry_remove_meta() {
        let mut e = LangpacksEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn langpacks_entry_activate_deactivate() {
        let mut e = LangpacksEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn langpacks_config_add_sorted() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("lo", "Lo").with_priority(1));
        c.add(LangpacksEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn langpacks_config_capacity() {
        let mut c = LangpacksConfig::new(1);
        assert!(c.add(LangpacksEntry::new("a", "A")));
        assert!(!c.add(LangpacksEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn langpacks_config_remove() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn langpacks_config_get() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn langpacks_config_active_entries() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "A"));
        c.add(LangpacksEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn langpacks_config_enable_disable() {
        let mut c = LangpacksConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn langpacks_config_clear() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn langpacks_config_find_by_label() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn langpacks_config_top_n() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "A").with_priority(1));
        c.add(LangpacksEntry::new("b", "B").with_priority(2));
        c.add(LangpacksEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn langpacks_config_deactivate_activate_all() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "A"));
        c.add(LangpacksEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn langpacks_config_highest_priority() {
        let mut c = LangpacksConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(LangpacksEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn langpacks_config_contains() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn langpacks_config_labels() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "Alpha"));
        c.add(LangpacksEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn langpacks_config_drain_inactive() {
        let mut c = LangpacksConfig::new(10);
        c.add(LangpacksEntry::new("a", "A"));
        c.add(LangpacksEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for langpacks
    #[test]
    fn xa_langpacks_ring_new() {
        let rb = super::XaLangpacksRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_langpacks_ring_push_len() {
        let mut rb = super::XaLangpacksRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_langpacks_ring_wrap() {
        let mut rb = super::XaLangpacksRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_langpacks_ring_mean_empty() {
        let rb = super::XaLangpacksRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_langpacks_ring_mean_values() {
        let mut rb = super::XaLangpacksRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_langpacks_ring_min_max() {
        let mut rb = super::XaLangpacksRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_langpacks_ring_iter() {
        let mut rb = super::XaLangpacksRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_langpacks_counter_new() {
        let c = super::XaLangpacksCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_langpacks_counter_inc() {
        let mut c = super::XaLangpacksCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_langpacks_counter_inc_by() {
        let mut c = super::XaLangpacksCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_langpacks_counter_reset() {
        let mut c = super::XaLangpacksCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_langpacks_counter_clear() {
        let mut c = super::XaLangpacksCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_langpacks_counter_default() {
        let c = super::XaLangpacksCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 107 ----

    #[test]
    fn xc_107_pool_new_empty() {
        let pool: super::Xc107Pool<i32> = super::Xc107Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_107_pool_release_acquire() {
        let mut pool = super::Xc107Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_107_pool_acquire_empty() {
        let mut pool: super::Xc107Pool<i32> = super::Xc107Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_107_pool_full() {
        let mut pool = super::Xc107Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_107_pool_drain() {
        let mut pool = super::Xc107Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_107_pool_stats() {
        let mut pool = super::Xc107Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_107_pool_clear() {
        let mut pool = super::Xc107Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_107_pool_shrink() {
        let mut pool = super::Xc107Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_107_pool_default() {
        let pool: super::Xc107Pool<String> = super::Xc107Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_107_pool_extend() {
        let mut pool = super::Xc107Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_107_pool_retain() {
        let mut pool = super::Xc107Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_107_scheduler_round_robin() {
        let mut sched = super::Xc107Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_107_scheduler_empty() {
        let mut sched = super::Xc107Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_107_scheduler_reset() {
        let mut sched = super::Xc107Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_107_scheduler_add_remove() {
        let mut sched = super::Xc107Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_107_scheduler_targets() {
        let sched = super::Xc107Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_107_hash_empty() {
        assert_eq!(super::xc_107_hash(b""), 5381);
    }

    #[test]
    fn xc_107_hash_data() {
        let h = super::xc_107_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_107_hash(b"hello"), h);
    }

    #[test]
    fn xc_107_reverse_str() {
        assert_eq!(super::xc_107_reverse("abc"), "cba");
        assert_eq!(super::xc_107_reverse(""), "");
    }


    // --- xd_55 deepening tests ---

    #[test]
    fn xd_55_sm_initial_state() {
        let sm = Xd55StateMachine::new();
        assert_eq!(sm.current_state(), Xd55State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_55_sm_valid_idle_to_running() {
        let mut sm = Xd55StateMachine::new();
        assert!(sm.transition(Xd55State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd55State::Running);
    }

    #[test]
    fn xd_55_sm_valid_running_to_paused() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        assert!(sm.transition(Xd55State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd55State::Paused);
    }

    #[test]
    fn xd_55_sm_valid_running_to_done() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        assert!(sm.transition(Xd55State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd55State::Done);
    }

    #[test]
    fn xd_55_sm_valid_paused_to_running() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        sm.transition(Xd55State::Paused).unwrap();
        assert!(sm.transition(Xd55State::Running).is_ok());
    }

    #[test]
    fn xd_55_sm_valid_done_to_idle() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        sm.transition(Xd55State::Done).unwrap();
        assert!(sm.transition(Xd55State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd55State::Idle);
    }

    #[test]
    fn xd_55_sm_invalid_idle_to_done() {
        let mut sm = Xd55StateMachine::new();
        assert!(sm.transition(Xd55State::Done).is_err());
    }

    #[test]
    fn xd_55_sm_invalid_idle_to_paused() {
        let mut sm = Xd55StateMachine::new();
        assert!(sm.transition(Xd55State::Paused).is_err());
    }

    #[test]
    fn xd_55_sm_history_tracking() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        sm.transition(Xd55State::Paused).unwrap();
        sm.transition(Xd55State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd55State::Idle);
        assert_eq!(sm.history()[0].to, Xd55State::Running);
        assert_eq!(sm.history()[1].from, Xd55State::Running);
        assert_eq!(sm.history()[2].to, Xd55State::Done);
    }

    #[test]
    fn xd_55_sm_serialize_deserialize() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd55StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd55State::Running));
    }

    #[test]
    fn xd_55_sm_deserialize_invalid() {
        assert_eq!(Xd55StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_55_sm_reset() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd55State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_55_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd55EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd55Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_55_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd55EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd55Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd55Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_55_bus_unsubscribe() {
        let mut bus = Xd55EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_55_event_kind_and_payload() {
        let e = Xd55Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd55Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_55_bus_clear_history() {
        let mut bus = Xd55EventBus::new();
        bus.publish(Xd55Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_55_sm_step_counter_increments() {
        let mut sm = Xd55StateMachine::new();
        sm.transition(Xd55State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd55State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #53 --

    #[test]
    fn xf53_trie_insert_search() {
        let mut t = Xf53Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf53_trie_starts_with() {
        let mut t = Xf53Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf53_trie_remove() {
        let mut t = Xf53Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf53_trie_word_count() {
        let mut t = Xf53Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf53_trie_longest_prefix() {
        let mut t = Xf53Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf53_trie_all_words() {
        let mut t = Xf53Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf53_trie_autocomplete() {
        let mut t = Xf53Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf53_trie_empty_search() {
        let t = Xf53Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf53_bloom_add_contains() {
        let mut bf = Xf53BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf53_bloom_probably_absent() {
        let bf = Xf53BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf53_bloom_false_positive_rate() {
        let mut bf = Xf53BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf53_bloom_clear() {
        let mut bf = Xf53BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf53_bloom_union() {
        let mut a = Xf53BloomFilter::xf_new(512, 2);
        let mut b = Xf53BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf53_bloom_intersection_estimate() {
        let mut a = Xf53BloomFilter::xf_new(512, 2);
        let mut b = Xf53BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf53_bloom_union_size_mismatch() {
        let a = Xf53BloomFilter::xf_new(256, 2);
        let b = Xf53BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
