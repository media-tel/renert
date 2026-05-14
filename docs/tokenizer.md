# Токенизатор

Токенизатор в RENERT реализован на регулярных выражениях. Для каждого типа токена есть правило с регуляркой:
```
TokenType::Russian: r"[А-Яа-яЁё]+",
TokenType::Email: r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+",
TokenType::Phone: r"(?:\+?\d{10,14}|\+?\d[\s_-]?\(?\d{3}\)?(?:[\s_-]?\d{2,4}){3})",
TokenType::Domain: r"(?:[a-zA-Z0-9-]+\.)+[a-zA-Z][a-zA-Z0-9-]*",
TokenType::Latin: r"[a-zA-Z]+",
TokenType::Int: r"\d+",
TokenType::Punct: r#"[-\\/!#$%&()\[\]\*\+,\.:;<=>?@^_`{|}~#…"\'«»„“ʼʻ”]"#,
TokenType::EOL: r"[\n\r]+",
TokenType::Other: r"\S",
```

При инициализации токенизатор берёт фиксированный список правил токенизации, после чего можно токенизировать текст:
```
use renert::token::Tokenizer;

fn main() {
    let tokenizer = Tokenizer::new();
    let tokens = tokenizer.tokenize("ул. Ленина д. 10");
    println!("{:#?}", tokens);
}
```
Output:
```bash
[
    Token {
        value: "ул",
        span: Span {
            start: 0,
            stop: 4,
        },
        token_type: Russian,
    },
    Token {
        value: ".",
        span: Span {
            start: 4,
            stop: 5,
        },
        token_type: Punct,
    },
    Token {
        value: "Ленина",
        span: Span {
            start: 6,
            stop: 18,
        },
        token_type: Russian,
    },
    Token {
        value: "д",
        span: Span {
            start: 19,
            stop: 21,
        },
        token_type: Russian,
    },
    Token {
        value: ".",
        span: Span {
            start: 21,
            stop: 22,
        },
        token_type: Punct,
    },
    Token {
        value: "10",
        span: Span {
            start: 23,
            stop: 25,
        },
        token_type: Int,
    },
]
```

Можно заметить, что Span не в количестве символов, а в байтах, поэтому "ул" занимает 4 байта, а не 2 символа.

Также можно использовать метод `split` для получения только значений токенов:
```
use renert::token::Tokenizer;

fn main() {
    let tokenizer = Tokenizer::new();
    let tokens = tokenizer.split("ул. Ленина д. 10");
    println!("{:?}", tokens);
}
```
Output:
```bash
["ул", ".", "Ленина", "д", ".", "10"]
```

По умолчанию, RENERT использует не `Tokenizer`, а `MorphTokenizer`. Это обёртка над `Tokenizer`, которая автоматически добавляет морфологию к токенам.

Для каждого токена типа `TokenType::Russian` будет выполнен морфологический анализ с помощью библиотеки`morph-rs` и токен оборачивается в `AnyToken::Morph(MorphToken {...})`.
```rust
use renert::token::MorphTokenizer;

fn main() {
    let morph_tokenizer = MorphTokenizer::open().unwrap();
    let tokens = morph_tokenizer.tokenize("марки стали");
    println!("{:#?}", tokens);
}
```
Output:
```bash
[
    Morph(
        MorphToken {
            base: Token {
                value: "марки",
                span: Span {
                    start: 0,
                    stop: 10,
                },
                token_type: Russian,
            },
            forms: [
                Form {
                    normalized: "марк",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "Name",
                            "anim",
                            "masc",
                            "nomn",
                            "plur",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "марки",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Animate,
                                ),
                                Case(
                                    Nominativus,
                                ),
                                Gender(
                                    Masculine,
                                ),
                                Number(
                                    Plural,
                                ),
                                Other(
                                    Name,
                                ),
                            ],
                            normal_form: "марк",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "марка",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "femn",
                            "inan",
                            "nomn",
                            "plur",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "марки",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Nominativus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Plural,
                                ),
                            ],
                            normal_form: "марка",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "марка",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "femn",
                            "gent",
                            "inan",
                            "sing",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "марки",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Genetivus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Singular,
                                ),
                            ],
                            normal_form: "марка",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "марка",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "accs",
                            "femn",
                            "inan",
                            "plur",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "марки",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Accusativus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Plural,
                                ),
                            ],
                            normal_form: "марка",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "маркий",
                    grams: Grams {
                        values: {
                            "ADJS",
                            "Qual",
                            "plur",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "марки",
                            tags: [
                                ParteSpeech(
                                    AdjectiveShort,
                                ),
                                Number(
                                    Plural,
                                ),
                                Other(
                                    Quality,
                                ),
                            ],
                            normal_form: "маркий",
                            method: Dictionary,
                        },
                    ),
                },
            ],
        },
    ),
    Morph(
        MorphToken {
            base: Token {
                value: "стали",
                span: Span {
                    start: 11,
                    stop: 21,
                },
                token_type: Russian,
            },
            forms: [
                Form {
                    normalized: "сталь",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "femn",
                            "inan",
                            "nomn",
                            "plur",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "стали",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Nominativus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Plural,
                                ),
                            ],
                            normal_form: "сталь",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "сталь",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "femn",
                            "gent",
                            "inan",
                            "sing",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "стали",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Genetivus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Singular,
                                ),
                            ],
                            normal_form: "сталь",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "сталь",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "datv",
                            "femn",
                            "inan",
                            "sing",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "стали",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Dativus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Singular,
                                ),
                            ],
                            normal_form: "сталь",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "сталь",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "accs",
                            "femn",
                            "inan",
                            "plur",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "стали",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Accusativus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Plural,
                                ),
                            ],
                            normal_form: "сталь",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "сталь",
                    grams: Grams {
                        values: {
                            "NOUN",
                            "femn",
                            "inan",
                            "loct",
                            "sing",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "стали",
                            tags: [
                                ParteSpeech(
                                    Noun,
                                ),
                                Animacy(
                                    Inanimate,
                                ),
                                Case(
                                    Locativus,
                                ),
                                Gender(
                                    Feminine,
                                ),
                                Number(
                                    Singular,
                                ),
                            ],
                            normal_form: "сталь",
                            method: Dictionary,
                        },
                    ),
                },
                Form {
                    normalized: "стать",
                    grams: Grams {
                        values: {
                            "VERB",
                            "indc",
                            "intr",
                            "past",
                            "perf",
                            "plur",
                        },
                    },
                    raw: Some(
                        ParsedWord {
                            word: "стали",
                            tags: [
                                ParteSpeech(
                                    Verb,
                                ),
                                Aspect(
                                    Perfetto,
                                ),
                                Mood(
                                    Indicativo,
                                ),
                                Number(
                                    Plural,
                                ),
                                Trans(
                                    Intransitive,
                                ),
                                Tense(
                                    Past,
                                ),
                            ],
                            normal_form: "стать",
                            method: Dictionary,
                        },
                    ),
                },
            ],
        },
    ),
]
```

Для каждого токена `MOrPh-rS` возвращает набор граммем. Например, "NOUN, sing, femn" — "существительное в единственном числе женского рода". 
Полный список в [документации словаря `OpenCorpora`](https://opencorpora.org/dict.php?act=gram).

Вне контекста слово имеет несколько вариантов разбора. Например, "стали" — глагол (VERB) во фразе "мы стали лучше" и существительное (NOUN) в "марки стали":