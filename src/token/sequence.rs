//! Утилиты для работы с последовательностями токенов.
//!
//! Этот модуль содержит функции, которые помогают:
//! - склеивать токены обратно в строку (в исходном виде или нормализованном)
//! - склеивать [`AnyToken`] (Plain/Morph) единым способом
//! - опционально склонять (inflect) морфологические токены в заданные граммемы
//! - получать общий [`Span`] для группы токенов
//!
//! ## Как собираются токены
//!
//! Функции `join_*` используют простое правило вставки пробелов:
//! пробел добавляется **только если** между токенами был разрыв в исходном тексте,
//! то есть `next.span.start > prev.span.stop`.
//!
//! Это позволяет восстанавливать фразу близко к исходному виду,
//! не добавляя лишние пробелы вокруг пунктуации, если токенизатор выдаёт
//! правильные `Span`.
//!
//! Пример:
//! - `"ул. Ленина"`: `["ул"(0..2), "."(2..3), "Ленина"(4..9)]`
//!   → пробела между `"ул"` и `"."` нет (стык), пробел между `"."` и `"Ленина"` есть (разрыв).
//!
//! ## Нормализация vs Инфлекция
//!
//! - `join_*_normalized_*` склеивает токены в нижнем регистре (или по морфологической нормализации
//!   для [`AnyToken::Morph`]).
//! - `join_any_inflected_tokens` — более сильная операция: пытается привести морфологические токены
//!   к указанным граммемам (`nomn+sing` по умолчанию, если `grams == None`), используя `Form::inflect()`.
//!
//! Примечание: Инфлекция зависит от `morph_rs::MorphAnalyzer` и корректно работает только для токенов,
//! у которых есть `Form.raw` (ParsedWord).

use morph_rs::morph::grammemes::Grammem;

use super::morph_tokenizer::{global_morph_tokenizer, Analyser};
use crate::morph::morph::parse_opencorpora_grammemes;
pub use crate::span::*;
use crate::token::tokenizer::AnyToken;
use crate::token::{Token, TokenType};

