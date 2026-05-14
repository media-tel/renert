"""Yargy rules for parsing Russian addresses.

This module defines yargy grammar rules for extracting address components
such as cities, streets, buildings, apartments, etc.
"""

from yargy import and_, or_, rule  # type: ignore[import-not-found]
from yargy.interpretation import fact  # type: ignore[import-not-found]
from yargy.predicates import (  # type: ignore[import-not-found]
    caseless,
    dictionary,
    eq,
    gram,
    in_caseless,
    is_title,
    normalized,
)
from yargy.predicates import (
    type as yargy_type,
)

AddrPart = fact("AddrPart", ["value"])
Index = fact("Index", ["value"])
Strana = fact("Strana", ["name", "type"])
Oblast = fact("Oblast", ["name", "type"])
Krai = fact("Krai", ["name", "type"])
Respublika = fact("Respublika", ["name", "type"])
AutoOkrug = fact("AutoOkrug", ["name", "type"])
Raion = fact("Raion", ["name", "type"])
Gorod = fact("Gorod", ["name", "type"])
Ulitsa = fact("Ulitsa", ["name", "type"])
Dom = fact("Dom", ["number", "type"])
Stroenie = fact("Stroenie", ["number", "type"])
Korpus = fact("Korpus", ["number", "type"])
Kvartira = fact("Kvartira", ["number", "type"])
Komnata = fact("Komnata", ["number", "type"])
Ofis = fact("Ofis", ["number", "type"])
Pomeshenie = fact("Pomeshenie", ["number", "type"])
Uchastok = fact("Uchastok", ["number", "type"])
Mikroraion = fact("Mikroraion", ["name", "type"])
Territoriya = fact("Territoriya", ["name", "type"])
Metro = fact("Metro", ["name", "type"])
DoVostrebovaniya = fact("DoVostrebovaniya", ["marker"])
AbonentBox = fact("AbonentBox", ["number", "type"])

DASH = eq("-")
DOT = eq(".")
COMMA = eq(",")
INT = yargy_type("INT")
TITLE = is_title()
NOUN = gram("NOUN")
ADJF = gram("ADJF")
LETTER = and_(or_(yargy_type("LATIN"), yargy_type("RU")), or_(is_title(), gram("Name")))

INITIALS = or_(rule(LETTER, DOT, LETTER, DOT), rule(LETTER, DOT, LETTER, DOT, LETTER, DOT))

INDEX = rule(INT).interpretation(Index.value).interpretation(Index)

STRANA_RF = or_(
    rule(in_caseless({"рф"}), DOT.optional()),
    rule(in_caseless({"ru"}), DOT.optional()),
).interpretation(Strana.type.const("рф."))

STRANA_ROSSIYA = or_(
    rule(in_caseless({"россия"})),
    rule(caseless("рос"), DOT),
).interpretation(Strana.type.const("россия."))

STRANA_FULL = rule(
    normalized("российский"), normalized("федерация"),
).interpretation(Strana.type.const("страна."))

STRANA = or_(
    STRANA_RF,
    STRANA_ROSSIYA,
    STRANA_FULL,
).interpretation(Strana)

RESPUBLIKA_WORDS = or_(rule(caseless("респ"), DOT.optional()), rule(normalized("республика"))).interpretation(
    Respublika.type.const("респ.")
)

RESPUBLIKA_ADJF = or_(
    rule(
        dictionary(
            {
                "удмуртский",
                "удмуртская",
                "чеченский",
                "чеченская",
                "чувашский",
                "чувашская",
            }
        )
    ),
    rule(caseless("карачаево"), DASH.optional(), normalized("черкесский")),
    rule(caseless("кабардино"), DASH.optional(), normalized("балкарский")),
).interpretation(Respublika.name)

RESPUBLIKA_NAME = or_(
    rule(
        dictionary(
            {
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
            }
        )
    ),
    rule(caseless("марий"), caseless("эл")),
    rule(normalized("северный"), normalized("осетия"), rule("-", normalized("алания")).optional()),
).interpretation(Respublika.name)

RESPUBLIKA_ABBR = in_caseless(
    {
        "кбр",
        "кчр",
        "рт",
    }
).interpretation(Respublika.name)

RESPUBLIKA = or_(
    rule(RESPUBLIKA_ADJF, RESPUBLIKA_WORDS),
    rule(RESPUBLIKA_WORDS, RESPUBLIKA_ADJF),
    rule(RESPUBLIKA_WORDS, RESPUBLIKA_NAME),
    rule(RESPUBLIKA_ABBR),
).interpretation(Respublika)

KRAI_WORDS = normalized("край").interpretation(Krai.type.const("край"))

KRAI_NAME = dictionary(
    {
        "алтайский",
        "забайкальский",
        "камчатский",
        "краснодарский",
        "красноярский",
        "пермский",
        "приморский",
        "ставропольский",
        "хабаровский",
    }
).interpretation(Krai.name)

KRAI = or_(rule(KRAI_NAME, KRAI_WORDS), rule(KRAI_WORDS, KRAI_NAME)).interpretation(Krai)

OBLAST_WORDS = or_(rule(normalized("область")), rule(caseless("обл"), DOT.optional())).interpretation(
    Oblast.type.const("обл.")
)

AUTO_OBLAST_WORDS = or_(
    rule(caseless("а"), DOT.optional(), caseless("обл"), DOT.optional()),
    rule(caseless("аобл")),
    rule(normalized("автономная"), normalized("область")),
).interpretation(Oblast.type.const("а.обл."))

OKRUG_REGION_WORDS = or_(
    rule(normalized("округ")),
).interpretation(Oblast.type.const("округ"))

OBLAST_NAME = dictionary(
    {
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
    }
).interpretation(Oblast.name)

OBLAST = or_(
    rule(OBLAST_NAME, OBLAST_WORDS),
    rule(OBLAST_WORDS, OBLAST_NAME),
    rule(OBLAST_NAME, AUTO_OBLAST_WORDS),
    rule(AUTO_OBLAST_WORDS, OBLAST_NAME),
    rule(OBLAST_NAME, OKRUG_REGION_WORDS),
    rule(OKRUG_REGION_WORDS, OBLAST_NAME),
).interpretation(Oblast)

AUTO_OKRUG_NAME = or_(
    rule(
        dictionary(
            {
                "чукотский",
                "ненецкий",
                "еврейский",
                "еврейская",
            }
        )
    ),
    rule(caseless("ямало"), "-", normalized("ненецкий")),
).interpretation(AutoOkrug.name)

AUTO_OKRUG_WORDS = or_(
    rule(normalized("автономный"), normalized("округ")),
    rule(normalized("автономный"), normalized("область")),
    rule(caseless("авт"), DOT.optional(), normalized("округ")),
    rule(caseless("авт"), DOT.optional(), normalized("область")),
    rule(caseless("авт"), DOT.optional(), caseless("обл"), DOT.optional()),
    rule(caseless("авт"), DOT.optional(), caseless("окр"), DOT.optional()),
    rule(caseless("ао")),
    rule(caseless("а"), "/", caseless("о")),
    rule(caseless("а"), "/", caseless("обл"), DOT.optional()),
).interpretation(AutoOkrug.type.const("а.окр."))

HANTI = rule(caseless("ханты"), "-", normalized("мансийский")).interpretation(AutoOkrug.name)

AUTO_OKRUG = or_(
    rule(AUTO_OKRUG_NAME, AUTO_OKRUG_WORDS),
    rule(AUTO_OKRUG_WORDS, AUTO_OKRUG_NAME),
    or_(
        rule(HANTI, AUTO_OKRUG_WORDS, "-", normalized("югра")),
        rule(HANTI, AUTO_OKRUG_WORDS),
        rule(caseless("хмао")).interpretation(AutoOkrug.name),
        rule(caseless("хмао"), "-", caseless("югра")).interpretation(AutoOkrug.name),
        rule(caseless("янао")).interpretation(AutoOkrug.name),
        rule(caseless("нао")).interpretation(AutoOkrug.name),
        rule(caseless("еао")).interpretation(AutoOkrug.name),
        rule(caseless("чао")).interpretation(AutoOkrug.name),
    ),
).interpretation(AutoOkrug)

RAION_WORDS = or_(
    rule(in_caseless({"р", "p"}), "-", in_caseless({"он", "н", "oн"}), DOT.optional()),
    rule(normalized("район"), DOT.optional()),
).interpretation(Raion.type.const("р-н"))

RAION_ULUSS_WORDS = or_(
    rule(caseless("у"), DOT.optional()),
    rule(normalized("улус")),
    rule(normalized("улуус")),
).interpretation(Raion.type.const("у."))

