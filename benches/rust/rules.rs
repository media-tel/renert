use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::predicates::constructors::{
    and, caseless, dictionary, gram, in_caseless, is_title, is_token_type, normalized,
    or as pred_or,
};
use renert::RuleRegistry;
use renert::{or_, pred, rule, term, RuleBuilder, RuleId};

// ---------------------------------------------------------------------------
// Факты
// ---------------------------------------------------------------------------

fact!(pub AddrPart => [value]);
fact!(pub Index => [value]);
fact!(pub Strana => [name]);
fact!(pub Oblast => [name, kind]);
fact!(pub Krai => [name, kind]);
fact!(pub Respublika => [name, kind]);
fact!(pub AutoOkrug => [name, kind]);
fact!(pub Raion => [name, kind]);
fact!(pub Gorod => [name, kind]);
fact!(pub Ulitsa => [name, kind]);
fact!(pub Dom => [number, kind]);
fact!(pub Stroenie => [number, kind]);
fact!(pub Korpus => [number, kind]);
fact!(pub Kvartira => [number, kind]);
fact!(pub Komnata => [number, kind]);
fact!(pub Ofis => [number, kind]);
fact!(pub Pomeshenie => [number, kind]);
fact!(pub Uchastok => [number, kind]);
fact!(pub Mikroraion => [name, kind]);
fact!(pub Territoriya => [name, kind]);
fact!(pub Metro => [name, kind]);
fact!(pub DoVostrebovaniya => [marker]);
fact!(pub AbonentBox => [number, kind]);

// ---------------------------------------------------------------------------
// Построение всех правил для разбора адресов.
//
// Возвращает реестр правил и идентификатор корневого правила ADDR.
// ---------------------------------------------------------------------------