fn join_spanned<'a, T: 'a, I, FSpan, FWrite>(
    tokens: I,
    span_of: FSpan,
    mut write_value: FWrite,
) -> String
where
    I: IntoIterator<Item = &'a T>,
    FSpan: Fn(&T) -> Span,
    FWrite: FnMut(&mut String, &'a T),
{
    let mut out = String::new();
    let mut prev_stop: Option<usize> = None;

    for token in tokens {
        let span = span_of(token);
        if prev_stop.is_some_and(|stop| span.start > stop) {
            out.push(' ');
        }
        write_value(&mut out, token);
        prev_stop = Some(span.stop);
    }

    out
}

/// Склеивает последовательность [`Token`] в строку, используя `Span` для пробелов.
///
/// Между токенами вставляется пробел, если в исходном тексте был разрыв:
/// `next.span.start > prev.span.stop`.
///
/// Полезно для:
/// - восстановления текстового фрагмента по токенам
/// - отладочной печати результатов парсинга
///
/// ## Замечание про пунктуацию
/// Если пунктуация “приклеена” к слову (например `ул.`), то у неё будет span без разрыва,
/// и пробел не вставится.
pub fn join_tokens<'a, I>(tokens: I) -> String
where
    I: IntoIterator<Item = &'a Token<'a>>,
{
    join_spanned(tokens, |t| t.span, |out, t| out.push_str(t.value.as_ref()))
}

/// Склеивает [`Token`] в строку, предварительно нормализуя каждый токен.
///
/// Нормализация выполняется через [`Token::normalized_value`],
/// то есть по умолчанию это `to_lowercase()`.
pub fn join_normalized_tokens<'a, I>(tokens: I) -> String
where
    I: IntoIterator<Item = &'a Token<'a>>,
{
    join_spanned(
        tokens,
        |t| t.span,
        |out, t| out.push_str(t.normalized_value().as_ref()),
    )
}

/// Склеивает [`Token`] в строку, пытаясь привести русские токены к указанным граммемам.
///
/// API-совместимый аналог yargy `join_inflected_tokens(tokens, grams)`.
///
/// - `grams == None` означает inflect по умолчанию (`nomn + sing`) внутри `Form::inflect`.
/// - Для не-русских токенов используется fallback `Token::normalized_value()`.
/// - Если во входе нет русских токенов, морфологический словарь не инициализируется.
pub fn join_inflected_tokens<'a, I>(tokens: I, grams: Option<Vec<String>>) -> String
where
    I: IntoIterator<Item = &'a Token<'a>>,
{
    join_inflected_tokens_ref(tokens, grams.as_deref())
}

/// Borrowed-grams variant for hot paths that already store grams internally.
pub fn join_inflected_tokens_ref<'a, I>(tokens: I, grams: Option<&[String]>) -> String
where
    I: IntoIterator<Item = &'a Token<'a>>,
{
    let tokens: Vec<&Token<'a>> = tokens.into_iter().collect();
    if tokens.is_empty() {
        return String::new();
    }

    if !tokens.iter().any(|t| t.token_type == TokenType::Russian) {
        return join_normalized_tokens(tokens.iter().copied());
    }

    let morph = global_morph_tokenizer();
    let analyzer = match &morph.morph {
        Analyser::MorphAnalyzer(inner) => &inner.analyzer,
        Analyser::CachedMorphAnalyzer(inner) => inner.analyzer(),
    };
    let grams = parse_opencorpora_grammemes(grams);

    join_spanned(
        tokens,
        |t| t.span,
        |out, t| {
            let value = if t.token_type == TokenType::Russian {
                let forms = morph.morph.parse(t.value.as_ref());
                if let Some(form) = forms.first() {
                    form.inflect(analyzer, grams.clone())
                } else {
                    t.normalized_value().into_owned()
                }
            } else {
                t.normalized_value().into_owned()
            };

            out.push_str(&value);
        },
    )
}

/// Склеивает последовательность [`AnyToken`] (Plain/Morph) в исходную строку.
///
/// Поведение по пробелам аналогично [`join_tokens`].
/// Для значения токена используется [`AnyToken::value`].
pub fn join_any_tokens<'a, I>(tokens: I) -> String
where
    I: IntoIterator<Item = &'a AnyToken<'a>>,
{
    join_spanned(tokens, |t| t.span(), |out, t| out.push_str(t.value()))
}

/// Склеивает [`AnyToken`] в строку, используя нормализованное значение каждого токена.
///
/// Для [`AnyToken::Plain`] это `to_lowercase()`.
/// Для [`AnyToken::Morph`] — `MorphToken::normalized()` (обычно `Form.normalized` первого разбора).
///
/// Примечание: Для морфологических токенов нормализация может зависеть от порядка разборов.
/// Если слово неоднозначно, нормализованная форма может быть не единственной возможной.
pub fn join_any_normalized_tokens<'a, I>(tokens: I) -> String
where
    I: IntoIterator<Item = &'a AnyToken<'a>>,
{
    join_spanned(
        tokens,
        |t| t.span(),
        |out, t| out.push_str(t.normalized().as_ref()),
    )
}

/// Склеивает [`AnyToken`] в строку, склоняя морфологические токены в заданные граммемы.
///
/// - Для [`AnyToken::Morph`]:
///   - если есть хотя бы одна форма, берётся первая и вызывается `form.inflect(analyzer, grams)`
///   - если форм нет, fallback на `mt.normalized()`
/// - Для [`AnyToken::Plain`]: используется `tok.normalized_value()`
///
/// `grams == None` означает поведение по умолчанию внутри [`Form::inflect`](crate::morph::models::Form::inflect)
/// (обычно `nomn + sing`).
///
/// ## Когда использовать
/// - генерация нормализованных фраз в нужном падеже/числе
/// - приведение результата парсинга к каноническому виду
///
/// ## Ограничения
/// Для корректной инфлекции в `Form` должен быть `raw: Some(ParsedWord)`,
/// иначе `Form::inflect` вернёт `Form.normalized` без изменения.
pub fn join_any_inflected_tokens<'a, I>(
    tokens: I,
    analyzer: &morph_rs::MorphAnalyzer,
    grams: Option<Vec<Grammem>>,
) -> String
where
    I: IntoIterator<Item = &'a AnyToken<'a>>,
{
    join_spanned(
        tokens,
        |t| t.span(),
        |out, t| {
            let value = match t {
                AnyToken::Morph(mt) => {
                    if let Some(form) = mt.forms.first() {
                        form.inflect(analyzer, grams.clone())
                    } else {
                        mt.normalized().into_owned()
                    }
                }
                AnyToken::Plain(tok) => tok.normalized_value().into_owned(),
            };

            out.push_str(&value);
        },
    )
}