GOROD_OKRUG_WORDS = or_(
    rule(caseless("г"), DOT.optional(), caseless("о"), DOT.optional()),
    rule(normalized("городской"), normalized("округ")),
).interpretation(Raion.type.const("г.о."))

MUN_RAION_WORDS = or_(
    rule(caseless("м"), DOT.optional(), caseless("р"), "-", caseless("н")),
    rule(caseless("м.р-н")),
    rule(normalized("муниципальный"), normalized("район")),
).interpretation(Raion.type.const("м.р-н"))

MUN_OKRUG_WORDS = or_(
    rule(caseless("м"), DOT.optional(), caseless("о"), DOT.optional()),
    rule(caseless("м.о.")),
    rule(normalized("муниципальный"), normalized("округ")),
).interpretation(Raion.type.const("м.о."))

VNUT_TER_WORDS = or_(
    rule(caseless("вн"), DOT.optional(), caseless("тер"), DOT.optional(), caseless("г"), DOT.optional()),
    rule(normalized("внутригородская"), normalized("территория")),
).interpretation(Raion.type.const("вн.тер.г."))

POSELENIE_WORDS = or_(
    rule(normalized("поселение")),
).interpretation(Raion.type.const("пос."))

FED_TER_WORDS = or_(
    rule(caseless("ф"), DOT.optional(), caseless("т"), DOT.optional()),
    rule(normalized("федеральная"), normalized("территория")),
).interpretation(Raion.type.const("ф.т."))

VNUT_RAION_WORDS = or_(
    rule(caseless("вн"), DOT.optional(), caseless("р"), "-", caseless("н")),
    rule(normalized("внутригородской"), normalized("район")),
).interpretation(Raion.type.const("вн.р-н"))

MEZHSEL_TER_WORDS = or_(
    rule(caseless("межсел"), DOT.optional(), caseless("тер"), DOT.optional()),
    rule(normalized("межселенная"), normalized("территория")),
).interpretation(Raion.type.const("межсел.тер."))

IMENI_WORDS = or_(rule(caseless("им"), DOT.optional()), rule(normalized("имени")))

ABBR = dictionary({"влксм", "ссср", "ркка", "рвсн", "ким", "мжк", "нгду"})

RAION_SIMPLE_NAME = and_(ADJF, TITLE)

RAION_NAME = or_(
    rule(RAION_SIMPLE_NAME),
    rule(TITLE, DASH, RAION_SIMPLE_NAME),
    rule(TITLE, DASH, TITLE),
    rule(IMENI_WORDS, TITLE, TITLE),
    rule(IMENI_WORDS, TITLE),
    rule(INT, DASH, caseless("й")),
    rule(INT, DASH, caseless("ый")),
    rule(INT, DASH, caseless("я")),
    rule(RAION_SIMPLE_NAME, normalized("городской"), normalized("округ")),
    rule(TITLE, DASH, RAION_SIMPLE_NAME, normalized("городской"), normalized("округ")),
    rule(TITLE, DASH, TITLE, normalized("городской"), normalized("округ")),
).interpretation(Raion.name)

RAION = or_(
    rule(RAION_WORDS, RAION_NAME),
    rule(RAION_NAME, RAION_WORDS),
    rule(RAION_ULUSS_WORDS, RAION_NAME),
    rule(RAION_NAME, RAION_ULUSS_WORDS),
    rule(GOROD_OKRUG_WORDS, RAION_NAME),
    rule(RAION_NAME, GOROD_OKRUG_WORDS),
    rule(MUN_RAION_WORDS, RAION_NAME),
    rule(RAION_NAME, MUN_RAION_WORDS),
    rule(MUN_OKRUG_WORDS, RAION_NAME),
    rule(RAION_NAME, MUN_OKRUG_WORDS),
    rule(VNUT_TER_WORDS, RAION_NAME),
    rule(RAION_NAME, VNUT_TER_WORDS),
    rule(POSELENIE_WORDS, RAION_NAME),
    rule(RAION_NAME, POSELENIE_WORDS),
    rule(FED_TER_WORDS, RAION_NAME),
    rule(RAION_NAME, FED_TER_WORDS),
    rule(VNUT_RAION_WORDS, RAION_NAME),
    rule(RAION_NAME, VNUT_RAION_WORDS),
    rule(MEZHSEL_TER_WORDS, RAION_NAME),
    rule(RAION_NAME, MEZHSEL_TER_WORDS),
).interpretation(Raion)

GFZ_WORDS = or_(
    rule(caseless("г"), DOT.optional(), caseless("ф"), DOT.optional(), caseless("з"), DOT.optional()),
    rule(normalized("город"), normalized("федеральный"), normalized("значение")),
).interpretation(Gorod.type.const("г.ф.з."))

GOROD_WORDS = or_(
    rule(normalized("город")),
    rule(caseless("гор"), DOT.optional()),
    rule(caseless("г"), DOT.optional()),
).interpretation(Gorod.type.const("г."))

POSELOK_WORDS = or_(
    rule(caseless("п"), DOT.optional()), rule(caseless("пос"), DOT.optional()), rule(normalized("поселок"))
).interpretation(Gorod.type.const("п."))

PGT_WORDS = or_(
    rule(caseless("пгт"), DOT.optional()), rule(normalized("поселок"), normalized("городского"), normalized("типа"))
).interpretation(Gorod.type.const("пгт."))

SELO_WORDS = or_(rule(caseless("с"), DOT.optional()), rule(normalized("село"))).interpretation(Gorod.type.const("с."))

DEREVNYA_WORDS = or_(rule(caseless("д"), DOT.optional()), rule(normalized("деревня"))).interpretation(
    Gorod.type.const("д")
)

RP_WORDS = or_(rule(caseless("рп"), DOT.optional()), rule(normalized("рабочий"), normalized("поселок"))).interpretation(
    Gorod.type.const("рп")
)

HUTOR_WORDS = or_(rule(caseless("х"), DOT.optional()), rule(normalized("хутор"))).interpretation(
    Gorod.type.const("х.")
)

GOROD_SIMPLE_NAME = and_(TITLE, or_(NOUN, ADJF))

GOROD_COMPLEX_NAME = or_(
    rule(GOROD_SIMPLE_NAME, DASH, GOROD_SIMPLE_NAME),
    rule(TITLE, DASH, TITLE),
    rule(TITLE, DASH, caseless("на"), DASH, TITLE),
)

GOROD_NAME = or_(
    rule(GOROD_SIMPLE_NAME, IMENI_WORDS, TITLE),
    rule(GOROD_COMPLEX_NAME),
    rule(GOROD_SIMPLE_NAME),
    rule(INT, DASH, caseless("й"), TITLE),
    rule(INT, DASH, caseless("ый"), TITLE),
    rule(INT, DASH, caseless("я"), TITLE),
    rule(INT, DASH, caseless("е"), TITLE),
    rule(INT, TITLE),
    rule(INT, DASH, caseless("й")),
    rule(INT, DASH, caseless("ый")),
    rule(INT, DASH, caseless("я")),
    rule(INT, DASH, caseless("е")),
    rule(INT),
    rule(TITLE),
    rule(ABBR),
    rule(caseless("пмк"), DASH, INT),
    rule(caseless("дск"), DASH, INT),
).interpretation(Gorod.name)

STANCIYA_WORDS = or_(rule(caseless("ст"), DOT), rule(normalized("станция"))).interpretation(Gorod.type.const("ст."))

TER_WORDS = or_(rule(caseless("тер"), DOT.optional()), rule(normalized("территория"))).interpretation(
    Gorod.type.const("тер.")
)

SELSKOE_POSELENIE_WORDS = or_(
    rule(caseless("с"), "/", caseless("п")), rule(normalized("сельское"), normalized("поселение"))
).interpretation(Gorod.type.const("с/п"))

POSELOK_PRI_STANCII_WORDS = or_(rule(caseless("п/ст")), rule(caseless("п"), "/", caseless("ст"))).interpretation(
    Gorod.type.const("п/ст")
)

STANICA_WORDS = or_(
    rule(caseless("ст-ца"), DOT.optional()),
    rule(caseless("ст"), "-", caseless("ца"), DOT.optional()),
    rule(normalized("станица")),
).interpretation(Gorod.type.const("ст-ца"))

MIKRORAION_WORDS = or_(rule(caseless("мкр"), DOT.optional()), rule(normalized("микрорайон"))).interpretation(
    Mikroraion.type.const("мкр.")
)

SNT_GOROD_WORDS = or_(
    rule(caseless("снт"), DOT.optional()), rule(normalized("садоводческое"), normalized("товарищество"))
).interpretation(Gorod.type.const("снт"))

