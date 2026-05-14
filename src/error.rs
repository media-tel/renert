//! Ошибки публичного API крейта `RENERT` (в коде зависимостей — `renert`).
//!
//! Здесь собраны [`enum@Error`] и вложенные типы ([`DictError`], [`MorphError`] и т.д.).
//! Они возникают в [`crate::init`], [`crate::load`], при открытии ресурсов (`*_::open()`) и при
//! валидации правил.
//!
//! ## Алиас [`Result`]
//!
//! [`Result`] — это [`std::result::Result`] с типом ошибки [`enum@Error`]. В коде, который зависит от
//! пакета `RENERT`, алиас доступен как **`renert::error::Result<T>`**. Его имеет смысл
//! использовать там, где вы пробрасываете ошибки библиотеки оператором `?`: все варианты из
//! публичных функций приводятся к одному [`enum@Error`], и компилятор не заставляет перечислять каждый
//! конкретный подтип.
//!
//! Это **не обязательный** контракт для всего вашего приложения: можно писать
//! `std::result::Result<T, renert::error::Error>` явно или преобразовывать [`enum@Error`] в свой
//! тип (`thiserror`, `anyhow` и т.п.). Алиас лишь делает сигнатуры короче и единообразнее для
//! слоя, который напрямую вызывает RENERT.
//!
//! Удобный импорт (в том числе в примерах в документации крейта):
//!
//! ```rust
//! use renert::error;
//!
//! fn run() -> error::Result<()> {
//!     renert::init("dict.xml", "data/dict")?;
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;

use crate::rule::builder::RuleId;
use thiserror::Error;

/// Краткая запись для [`std::result::Result`], у которой тип ошибки — [`enum@Error`].
///
/// В зависимостях Cargo это **`renert::error::Result<T>`** — рекомендуемый возвращаемый тип
/// для функций, которые вызывают публичный API RENERT и пробрасывают его ошибки через `?`.
/// Так вы фиксируете один верхнеуровневый тип сбоя и упрощаете сигнатуры по сравнению с полным
/// путём к [`enum@Error`].
///
/// Для приложений с собственной иерархией ошибок алиас не навязывается: достаточно принимать
/// или мапить [`enum@Error`] там, где это нужно.
pub type Result<T> = std::result::Result<T, Error>;

/// Верхнеуровневая ошибка библиотеки.
///
/// Объединяет только те виды ошибок, которые реально возникают
/// в публичных entrypoint-функциях: [`crate::init`], `open`, `validate`.
///
/// Ошибки интерпретации и фактов ([`crate::interpretation::InterpretatorError`],
/// [`crate::interpretation::FactError`]) остаются отдельными публичными типами в модуле
/// [`interpretation`](crate::interpretation): они не преобразуются в этот [`enum@Error`] и
/// обрабатываются на слое извлечения фактов / интерпретаторов.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Dict(#[from] DictError),
    #[error(transparent)]
    Morph(#[from] MorphError),
    #[error(transparent)]
    Tokenizer(#[from] TokenizerError),
    #[error(transparent)]
    RuleValidation(#[from] RuleValidationError),
}

/// Ошибки поиска/подготовки словаря.
#[derive(Debug, Error)]
pub enum DictError {
    #[error("XML file does not exist: {path}")]
    XmlFileDoesNotExist { path: PathBuf },
    #[error("XML path is not a file: {path}")]
    XmlPathIsNotFile { path: PathBuf },
    #[error("Failed to create cache dir {path}: {source}")]
    CreateCacheDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Dictionary build thread panicked")]
    BuildThreadPanicked,
    #[error("Failed to build dictionary from {xml}: {source}")]
    BuildDictionaryFailed {
        xml: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error("YARGY_DICT_DIR={value} is set but dict.json+dict.fst not found there")]
    InvalidEnvDictDir { value: String },
    #[error("Directory {path} must contain both dict.json and dict.fst")]
    DictDirIncomplete { path: PathBuf },
    #[error(
        "dict.json+dict.fst not found in expected locations:\n  - YARGY_DICT_DIR (not set)\n  - {primary}\n  - {fallback}\nHint: call renert::init(xml, cache_dir) or renert::load(dict_dir) before using the parser, or set the YARGY_DICT_DIR environment variable."
    )]
    DictFilesNotFound { primary: PathBuf, fallback: PathBuf },
}

/// Ошибки морфологического анализатора.
#[derive(Debug, Error)]
pub enum MorphError {
    #[error(transparent)]
    Dict(#[from] DictError),
    #[error("Failed to open morph dict at {path}: {source}")]
    OpenDictFailed {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error("Invalid grammeme: {grammem}")]
    InvalidGrammeme { grammem: String },
}

/// Ошибки `MorphTokenizer`.
#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error(transparent)]
    Morph(#[from] MorphError),
    #[error("renert::init or load has already been called")]
    AlreadyInitialized,
}

/// Ошибки структурной валидации правил.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuleValidationError {
    #[error("Rule {rule_id:?} has no productions")]
    RuleHasNoProductions { rule_id: RuleId },
    #[error("Rule {rule_id:?}, production {production_idx} is empty")]
    EmptyProduction {
        rule_id: RuleId,
        production_idx: usize,
    },
    #[error("Rule {rule_id:?} has undefined forward")]
    UndefinedForward { rule_id: RuleId },
}