/// Возвращает общий [`Span`] для среза [`Token`].
///
/// Если список пустой — `None`.
/// Иначе возвращается диапазон от начала первого токена до конца последнего.
///
/// Предполагается, что `tokens` уже в правильном порядке (как в тексте).
pub fn get_tokens_span<'a>(tokens: &'a [Token<'a>]) -> Option<Span> {
    let head = tokens.first()?;
    let tail = tokens.last()?;
    Some(Span::new(head.span.start, tail.span.stop))
}

/// Возвращает общий [`Span`] для среза [`AnyToken`].
///
/// Аналог [`get_tokens_span`], но для последовательности, где токены могут быть
/// обычными или морфологическими.
pub fn get_any_tokens_span<'a>(tokens: &'a [AnyToken<'a>]) -> Option<Span> {
    let head = tokens.first()?;
    let tail = tokens.last()?;
    Some(Span::new(head.span().start, tail.span().stop))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::models::Form;
    use crate::morph::models::Grams;
    use crate::token::{TokenType, Tokenizer};

    fn form(norm: &str) -> Form {
        Form::new(norm.to_string(), Grams::default(), None)
    }

    #[test]
    fn test_join_tokens_like_yargy() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("pi =        3.14");

        // join_tokens должен вставлять пробел, если есть разрыв по span
        assert_eq!(join_tokens(tokens.iter()), "pi = 3.14");
    }

    #[test]
    fn test_join_normalized_tokens() {
        let tokens = vec![
            Token::new("LoNdon", Span::new(0, 6), TokenType::Latin),
            Token::new("Baker", Span::new(7, 12), TokenType::Latin),
        ];
        assert_eq!(join_normalized_tokens(tokens.iter()), "london baker");
    }

    #[test]
    fn test_join_inflected_tokens_fallback_for_non_russian() {
        let tokens = vec![
            Token::new("LoNdon", Span::new(0, 6), TokenType::Latin),
            Token::new("Baker", Span::new(7, 12), TokenType::Latin),
        ];
        assert_eq!(
            join_inflected_tokens(tokens.iter(), Some(vec!["nomn".to_string()])),
            "london baker"
        );
    }

    #[test]
    fn test_join_any_tokens_and_normalized() {
        let t1 = Token::new("DVD", Span::new(0, 3), TokenType::Latin);
        let t2 = Token::new("-", Span::new(3, 4), TokenType::Punct);
        let t3 = Token::new("ДИСКИ", Span::new(4, 4 + "ДИСКИ".len()), TokenType::Russian)
            .morphed(vec![form("диск")]);

        let any = vec![
            AnyToken::Plain(t1),
            AnyToken::Plain(t2),
            AnyToken::Morph(t3),
        ];

        assert_eq!(join_any_tokens(any.iter()), "DVD-ДИСКИ");
        assert_eq!(join_any_normalized_tokens(any.iter()), "dvd-диск");
    }

    #[test]
    fn test_get_tokens_span_and_any_tokens_span() {
        let a = Token::new("A", Span::new(2, 3), TokenType::Latin);
        let b = Token::new("B", Span::new(10, 11), TokenType::Latin);

        let span = get_tokens_span(&[a.clone(), b.clone()]).unwrap();
        assert_eq!(span, Span::new(2, 11));

        let any = vec![AnyToken::Plain(a), AnyToken::Plain(b)];
        let span2 = get_any_tokens_span(&any).unwrap();
        assert_eq!(span2, Span::new(2, 11));
    }
}