SAD_WORDS = or_(rule(caseless("сад"), DOT.optional()), rule(normalized("садоводство"))).interpretation(
    Gorod.type.const("сад")
)

PROM_RAION_WORDS = or_(
    rule(caseless("п/р")),
    rule(caseless("п"), "/", caseless("р")),
    rule(normalized("промышленный"), normalized("район")),
).interpretation(Mikroraion.type.const("п/р"))

ZHILOY_RAION_WORDS = or_(
    rule(caseless("ж"), "/", caseless("р")),
    rule(caseless("жилрайон")),
    rule(normalized("жилой"), normalized("район")),
).interpretation(Mikroraion.type.const("ж/р"))

GORODOK_WORDS = or_(
    rule(caseless("г-к"), DOT.optional()),
    rule(caseless("г"), "-", caseless("к"), DOT.optional()),
    rule(normalized("городок")),
).interpretation(Mikroraion.type.const("г-к"))

DNP_GOROD_WORDS = or_(
    rule(caseless("днп"), DOT.optional()), rule(normalized("дачное"), normalized("партнерство"))
).interpretation(Gorod.type.const("днп"))

GSK_GOROD_WORDS = or_(
    rule(caseless("гск"), DOT.optional()),
    rule(normalized("гаражно"), "-", normalized("строительный"), normalized("кооператив")),
).interpretation(Gorod.type.const("гск"))

OSTROV_WORDS = or_(rule(caseless("остров"))).interpretation(Gorod.type.const("остров"))

MESTOROZHD_WORDS = or_(rule(normalized("месторождение")), rule(caseless("месторожд"), DOT.optional())).interpretation(
    Gorod.type.const("месторожд.")
)

AUL_WORDS = or_(rule(normalized("аул")), rule(caseless("аул"))).interpretation(Gorod.type.const("аул"))

GP_WORDS = or_(
    rule(caseless("гп"), DOT.optional()), rule(normalized("городское"), normalized("поселение"))
).interpretation(Gorod.type.const("гп."))

NP_WORDS = or_(
    rule(caseless("нп"), DOT.optional()), rule(normalized("населенный"), normalized("пункт"))
).interpretation(Gorod.type.const("нп."))

SLOBODA_WORDS = or_(rule(caseless("сл"), DOT.optional()), rule(normalized("слобода"))).interpretation(
    Gorod.type.const("сл.")
)

RAZEZD_WORDS = or_(rule(caseless("рзд"), DOT.optional()), rule(normalized("разъезд"))).interpretation(
    Gorod.type.const("рзд.")
)

DACHNIY_POSELOK_WORDS = or_(
    rule(caseless("дп"), DOT.optional()), rule(normalized("дачный"), normalized("поселок"))
).interpretation(Gorod.type.const("дп."))

AAL_WORDS = or_(rule(caseless("аал"))).interpretation(Gorod.type.const("аал"))

KP_WORDS = or_(
    rule(caseless("кп"), DOT.optional()),
    rule(normalized("курортный"), normalized("поселок")),
).interpretation(Gorod.type.const("кп."))

ULUSS_WORDS = or_(
    rule(caseless("у"), DOT.optional()),
    rule(normalized("улус")),
    rule(normalized("улуус")),
).interpretation(Gorod.type.const("у."))

MESTECHKO_WORDS = or_(
    rule(caseless("м"), "-", caseless("ко")),
    rule(normalized("местечко")),
).interpretation(Gorod.type.const("м-ко"))

POCHINOK_WORDS = or_(
    rule(caseless("п"), "-", caseless("к")),
    rule(normalized("починок")),
).interpretation(Gorod.type.const("п-к"))

ARBAN_WORDS = or_(rule(normalized("арбан"))).interpretation(Gorod.type.const("арбан"))

VYSELKI_WORDS = or_(
    rule(caseless("высел"), DOT.optional()),
    rule(caseless("в"), "-", caseless("ки")),
    rule(normalized("выселки")),
).interpretation(Gorod.type.const("в-ки"))

SP_WORDS = or_(
    rule(caseless("сп"), DOT.optional()),
    rule(normalized("сельский"), normalized("поселок")),
).interpretation(Gorod.type.const("сп."))

GP_GOROD_WORDS = or_(
    rule(caseless("гп"), DOT.optional()),
    rule(normalized("городской"), normalized("поселок")),
).interpretation(Gorod.type.const("гп."))

VOLOST_WORDS = or_(rule(normalized("волость"))).interpretation(Gorod.type.const("волость"))

MASSIV_WORDS = or_(rule(normalized("массив"))).interpretation(Gorod.type.const("массив"))

POGOST_WORDS = or_(rule(normalized("погост"))).interpretation(Gorod.type.const("погост"))

ZAIMKA_WORDS = or_(
    rule(normalized("заимка")),
    rule(caseless("з"), "-", caseless("ка")),
).interpretation(Gorod.type.const("з-ка"))

KAZARMA_WORDS = or_(rule(normalized("казарма"))).interpretation(Gorod.type.const("казарма"))

KISHLAK_WORDS = or_(
    rule(caseless("киш"), DOT.optional()),
    rule(normalized("кишлак")),
).interpretation(Gorod.type.const("киш."))

KORDON_WORDS = or_(rule(normalized("кордон"))).interpretation(Gorod.type.const("кордон"))

ZHILZONA_WORDS = or_(
    rule(caseless("жилзона")),
    rule(normalized("жилая"), normalized("зона")),
).interpretation(Gorod.type.const("жилзона"))

AVTODOROGA_WORDS = or_(rule(normalized("автодорога"))).interpretation(Gorod.type.const("автодорога"))

ZIMOVIE_WORDS = or_(
    rule(caseless("зим"), DOT),
    rule(normalized("зимовье")),
).interpretation(Gorod.type.const("зим."))

LESPROMHOZ_WORDS = or_(
    rule(caseless("лпх")),
    rule(normalized("леспромхоз")),
).interpretation(Gorod.type.const("лпх"))

POCHTA_WORDS = or_(
    rule(caseless("п"), "/", caseless("о")),
    rule(caseless("п/о")),
    rule(normalized("почтовое"), normalized("отделение")),
).interpretation(Gorod.type.const("п/о"))

SELSKAYA_ADM_WORDS = or_(
    rule(caseless("с"), "/", caseless("а")),
    rule(normalized("сельская"), normalized("администрация")),
).interpretation(Gorod.type.const("с/а"))

SELSKOE_MO_WORDS = or_(
    rule(caseless("с"), "/", caseless("мо")),
    rule(normalized("сельское"), normalized("муниципальное"), normalized("образование")),
).interpretation(Gorod.type.const("с/мо"))

SELSKIY_OKRUG_WORDS = or_(
    rule(caseless("с"), "/", caseless("о")),
    rule(normalized("сельский"), normalized("округ")),
).interpretation(Gorod.type.const("с/о"))

SELSOVET_WORDS = or_(
    rule(caseless("с"), "/", caseless("с")),
    rule(normalized("сельсовет")),
).interpretation(Gorod.type.const("с/с"))

POS_RZD_WORDS = or_(
    rule(caseless("пос"), DOT.optional(), caseless("рзд"), DOT.optional()),
    rule(normalized("поселок"), normalized("разъезд")),
).interpretation(Gorod.type.const("пос.рзд."))

FERMA_WORDS = or_(rule(normalized("ферма"))).interpretation(Gorod.type.const("ферма"))

YURTY_WORDS = or_(
    rule(normalized("юрты")),
).interpretation(Gorod.type.const("ю."))

ZHT_WORDS = or_(
    rule(caseless("жт")),
    rule(normalized("животноводческая"), normalized("точка")),
).interpretation(Gorod.type.const("жт"))

PLAN_RAION_WORDS = or_(
    rule(caseless("пл"), DOT.optional(), caseless("р"), "-", caseless("н")),
    rule(normalized("планировочный"), normalized("район")),
).interpretation(Gorod.type.const("пл.р-н"))

ZHD_ST_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("ст"), DOT.optional()),
    rule(normalized("железнодорожная"), normalized("станция")),
).interpretation(Gorod.type.const("ж/д ст."))

ZHD_RZD_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("рзд"), DOT.optional()),
    rule(normalized("железнодорожный"), normalized("разъезд")),
).interpretation(Gorod.type.const("ж/д рзд."))

ZHD_PLATF_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("платф"), DOT.optional()),
    rule(caseless("ж"), "/", caseless("д"), caseless("пл"), "-", caseless("ма")),
    rule(normalized("железнодорожная"), normalized("платформа")),
).interpretation(Gorod.type.const("ж/д пл-ма"))