/// Строит грамматику адресов и возвращает (`RuleRegistry`, `RuleId` корня).
///
/// Функция должна вызываться после [`renert::init`].
// Ряд промежуточных переменных намеренно не используется в addr_part
// (аналогично исходному Python-коду), поэтому предупреждения подавлены.
#[allow(unused_variables)]
pub fn build_address_rules() -> (RuleRegistry<'static>, RuleId) {
    // -----------------------------------------------------------------------
    // Примитивы
    // -----------------------------------------------------------------------

    // Один раз создаём узлы и переиспользуем через `.clone()` — иначе каждый вызов
    // порождал бы новый `Arc<Rule>` и ломал мемоизацию при `normalized()` / реестре.
    let dash = term("-");
    let dot = term(".");
    let slash = term("/");
    let int_tok = pred(is_token_type("Int"));
    let title_tok = pred(is_title());
    let noun = pred(gram("NOUN"));
    let adjf = pred(gram("ADJF"));

    // `normalized(full) | (caseless(abbr) + dot.optional())`
    let abbr_dot = |abbr: &'static str, full: &'static str| {
        pred(normalized(full)) | (pred(caseless(abbr)) + dot.clone().optional())
    };

    let both_orders = |a: RuleBuilder<'static>, b: RuleBuilder<'static>| {
        or_([rule([a.clone(), b.clone()]), rule([b, a])])
    };

    // LETTER (первое определение) — используется в INITIALS
    let letter_name = pred(and(vec![
        pred_or(vec![is_token_type("Latin"), is_token_type("Russian")]),
        pred_or(vec![is_title(), gram("Name")]),
    ]));

    let initials = rule([
        letter_name.clone(),
        dot.clone(),
        letter_name.clone(),
        dot.clone(),
    ]) | rule([
        letter_name.clone(),
        dot.clone(),
        letter_name.clone(),
        dot.clone(),
        letter_name.clone(),
        dot.clone(),
    ]);

    // -----------------------------------------------------------------------
    // INDEX
    // -----------------------------------------------------------------------

    let index = int_tok
        .clone()
        .interpretation(Index::value)
        .interpretation_fact::<Index>();

    // -----------------------------------------------------------------------
    // STRANA
    // -----------------------------------------------------------------------

    let strana = (pred(in_caseless(&["россия", "рф"]))
        | (pred(normalized("российский")) + pred(normalized("федерация"))))
    .interpretation(Strana::name)
    .interpretation_fact::<Strana>();

    // -----------------------------------------------------------------------
    // RESPUBLIKA
    // -----------------------------------------------------------------------

    let respublika_words = ((pred(caseless("респ")) + dot.clone().optional())
        | pred(normalized("республика")))
    .interpretation(Respublika::kind.r#const("респ."));

    let respublika_adjf = (pred(dictionary(&[
        "удмуртский",
        "удмуртская",
        "чеченский",
        "чеченская",
        "чувашский",
        "чувашская",
    ])) | (pred(caseless("карачаево"))
        + dash.clone().optional()
        + pred(normalized("черкесский")))
        | (pred(caseless("кабардино")) + dash.clone().optional() + pred(normalized("балкарский"))))
    .interpretation(Respublika::name);

    let respublika_name = (pred(dictionary(&[
        "адыгея",
        "алтай",
        "башкортостан",
        "бурятия",
        "дагестан",
        "ингушетия",
        "калмыкия",
        "карелия",
        "коми",
        "крым",
        "мордовия",
        "татарстан",
        "тыва",
        "удмуртия",
        "хакасия",
        "саха",
        "якутия",
    ])) | (pred(caseless("марий")) + pred(caseless("эл")))
        | (pred(normalized("северный"))
            + pred(normalized("осетия"))
            + rule([dash.clone(), pred(normalized("алания"))]).optional()))
    .interpretation(Respublika::name);

    let respublika_abbr = pred(in_caseless(&["кбр", "кчр", "рт"])).interpretation(Respublika::name);

    let respublika = or_([
        rule([respublika_adjf.clone(), respublika_words.clone()]),
        rule([respublika_words.clone(), respublika_adjf.clone()]),
        rule([respublika_words.clone(), respublika_name.clone()]),
        respublika_abbr,
    ])
    .interpretation_fact::<Respublika>();

    // -----------------------------------------------------------------------
    // KRAI
    // -----------------------------------------------------------------------

    let krai_words = pred(normalized("край")).interpretation(Krai::kind.r#const("край"));

    let krai_name = pred(dictionary(&[
        "алтайский",
        "забайкальский",
        "камчатский",
        "краснодарский",
        "красноярский",
        "пермский",
        "приморский",
        "ставропольский",
        "хабаровский",
    ]))
    .interpretation(Krai::name);

    let krai = both_orders(krai_name.clone(), krai_words.clone()).interpretation_fact::<Krai>();

    // -----------------------------------------------------------------------
    // OBLAST
    // -----------------------------------------------------------------------

    let oblast_words = (pred(normalized("область"))
        | (pred(caseless("обл")) + dot.clone().optional()))
    .interpretation(Oblast::kind.r#const("обл."));

    let auto_oblast_words = ((pred(caseless("а"))
        + dot.clone().optional()
        + pred(caseless("обл"))
        + dot.clone().optional())
        | pred(caseless("аобл"))
        | (pred(normalized("автономная")) + pred(normalized("область"))))
    .interpretation(Oblast::kind.r#const("а.обл."));

    let okrug_region_words =
        pred(normalized("округ")).interpretation(Oblast::kind.r#const("округ"));

    let oblast_name = pred(dictionary(&[
        "амурский",
        "архангельский",
        "астраханский",
        "белгородский",
        "брянский",
        "владимирский",
        "волгоградский",
        "вологодский",
        "воронежский",
        "ивановский",
        "иркутский",
        "калининградский",
        "калужский",
        "кемеровский",
        "кировский",
        "костромской",
        "курганский",
        "курский",
        "ленинградский",
        "липецкий",
        "магаданский",
        "московский",
        "мурманский",
        "нижегородский",
        "новгородский",
        "новосибирский",
        "омский",
        "оренбургский",
        "орловский",
        "пензенский",
        "псковский",
        "ростовский",
        "рязанский",
        "самарский",
        "саратовский",
        "сахалинский",
        "свердловский",
        "смоленский",
        "тамбовский",
        "тверской",
        "томский",
        "тульский",
        "тюменский",
        "ульяновский",
        "челябинский",
        "ярославский",
    ]))
    .interpretation(Oblast::name);

    let oblast = or_([
        rule([oblast_name.clone(), oblast_words.clone()]),
        rule([oblast_words.clone(), oblast_name.clone()]),
        rule([oblast_name.clone(), auto_oblast_words.clone()]),
        rule([auto_oblast_words.clone(), oblast_name.clone()]),
        rule([oblast_name.clone(), okrug_region_words.clone()]),
        rule([okrug_region_words.clone(), oblast_name.clone()]),
    ])
    .interpretation_fact::<Oblast>();

    // -----------------------------------------------------------------------
    // AUTO_OKRUG
    // -----------------------------------------------------------------------

    let auto_okrug_name =
        (pred(dictionary(&[
            "чукотский",
            "ненецкий",
            "еврейский",
            "еврейская",
        ])) | (pred(caseless("ямало")) + dash.clone() + pred(normalized("ненецкий"))))
        .interpretation(AutoOkrug::name);

    let auto_okrug_words = ((pred(normalized("автономный")) + pred(normalized("округ")))
        | (pred(normalized("автономный")) + pred(normalized("область")))
        | (pred(caseless("авт")) + dot.clone().optional() + pred(normalized("округ")))
        | (pred(caseless("авт")) + dot.clone().optional() + pred(normalized("область")))
        | (pred(caseless("авт"))
            + dot.clone().optional()
            + pred(caseless("обл"))
            + dot.clone().optional())
        | (pred(caseless("авт"))
            + dot.clone().optional()
            + pred(caseless("окр"))
            + dot.clone().optional())
        | pred(caseless("ао"))
        | (pred(caseless("а")) + slash.clone() + pred(caseless("о")))
        | (pred(caseless("а")) + slash.clone() + pred(caseless("обл")) + dot.clone().optional()))
    .interpretation(AutoOkrug::kind.r#const("а.окр."));

    let hanti = (pred(caseless("ханты")) + dash.clone() + pred(normalized("мансийский")))
        .interpretation(AutoOkrug::name);

    let auto_okrug = or_([
        rule([auto_okrug_name.clone(), auto_okrug_words.clone()]),
        rule([auto_okrug_words.clone(), auto_okrug_name.clone()]),
        rule([
            hanti.clone(),
            auto_okrug_words.clone(),
            dash.clone(),
            pred(normalized("югра")),
        ]),
        rule([hanti.clone(), auto_okrug_words.clone()]),
        pred(caseless("хмао")).interpretation(AutoOkrug::name),
        (pred(caseless("хмао")) + dash.clone() + pred(caseless("югра")))
            .interpretation(AutoOkrug::name),
        pred(caseless("янао")).interpretation(AutoOkrug::name),
        pred(caseless("нао")).interpretation(AutoOkrug::name),
        pred(caseless("еао")).interpretation(AutoOkrug::name),
        pred(caseless("чао")).interpretation(AutoOkrug::name),
    ])
    .interpretation_fact::<AutoOkrug>();

    // -----------------------------------------------------------------------
    // RAION
    // -----------------------------------------------------------------------

    let raion_words = ((pred(in_caseless(&["р", "p"]))
        + dash.clone()
        + pred(in_caseless(&["он", "н", "oн"]))
        + dot.clone().optional())
        | (pred(normalized("район")) + dot.clone().optional()))
    .interpretation(Raion::kind.r#const("р-н"));

    let raion_uluss_words = ((pred(caseless("у")) + dot.clone().optional())
        | pred(normalized("улус"))
        | pred(normalized("улуус")))
    .interpretation(Raion::kind.r#const("у."));

    let gorod_okrug_words = ((pred(caseless("г"))
        + dot.clone().optional()
        + pred(caseless("о"))
        + dot.clone().optional())
        | (pred(normalized("городской")) + pred(normalized("округ"))))
    .interpretation(Raion::kind.r#const("г.о."));

    let mun_raion_words = ((pred(caseless("м"))
        + dot.clone().optional()
        + pred(caseless("р"))
        + dash.clone()
        + pred(caseless("н")))
        | pred(caseless("м.р-н"))
        | (pred(normalized("муниципальный")) + pred(normalized("район"))))
    .interpretation(Raion::kind.r#const("м.р-н"));

    let mun_okrug_words = ((pred(caseless("м"))
        + dot.clone().optional()
        + pred(caseless("о"))
        + dot.clone().optional())
        | pred(caseless("м.о."))
        | (pred(normalized("муниципальный")) + pred(normalized("округ"))))
    .interpretation(Raion::kind.r#const("м.о."));

    let vnut_ter_words = ((pred(caseless("вн"))
        + dot.clone().optional()
        + pred(caseless("тер"))
        + dot.clone().optional()
        + pred(caseless("г"))
        + dot.clone().optional())
        | (pred(normalized("внутригородская")) + pred(normalized("территория"))))
    .interpretation(Raion::kind.r#const("вн.тер.г."));

    let poselenie_words = pred(normalized("поселение")).interpretation(Raion::kind.r#const("пос."));

    let fed_ter_words = ((pred(caseless("ф"))
        + dot.clone().optional()
        + pred(caseless("т"))
        + dot.clone().optional())
        | (pred(normalized("федеральная")) + pred(normalized("территория"))))
    .interpretation(Raion::kind.r#const("ф.т."));

    let vnut_raion_words = ((pred(caseless("вн"))
        + dot.clone().optional()
        + pred(caseless("р"))
        + dash.clone()
        + pred(caseless("н")))
        | (pred(normalized("внутригородской")) + pred(normalized("район"))))
    .interpretation(Raion::kind.r#const("вн.р-н"));

    let mezhsel_ter_words = ((pred(caseless("межсел"))
        + dot.clone().optional()
        + pred(caseless("тер"))
        + dot.clone().optional())
        | (pred(normalized("межселенная")) + pred(normalized("территория"))))
    .interpretation(Raion::kind.r#const("межсел.тер."));

    let imeni_words = (pred(caseless("им")) + dot.clone().optional()) | pred(normalized("имени"));

    let abbr = pred(dictionary(&[
        "влксм",
        "ссср",
        "ркка",
        "рвсн",
        "ким",
        "мжк",
        "нгду",
    ]));

    let raion_simple_name = pred(and(vec![gram("ADJF"), is_title()]));

    let raion_name = or_([
        raion_simple_name.clone(),
        rule([title_tok.clone(), dash.clone(), raion_simple_name.clone()]),
        rule([title_tok.clone(), dash.clone(), title_tok.clone()]),
        rule([imeni_words.clone(), title_tok.clone(), title_tok.clone()]),
        rule([imeni_words.clone(), title_tok.clone()]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("й"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("ый"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("я"))]),
        rule([
            raion_simple_name.clone(),
            pred(normalized("городской")),
            pred(normalized("округ")),
        ]),
        rule([
            title_tok.clone(),
            dash.clone(),
            raion_simple_name.clone(),
            pred(normalized("городской")),
            pred(normalized("округ")),
        ]),
        rule([
            title_tok.clone(),
            dash.clone(),
            title_tok.clone(),
            pred(normalized("городской")),
            pred(normalized("округ")),
        ]),
    ])
    .interpretation(Raion::name);

    let raion = or_([
        rule([raion_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), raion_words.clone()]),
        rule([raion_uluss_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), raion_uluss_words.clone()]),
        rule([gorod_okrug_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), gorod_okrug_words.clone()]),
        rule([mun_raion_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), mun_raion_words.clone()]),
        rule([mun_okrug_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), mun_okrug_words.clone()]),
        rule([vnut_ter_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), vnut_ter_words.clone()]),
        rule([poselenie_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), poselenie_words.clone()]),
        rule([fed_ter_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), fed_ter_words.clone()]),
        rule([vnut_raion_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), vnut_raion_words.clone()]),
        rule([mezhsel_ter_words.clone(), raion_name.clone()]),
        rule([raion_name.clone(), mezhsel_ter_words.clone()]),
    ])
    .interpretation_fact::<Raion>();

    // -----------------------------------------------------------------------
    // GOROD
    // -----------------------------------------------------------------------

    let gfz_words = ((pred(caseless("г"))
        + dot.clone().optional()
        + pred(caseless("ф"))
        + dot.clone().optional()
        + pred(caseless("з"))
        + dot.clone().optional())
        | (pred(normalized("город"))
            + pred(normalized("федеральный"))
            + pred(normalized("значение"))))
    .interpretation(Gorod::kind.r#const("г.ф.з."));

    let gorod_words = abbr_dot("г", "город").interpretation(Gorod::kind.r#const("г."));

    let poselok_words = ((pred(caseless("п")) + dot.clone().optional())
        | (pred(caseless("пос")) + dot.clone().optional())
        | pred(normalized("поселок")))
    .interpretation(Gorod::kind.r#const("п."));

    let pgt_words = ((pred(caseless("пгт")) + dot.clone().optional())
        | (pred(normalized("поселок"))
            + pred(normalized("городского"))
            + pred(normalized("типа"))))
    .interpretation(Gorod::kind.r#const("пгт."));

    let selo_words = ((pred(caseless("с")) + dot.clone().optional()) | pred(normalized("село")))
        .interpretation(Gorod::kind.r#const("с."));

    let derevnya_words = ((pred(caseless("д")) + dot.clone().optional())
        | pred(normalized("деревня")))
    .interpretation(Gorod::kind.r#const("д"));

    let rp_words = ((pred(caseless("рп")) + dot.clone().optional())
        | (pred(normalized("рабочий")) + pred(normalized("поселок"))))
    .interpretation(Gorod::kind.r#const("рп"));

    let hutor_words = ((pred(caseless("х")) + dot.clone().optional()) | pred(normalized("хутор")))
        .interpretation(Gorod::kind.r#const("х."));

    let gorod_simple_name = pred(and(vec![
        is_title(),
        pred_or(vec![gram("NOUN"), gram("ADJF")]),
    ]));

    let gorod_complex_name = or_([
        rule([
            gorod_simple_name.clone(),
            dash.clone(),
            gorod_simple_name.clone(),
        ]),
        rule([title_tok.clone(), dash.clone(), title_tok.clone()]),
        rule([
            title_tok.clone(),
            dash.clone(),
            pred(caseless("на")),
            dash.clone(),
            title_tok.clone(),
        ]),
    ]);

    // Общее ядро имён (город / микрорайон): один раз в графе, дальше только расширения.
    let shared_geo_name_core = or_([
        rule([
            gorod_simple_name.clone(),
            imeni_words.clone(),
            title_tok.clone(),
        ]),
        gorod_complex_name.clone(),
        gorod_simple_name.clone(),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("й")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ый")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("я")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("е")),
            title_tok.clone(),
        ]),
        rule([int_tok.clone(), title_tok.clone()]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("й"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("ый"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("я"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("е"))]),
        int_tok.clone(),
    ]);

    let gorod_name = (shared_geo_name_core.clone()
        | title_tok.clone()
        | abbr.clone()
        | rule([pred(caseless("пмк")), dash.clone(), int_tok.clone()])
        | rule([pred(caseless("дск")), dash.clone(), int_tok.clone()]))
    .interpretation(Gorod::name);

    let stanciya_words = (rule([pred(caseless("ст")), dot.clone()]) | pred(normalized("станция")))
        .interpretation(Gorod::kind.r#const("ст."));

    let ter_words = ((pred(caseless("тер")) + dot.clone().optional())
        | pred(normalized("территория")))
    .interpretation(Gorod::kind.r#const("тер."));

    let selskoe_poselenie_words = (rule([pred(caseless("с")), slash.clone(), pred(caseless("п"))])
        | (pred(normalized("сельское")) + pred(normalized("поселение"))))
    .interpretation(Gorod::kind.r#const("с/п"));

    let poselok_pri_stancii_words = (pred(caseless("п/ст"))
        | rule([pred(caseless("п")), slash.clone(), pred(caseless("ст"))]))
    .interpretation(Gorod::kind.r#const("п/ст"));

    let stanica_words = ((pred(caseless("ст-ца")) + dot.clone().optional())
        | (pred(caseless("ст")) + dash.clone() + pred(caseless("ца")) + dot.clone().optional())
        | pred(normalized("станица")))
    .interpretation(Gorod::kind.r#const("ст-ца"));

    let mikroraion_words = ((pred(caseless("мкр")) + dot.clone().optional())
        | pred(normalized("микрорайон")))
    .interpretation(Mikroraion::kind.r#const("мкр."));

    let snt_gorod_words = ((pred(caseless("снт")) + dot.clone().optional())
        | (pred(normalized("садоводческое")) + pred(normalized("товарищество"))))
    .interpretation(Gorod::kind.r#const("снт"));

    let sad_words = ((pred(caseless("сад")) + dot.clone().optional())
        | pred(normalized("садоводство")))
    .interpretation(Gorod::kind.r#const("сад"));

    let prom_raion_words = (pred(caseless("п/р"))
        | rule([pred(caseless("п")), slash.clone(), pred(caseless("р"))])
        | (pred(normalized("промышленный")) + pred(normalized("район"))))
    .interpretation(Mikroraion::kind.r#const("п/р"));

    let zhiloy_raion_words = (rule([pred(caseless("ж")), slash.clone(), pred(caseless("р"))])
        | pred(caseless("жилрайон"))
        | (pred(normalized("жилой")) + pred(normalized("район"))))
    .interpretation(Mikroraion::kind.r#const("ж/р"));

    let gorodok_words = ((pred(caseless("г-к")) + dot.clone().optional())
        | (pred(caseless("г")) + dash.clone() + pred(caseless("к")) + dot.clone().optional())
        | pred(normalized("городок")))
    .interpretation(Mikroraion::kind.r#const("г-к"));

    let dnp_gorod_words = ((pred(caseless("днп")) + dot.clone().optional())
        | (pred(normalized("дачное")) + pred(normalized("партнерство"))))
    .interpretation(Gorod::kind.r#const("днп"));

    let gsk_gorod_words = ((pred(caseless("гск")) + dot.clone().optional())
        | (pred(normalized("гаражно"))
            + dash.clone()
            + pred(normalized("строительный"))
            + pred(normalized("кооператив"))))
    .interpretation(Gorod::kind.r#const("гск"));

    let ostrov_words = pred(caseless("остров")).interpretation(Gorod::kind.r#const("остров"));

    let mestorozhd_words = (pred(normalized("месторождение"))
        | (pred(caseless("месторожд")) + dot.clone().optional()))
    .interpretation(Gorod::kind.r#const("месторожд."));

    let aul_words = (pred(normalized("аул")) | pred(caseless("аул")))
        .interpretation(Gorod::kind.r#const("аул"));

    let gp_words = ((pred(caseless("гп")) + dot.clone().optional())
        | (pred(normalized("городское")) + pred(normalized("поселение"))))
    .interpretation(Gorod::kind.r#const("гп."));

    let np_words = ((pred(caseless("нп")) + dot.clone().optional())
        | (pred(normalized("населенный")) + pred(normalized("пункт"))))
    .interpretation(Gorod::kind.r#const("нп."));

    let sloboda_words = ((pred(caseless("сл")) + dot.clone().optional())
        | pred(normalized("слобода")))
    .interpretation(Gorod::kind.r#const("сл."));

    let razezd_words = ((pred(caseless("рзд")) + dot.clone().optional())
        | pred(normalized("разъезд")))
    .interpretation(Gorod::kind.r#const("рзд."));

    let dachniy_poselok_words = ((pred(caseless("дп")) + dot.clone().optional())
        | (pred(normalized("дачный")) + pred(normalized("поселок"))))
    .interpretation(Gorod::kind.r#const("дп."));

    let aal_words = pred(caseless("аал")).interpretation(Gorod::kind.r#const("аал"));

    let kp_words = ((pred(caseless("кп")) + dot.clone().optional())
        | (pred(normalized("курортный")) + pred(normalized("поселок"))))
    .interpretation(Gorod::kind.r#const("кп."));

    let uluss_words = ((pred(caseless("у")) + dot.clone().optional())
        | pred(normalized("улус"))
        | pred(normalized("улуус")))
    .interpretation(Gorod::kind.r#const("у."));

    let mestechko_words = (rule([pred(caseless("м")), dash.clone(), pred(caseless("ко"))])
        | pred(normalized("местечко")))
    .interpretation(Gorod::kind.r#const("м-ко"));

    let pochinok_words = (rule([pred(caseless("п")), dash.clone(), pred(caseless("к"))])
        | pred(normalized("починок")))
    .interpretation(Gorod::kind.r#const("п-к"));

    let arban_words = pred(normalized("арбан")).interpretation(Gorod::kind.r#const("арбан"));

    let vyselki_words = ((pred(caseless("высел")) + dot.clone().optional())
        | rule([pred(caseless("в")), dash.clone(), pred(caseless("ки"))])
        | pred(normalized("выселки")))
    .interpretation(Gorod::kind.r#const("в-ки"));

    let sp_words = ((pred(caseless("сп")) + dot.clone().optional())
        | (pred(normalized("сельский")) + pred(normalized("поселок"))))
    .interpretation(Gorod::kind.r#const("сп."));

    let gp_gorod_words = ((pred(caseless("гп")) + dot.clone().optional())
        | (pred(normalized("городской")) + pred(normalized("поселок"))))
    .interpretation(Gorod::kind.r#const("гп."));

    let volost_words = pred(normalized("волость")).interpretation(Gorod::kind.r#const("волость"));

    let massiv_words = pred(normalized("массив")).interpretation(Gorod::kind.r#const("массив"));

    let pogost_words = pred(normalized("погост")).interpretation(Gorod::kind.r#const("погост"));

    let zaimka_words = (pred(normalized("заимка"))
        | rule([pred(caseless("з")), dash.clone(), pred(caseless("ка"))]))
    .interpretation(Gorod::kind.r#const("з-ка"));

    let kazarma_words = pred(normalized("казарма")).interpretation(Gorod::kind.r#const("казарма"));

    let kishlak_words = ((pred(caseless("киш")) + dot.clone().optional())
        | pred(normalized("кишлак")))
    .interpretation(Gorod::kind.r#const("киш."));

    let kordon_words = pred(normalized("кордон")).interpretation(Gorod::kind.r#const("кордон"));

    let zhilzona_words = (pred(caseless("жилзона"))
        | (pred(normalized("жилая")) + pred(normalized("зона"))))
    .interpretation(Gorod::kind.r#const("жилзона"));

    let avtodoroga_words =
        pred(normalized("автодорога")).interpretation(Gorod::kind.r#const("автодорога"));

    let zimovie_words = (rule([pred(caseless("зим")), dot.clone()]) | pred(normalized("зимовье")))
        .interpretation(Gorod::kind.r#const("зим."));

    let lespromhoz_words = (pred(caseless("лпх")) | pred(normalized("леспромхоз")))
        .interpretation(Gorod::kind.r#const("лпх"));

    let pochta_words = (rule([pred(caseless("п")), slash.clone(), pred(caseless("о"))])
        | pred(caseless("п/о"))
        | (pred(normalized("почтовое")) + pred(normalized("отделение"))))
    .interpretation(Gorod::kind.r#const("п/о"));

    let selskaya_adm_words = (rule([pred(caseless("с")), slash.clone(), pred(caseless("а"))])
        | (pred(normalized("сельская")) + pred(normalized("администрация"))))
    .interpretation(Gorod::kind.r#const("с/а"));

    let selskoe_mo_words = (rule([pred(caseless("с")), slash.clone(), pred(caseless("мо"))])
        | (pred(normalized("сельское"))
            + pred(normalized("муниципальное"))
            + pred(normalized("образование"))))
    .interpretation(Gorod::kind.r#const("с/мо"));

    let selskiy_okrug_words = (rule([pred(caseless("с")), slash.clone(), pred(caseless("о"))])
        | (pred(normalized("сельский")) + pred(normalized("округ"))))
    .interpretation(Gorod::kind.r#const("с/о"));

    let selsovet_words = (rule([pred(caseless("с")), slash.clone(), pred(caseless("с"))])
        | pred(normalized("сельсовет")))
    .interpretation(Gorod::kind.r#const("с/с"));

    let pos_rzd_words = ((pred(caseless("пос"))
        + dot.clone().optional()
        + pred(caseless("рзд"))
        + dot.clone().optional())
        | (pred(normalized("поселок")) + pred(normalized("разъезд"))))
    .interpretation(Gorod::kind.r#const("пос.рзд."));

    let ferma_words = pred(normalized("ферма")).interpretation(Gorod::kind.r#const("ферма"));

    let yurty_words = pred(normalized("юрты")).interpretation(Gorod::kind.r#const("ю."));

    let zht_words = (pred(caseless("жт"))
        | (pred(normalized("животноводческая")) + pred(normalized("точка"))))
    .interpretation(Gorod::kind.r#const("жт"));

    let plan_raion_words = ((pred(caseless("пл"))
        + dot.clone().optional()
        + pred(caseless("р"))
        + dash.clone()
        + pred(caseless("н")))
        | (pred(normalized("планировочный")) + pred(normalized("район"))))
    .interpretation(Gorod::kind.r#const("пл.р-н"));

    let zhd_st_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("ст"))
        + dot.clone().optional())
        | (pred(normalized("железнодорожная")) + pred(normalized("станция"))))
    .interpretation(Gorod::kind.r#const("ж/д ст."));

    let zhd_rzd_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("рзд"))
        + dot.clone().optional())
        | (pred(normalized("железнодорожный")) + pred(normalized("разъезд"))))
    .interpretation(Gorod::kind.r#const("ж/д рзд."));

    let zhd_platf_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("платф"))
        + dot.clone().optional())
        | (pred(caseless("ж"))
            + slash.clone()
            + pred(caseless("д"))
            + pred(caseless("пл"))
            + dash.clone()
            + pred(caseless("ма")))
        | (pred(normalized("железнодорожная")) + pred(normalized("платформа"))))
    .interpretation(Gorod::kind.r#const("ж/д пл-ма"));

    let zhd_budka_words =
        ((pred(caseless("ж")) + slash.clone() + pred(caseless("д")) + pred(normalized("будка")))
            | (pred(caseless("ж"))
                + slash.clone()
                + pred(caseless("д"))
                + pred(caseless("б"))
                + dash.clone()
                + pred(caseless("ка")))
            | (pred(normalized("железнодорожная")) + pred(normalized("будка"))))
        .interpretation(Gorod::kind.r#const("ж/д б-ка"));

    let zhd_kazarma_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("казарм"))
        + dot.clone().optional())
        | (pred(caseless("ж"))
            + slash.clone()
            + pred(caseless("д"))
            + pred(caseless("к"))
            + dash.clone()
            + pred(caseless("ма")))
        | (pred(normalized("железнодорожная")) + pred(normalized("казарма"))))
    .interpretation(Gorod::kind.r#const("ж/д к-ма"));

    let zhd_op_words =
        ((pred(caseless("ж")) + slash.clone() + pred(caseless("д")) + pred(caseless("оп")))
            | (pred(caseless("ж"))
                + slash.clone()
                + pred(caseless("д"))
                + pred(caseless("о"))
                + dot.clone()
                + pred(caseless("п"))
                + dot.clone().optional())
            | (pred(normalized("железнодорожный"))
                + pred(normalized("остановочный"))
                + pred(normalized("пункт"))))
        .interpretation(Gorod::kind.r#const("ж/д о.п."));

    let zhd_post_words =
        ((pred(caseless("ж")) + slash.clone() + pred(caseless("д")) + pred(caseless("пост")))
            | (pred(normalized("железнодорожный")) + pred(normalized("пост"))))
        .interpretation(Gorod::kind.r#const("ж/д_пост"));

    let zhd_blokpost_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("бл"))
        + dash.clone()
        + pred(caseless("ст")))
        | (pred(normalized("железнодорожный")) + pred(normalized("блокпост"))))
    .interpretation(Gorod::kind.r#const("ж/д бл-ст"));

    let zhd_vetka_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("в"))
        + dash.clone()
        + pred(caseless("ка")))
        | (pred(normalized("железнодорожная")) + pred(normalized("ветка"))))
    .interpretation(Gorod::kind.r#const("ж/д в-ка"));

    let zhd_kombinat_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("к"))
        + dash.clone()
        + pred(caseless("т")))
        | (pred(normalized("железнодорожный")) + pred(normalized("комбинат"))))
    .interpretation(Gorod::kind.r#const("ж/д к-т"));

    let zhd_ploschadka_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("пл"))
        + dash.clone()
        + pred(caseless("ка")))
        | (pred(normalized("железнодорожная")) + pred(normalized("площадка"))))
    .interpretation(Gorod::kind.r#const("ж/д пл-ка"));

    let zhd_put_post_words = ((pred(caseless("ж"))
        + slash.clone()
        + pred(caseless("д"))
        + pred(caseless("п"))
        + dot.clone()
        + pred(caseless("п"))
        + dot.clone().optional())
        | (pred(normalized("железнодорожный"))
            + pred(normalized("путевой"))
            + pred(normalized("пост"))))
    .interpretation(Gorod::kind.r#const("ж/д п.п."));

    // METRO
    let metro_words = ((pred(normalized("метро")) + pred(normalized("станция")))
        | pred(normalized("метро"))
        | rule([pred(caseless("м")), dot.clone()]))
    .interpretation(Metro::kind.r#const("метро"));

    let metro_name = or_([
        rule([term("\""), title_tok.clone(), term("\"")]),
        rule([term("\""), title_tok.clone(), title_tok.clone(), term("\"")]),
        title_tok.clone(),
        rule([title_tok.clone(), dash.clone(), title_tok.clone()]),
    ])
    .interpretation(Metro::name);

    let metro = rule([metro_words, metro_name]).interpretation_fact::<Metro>();

    // Крупные города (без типа)
    let gorod_big_cities = pred(dictionary(&[
        "москва",
        "санкт-петербург",
        "новосибирск",
        "екатеринбург",
        "нижний новгород",
        "казань",
        "челябинск",
        "омск",
        "самара",
        "ростов-на-дону",
        "уфа",
        "красноярск",
        "воронеж",
        "пермь",
        "волгоград",
        "краснодар",
        "саратов",
        "тюмень",
        "тольятти",
        "ижевск",
        "барнаул",
        "ульяновск",
        "иркутск",
        "хабаровск",
        "ярославль",
        "владивосток",
        "махачкала",
        "томск",
        "оренбург",
        "кемерово",
        "новокузнецк",
        "рязань",
        "астрахань",
        "пенза",
        "набережные челны",
        "липецк",
        "тула",
        "киров",
        "чебоксары",
        "калининград",
        "брянск",
        "курск",
        "иваново",
        "магнитогорск",
        "тверь",
        "ставрополь",
        "симферополь",
        "нижний тагил",
        "белгород",
        "архангельск",
        "владимир",
        "севастополь",
        "сочи",
        "курган",
        "смоленск",
        "калуга",
        "чита",
        "орел",
        "волжский",
        "череповец",
        "владикавказ",
        "мурманск",
        "сургут",
        "вологда",
        "саранск",
        "тамбов",
        "стерлитамак",
        "грозный",
        "якутск",
        "кострома",
        "петрозаводск",
        "таганрог",
        "нижневартовск",
        "йошкар-ола",
        "братск",
        "новороссийск",
        "дзержинск",
        "шахты",
        "нальчик",
        "орск",
        "сыктывкар",
        "нижнекамск",
        "ангарск",
        "старый оскол",
        "великий новгород",
        "балашиха",
        "благовещенск",
        "прокопьевск",
        "химки",
        "псков",
        "бийск",
        "энгельс",
        "рыбинск",
        "балаково",
        "северодвинск",
        "армавир",
        "подольск",
        "королев",
        "сызрань",
        "каменск-уральский",
        "мытищи",
        "люберцы",
        "волгодонск",
        "новочеркасск",
        "абакан",
        "находка",
        "уссурийск",
        "березники",
        "салават",
        "электросталь",
        "миасс",
        "первоуральск",
        "рубцовск",
        "альметьевск",
        "коломна",
        "керчь",
        "майкоп",
        "одинцово",
        "красногорск",
        "серпухов",
        "щелково",
        "домодедово",
        "раменское",
        "орехово-зуево",
        "дубна",
        "пушкино",
        "жуковский",
        "ногинск",
        "сергиев посад",
        "щербинка",
        "климовск",
        "клин",
        "егорьевск",
        "чехов",
        "видное",
        "истра",
        "лобня",
        "шатура",
        "звенигород",
        "луховицы",
        "солнечногорск",
        "волоколамск",
        "ступино",
        "зеленоград",
        "павловский посад",
        "долгопрудный",
        "реутов",
        "лыткарино",
        "ивантеевка",
        "фрязино",
        "дмитров",
        "кашира",
        "наро-фоминск",
        "воскресенск",
        "протвино",
        "можайск",
        "лосино-петровский",
        "электрогорск",
        "шаховская",
        "талдом",
        "озеры",
        "бронницы",
        "черноголовка",
        "котельники",
        "красноармейск",
        "электроугли",
        "рошаль",
        "зарайск",
        "руза",
        "дзержинский",
        "красная поляна",
        "кубинка",
        "дедовск",
        "балабаново",
        "боровск",
        "белоусово",
        "калужская",
        "жизнь",
        "износки",
        "кондрово",
        "козельск",
        "людиново",
        "малоярославец",
        "медынь",
        "мещовск",
        "мосальск",
        "спас-деменск",
        "сухиничи",
        "таруса",
        "юхнов",
        "обнинск",
        "краснозаводск",
        "пересвет",
        "хотьково",
        "струнино",
        "карабаново",
        "александров",
        "кольчугино",
        "вязники",
        "гороховец",
        "гусь-хрустальный",
        "камешково",
        "киржач",
        "ковров",
        "меленки",
        "муром",
        "петушки",
        "покров",
        "радужный",
        "собинка",
        "судогда",
        "суздаль",
        "юрьев-польский",
        "заринск",
        "златоуст",
        "норильск",
        "кызыл",
        "великие луки",
        "боготол",
        "иланский",
        "кодинск",
        "осташков",
        "славянск-на-кубани",
        "туапсе",
        "кореновский",
        "туймазинский",
        "беслан",
        "ильский",
        "соликамск",
        "новоселово",
        "золотухино",
        "ершичи",
        "пятигорск",
        "урюпинск",
        "куровское",
        "ершово",
        "шарья",
        "семилукский",
        "комсомольск-на-амуре",
        "пыть-ях",
        "биробиджан",
        "пено",
        "нива",
        "куанда",
        "новинки",
    ]))
    .interpretation(Gorod::name)
    .interpretation_fact::<Gorod>();

    let gorod = or_([
        rule([gfz_words, gorod_name.clone()]),
        rule([gorod_words, gorod_name.clone()]),
        rule([poselok_words, gorod_name.clone()]),
        rule([pgt_words, gorod_name.clone()]),
        rule([selo_words, gorod_name.clone()]),
        rule([derevnya_words, gorod_name.clone()]),
        rule([rp_words, gorod_name.clone()]),
        rule([hutor_words, gorod_name.clone()]),
        rule([stanciya_words, gorod_name.clone()]),
        rule([selskoe_poselenie_words, gorod_name.clone()]),
        rule([poselok_pri_stancii_words, gorod_name.clone()]),
        rule([stanica_words, gorod_name.clone()]),
        rule([sad_words, gorod_name.clone()]),
        rule([ostrov_words, gorod_name.clone()]),
        rule([mestorozhd_words, gorod_name.clone()]),
        rule([aul_words, gorod_name.clone()]),
        rule([gp_words, gorod_name.clone()]),
        rule([np_words, gorod_name.clone()]),
        rule([sloboda_words, gorod_name.clone()]),
        rule([razezd_words, gorod_name.clone()]),
        rule([dachniy_poselok_words, gorod_name.clone()]),
        rule([aal_words, gorod_name.clone()]),
        rule([kp_words, gorod_name.clone()]),
        rule([mestechko_words, gorod_name.clone()]),
        rule([pochinok_words, gorod_name.clone()]),
        rule([arban_words, gorod_name.clone()]),
        rule([vyselki_words, gorod_name.clone()]),
        rule([sp_words, gorod_name.clone()]),
        rule([gp_gorod_words, gorod_name.clone()]),
        rule([volost_words, gorod_name.clone()]),
        rule([massiv_words, gorod_name.clone()]),
        rule([pogost_words, gorod_name.clone()]),
        rule([zaimka_words, gorod_name.clone()]),
        rule([kazarma_words, gorod_name.clone()]),
        rule([kishlak_words, gorod_name.clone()]),
        rule([kordon_words, gorod_name.clone()]),
        rule([zhilzona_words, gorod_name.clone()]),
        rule([avtodoroga_words, gorod_name.clone()]),
        rule([zimovie_words, gorod_name.clone()]),
        rule([lespromhoz_words, gorod_name.clone()]),
        rule([pochta_words, gorod_name.clone()]),
        rule([selskaya_adm_words, gorod_name.clone()]),
        rule([selskoe_mo_words, gorod_name.clone()]),
        rule([selskiy_okrug_words, gorod_name.clone()]),
        rule([selsovet_words, gorod_name.clone()]),
        rule([pos_rzd_words, gorod_name.clone()]),
        rule([ferma_words, gorod_name.clone()]),
        rule([yurty_words, gorod_name.clone()]),
        rule([zht_words, gorod_name.clone()]),
        rule([plan_raion_words, gorod_name.clone()]),
        rule([zhd_st_words, gorod_name.clone()]),
        rule([zhd_rzd_words, gorod_name.clone()]),
        rule([zhd_platf_words, gorod_name.clone()]),
        rule([zhd_budka_words, gorod_name.clone()]),
        rule([zhd_kazarma_words, gorod_name.clone()]),
        rule([zhd_op_words, gorod_name.clone()]),
        rule([zhd_post_words, gorod_name.clone()]),
        rule([zhd_blokpost_words, gorod_name.clone()]),
        rule([zhd_vetka_words, gorod_name.clone()]),
        rule([zhd_kombinat_words, gorod_name.clone()]),
        rule([zhd_ploschadka_words, gorod_name.clone()]),
        rule([zhd_put_post_words, gorod_name.clone()]),
        gorod_big_cities,
    ])
    .interpretation_fact::<Gorod>();

    // -----------------------------------------------------------------------
    // MIKRORAION
    // -----------------------------------------------------------------------

    let kvartal_words = ((pred(caseless("кв-л")) + dot.clone().optional())
        | (pred(caseless("кв")) + dash.clone() + pred(caseless("л")) + dot.clone().optional())
        | pred(normalized("квартал")))
    .interpretation(Mikroraion::kind.r#const("кв-л"));

    let promzona_words = ((pred(caseless("промзона")) + dot.clone().optional())
        | (pred(normalized("промышленная")) + pred(normalized("зона"))))
    .interpretation(Mikroraion::kind.r#const("промзона"));

    let zona_words = pred(normalized("зона")).interpretation(Mikroraion::kind.r#const("зона"));

    let mestnost_words =
        pred(normalized("местность")).interpretation(Mikroraion::kind.r#const("местность"));

    let nkp_words = (rule([pred(caseless("н")), slash.clone(), pred(caseless("п"))])
        | (pred(normalized("некоммерческое")) + pred(normalized("партнерство"))))
    .interpretation(Mikroraion::kind.r#const("н/п"));

    let mikroraion_name = (shared_geo_name_core.clone()
        | rule([term("\""), title_tok.clone(), title_tok.clone(), term("\"")])
        | rule([term("\""), title_tok.clone(), term("\"")])
        | rule([title_tok.clone(), title_tok.clone()])
        | title_tok.clone())
    .interpretation(Mikroraion::name);

    let mikroraion = or_([
        rule([mikroraion_words.clone(), mikroraion_name.clone()]),
        rule([kvartal_words, mikroraion_name.clone()]),
        rule([promzona_words, mikroraion_name.clone()]),
        rule([prom_raion_words, mikroraion_name.clone()]),
        rule([zhiloy_raion_words, mikroraion_name.clone()]),
        rule([gorodok_words, mikroraion_name.clone()]),
        rule([zona_words, mikroraion_name.clone()]),
        rule([mestnost_words, mikroraion_name.clone()]),
        rule([nkp_words, mikroraion_name.clone()]),
    ])
    .interpretation_fact::<Mikroraion>();

    // -----------------------------------------------------------------------
    // TERRITORIYA
    // -----------------------------------------------------------------------

    let territoriya_name = or_([
        rule([
            gorod_simple_name.clone(),
            imeni_words.clone(),
            title_tok.clone(),
        ]),
        gorod_complex_name.clone(),
        gorod_simple_name.clone(),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("й")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ый")),
            title_tok.clone(),
        ]),
        rule([int_tok.clone(), title_tok.clone()]),
        rule([title_tok.clone(), title_tok.clone(), title_tok.clone()]),
        rule([title_tok.clone(), title_tok.clone()]),
        rule([title_tok.clone(), dash.clone(), int_tok.clone()]),
        title_tok.clone(),
        int_tok.clone(),
    ])
    .interpretation(Territoriya::name);

    let ter_base_words = ((pred(caseless("тер")) + dot.clone().optional())
        | pred(normalized("территория")))
    .interpretation(Territoriya::kind.r#const("тер."));

    let snt_ter_words = ((pred(caseless("снт")) + dot.clone().optional())
        | (pred(normalized("садоводческое")) + pred(normalized("товарищество"))))
    .interpretation(Territoriya::kind.r#const("снт"));

    let dnp_ter_words = ((pred(caseless("днп")) + dot.clone().optional())
        | (pred(normalized("дачное")) + pred(normalized("партнерство"))))
    .interpretation(Territoriya::kind.r#const("днп"));

    let gsk_ter_words = ((pred(caseless("гск")) + dot.clone().optional())
        | (pred(normalized("гаражно"))
            + dash.clone()
            + pred(normalized("строительный"))
            + pred(normalized("кооператив"))))
    .interpretation(Territoriya::kind.r#const("гск"));

    let fh_ter_words = (rule([pred(caseless("ф")), slash.clone(), pred(caseless("х"))])
        | pred(caseless("ф/х"))
        | (pred(normalized("фермерское")) + pred(normalized("хозяйство"))))
    .interpretation(Territoriya::kind.r#const("ф/х"));

    let usadba_ter_words = ((pred(caseless("ус")) + dot.clone().optional())
        | pred(normalized("усадьба")))
    .interpretation(Territoriya::kind.r#const("ус."));

    let st_ter_words = (rule([pred(caseless("с")), slash.clone(), pred(caseless("т"))])
        | pred(caseless("с/т"))
        | (pred(normalized("садовое")) + pred(normalized("товарищество"))))
    .interpretation(Territoriya::kind.r#const("с/т"));

    let ter_gsk_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("гск")))
        | (pred(normalized("территория")) + pred(caseless("гск"))))
    .interpretation(Territoriya::kind.r#const("тер. ГСК"));

    let ter_dno_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("дно")))
        | (pred(normalized("территория")) + pred(caseless("дно"))))
    .interpretation(Territoriya::kind.r#const("тер. ДНО"));

    let ter_dnt_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("днт")))
        | (pred(normalized("территория")) + pred(caseless("днт"))))
    .interpretation(Territoriya::kind.r#const("тер. ДНТ"));

    let ter_dpk_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("дпк")))
        | (pred(normalized("территория")) + pred(caseless("дпк"))))
    .interpretation(Territoriya::kind.r#const("тер. ДПК"));

    let ter_ont_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("онт")))
        | (pred(normalized("территория")) + pred(caseless("онт"))))
    .interpretation(Territoriya::kind.r#const("тер. ОНТ"));

    let ter_opk_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("опк")))
        | (pred(normalized("территория")) + pred(caseless("опк"))))
    .interpretation(Territoriya::kind.r#const("тер. ОПК"));

    let ter_pk_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("пк")))
        | (pred(normalized("территория")) + pred(caseless("пк"))))
    .interpretation(Territoriya::kind.r#const("тер. ПК"));

    let ter_sno_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("сно")))
        | (pred(normalized("территория")) + pred(caseless("сно"))))
    .interpretation(Territoriya::kind.r#const("тер. СНО"));

    let ter_snp_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("снп")))
        | (pred(normalized("территория")) + pred(caseless("снп"))))
    .interpretation(Territoriya::kind.r#const("тер. СНП"));

    let ter_spk_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("спк")))
        | (pred(normalized("территория")) + pred(caseless("спк"))))
    .interpretation(Territoriya::kind.r#const("тер. СПК"));

    let ter_tsz_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("тсж")))
        | (pred(normalized("территория")) + pred(caseless("тсж"))))
    .interpretation(Territoriya::kind.r#const("тер. ТСЖ"));

    let ter_tsn_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("тсн")))
        | (pred(normalized("территория")) + pred(caseless("тсн"))))
    .interpretation(Territoriya::kind.r#const("тер. ТСН"));

    let ter_dnp_ter_words =
        ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("днп")))
            | (pred(normalized("территория")) + pred(caseless("днп"))))
        .interpretation(Territoriya::kind.r#const("тер. ДНП"));

    let ter_ono_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("оно")))
        | (pred(normalized("территория")) + pred(caseless("оно"))))
    .interpretation(Territoriya::kind.r#const("тер. ОНО"));

    let ter_onp_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("онп")))
        | (pred(normalized("территория")) + pred(caseless("онп"))))
    .interpretation(Territoriya::kind.r#const("тер. ОНП"));

    let ter_snt_words = ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("снт")))
        | (pred(normalized("территория")) + pred(caseless("снт"))))
    .interpretation(Territoriya::kind.r#const("тер. СНТ"));

    let ter_sosn_words =
        ((pred(caseless("тер")) + dot.clone().optional() + pred(caseless("сосн")))
            | (pred(normalized("территория")) + pred(caseless("сосн"))))
        .interpretation(Territoriya::kind.r#const("тер.СОСН"));

    let ter_fh_words = ((pred(caseless("тер"))
        + dot.clone().optional()
        + pred(caseless("ф"))
        + dot.clone().optional()
        + pred(caseless("х"))
        + dot.clone().optional())
        | (pred(normalized("территория")) + pred(caseless("фх"))))
    .interpretation(Territoriya::kind.r#const("тер.ф.х."));

    let territoriya = or_([
        rule([ter_base_words, territoriya_name.clone()]),
        rule([snt_ter_words, territoriya_name.clone()]),
        rule([dnp_ter_words, territoriya_name.clone()]),
        rule([gsk_ter_words, territoriya_name.clone()]),
        rule([fh_ter_words, territoriya_name.clone()]),
        rule([usadba_ter_words, territoriya_name.clone()]),
        rule([st_ter_words, territoriya_name.clone()]),
        rule([ter_gsk_words, territoriya_name.clone()]),
        rule([ter_dno_words, territoriya_name.clone()]),
        rule([ter_dnt_words, territoriya_name.clone()]),
        rule([ter_dpk_words, territoriya_name.clone()]),
        rule([ter_ont_words, territoriya_name.clone()]),
        rule([ter_opk_words, territoriya_name.clone()]),
        rule([ter_pk_words, territoriya_name.clone()]),
        rule([ter_sno_words, territoriya_name.clone()]),
        rule([ter_snp_words, territoriya_name.clone()]),
        rule([ter_spk_words, territoriya_name.clone()]),
        rule([ter_tsz_words, territoriya_name.clone()]),
        rule([ter_tsn_words, territoriya_name.clone()]),
        rule([ter_dnp_ter_words, territoriya_name.clone()]),
        rule([ter_ono_words, territoriya_name.clone()]),
        rule([ter_onp_words, territoriya_name.clone()]),
        rule([ter_snt_words, territoriya_name.clone()]),
        rule([ter_sosn_words, territoriya_name.clone()]),
        rule([ter_fh_words, territoriya_name.clone()]),
    ])
    .interpretation_fact::<Territoriya>();

    // -----------------------------------------------------------------------
    // ULITSA
    // -----------------------------------------------------------------------

    let ulitsa_words = (pred(normalized("улица"))
        | (pred(caseless("ул")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("ул."));

    let prospekt_words = ((pred(in_caseless(&["пр", "просп", "пркт", "пр-кт", "пр-т"]))
        + dot.clone().optional())
        | (pred(caseless("пр"))
            + dash.clone()
            + pred(in_caseless(&["кт", "т"]))
            + dot.clone().optional())
        | (pred(caseless("пр"))
            + dot.clone().optional()
            + pred(in_caseless(&["кт", "т"]))
            + dot.clone().optional())
        | (pred(caseless("пр")) + dot.clone().optional())
        | pred(normalized("проспект")))
    .interpretation(Ulitsa::kind.r#const("пр-кт"));

    let pereulok_words = (pred(normalized("переулок"))
        | (pred(caseless("пер")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("пер."));

    let proezd_words = ((pred(in_caseless(&["пр-езд", "пр-зд", "пр-д", "прз"]))
        + dot.clone().optional())
        | (pred(caseless("пр")) + dash.clone() + pred(caseless("д")) + dot.clone().optional())
        | (pred(normalized("проезд")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("пр-д"));

    let shosse_words = (pred(normalized("шоссе")) | (pred(caseless("ш")) + dot.clone().optional()))
        .interpretation(Ulitsa::kind.r#const("ш."));

    let bulvar_words = (pred(normalized("бульвар"))
        | (pred(caseless("б")) + dash.clone() + pred(caseless("р")) + dot.clone().optional())
        | (pred(caseless("бул")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("б-р"));

    let nabereg_words = (pred(normalized("набережная"))
        | (pred(caseless("наб")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("наб."));

    let doroga_words = (pred(normalized("дорога"))
        | (pred(caseless("дор")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("дор."));

    let alleya_words = (pred(normalized("аллея"))
        | (pred(caseless("ал")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("ал."));

    let ploshad_words = (pred(normalized("площадь"))
        | (pred(caseless("пл")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("пл."));

    let liniya_words = (pred(normalized("линия"))
        | (pred(caseless("лн")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("лн."));

    let kilometr_words = (pred(normalized("километр"))
        | (pred(caseless("км")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("км"));

    let tupik_words = (pred(normalized("тупик"))
        | (pred(caseless("туп")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("туп."));

    let trakt_words = pred(normalized("тракт")).interpretation(Ulitsa::kind.r#const("тракт"));

    let val_words = pred(normalized("вал")).interpretation(Ulitsa::kind.r#const("вал"));

    let vezd_words = (pred(normalized("въезд")) | (pred(caseless("взд")) + dot.clone().optional()))
        .interpretation(Ulitsa::kind.r#const("взд."));

    let kolco_words = (pred(normalized("кольцо"))
        | rule([pred(caseless("к")), dash.clone(), pred(caseless("цо"))]))
    .interpretation(Ulitsa::kind.r#const("к-цо"));

    let skver_words = (pred(normalized("сквер"))
        | rule([pred(caseless("с")), dash.clone(), pred(caseless("р"))]))
    .interpretation(Ulitsa::kind.r#const("с-р"));

    let spusk_words = (pred(normalized("спуск"))
        | rule([pred(caseless("с")), dash.clone(), pred(caseless("к"))]))
    .interpretation(Ulitsa::kind.r#const("с-к"));

    let prosek_words = (pred(normalized("просек"))
        | pred(normalized("просека"))
        | rule([pred(caseless("пр")), dash.clone(), pred(caseless("к"))])
        | rule([pred(caseless("пр")), dash.clone(), pred(caseless("ка"))]))
    .interpretation(Ulitsa::kind.r#const("пр-к"));

    let proulok_words = (pred(normalized("проулок"))
        | (pred(caseless("проул")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("проул."));

    let ryady_words = (pred(normalized("ряды")) | pred(normalized("ряд")))
        .interpretation(Ulitsa::kind.r#const("ряды"));

    let pereezd_words = (pred(normalized("переезд"))
        | rule([pred(caseless("пер")), dash.clone(), pred(caseless("д"))]))
    .interpretation(Ulitsa::kind.r#const("пер-д"));

    let most_words = pred(normalized("мост")).interpretation(Ulitsa::kind.r#const("мост"));

    let park_words = pred(normalized("парк")).interpretation(Ulitsa::kind.r#const("парк"));

    let magistral_words = (pred(normalized("магистраль"))
        | (pred(caseless("мгстр")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("мгстр."));

    let sezd_words = (pred(normalized("съезд")) | (pred(caseless("сзд")) + dot.clone().optional()))
        .interpretation(Ulitsa::kind.r#const("сзд."));

    let bereg_words = (pred(normalized("берег"))
        | rule([pred(caseless("б")), dash.clone(), pred(caseless("г"))]))
    .interpretation(Ulitsa::kind.r#const("б-г"));

    let proselok_words = (pred(normalized("проселок"))
        | rule([pred(caseless("пр")), dash.clone(), pred(caseless("лок"))]))
    .interpretation(Ulitsa::kind.r#const("пр-лок"));

    let zaezd_words = (pred(normalized("заезд"))
        | (pred(caseless("ззд")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("ззд."));

    let ploschadka_words = (pred(normalized("площадка"))
        | rule([pred(caseless("пл")), dash.clone(), pred(caseless("ка"))]))
    .interpretation(Ulitsa::kind.r#const("пл-ка"));

    let balka_words = pred(normalized("балка")).interpretation(Ulitsa::kind.r#const("балка"));

    let bugor_words = pred(normalized("бугор")).interpretation(Ulitsa::kind.r#const("бугор"));

    let vzvoz_words = (pred(normalized("взвоз"))
        | (pred(caseless("взв")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("взв."));

    let kosa_words = pred(normalized("коса")).interpretation(Ulitsa::kind.r#const("коса"));

    let mayak_words = pred(normalized("маяк")).interpretation(Ulitsa::kind.r#const("маяк"));

    let platforma_words = (pred(normalized("платформа"))
        | (pred(caseless("платф")) + dot.clone().optional()))
    .interpretation(Ulitsa::kind.r#const("платф."));

    let polustanok_words =
        pred(normalized("полустанок")).interpretation(Ulitsa::kind.r#const("полустанок"));

    let port_words = pred(normalized("порт")).interpretation(Ulitsa::kind.r#const("порт"));

    // Модификаторы для ULITSA_NAME
    let modifier_words = rule([
        pred(dictionary(&[
            "большой",
            "малый",
            "средний",
            "верхний",
            "нижний",
            "северный",
            "первый",
            "второй",
            "третий",
            "старый",
            "новый",
        ])),
        dash.clone().optional(),
    ]);

    let rank_words = pred(dictionary(&[
        "архитектора",
        "профессора",
        "генерала",
        "маршала",
        "полковника",
        "капитана",
        "академика",
        "митрополита",
    ]));

    let person_role_words = pred(dictionary(&[
        "писателя",
        "поэта",
        "героя",
        "газеты",
        "генерала",
        "маршала",
        "полковника",
        "капитана",
        "сержанта",
        "красноармейца",
        "лётчика",
        "краеведа",
        "профессора",
        "архитектора",
        "академика",
        "митрополита",
    ]));

    let ulitsa_name = or_([
        pred(and(vec![
            pred_or(vec![gram("ADJF"), and(vec![gram("NOUN"), gram("gent")])]),
            is_title(),
        ])),
        rule([
            title_tok.clone(),
            dash.clone().optional(),
            title_tok.clone(),
        ]),
        rule([modifier_words.clone().optional(), title_tok.clone()]),
        rule([
            pred(caseless("с")),
            slash.clone(),
            pred(caseless("п")),
            title_tok.clone(),
        ]),
        rule([imeni_words.clone(), title_tok.clone()]),
        rule([imeni_words.clone(), title_tok.clone(), title_tok.clone()]),
        rule([
            imeni_words.clone(),
            int_tok.clone(),
            title_tok.clone(),
            title_tok.clone(),
        ]),
        rule([
            imeni_words.clone(),
            int_tok.clone(),
            pred(normalized("лет")),
            title_tok.clone(),
        ]),
        rule([rank_words.clone(), initials.clone(), title_tok.clone()]),
        rule([rank_words.clone(), title_tok.clone(), initials.clone()]),
        rule([pred(caseless("м")), dot.clone(), title_tok.clone()]),
        rule([initials.clone(), title_tok.clone()]),
        initials.clone(),
        rule([title_tok.clone(), initials.clone()]),
        rule([
            imeni_words.clone(),
            dot.clone().optional(),
            person_role_words.clone(),
            title_tok.clone(),
        ]),
        rule([
            imeni_words.clone(),
            dot.clone().optional(),
            person_role_words.clone(),
            title_tok.clone(),
            title_tok.clone(),
        ]),
        rule([
            imeni_words.clone(),
            dot.clone().optional(),
            pred(normalized("газета")),
            term("\""),
            title_tok.clone(),
            title_tok.clone(),
            term("\""),
        ]),
        rule([
            imeni_words.clone(),
            dot.clone(),
            pred(normalized("газета")),
            term("\""),
            title_tok.clone(),
            title_tok.clone(),
            term("\""),
        ]),
        rule([
            imeni_words.clone(),
            dot.clone().optional(),
            pred(normalized("газеты")),
            term("\""),
            title_tok.clone(),
            title_tok.clone(),
            term("\""),
        ]),
        rule([
            imeni_words.clone(),
            dot.clone().optional(),
            pred(normalized("газета")),
            term("'"),
            title_tok.clone(),
            title_tok.clone(),
            term("'"),
        ]),
        rule([
            imeni_words.clone(),
            dot.clone(),
            pred(normalized("газета")),
            term("'"),
            title_tok.clone(),
            title_tok.clone(),
            term("'"),
        ]),
        rule([
            imeni_words.clone(),
            dot.clone().optional(),
            pred(normalized("газеты")),
            term("'"),
            title_tok.clone(),
            title_tok.clone(),
            term("'"),
        ]),
        rule([term("\""), title_tok.clone(), term("\"")]),
        rule([term("\""), title_tok.clone(), title_tok.clone(), term("\"")]),
        rule([term("'"), title_tok.clone(), term("'")]),
        rule([term("'"), title_tok.clone(), title_tok.clone(), term("'")]),
        rule([
            imeni_words.clone(),
            pred(dictionary(&["гвардии"])),
            pred(dictionary(&[
                "красноармейца",
                "сержанта",
                "майора",
                "капитана",
                "полковника",
            ])),
            title_tok.clone(),
        ]),
        rule([
            imeni_words.clone(),
            pred(dictionary(&["братьев"])),
            title_tok.clone(),
        ]),
        int_tok.clone(),
        rule([int_tok.clone(), title_tok.clone()]),
        rule([int_tok.clone(), title_tok.clone(), title_tok.clone()]),
        rule([int_tok.clone(), pred(normalized("лет")), title_tok.clone()]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("я")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("й")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("е")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("го")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ой")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ая")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ый")),
            title_tok.clone(),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ий")),
            title_tok.clone(),
        ]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("я"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("й"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("е"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("ый"))]),
        rule([int_tok.clone(), dash.clone(), pred(caseless("ой"))]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ая")),
            pred(normalized("линия")),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("я")),
            pred(normalized("линия")),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("й")),
            pred(normalized("ключ")),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(caseless("ый")),
            pred(normalized("ключ")),
        ]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(normalized("летие")),
            title_tok.clone(),
        ]),
        rule([int_tok.clone(), pred(normalized("лет")), abbr.clone()]),
        rule([
            int_tok.clone(),
            dash.clone(),
            pred(normalized("летия")),
            abbr.clone(),
        ]),
        rule([imeni_words.clone(), abbr.clone()]),
        abbr.clone(),
        pred(caseless("мкад")),
        rule([pred(caseless("мкад")), int_tok.clone()]),
        rule([
            pred(caseless("мкад")),
            int_tok.clone(),
            dash.clone(),
            pred(caseless("й")),
        ]),
        rule([title_tok.clone(), dash.clone(), noun.clone()]),
        rule([title_tok.clone(), dash.clone(), adjf.clone()]),
        noun.clone(),
        adjf.clone(),
    ])
    .interpretation(Ulitsa::name);

    // Постпозиционные варианты улиц (тип следует после названия)
    let ulitsa_post_prospekt = rule([title_tok.clone(), pred(normalized("проспект"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_shosse = rule([title_tok.clone(), pred(normalized("шоссе"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_bulvar = rule([title_tok.clone(), pred(normalized("бульвар"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_nabereg = rule([title_tok.clone(), pred(normalized("набережная"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_tupik = rule([title_tok.clone(), pred(normalized("тупик"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_pereulok = rule([title_tok.clone(), pred(normalized("переулок"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_proezd = rule([title_tok.clone(), pred(normalized("проезд"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_alleya = rule([title_tok.clone(), pred(normalized("аллея"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_ploshad = rule([
        title_tok.clone().interpretation(Ulitsa::name),
        abbr_dot("пл", "площадь").interpretation(Ulitsa::kind.r#const("пл.")),
    ]);
    let ulitsa_post_trakt = rule([title_tok.clone(), pred(normalized("тракт"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_liniya = rule([title_tok.clone(), pred(normalized("линия"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_doroga = rule([title_tok.clone(), pred(normalized("дорога"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_val = rule([title_tok.clone(), pred(normalized("вал"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_kolco = rule([title_tok.clone(), pred(normalized("кольцо"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_spusk = rule([title_tok.clone(), pred(normalized("спуск"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_most = rule([title_tok.clone(), pred(normalized("мост"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_park = rule([title_tok.clone(), pred(normalized("парк"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_sezd = rule([title_tok.clone(), pred(normalized("съезд"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post_bereg = rule([title_tok.clone(), pred(normalized("берег"))])
        .interpretation(Ulitsa::name)
        .interpretation_fact::<Ulitsa>();
    let ulitsa_post2_prospekt = rule([
        title_tok.clone(),
        title_tok.clone(),
        pred(normalized("проспект")),
    ])
    .interpretation(Ulitsa::name)
    .interpretation_fact::<Ulitsa>();
    let ulitsa_post2_shosse = rule([
        title_tok.clone(),
        title_tok.clone(),
        pred(normalized("шоссе")),
    ])
    .interpretation(Ulitsa::name)
    .interpretation_fact::<Ulitsa>();
    let ulitsa_post2_bulvar = rule([
        title_tok.clone(),
        title_tok.clone(),
        pred(normalized("бульвар")),
    ])
    .interpretation(Ulitsa::name)
    .interpretation_fact::<Ulitsa>();

    let ulitsa = or_([
        rule([ulitsa_words, ulitsa_name.clone()]),
        rule([prospekt_words, ulitsa_name.clone()]),
        rule([pereulok_words, ulitsa_name.clone()]),
        rule([proezd_words, ulitsa_name.clone()]),
        rule([shosse_words, ulitsa_name.clone()]),
        rule([bulvar_words, ulitsa_name.clone()]),
        rule([nabereg_words, ulitsa_name.clone()]),
        rule([doroga_words, ulitsa_name.clone()]),
        rule([alleya_words, ulitsa_name.clone()]),
        rule([ploshad_words, ulitsa_name.clone()]),
        rule([liniya_words, ulitsa_name.clone()]),
        rule([kilometr_words, ulitsa_name.clone()]),
        rule([tupik_words, ulitsa_name.clone()]),
        rule([trakt_words, ulitsa_name.clone()]),
        rule([val_words, ulitsa_name.clone()]),
        rule([vezd_words, ulitsa_name.clone()]),
        rule([kolco_words, ulitsa_name.clone()]),
        rule([skver_words, ulitsa_name.clone()]),
        rule([spusk_words, ulitsa_name.clone()]),
        rule([prosek_words, ulitsa_name.clone()]),
        rule([proulok_words, ulitsa_name.clone()]),
        rule([ryady_words, ulitsa_name.clone()]),
        rule([pereezd_words, ulitsa_name.clone()]),
        rule([most_words, ulitsa_name.clone()]),
        rule([park_words, ulitsa_name.clone()]),
        rule([magistral_words, ulitsa_name.clone()]),
        rule([sezd_words, ulitsa_name.clone()]),
        rule([bereg_words, ulitsa_name.clone()]),
        rule([proselok_words, ulitsa_name.clone()]),
        rule([zaezd_words, ulitsa_name.clone()]),
        rule([ploschadka_words, ulitsa_name.clone()]),
        rule([balka_words, ulitsa_name.clone()]),
        rule([bugor_words, ulitsa_name.clone()]),
        rule([vzvoz_words, ulitsa_name.clone()]),
        rule([kosa_words, ulitsa_name.clone()]),
        rule([mayak_words, ulitsa_name.clone()]),
        rule([platforma_words, ulitsa_name.clone()]),
        rule([polustanok_words, ulitsa_name.clone()]),
        rule([port_words, ulitsa_name.clone()]),
        ulitsa_post_prospekt,
        ulitsa_post_shosse,
        ulitsa_post_bulvar,
        ulitsa_post_nabereg,
        ulitsa_post_tupik,
        ulitsa_post_pereulok,
        ulitsa_post_proezd,
        ulitsa_post_alleya,
        ulitsa_post_ploshad,
        ulitsa_post_trakt,
        ulitsa_post_liniya,
        ulitsa_post_doroga,
        ulitsa_post_val,
        ulitsa_post_kolco,
        ulitsa_post_spusk,
        ulitsa_post_most,
        ulitsa_post_park,
        ulitsa_post_sezd,
        ulitsa_post_bereg,
        ulitsa_post2_prospekt,
        ulitsa_post2_shosse,
        ulitsa_post2_bulvar,
    ])
    .interpretation_fact::<Ulitsa>();

    // -----------------------------------------------------------------------
    // DOM
    // Второе определение LETTER — включает все буквы алфавита (кириллица + латиница)
    // -----------------------------------------------------------------------

    let letter = pred(in_caseless(&[
        "а", "б", "в", "г", "д", "е", "ё", "ж", "з", "и", "й", "к", "л", "м", "н", "о", "п", "р",
        "с", "т", "у", "ф", "х", "ц", "ч", "ш", "щ", "ъ", "ы", "ь", "э", "ю", "я", "ф", "a", "b",
        "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t",
        "u", "v", "w", "x", "y", "z",
    ]));

    let dom_words = abbr_dot("д", "дом").interpretation(Dom::kind.r#const("д."));

    let vladenie_words = ((pred(caseless("вл")) + dot.clone().optional())
        | (pred(caseless("двлд")) + dot.clone().optional())
        | pred(normalized("владение")))
    .interpretation(Dom::kind.r#const("влд."));

    let zdanie_words = ((pred(caseless("зд")) + dot.clone().optional())
        | pred(normalized("здание")))
    .interpretation(Dom::kind.r#const("зд."));

    let dom_number = or_([
        rule([int_tok.clone(), dash.clone(), letter.clone()]),
        rule([int_tok.clone(), letter.clone()]),
        rule([
            int_tok.clone(),
            slash.clone(),
            int_tok.clone(),
            letter.clone(),
        ]),
        rule([int_tok.clone(), slash.clone(), int_tok.clone()]),
        int_tok.clone(),
        letter.clone(),
        rule([int_tok.clone(), dash.clone(), int_tok.clone()]),
        rule([int_tok.clone(), pred(caseless("литер")), letter.clone()]),
        rule([
            int_tok.clone(),
            pred(caseless("лит")),
            dot.clone().optional(),
            letter.clone(),
        ]),
    ])
    .interpretation(Dom::number);

    let dom = or_([
        rule([dom_words, dom_number.clone()]),
        rule([vladenie_words, dom_number.clone()]),
        rule([zdanie_words, dom_number.clone()]),
    ])
    .interpretation_fact::<Dom>();

    // -----------------------------------------------------------------------
    // STROENIE
    // -----------------------------------------------------------------------

    let stroenie_words = (pred(normalized("строение"))
        | (pred(caseless("стр")) + dot.clone().optional())
        | (pred(caseless("с")) + dot.clone().optional()))
    .interpretation(Stroenie::kind.r#const("стр."));

    let stroenie_number = or_([
        rule([int_tok.clone(), dash.clone(), letter.clone()]),
        rule([int_tok.clone(), letter.clone()]),
        int_tok.clone(),
        letter.clone(),
        rule([int_tok.clone(), dash.clone(), int_tok.clone()]),
        rule([title_tok.clone(), dash.clone(), int_tok.clone()]),
        rule([pred(caseless("литер")), letter.clone()]),
        rule([pred(caseless("литер")), int_tok.clone()]),
        rule([
            pred(caseless("лит")),
            dot.clone().optional(),
            letter.clone(),
        ]),
    ])
    .interpretation(Stroenie::number);

    let stroenie = rule([stroenie_words, stroenie_number]).interpretation_fact::<Stroenie>();

    // -----------------------------------------------------------------------
    // KORPUS
    // -----------------------------------------------------------------------

    let korpus_words = (pred(normalized("корпус"))
        | (pred(caseless("корп")) + dot.clone().optional())
        | (pred(caseless("к")) + dot.clone().optional())
        | rule([slash.clone(), pred(caseless("к")), dot.clone().optional()])
        | rule([
            slash.clone(),
            pred(caseless("корп")),
            dot.clone().optional(),
        ]))
    .interpretation(Korpus::kind.r#const("к."));

    let korpus_number = or_([
        rule([int_tok.clone(), letter.clone()]),
        int_tok.clone(),
        letter.clone(),
        rule([pred(caseless("литер")), letter.clone()]),
        rule([pred(caseless("литер")), int_tok.clone()]),
        rule([pred(normalized("башня")), letter.clone()]),
        rule([pred(normalized("башня")), int_tok.clone()]),
    ])
    .interpretation(Korpus::number);

    let korpus = rule([korpus_words, korpus_number]).interpretation_fact::<Korpus>();

    // -----------------------------------------------------------------------
    // KVARTIRA
    // -----------------------------------------------------------------------

    let kvartira_words = (pred(normalized("квартира"))
        | (pred(caseless("кв")) + dot.clone().optional()))
    .interpretation(Kvartira::kind.r#const("кв."));

    let kvartira_number = or_([
        rule([int_tok.clone(), dash.clone(), letter.clone()]),
        rule([int_tok.clone(), letter.clone()]),
        rule([int_tok.clone(), slash.clone(), int_tok.clone()]),
        int_tok.clone(),
    ])
    .interpretation(Kvartira::number);

    let kvartira = rule([kvartira_words, kvartira_number]).interpretation_fact::<Kvartira>();

    // -----------------------------------------------------------------------
    // KOMNATA
    // -----------------------------------------------------------------------

    let komnata_words = (pred(normalized("комната"))
        | (pred(caseless("комн")) + dot.clone().optional())
        | (pred(caseless("ком")) + dot.clone().optional()))
    .interpretation(Komnata::kind.r#const("комн."));

    let komnata_number = or_([int_tok.clone(), rule([int_tok.clone(), letter.clone()])])
        .interpretation(Komnata::number);

    let komnata = rule([komnata_words, komnata_number]).interpretation_fact::<Komnata>();

    // -----------------------------------------------------------------------
    // OFIS
    // -----------------------------------------------------------------------

    let ofis_words = abbr_dot("оф", "офис").interpretation(Ofis::kind.r#const("офис"));

    let ofis_number = or_([int_tok.clone(), rule([int_tok.clone(), letter.clone()])])
        .interpretation(Ofis::number);

    let ofis = rule([ofis_words, ofis_number]).interpretation_fact::<Ofis>();

    // -----------------------------------------------------------------------
    // POMESHENIE
    // -----------------------------------------------------------------------

    let pomeshenie_words = (pred(normalized("помещение"))
        | (pred(caseless("пом")) + dot.clone().optional()))
    .interpretation(Pomeshenie::kind.r#const("помещ."));

    let pomeshenie_number = or_([
        int_tok.clone(),
        rule([int_tok.clone(), letter.clone()]),
        rule([int_tok.clone(), dash.clone(), letter.clone()]),
    ])
    .interpretation(Pomeshenie::number);

    let pomeshenie =
        rule([pomeshenie_words, pomeshenie_number]).interpretation_fact::<Pomeshenie>();

    // -----------------------------------------------------------------------
    // UCHASTOK
    // -----------------------------------------------------------------------

    let uchastok_words = ((pred(caseless("уч")) + dot.clone().optional())
        | pred(normalized("участок")))
    .interpretation(Uchastok::kind.r#const("уч."));

    let uchastok_number = or_([
        int_tok.clone(),
        rule([int_tok.clone(), letter.clone()]),
        rule([int_tok.clone(), slash.clone(), int_tok.clone()]),
    ])
    .interpretation(Uchastok::number);

    let uchastok = rule([uchastok_words, uchastok_number]).interpretation_fact::<Uchastok>();

    // -----------------------------------------------------------------------
    // DO_VOSTREBOVANIYA
    // -----------------------------------------------------------------------

    let do_vostrebovaniya_words = (rule([pred(caseless("до")), pred(caseless("востребования"))])
        | rule([
            pred(caseless("до")),
            pred(caseless("востреб")),
            dot.clone().optional(),
        ])
        | rule([
            pred(caseless("до")),
            pred(caseless("востр")),
            dot.clone().optional(),
        ])
        | pred(caseless("довостребования")))
    .interpretation(DoVostrebovaniya::marker.r#const("до востребования"));

    let do_vostrebovaniya = do_vostrebovaniya_words.interpretation_fact::<DoVostrebovaniya>();

    // -----------------------------------------------------------------------
    // ABONENT_BOX
    // -----------------------------------------------------------------------

    let abonent_box_words = (rule([pred(caseless("а")), slash.clone(), pred(caseless("я"))])
        | pred(caseless("а/я"))
        | rule([
            pred(caseless("аб")),
            dot.clone().optional(),
            pred(caseless("ящ")),
            dot.clone().optional(),
        ])
        | rule([
            pred(caseless("аб")),
            dot.clone().optional(),
            pred(normalized("ящик")),
        ])
        | (pred(normalized("абонентский")) + pred(normalized("ящик")))
        | rule([pred(caseless("п")), slash.clone(), pred(caseless("я"))])
        | pred(caseless("п/я"))
        | (pred(normalized("почтовый")) + pred(normalized("ящик"))))
    .interpretation(AbonentBox::kind.r#const("а/я"));

    let abonent_box_number = or_([
        int_tok.clone(),
        rule([int_tok.clone(), letter.clone()]),
        rule([int_tok.clone(), dash.clone(), int_tok.clone()]),
    ])
    .interpretation(AbonentBox::number);

    let abonent_box =
        rule([abonent_box_words, abonent_box_number]).interpretation_fact::<AbonentBox>();

    // -----------------------------------------------------------------------
    // ADDR_PART / ADDR — корневое правило
    // -----------------------------------------------------------------------

    let addr_part = or_([
        strana,
        respublika,
        krai,
        oblast,
        auto_okrug,
        raion,
        gorod,
        mikroraion,
        territoriya,
        ulitsa,
        dom,
        stroenie,
        korpus,
        kvartira,
        komnata,
        ofis,
        pomeshenie,
        uchastok,
        metro,
        do_vostrebovaniya,
        abonent_box,
    ]);

    // addr = addr_part
    let addr = addr_part;

    let mut registry = RuleRegistry::new();
    let addr_id = registry.add(addr.build(()));
    registry.validate().expect("address rules must be valid");

    (registry, addr_id)
}
