//! Загрузка, поиск и подготовка морфологического словаря.
//!
//! Модуль отвечает за:
//! - определение расположения словаря morph-rs (`dict.json` + `dict.fst`);
//! - подготовку словаря из XML OpenCorpora (функция [`prepare_dictionary`]).
//!
//! ## Алгоритм поиска по умолчанию ([`dict_dir_default`])
//!
//! 1. `YARGY_DICT_DIR` (переменная окружения) — если задана и каталог валиден.
//! 2. `<crate_root>/data/dict` — основной вариант для standalone-проекта.
//! 3. `<crate_root>/../data/dict` — для workspace.
//!
//! Каталог считается валидным, если в нём присутствуют **оба файла**:
//! - `dict.json`
//! - `dict.fst`

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::error::DictError;
use morph_rs::Language;

/// Проверяет, что каталог содержит оба файла словаря (`dict.json` и `dict.fst`).
pub fn has_dict_files(dir: &Path) -> bool {
    dir.join("dict.json").is_file() && dir.join("dict.fst").is_file()
}

/// Подготавливает словарь morph-rs из XML OpenCorpora.
///
/// Если `cache_dir` уже содержит `dict.json` и `dict.fst`, сборка пропускается.
/// Иначе вызывается `morph_rs::MorphAnalyzer::create`, которая парсит XML
/// и записывает бинарные файлы в `cache_dir`.
///
/// ## Аргументы
/// - `xml` — путь к файлу словаря OpenCorpora (`dict.opcorpora.xml`).
/// - `cache_dir` — каталог для сохранения/поиска `dict.json` и `dict.fst`.
///
/// ## Ошибки
/// Возвращает `Err`, если XML не найден, не является файлом,
/// или если `morph_rs::MorphAnalyzer::create` завершилась с ошибкой.
pub fn prepare_dictionary(
    xml: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
) -> Result<(), DictError> {
    let xml = xml.as_ref().to_path_buf();
    let cache_dir = cache_dir.as_ref().to_path_buf();

    println!("Starting dictionary unpacking");

    if !xml.exists() {
        return Err(DictError::XmlFileDoesNotExist { path: xml });
    }
    if !xml.is_file() {
        return Err(DictError::XmlPathIsNotFile { path: xml });
    }

    if has_dict_files(&cache_dir) {
        println!("Files are already unpacked in out_dir");
        return Ok(());
    }

    std::fs::create_dir_all(&cache_dir).map_err(|source| DictError::CreateCacheDir {
        path: cache_dir.clone(),
        source,
    })?;

    let fst_path = cache_dir.join("dict.fst");
    let json_path = cache_dir.join("dict.json");
    let mut fst_reported = fst_path.is_file();
    let mut json_reported = json_path.is_file();

    let build_xml = xml.clone();
    let build_cache_dir = cache_dir.clone();
    let build_handle = thread::spawn(move || {
        morph_rs::MorphAnalyzer::create(&build_xml, &build_cache_dir, Language::Russian)
    });

    loop {
        if !fst_reported && fst_path.is_file() {
            println!("Unpacking an fst file");
            fst_reported = true;
        }
        if !json_reported && json_path.is_file() {
            println!("Unpacking a JSON file");
            json_reported = true;
        }
        if build_handle.is_finished() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let build_result = build_handle
        .join()
        .map_err(|_| DictError::BuildThreadPanicked)?;
    build_result.map_err(|e| DictError::BuildDictionaryFailed {
        xml: xml.clone(),
        source: Box::new(e),
    })?;

    if !fst_reported && fst_path.is_file() {
        println!("Unpacking an fst file");
    }
    if !json_reported && json_path.is_file() {
        println!("Unpacking a JSON file");
    }

    println!("End of unpacking");

    Ok(())
}

/// Возвращает путь к каталогу со словарём morph-rs по умолчанию.
///
/// Порядок поиска:
/// 1. Переменная окружения `YARGY_DICT_DIR`.
/// 2. `<CARGO_MANIFEST_DIR>/data/dict`.
/// 3. `<CARGO_MANIFEST_DIR>/../data/dict`.
///
/// Используется как fallback, когда [`crate::init`] не вызывался.
pub fn dict_dir_default() -> Result<PathBuf, DictError> {
    if let Ok(env_dir) = std::env::var("YARGY_DICT_DIR") {
        let p = PathBuf::from(&env_dir);
        if has_dict_files(&p) {
            return Ok(p);
        }
        return Err(DictError::InvalidEnvDictDir { value: env_dir });
    }

    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let p1 = base.join("data/dict");
    if has_dict_files(&p1) {
        return Ok(p1);
    }

    let p2 = base.join("../data/dict");
    if has_dict_files(&p2) {
        return Ok(p2);
    }

    Err(DictError::DictFilesNotFound {
        primary: p1,
        fallback: p2,
    })
}