ZHD_BUDKA_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), normalized("будка")),
    rule(caseless("ж"), "/", caseless("д"), caseless("б"), "-", caseless("ка")),
    rule(normalized("железнодорожная"), normalized("будка")),
).interpretation(Gorod.type.const("ж/д б-ка"))

ZHD_KAZARMA_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("казарм"), DOT.optional()),
    rule(caseless("ж"), "/", caseless("д"), caseless("к"), "-", caseless("ма")),
    rule(normalized("железнодорожная"), normalized("казарма")),
).interpretation(Gorod.type.const("ж/д к-ма"))

ZHD_OP_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("оп")),
    rule(caseless("ж"), "/", caseless("д"), caseless("о"), DOT, caseless("п"), DOT.optional()),
    rule(normalized("железнодорожный"), normalized("остановочный"), normalized("пункт")),
).interpretation(Gorod.type.const("ж/д о.п."))

ZHD_POST_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("пост")),
    rule(normalized("железнодорожный"), normalized("пост")),
).interpretation(Gorod.type.const("ж/д_пост"))

ZHD_BLOKPOST_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("бл"), "-", caseless("ст")),
    rule(normalized("железнодорожный"), normalized("блокпост")),
).interpretation(Gorod.type.const("ж/д бл-ст"))

ZHD_VETKA_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("в"), "-", caseless("ка")),
    rule(normalized("железнодорожная"), normalized("ветка")),
).interpretation(Gorod.type.const("ж/д в-ка"))

ZHD_KOMBINAT_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("к"), "-", caseless("т")),
    rule(normalized("железнодорожный"), normalized("комбинат")),
).interpretation(Gorod.type.const("ж/д к-т"))

ZHD_PLOSCHADKA_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("пл"), "-", caseless("ка")),
    rule(normalized("железнодорожная"), normalized("площадка")),
).interpretation(Gorod.type.const("ж/д пл-ка"))

ZHD_PUT_POST_WORDS = or_(
    rule(caseless("ж"), "/", caseless("д"), caseless("п"), DOT, caseless("п"), DOT.optional()),
    rule(normalized("железнодорожный"), normalized("путевой"), normalized("пост")),
).interpretation(Gorod.type.const("ж/д п.п."))

METRO_WORDS = or_(
    rule(normalized("метро"), normalized("станция")),
    rule(normalized("метро")),
    rule(caseless("м"), DOT),
).interpretation(Metro.type.const("метро"))

METRO_NAME = or_(
    rule(eq('"'), TITLE, eq('"')), rule(eq('"'), TITLE, TITLE, eq('"')), rule(TITLE), rule(TITLE, DASH, TITLE)
).interpretation(Metro.name)

METRO = rule(METRO_WORDS, METRO_NAME).interpretation(Metro)

GOROD = or_(
    rule(GFZ_WORDS, GOROD_NAME),
    rule(GOROD_WORDS, GOROD_NAME),
    rule(POSELOK_WORDS, GOROD_NAME),
    rule(PGT_WORDS, GOROD_NAME),
    rule(SELO_WORDS, GOROD_NAME),
    rule(DEREVNYA_WORDS, GOROD_NAME),
    rule(RP_WORDS, GOROD_NAME),
    rule(HUTOR_WORDS, GOROD_NAME),
    rule(STANCIYA_WORDS, GOROD_NAME),
    rule(SELSKOE_POSELENIE_WORDS, GOROD_NAME),
    rule(POSELOK_PRI_STANCII_WORDS, GOROD_NAME),
    rule(STANICA_WORDS, GOROD_NAME),
    rule(SAD_WORDS, GOROD_NAME),
    rule(OSTROV_WORDS, GOROD_NAME),
    rule(MESTOROZHD_WORDS, GOROD_NAME),
    rule(AUL_WORDS, GOROD_NAME),
    rule(GP_WORDS, GOROD_NAME),
    rule(NP_WORDS, GOROD_NAME),
    rule(SLOBODA_WORDS, GOROD_NAME),
    rule(RAZEZD_WORDS, GOROD_NAME),
    rule(DACHNIY_POSELOK_WORDS, GOROD_NAME),
    rule(AAL_WORDS, GOROD_NAME),
    rule(KP_WORDS, GOROD_NAME),
    rule(MESTECHKO_WORDS, GOROD_NAME),
    rule(POCHINOK_WORDS, GOROD_NAME),
    rule(ARBAN_WORDS, GOROD_NAME),
    rule(VYSELKI_WORDS, GOROD_NAME),
    rule(SP_WORDS, GOROD_NAME),
    rule(GP_GOROD_WORDS, GOROD_NAME),
    rule(VOLOST_WORDS, GOROD_NAME),
    rule(MASSIV_WORDS, GOROD_NAME),
    rule(POGOST_WORDS, GOROD_NAME),
    rule(ZAIMKA_WORDS, GOROD_NAME),
    rule(KAZARMA_WORDS, GOROD_NAME),
    rule(KISHLAK_WORDS, GOROD_NAME),
    rule(KORDON_WORDS, GOROD_NAME),
    rule(ZHILZONA_WORDS, GOROD_NAME),
    rule(AVTODOROGA_WORDS, GOROD_NAME),
    rule(ZIMOVIE_WORDS, GOROD_NAME),
    rule(LESPROMHOZ_WORDS, GOROD_NAME),
    rule(POCHTA_WORDS, GOROD_NAME),
    rule(SELSKAYA_ADM_WORDS, GOROD_NAME),
    rule(SELSKOE_MO_WORDS, GOROD_NAME),
    rule(SELSKIY_OKRUG_WORDS, GOROD_NAME),
    rule(SELSOVET_WORDS, GOROD_NAME),
    rule(POS_RZD_WORDS, GOROD_NAME),
    rule(FERMA_WORDS, GOROD_NAME),
    rule(YURTY_WORDS, GOROD_NAME),
    rule(ZHT_WORDS, GOROD_NAME),
    rule(PLAN_RAION_WORDS, GOROD_NAME),
    rule(ZHD_ST_WORDS, GOROD_NAME),
    rule(ZHD_RZD_WORDS, GOROD_NAME),
    rule(ZHD_PLATF_WORDS, GOROD_NAME),
    rule(ZHD_BUDKA_WORDS, GOROD_NAME),
    rule(ZHD_KAZARMA_WORDS, GOROD_NAME),
    rule(ZHD_OP_WORDS, GOROD_NAME),
    rule(ZHD_POST_WORDS, GOROD_NAME),
    rule(ZHD_BLOKPOST_WORDS, GOROD_NAME),
    rule(ZHD_VETKA_WORDS, GOROD_NAME),
    rule(ZHD_KOMBINAT_WORDS, GOROD_NAME),
    rule(ZHD_PLOSCHADKA_WORDS, GOROD_NAME),
    rule(ZHD_PUT_POST_WORDS, GOROD_NAME),
    rule(
        dictionary(
            {
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
            }
        )
    )
    .interpretation(Gorod.name)
    .interpretation(Gorod),
).interpretation(Gorod)

KVARTAL_WORDS = or_(
    rule(caseless("кв-л"), DOT.optional()),
    rule(caseless("кв"), "-", caseless("л"), DOT.optional()),
    rule(normalized("квартал")),
).interpretation(Mikroraion.type.const("кв-л"))

PROMZONA_WORDS = or_(
    rule(caseless("промзона"), DOT.optional()), rule(normalized("промышленная"), normalized("зона"))
).interpretation(Mikroraion.type.const("промзона"))

ZONA_WORDS = or_(
    rule(normalized("зона")),
).interpretation(Mikroraion.type.const("зона"))

MESTNOST_WORDS = or_(
    rule(normalized("местность")),
).interpretation(Mikroraion.type.const("местность"))

NKP_WORDS = or_(
    rule(caseless("н"), "/", caseless("п")),
    rule(normalized("некоммерческое"), normalized("партнерство")),
).interpretation(Mikroraion.type.const("н/п"))

MIKRORAION_NAME = or_(
    rule(GOROD_SIMPLE_NAME, IMENI_WORDS, TITLE),
    rule(GOROD_COMPLEX_NAME),
    rule(GOROD_SIMPLE_NAME),
    rule(INT, DASH, caseless("й"), TITLE),
    rule(INT, DASH, caseless("ый"), TITLE),
    rule(INT, DASH, caseless("я"), TITLE),
    rule(INT, DASH, caseless("е"), TITLE),
    rule(INT, TITLE),
    rule(INT, DASH, caseless("й")),
    rule(INT, DASH, caseless("ый")),
    rule(INT, DASH, caseless("я")),
    rule(INT, DASH, caseless("е")),
    rule(INT),
    rule(eq('"'), TITLE, TITLE, eq('"')),
    rule(eq('"'), TITLE, eq('"')),
    rule(TITLE, TITLE),
    rule(TITLE),
).interpretation(Mikroraion.name)

MIKRORAION = or_(
    rule(MIKRORAION_WORDS, MIKRORAION_NAME),
    rule(KVARTAL_WORDS, MIKRORAION_NAME),
    rule(PROMZONA_WORDS, MIKRORAION_NAME),
    rule(PROM_RAION_WORDS, MIKRORAION_NAME),
    rule(ZHILOY_RAION_WORDS, MIKRORAION_NAME),
    rule(GORODOK_WORDS, MIKRORAION_NAME),
    rule(ZONA_WORDS, MIKRORAION_NAME),
    rule(MESTNOST_WORDS, MIKRORAION_NAME),
    rule(NKP_WORDS, MIKRORAION_NAME),
).interpretation(Mikroraion)

TERRITORIYA_NAME = or_(
    rule(GOROD_SIMPLE_NAME, IMENI_WORDS, TITLE),
    rule(GOROD_COMPLEX_NAME),
    rule(GOROD_SIMPLE_NAME),
    rule(INT, DASH, caseless("й"), TITLE),
    rule(INT, DASH, caseless("ый"), TITLE),
    rule(INT, TITLE),
    rule(TITLE, TITLE, TITLE),
    rule(TITLE, TITLE),
    rule(TITLE, DASH, INT),
    rule(TITLE),
    rule(INT),
).interpretation(Territoriya.name)

TER_BASE_WORDS = or_(rule(caseless("тер"), DOT.optional()), rule(normalized("территория"))).interpretation(
    Territoriya.type.const("тер.")
)

SNT_TER_WORDS = or_(
    rule(caseless("снт"), DOT.optional()), rule(normalized("садоводческое"), normalized("товарищество"))
).interpretation(Territoriya.type.const("снт"))

DNP_TER_WORDS = or_(
    rule(caseless("днп"), DOT.optional()), rule(normalized("дачное"), normalized("партнерство"))
).interpretation(Territoriya.type.const("днп"))

GSK_TER_WORDS = or_(
    rule(caseless("гск"), DOT.optional()),
    rule(normalized("гаражно"), "-", normalized("строительный"), normalized("кооператив")),
).interpretation(Territoriya.type.const("гск"))

FH_TER_WORDS = or_(
    rule(caseless("ф"), "/", caseless("х")),
    rule(caseless("ф/х")),
    rule(normalized("фермерское"), normalized("хозяйство")),
).interpretation(Territoriya.type.const("ф/х"))

USADBA_TER_WORDS = or_(
    rule(caseless("ус"), DOT.optional()),
    rule(normalized("усадьба")),
).interpretation(Territoriya.type.const("ус."))

ST_TER_WORDS = or_(
    rule(caseless("с"), "/", caseless("т")),
    rule(caseless("с/т")),
    rule(normalized("садовое"), normalized("товарищество")),
).interpretation(Territoriya.type.const("с/т"))

TER_GSK_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("гск")),
    rule(normalized("территория"), caseless("гск")),
).interpretation(Territoriya.type.const("тер. ГСК"))

TER_DNO_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("дно")),
    rule(normalized("территория"), caseless("дно")),
).interpretation(Territoriya.type.const("тер. ДНО"))

TER_DNT_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("днт")),
    rule(normalized("территория"), caseless("днт")),
).interpretation(Territoriya.type.const("тер. ДНТ"))

TER_DPK_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("дпк")),
    rule(normalized("территория"), caseless("дпк")),
).interpretation(Territoriya.type.const("тер. ДПК"))

TER_ONT_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("онт")),
    rule(normalized("территория"), caseless("онт")),
).interpretation(Territoriya.type.const("тер. ОНТ"))

TER_OPK_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("опк")),
    rule(normalized("территория"), caseless("опк")),
).interpretation(Territoriya.type.const("тер. ОПК"))

TER_PK_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("пк")),
    rule(normalized("территория"), caseless("пк")),
).interpretation(Territoriya.type.const("тер. ПК"))

TER_SNO_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("сно")),
    rule(normalized("территория"), caseless("сно")),
).interpretation(Territoriya.type.const("тер. СНО"))

TER_SNP_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("снп")),
    rule(normalized("территория"), caseless("снп")),
).interpretation(Territoriya.type.const("тер. СНП"))

TER_SPK_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("спк")),
    rule(normalized("территория"), caseless("спк")),
).interpretation(Territoriya.type.const("тер. СПК"))

TER_TSZ_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("тсж")),
    rule(normalized("территория"), caseless("тсж")),
).interpretation(Territoriya.type.const("тер. ТСЖ"))

TER_TSN_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("тсн")),
    rule(normalized("территория"), caseless("тсн")),
).interpretation(Territoriya.type.const("тер. ТСН"))

TER_DNP_TER_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("днп")),
    rule(normalized("территория"), caseless("днп")),
).interpretation(Territoriya.type.const("тер. ДНП"))

TER_ONO_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("оно")),
    rule(normalized("территория"), caseless("оно")),
).interpretation(Territoriya.type.const("тер. ОНО"))

TER_ONP_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("онп")),
    rule(normalized("территория"), caseless("онп")),
).interpretation(Territoriya.type.const("тер. ОНП"))

TER_SNT_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("снт")),
    rule(normalized("территория"), caseless("снт")),
).interpretation(Territoriya.type.const("тер. СНТ"))

TER_SOSN_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("сосн")),
    rule(normalized("территория"), caseless("сосн")),
).interpretation(Territoriya.type.const("тер.СОСН"))

TER_FH_WORDS = or_(
    rule(caseless("тер"), DOT.optional(), caseless("ф"), DOT.optional(), caseless("х"), DOT.optional()),
    rule(normalized("территория"), caseless("фх")),
).interpretation(Territoriya.type.const("тер.ф.х."))

TERRITORIYA = or_(
    rule(TER_BASE_WORDS, TERRITORIYA_NAME),
    rule(SNT_TER_WORDS, TERRITORIYA_NAME),
    rule(DNP_TER_WORDS, TERRITORIYA_NAME),
    rule(GSK_TER_WORDS, TERRITORIYA_NAME),
    rule(FH_TER_WORDS, TERRITORIYA_NAME),
    rule(USADBA_TER_WORDS, TERRITORIYA_NAME),
    rule(ST_TER_WORDS, TERRITORIYA_NAME),
    rule(TER_GSK_WORDS, TERRITORIYA_NAME),
    rule(TER_DNO_WORDS, TERRITORIYA_NAME),
    rule(TER_DNT_WORDS, TERRITORIYA_NAME),
    rule(TER_DPK_WORDS, TERRITORIYA_NAME),
    rule(TER_ONT_WORDS, TERRITORIYA_NAME),
    rule(TER_OPK_WORDS, TERRITORIYA_NAME),
    rule(TER_PK_WORDS, TERRITORIYA_NAME),
    rule(TER_SNO_WORDS, TERRITORIYA_NAME),
    rule(TER_SNP_WORDS, TERRITORIYA_NAME),
    rule(TER_SPK_WORDS, TERRITORIYA_NAME),
    rule(TER_TSZ_WORDS, TERRITORIYA_NAME),
    rule(TER_TSN_WORDS, TERRITORIYA_NAME),
    rule(TER_DNP_TER_WORDS, TERRITORIYA_NAME),
    rule(TER_ONO_WORDS, TERRITORIYA_NAME),
    rule(TER_ONP_WORDS, TERRITORIYA_NAME),
    rule(TER_SNT_WORDS, TERRITORIYA_NAME),
    rule(TER_SOSN_WORDS, TERRITORIYA_NAME),
    rule(TER_FH_WORDS, TERRITORIYA_NAME),
).interpretation(Territoriya)

ULITSA_WORDS = or_(rule(normalized("улица")), rule(caseless("ул"), DOT.optional())).interpretation(
    Ulitsa.type.const("ул.")
)

PROSPEKT_WORDS = or_(
    rule(in_caseless({"пр", "просп", "пркт", "пр-кт", "пр-т"}), DOT.optional()),
    rule(caseless("пр"), "-", in_caseless({"кт", "т"}), DOT.optional()),
    rule(caseless("пр"), DOT.optional(), in_caseless({"кт", "т"}), DOT.optional()),
    rule(caseless("пр"), DOT.optional()),
    rule(normalized("проспект")),
).interpretation(Ulitsa.type.const("пр-кт"))

PEREULOK_WORDS = or_(rule(normalized("переулок")), rule(caseless("пер"), DOT.optional())).interpretation(
    Ulitsa.type.const("пер.")
)

PROEZD_WORDS = or_(
    rule(in_caseless({"пр-езд", "пр-зд", "пр-д", "прз"}), DOT.optional()),
    rule(caseless("пр"), "-", caseless("д"), DOT.optional()),
    rule(normalized("проезд"), DOT.optional()),
).interpretation(Ulitsa.type.const("пр-д"))

SHOSSE_WORDS = or_(rule(normalized("шоссе")), rule(caseless("ш"), DOT.optional())).interpretation(
    Ulitsa.type.const("ш.")
)

BULVAR_WORDS = or_(
    rule(normalized("бульвар")),
    rule(caseless("б"), "-", caseless("р"), DOT.optional()),
    rule(caseless("бул"), DOT.optional()),
).interpretation(Ulitsa.type.const("б-р"))

NABEREG_WORDS = or_(rule(normalized("набережная")), rule(caseless("наб"), DOT.optional())).interpretation(
    Ulitsa.type.const("наб.")
)

DOROGA_WORDS = or_(rule(normalized("дорога")), rule(caseless("дор"), DOT.optional())).interpretation(
    Ulitsa.type.const("дор.")
)

ALLEYA_WORDS = or_(rule(normalized("аллея")), rule(caseless("ал"), DOT.optional())).interpretation(
    Ulitsa.type.const("ал.")
)

PLOSHAD_WORDS = or_(rule(normalized("площадь")), rule(caseless("пл"), DOT.optional())).interpretation(
    Ulitsa.type.const("пл.")
)

LINIYA_WORDS = or_(rule(normalized("линия")), rule(caseless("лн"), DOT.optional())).interpretation(
    Ulitsa.type.const("лн.")
)

KILOMETR_WORDS = or_(rule(normalized("километр")), rule(caseless("км"), DOT.optional())).interpretation(
    Ulitsa.type.const("км")
)

TUPIK_WORDS = or_(rule(normalized("тупик")), rule(caseless("туп"), DOT.optional())).interpretation(
    Ulitsa.type.const("туп.")
)

TRAKT_WORDS = or_(
    rule(normalized("тракт")),
    rule(caseless("тр"), DOT.optional()),
).interpretation(Ulitsa.type.const("тракт"))

VAL_WORDS = or_(rule(normalized("вал"))).interpretation(Ulitsa.type.const("вал"))

VEZD_WORDS = or_(
    rule(normalized("въезд")),
    rule(caseless("взд"), DOT.optional()),
).interpretation(Ulitsa.type.const("взд."))

KOLCO_WORDS = or_(
    rule(normalized("кольцо")),
    rule(caseless("к"), "-", caseless("цо")),
).interpretation(Ulitsa.type.const("к-цо"))

SKVER_WORDS = or_(
    rule(normalized("сквер")),
    rule(caseless("с"), "-", caseless("р")),
).interpretation(Ulitsa.type.const("с-р"))

SPUSK_WORDS = or_(
    rule(normalized("спуск")),
    rule(caseless("с"), "-", caseless("к")),
).interpretation(Ulitsa.type.const("с-к"))

PROSEK_WORDS = or_(
    rule(normalized("просек")),
    rule(normalized("просека")),
    rule(caseless("пр"), "-", caseless("к")),
    rule(caseless("пр"), "-", caseless("ка")),
).interpretation(Ulitsa.type.const("пр-к"))

PROULOK_WORDS = or_(
    rule(normalized("проулок")),
    rule(caseless("проул"), DOT.optional()),
).interpretation(Ulitsa.type.const("проул."))

RYADY_WORDS = or_(
    rule(normalized("ряды")),
    rule(normalized("ряд")),
).interpretation(Ulitsa.type.const("ряды"))

PEREEZD_WORDS = or_(
    rule(normalized("переезд")),
    rule(caseless("пер"), "-", caseless("д")),
).interpretation(Ulitsa.type.const("пер-д"))

MOST_WORDS = or_(rule(normalized("мост"))).interpretation(Ulitsa.type.const("мост"))

PARK_WORDS = or_(rule(normalized("парк"))).interpretation(Ulitsa.type.const("парк"))

MAGISTRAL_WORDS = or_(
    rule(normalized("магистраль")),
    rule(caseless("мгстр"), DOT.optional()),
).interpretation(Ulitsa.type.const("мгстр."))

SEZD_WORDS = or_(
    rule(normalized("съезд")),
    rule(caseless("сзд"), DOT.optional()),
).interpretation(Ulitsa.type.const("сзд."))

BEREG_WORDS = or_(
    rule(normalized("берег")),
    rule(caseless("б"), "-", caseless("г")),
).interpretation(Ulitsa.type.const("б-г"))

PROSELOK_WORDS = or_(
    rule(normalized("проселок")),
    rule(caseless("пр"), "-", caseless("лок")),
).interpretation(Ulitsa.type.const("пр-лок"))

ZAEZD_WORDS = or_(
    rule(normalized("заезд")),
    rule(caseless("ззд"), DOT.optional()),
).interpretation(Ulitsa.type.const("ззд."))

PLOSCHADKA_WORDS = or_(
    rule(normalized("площадка")),
    rule(caseless("пл"), "-", caseless("ка")),
).interpretation(Ulitsa.type.const("пл-ка"))

BALKA_WORDS = or_(rule(normalized("балка"))).interpretation(Ulitsa.type.const("балка"))

BUGOR_WORDS = or_(rule(normalized("бугор"))).interpretation(Ulitsa.type.const("бугор"))

VZVOZ_WORDS = or_(
    rule(normalized("взвоз")),
    rule(caseless("взв"), DOT.optional()),
).interpretation(Ulitsa.type.const("взв."))

KOSA_WORDS = or_(rule(normalized("коса"))).interpretation(Ulitsa.type.const("коса"))

MAYAK_WORDS = or_(rule(normalized("маяк"))).interpretation(Ulitsa.type.const("маяк"))

PLATFORMA_WORDS = or_(
    rule(normalized("платформа")),
    rule(caseless("платф"), DOT.optional()),
).interpretation(Ulitsa.type.const("платф."))

POLUSTANOK_WORDS = or_(rule(normalized("полустанок"))).interpretation(Ulitsa.type.const("полустанок"))

PORT_WORDS = or_(rule(normalized("порт"))).interpretation(Ulitsa.type.const("порт"))

ROD = gram("gent")
ADJS = gram("ADJS")

MODIFIER_WORDS = rule(
    dictionary(
        {
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
        }
    ),
    DASH.optional(),
)

ULITSA_NAME = or_(
    rule(and_(or_(ADJF, and_(NOUN, gram("gent"))), TITLE)),
    rule(TITLE, DASH.optional(), TITLE),
    rule(MODIFIER_WORDS.optional(), TITLE),
    rule(caseless("с"), "/", caseless("п"), TITLE),
    rule(IMENI_WORDS, TITLE),
    rule(IMENI_WORDS, TITLE, TITLE),
    rule(IMENI_WORDS, INT, TITLE, TITLE),
    rule(IMENI_WORDS, INT, normalized("лет"), TITLE),
    rule(
        dictionary(
            {"архитектора", "профессора", "генерала", "маршала", "полковника", "капитана", "академика", "митрополита"}
        ),
        INITIALS,
        TITLE,
    ),
    rule(
        dictionary(
            {"архитектора", "профессора", "генерала", "маршала", "полковника", "капитана", "академика", "митрополита"}
        ),
        TITLE,
        INITIALS,
    ),
    rule(caseless("м"), DOT, TITLE),
    rule(INITIALS, TITLE),
    rule(INITIALS),
    rule(TITLE, INITIALS),
    rule(
        IMENI_WORDS,
        DOT.optional(),
        dictionary(
            {
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
            }
        ),
        TITLE,
    ),
    rule(
        IMENI_WORDS,
        DOT.optional(),
        dictionary(
            {
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
            }
        ),
        TITLE,
        TITLE,
    ),
    rule(IMENI_WORDS, DOT.optional(), normalized("газета"), eq('"'), TITLE, TITLE, eq('"')),
    rule(IMENI_WORDS, DOT, normalized("газета"), eq('"'), TITLE, TITLE, eq('"')),
    rule(IMENI_WORDS, DOT.optional(), normalized("газеты"), eq('"'), TITLE, TITLE, eq('"')),
    rule(IMENI_WORDS, DOT.optional(), normalized("газета"), eq("'"), TITLE, TITLE, eq("'")),
    rule(IMENI_WORDS, DOT, normalized("газета"), eq("'"), TITLE, TITLE, eq("'")),
    rule(IMENI_WORDS, DOT.optional(), normalized("газеты"), eq("'"), TITLE, TITLE, eq("'")),
    rule(eq('"'), TITLE, eq('"')),
    rule(eq('"'), TITLE, TITLE, eq('"')),
    rule(eq("'"), TITLE, eq("'")),
    rule(eq("'"), TITLE, TITLE, eq("'")),
    rule(
        IMENI_WORDS,
        dictionary({"гвардии"}),
        dictionary({"красноармейца", "сержанта", "майора", "капитана", "полковника"}),
        TITLE,
    ),
    rule(IMENI_WORDS, dictionary({"братьев"}), TITLE),
    rule(INT),
    rule(INT, TITLE),
    rule(INT, TITLE, TITLE),
    rule(INT, normalized("лет"), TITLE),
    rule(INT, DASH, caseless("я"), TITLE),
    rule(INT, DASH, caseless("й"), TITLE),
    rule(INT, DASH, caseless("е"), TITLE),
    rule(INT, DASH, caseless("го"), TITLE),
    rule(INT, DASH, caseless("ой"), TITLE),
    rule(INT, DASH, caseless("ая"), TITLE),
    rule(INT, DASH, caseless("ый"), TITLE),
    rule(INT, DASH, caseless("ий"), TITLE),
    rule(INT, DASH, caseless("я")),
    rule(INT, DASH, caseless("й")),
    rule(INT, DASH, caseless("е")),
    rule(INT, DASH, caseless("ый")),
    rule(INT, DASH, caseless("ой")),
    rule(INT, DASH, caseless("ая"), normalized("линия")),
    rule(INT, DASH, caseless("я"), normalized("линия")),
    rule(INT, DASH, caseless("й"), normalized("ключ")),
    rule(INT, DASH, caseless("ый"), normalized("ключ")),
    rule(INT, DASH, normalized("летие"), TITLE),
    rule(INT, normalized("лет"), ABBR),
    rule(INT, DASH, normalized("летия"), ABBR),
    rule(IMENI_WORDS, ABBR),
    rule(ABBR),
    rule(caseless("мкад")),
    rule(caseless("мкад"), INT),
    rule(caseless("мкад"), INT, DASH, caseless("й")),
    rule(TITLE, DASH, NOUN),
    rule(TITLE, DASH, ADJF),
    rule(NOUN),
    rule(ADJF),
).interpretation(Ulitsa.name)

ULITSA_POST_PROSPEKT = rule(TITLE.interpretation(Ulitsa.name), PROSPEKT_WORDS)
ULITSA_POST_SHOSSE = rule(TITLE.interpretation(Ulitsa.name), SHOSSE_WORDS)
ULITSA_POST_BULVAR = rule(TITLE.interpretation(Ulitsa.name), BULVAR_WORDS)
ULITSA_POST_NABEREG = rule(TITLE.interpretation(Ulitsa.name), NABEREG_WORDS)
ULITSA_POST_TUPIK = rule(TITLE.interpretation(Ulitsa.name), TUPIK_WORDS)
ULITSA_POST_PEREULOK = rule(TITLE.interpretation(Ulitsa.name), PEREULOK_WORDS)
ULITSA_POST_PROEZD = rule(TITLE.interpretation(Ulitsa.name), PROEZD_WORDS)
ULITSA_POST_ALLEYA = rule(TITLE.interpretation(Ulitsa.name), ALLEYA_WORDS)
ULITSA_POST_PLOSHAD = rule(TITLE.interpretation(Ulitsa.name), PLOSHAD_WORDS)
ULITSA_POST_TRAKT = rule(TITLE.interpretation(Ulitsa.name), TRAKT_WORDS)
ULITSA_POST_LINIYA = rule(TITLE.interpretation(Ulitsa.name), LINIYA_WORDS)
ULITSA_POST_DOROGA = rule(TITLE.interpretation(Ulitsa.name), DOROGA_WORDS)
ULITSA_POST_VAL = rule(TITLE.interpretation(Ulitsa.name), VAL_WORDS)
ULITSA_POST_KOLCO = rule(TITLE.interpretation(Ulitsa.name), KOLCO_WORDS)
ULITSA_POST_SPUSK = rule(TITLE.interpretation(Ulitsa.name), SPUSK_WORDS)
ULITSA_POST_MOST = rule(TITLE.interpretation(Ulitsa.name), MOST_WORDS)
ULITSA_POST_PARK = rule(TITLE.interpretation(Ulitsa.name), PARK_WORDS)
ULITSA_POST_SEZD = rule(TITLE.interpretation(Ulitsa.name), SEZD_WORDS)
ULITSA_POST_BEREG = rule(TITLE.interpretation(Ulitsa.name), BEREG_WORDS)
ULITSA_POST2_PROSPEKT = rule(rule(TITLE, TITLE).interpretation(Ulitsa.name), PROSPEKT_WORDS)
ULITSA_POST2_SHOSSE = rule(rule(TITLE, TITLE).interpretation(Ulitsa.name), SHOSSE_WORDS)
ULITSA_POST2_BULVAR = rule(rule(TITLE, TITLE).interpretation(Ulitsa.name), BULVAR_WORDS)
ULITSA_POST2_PLOSHAD = rule(rule(TITLE, TITLE).interpretation(Ulitsa.name), PLOSHAD_WORDS)

ULITSA = or_(
    rule(ULITSA_WORDS, ULITSA_NAME),
    rule(PROSPEKT_WORDS, ULITSA_NAME),
    rule(PEREULOK_WORDS, ULITSA_NAME),
    rule(PROEZD_WORDS, ULITSA_NAME),
    rule(SHOSSE_WORDS, ULITSA_NAME),
    rule(BULVAR_WORDS, ULITSA_NAME),
    rule(NABEREG_WORDS, ULITSA_NAME),
    rule(DOROGA_WORDS, ULITSA_NAME),
    rule(ALLEYA_WORDS, ULITSA_NAME),
    rule(PLOSHAD_WORDS, ULITSA_NAME),
    rule(LINIYA_WORDS, ULITSA_NAME),
    rule(KILOMETR_WORDS, ULITSA_NAME),
    rule(TUPIK_WORDS, ULITSA_NAME),
    rule(TRAKT_WORDS, ULITSA_NAME),
    rule(VAL_WORDS, ULITSA_NAME),
    rule(VEZD_WORDS, ULITSA_NAME),
    rule(KOLCO_WORDS, ULITSA_NAME),
    rule(SKVER_WORDS, ULITSA_NAME),
    rule(SPUSK_WORDS, ULITSA_NAME),
    rule(PROSEK_WORDS, ULITSA_NAME),
    rule(PROULOK_WORDS, ULITSA_NAME),
    rule(RYADY_WORDS, ULITSA_NAME),
    rule(PEREEZD_WORDS, ULITSA_NAME),
    rule(MOST_WORDS, ULITSA_NAME),
    rule(PARK_WORDS, ULITSA_NAME),
    rule(MAGISTRAL_WORDS, ULITSA_NAME),
    rule(SEZD_WORDS, ULITSA_NAME),
    rule(BEREG_WORDS, ULITSA_NAME),
    rule(PROSELOK_WORDS, ULITSA_NAME),
    rule(ZAEZD_WORDS, ULITSA_NAME),
    rule(PLOSCHADKA_WORDS, ULITSA_NAME),
    rule(BALKA_WORDS, ULITSA_NAME),
    rule(BUGOR_WORDS, ULITSA_NAME),
    rule(VZVOZ_WORDS, ULITSA_NAME),
    rule(KOSA_WORDS, ULITSA_NAME),
    rule(MAYAK_WORDS, ULITSA_NAME),
    rule(PLATFORMA_WORDS, ULITSA_NAME),
    rule(POLUSTANOK_WORDS, ULITSA_NAME),
    rule(PORT_WORDS, ULITSA_NAME),
    rule(ULITSA_POST_PROSPEKT),
    rule(ULITSA_POST_SHOSSE),
    rule(ULITSA_POST_BULVAR),
    rule(ULITSA_POST_NABEREG),
    rule(ULITSA_POST_TUPIK),
    rule(ULITSA_POST_PEREULOK),
    rule(ULITSA_POST_PROEZD),
    rule(ULITSA_POST_ALLEYA),
    rule(ULITSA_POST_PLOSHAD),
    rule(ULITSA_POST_TRAKT),
    rule(ULITSA_POST_LINIYA),
    rule(ULITSA_POST_DOROGA),
    rule(ULITSA_POST_VAL),
    rule(ULITSA_POST_KOLCO),
    rule(ULITSA_POST_SPUSK),
    rule(ULITSA_POST_MOST),
    rule(ULITSA_POST_PARK),
    rule(ULITSA_POST_SEZD),
    rule(ULITSA_POST_BEREG),
    rule(ULITSA_POST2_PROSPEKT),
    rule(ULITSA_POST2_SHOSSE),
    rule(ULITSA_POST2_BULVAR),
    rule(ULITSA_POST2_PLOSHAD),
).interpretation(Ulitsa)

LETTER = in_caseless(set("абвгдеёжзийклмнопрстуфхцчшщъыьэюяфabcdefghijklmnopqrstuvwxyz"))

DOM_WORDS = or_(rule(normalized("дом")), rule(caseless("д"), DOT.optional())).interpretation(Dom.type.const("д."))

VLADENIE_WORDS = or_(
    rule(caseless("вл"), DOT.optional()), rule(caseless("двлд"), DOT.optional()), rule(normalized("владение"))
).interpretation(Dom.type.const("влд."))

ZDANIE_WORDS = or_(rule(caseless("зд"), DOT.optional()), rule(normalized("здание"))).interpretation(
    Dom.type.const("зд.")
)

DOM_NUMBER = or_(
    rule(INT, DASH, LETTER),
    rule(INT, LETTER),
    rule(INT, "/", INT, LETTER),
    rule(INT, "/", INT),
    rule(INT),
    rule(LETTER),
    rule(INT, DASH, INT),
    rule(INT, caseless("литер"), LETTER),
    rule(INT, caseless("лит"), DOT.optional(), LETTER),
).interpretation(Dom.number)

DOM = or_(
    rule(DOM_WORDS, DOM_NUMBER),
    rule(DOM_WORDS, COMMA, DOM_NUMBER),
    rule(VLADENIE_WORDS, DOM_NUMBER),
    rule(VLADENIE_WORDS, COMMA, DOM_NUMBER),
    rule(ZDANIE_WORDS, DOM_NUMBER),
    rule(ZDANIE_WORDS, COMMA, DOM_NUMBER),
).interpretation(Dom)

STROENIE_WORDS = or_(
    rule(normalized("строение")), rule(caseless("стр"), DOT.optional()), rule(caseless("с"), DOT.optional())
).interpretation(Stroenie.type.const("стр."))

STROENIE_NUMBER = or_(
    rule(INT, DASH, LETTER),
    rule(INT, LETTER),
    rule(INT),
    rule(LETTER),
    rule(INT, DASH, INT),
    rule(TITLE, DASH, INT),
    rule(caseless("литер"), LETTER),
    rule(caseless("литер"), INT),
    rule(caseless("лит"), DOT.optional(), LETTER),
).interpretation(Stroenie.number)

STROENIE = or_(
    rule(STROENIE_WORDS, STROENIE_NUMBER),
    rule(STROENIE_WORDS, COMMA, STROENIE_NUMBER),
).interpretation(Stroenie)

KORPUS_WORDS = or_(
    rule(normalized("корпус")),
    rule(caseless("корп"), DOT.optional()),
    rule(caseless("к"), DOT.optional()),
    rule("/", caseless("к"), DOT.optional()),
    rule("/", caseless("корп"), DOT.optional()),
).interpretation(Korpus.type.const("к."))

KORPUS_NUMBER = or_(
    rule(INT, LETTER),
    rule(INT),
    rule(LETTER),
    rule(caseless("литер"), LETTER),
    rule(caseless("литер"), INT),
    rule(normalized("башня"), LETTER),
    rule(normalized("башня"), INT),
).interpretation(Korpus.number)

KORPUS = or_(
    rule(KORPUS_WORDS, KORPUS_NUMBER),
    rule(KORPUS_WORDS, COMMA, KORPUS_NUMBER),
).interpretation(Korpus)

KVARTIRA_WORDS = or_(rule(normalized("квартира")), rule(caseless("кв"), DOT.optional())).interpretation(
    Kvartira.type.const("кв.")
)

KVARTIRA_NUMBER = or_(
    rule(INT, DASH, LETTER),
    rule(INT, LETTER),
    rule(INT, "/", INT),
    rule(INT),
).interpretation(Kvartira.number)

KVARTIRA = or_(
    rule(KVARTIRA_WORDS, KVARTIRA_NUMBER),
    rule(KVARTIRA_WORDS, COMMA, KVARTIRA_NUMBER),
).interpretation(Kvartira)

KOMNATA_WORDS = or_(
    rule(normalized("комната")),
    rule(caseless("комн"), DOT.optional()),
    rule(caseless("ком"), DOT.optional()),
).interpretation(Komnata.type.const("комн."))

KOMNATA_NUMBER = or_(rule(INT), rule(INT, LETTER)).interpretation(Komnata.number)

KOMNATA = or_(
    rule(KOMNATA_WORDS, KOMNATA_NUMBER),
    rule(KOMNATA_WORDS, COMMA, KOMNATA_NUMBER),
).interpretation(Komnata)

OFIS_WORDS = or_(rule(normalized("офис")), rule(caseless("оф"), DOT.optional())).interpretation(Ofis.type.const("офис"))

OFIS_NUMBER = or_(rule(INT), rule(INT, LETTER)).interpretation(Ofis.number)

OFIS = or_(
    rule(OFIS_WORDS, OFIS_NUMBER),
    rule(OFIS_WORDS, COMMA, OFIS_NUMBER),
).interpretation(Ofis)

POMESHENIE_WORDS = or_(rule(normalized("помещение")), rule(caseless("пом"), DOT.optional())).interpretation(
    Pomeshenie.type.const("помещ.")
)

POMESHENIE_NUMBER = or_(rule(INT), rule(INT, LETTER), rule(INT, DASH, LETTER)).interpretation(Pomeshenie.number)

POMESHENIE = or_(
    rule(POMESHENIE_WORDS, POMESHENIE_NUMBER),
    rule(POMESHENIE_WORDS, COMMA, POMESHENIE_NUMBER),
).interpretation(Pomeshenie)

UCHASTOK_WORDS = or_(rule(caseless("уч"), DOT.optional()), rule(normalized("участок"))).interpretation(
    Uchastok.type.const("уч.")
)

UCHASTOK_NUMBER = or_(rule(INT), rule(INT, LETTER), rule(INT, "/", INT)).interpretation(Uchastok.number)

UCHASTOK = or_(
    rule(UCHASTOK_WORDS, UCHASTOK_NUMBER),
    rule(UCHASTOK_WORDS, COMMA, UCHASTOK_NUMBER),
).interpretation(Uchastok)

DO_VOSTREBOVANIYA_WORDS = or_(
    rule(caseless("до"), caseless("востребования")),
    rule(caseless("до"), caseless("востреб"), DOT.optional()),
    rule(caseless("до"), caseless("востр"), DOT.optional()),
    rule(caseless("довостребования")),
).interpretation(DoVostrebovaniya.marker.const("до востребования"))

DO_VOSTREBOVANIYA = DO_VOSTREBOVANIYA_WORDS.interpretation(DoVostrebovaniya)

ABONENT_BOX_WORDS = or_(
    rule(caseless("а"), "/", caseless("я")),
    rule(caseless("а/я")),
    rule(caseless("аб"), DOT.optional(), caseless("ящ"), DOT.optional()),
    rule(caseless("аб"), DOT.optional(), normalized("ящик")),
    rule(normalized("абонентский"), normalized("ящик")),
    rule(caseless("п"), "/", caseless("я")),
    rule(caseless("п/я")),
    rule(normalized("почтовый"), normalized("ящик")),
).interpretation(AbonentBox.type.const("а/я"))

ABONENT_BOX_NUMBER = or_(
    rule(INT),
    rule(INT, LETTER),
    rule(INT, DASH, INT),
).interpretation(AbonentBox.number)

ABONENT_BOX = or_(
    rule(ABONENT_BOX_WORDS, ABONENT_BOX_NUMBER),
    rule(ABONENT_BOX_WORDS, COMMA, ABONENT_BOX_NUMBER),
).interpretation(AbonentBox)

ADDR_PART = or_(
    STRANA,
    RESPUBLIKA,
    KRAI,
    OBLAST,
    AUTO_OKRUG,
    RAION,
    GOROD,
    MIKRORAION,
    TERRITORIYA,
    ULITSA,
    DOM,
    STROENIE,
    KORPUS,
    KVARTIRA,
    KOMNATA,
    OFIS,
    POMESHENIE,
    UCHASTOK,
    METRO,
    DO_VOSTREBOVANIYA,
    ABONENT_BOX,
)

ADDR = ADDR_PART
