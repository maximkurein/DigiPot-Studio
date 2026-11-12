//! Static UI content: functional blocks, board facts and lessons. Same texts as
//! the web UI (`src/lib/blocks.ts`, `board.ts`, `lessons.ts`) — keep in sync.

use crate::app::View;

// ───────────── functional diagram (DS22059B, Figure 2-1 / §4–§5) ─────────────

/// Block of the overview diagram. Ids match `FieldDef.block`, `Metric.block`, `Issue.block`.
pub struct BlockDef {
    pub id: &'static str,
    pub title: &'static str,
    /// Colour token: sig | clk | ref | dig | pwr | ctl
    pub tone: &'static str,
    pub desc: &'static str,
    /// Position on the 1200×640 overview canvas.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const BLOCKS: &[BlockDef] = &[
    BlockDef {
        id: "spi",
        title: "SPI",
        tone: "dig",
        desc: "Последовательный интерфейс: 16-битные команды записи/чтения и 8-битные инкремент/декремент. У MCP41X1 один вывод SDI/SDO: чтение идёт по той же линии (3-проводная схема).",
        x: 120.0,
        y: 250.0,
        w: 150.0,
        h: 110.0,
    },
    BlockDef {
        id: "por",
        title: "POR / BOR",
        tone: "pwr",
        desc: "Сброс по питанию: при включении движок загружается из EEPROM (NV_WIPER0), TCON = 1FFh. Ниже 1.8 В кристалл держится в сбросе.",
        x: 120.0,
        y: 450.0,
        w: 150.0,
        h: 90.0,
    },
    BlockDef {
        id: "eeprom",
        title: "EEPROM",
        tone: "ref",
        desc: "Энергонезависимая память: положение движка при включении и 10 ячеек данных 06h…0Fh. Запись длится до 10 мс (EEWA = 1), ресурс ~1 млн циклов.",
        x: 340.0,
        y: 450.0,
        w: 170.0,
        h: 90.0,
    },
    BlockDef {
        id: "wiper",
        title: "Регистр движка",
        tone: "dig",
        desc: "WIPER0 — оперативное положение движка, 0…256 (257 отводов). Коды выше 100h тоже дают полную шкалу.",
        x: 340.0,
        y: 250.0,
        w: 170.0,
        h: 110.0,
    },
    BlockDef {
        id: "ladder",
        title: "Резистивная цепочка",
        tone: "sig",
        desc: "256 одинаковых резисторов R_S = R_AB / 256 между A и B и ключ отвода. R_AB: 5, 10, 50 или 100 кОм ±20 %, плюс сопротивление движка R_W ≈ 75 Ом.",
        x: 580.0,
        y: 250.0,
        w: 190.0,
        h: 110.0,
    },
    BlockDef {
        id: "tcon",
        title: "TCON",
        tone: "ctl",
        desc: "Ключи выводов: каждый из A, W, B можно отключить, а R0HW = 0 переводит потенциометр в режим отключения (A разомкнут, W соединён с B).",
        x: 580.0,
        y: 450.0,
        w: 190.0,
        h: 90.0,
    },
    BlockDef {
        id: "terminals",
        title: "Выводы A · W · B",
        tone: "sig",
        desc: "Аналоговые выводы потенциометра. Напряжение на них — только в пределах 0…VDD, ток через любой вывод — не больше 2.5 мА.",
        x: 860.0,
        y: 250.0,
        w: 190.0,
        h: 290.0,
    },
];

/// Wires between blocks: (from, to, tone).
pub const WIRES: &[(&str, &str, &str)] = &[
    ("spi", "wiper", "dig"),
    ("spi", "eeprom", "dig"),
    ("eeprom", "wiper", "ref"),
    ("por", "eeprom", "pwr"),
    ("wiper", "ladder", "dig"),
    ("ladder", "terminals", "sig"),
    ("tcon", "terminals", "ctl"),
];

pub fn block(id: &str) -> Option<&'static BlockDef> {
    BLOCKS.iter().find(|b| b.id == id)
}

// ───────────── DIGI POT click (MIKROE-923, HW REV 1.00) ─────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HotKind {
    Chip,
    Connector,
    Jumper,
    Power,
    TestPoint,
    Led,
    Switch,
}

impl HotKind {
    pub const ALL: [HotKind; 7] = [
        HotKind::Chip,
        HotKind::Connector,
        HotKind::Jumper,
        HotKind::Power,
        HotKind::TestPoint,
        HotKind::Led,
        HotKind::Switch,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HotKind::Chip => "кристалл",
            HotKind::Connector => "разъём",
            HotKind::Jumper => "джампер",
            HotKind::Power => "питание",
            HotKind::TestPoint => "контр. точка",
            HotKind::Led => "индикатор",
            HotKind::Switch => "переключатель",
        }
    }

    /// Colour token.
    pub fn tone(self) -> &'static str {
        match self {
            HotKind::Chip => "accent",
            HotKind::Connector => "clk",
            HotKind::Jumper => "warn",
            HotKind::Power => "pwr",
            HotKind::TestPoint => "ref",
            HotKind::Led => "ok",
            HotKind::Switch => "ctl",
        }
    }
}

pub struct Hotspot {
    pub id: &'static str,
    /// Designator as printed on the PCB.
    pub refdes: &'static str,
    pub title: &'static str,
    pub kind: HotKind,
    pub desc: &'static str,
    /// Position on the 1000×600 board drawing.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Block id to jump to on the overview.
    pub block: Option<&'static str>,
}

pub const BOARD_NAME: &str = "DIGI POT click (MIKROE-923)";
pub const BOARD_SIZE: &str = "формат click, разъём mikroBUS";
pub const BOARD_SUPPLY: &str = "3.3 В и 5 В с mikroBUS";

/// mikroBUS rows as drawn on the board view (top to bottom).
pub const MIKROBUS_LEFT: [&str; 8] = ["AN", "RST", "CS", "SCK", "MISO", "MOSI", "3.3V", "GND"];
pub const MIKROBUS_RIGHT: [&str; 8] = ["PWM", "INT", "RX", "TX", "SCL", "SDA", "5V", "GND"];
/// Pins the click actually uses (others are NC).
pub const USED_PINS: [&str; 6] = ["CS", "SCK", "MOSI", "3.3V", "5V", "GND"];

pub const HOTSPOTS: &[Hotspot] = &[
    Hotspot {
        id: "u1",
        refdes: "U1",
        title: "MCP4161 (цифровой потенциометр)",
        kind: HotKind::Chip,
        desc: "Microchip MCP4161: 8 бит (257 отводов), EEPROM, SPI. Суффикс в маркировке задаёт R_AB: 502 = 5 кОм, 103 = 10 кОм, 503 = 50 кОм, 104 = 100 кОм — сверьте с корпусом и выберите вариант в «Потенциометр».",
        x: 420.0,
        y: 230.0,
        w: 160.0,
        h: 140.0,
        block: Some("wiper"),
    },
    Hotspot {
        id: "mb-l",
        refdes: "mikroBUS ←",
        title: "Левый ряд mikroBUS",
        kind: HotKind::Connector,
        desc: "Используются CS, SCK, MOSI (на кристалле — совмещённый вывод SDI/SDO), 3.3V и GND. MISO на плате не разведён (NC): чтение идёт по линии MOSI по 3-проводной схеме. AN и RST не используются.",
        x: 60.0,
        y: 90.0,
        w: 110.0,
        h: 420.0,
        block: Some("spi"),
    },
    Hotspot {
        id: "mb-r",
        refdes: "mikroBUS →",
        title: "Правый ряд mikroBUS",
        kind: HotKind::Connector,
        desc: "Используются 5V и GND. PWM, INT, RX, TX, SCL, SDA не подключены.",
        x: 830.0,
        y: 90.0,
        w: 110.0,
        h: 420.0,
        block: Some("por"),
    },
    Hotspot {
        id: "term",
        refdes: "P0A · P0W · P0B",
        title: "Выводы потенциометра",
        kind: HotKind::Connector,
        desc: "Четыре контакта у края платы — выводы A, W, B и общий провод (проверьте порядок по шелкографии своей платы). Напряжения — только 0…VDD, ток — до 2.5 мА.",
        x: 330.0,
        y: 40.0,
        w: 340.0,
        h: 90.0,
        block: Some("terminals"),
    },
    Hotspot {
        id: "pwr",
        refdes: "PWR",
        title: "Питание кристалла",
        kind: HotKind::Power,
        desc: "Плата получает 3.3 В и 5 В с mikroBUS. От какого из них питается MCP4161 (перемычка или фиксированно) — проверьте на своей ревизии: VDD задаёт допустимый диапазон напряжений на A, W, B и уровни логики SPI.",
        x: 420.0,
        y: 440.0,
        w: 160.0,
        h: 80.0,
        block: Some("por"),
    },
];

/// First power-up checklist (Board view).
pub const CHECKLIST: &[&str] = &[
    "Прочитайте маркировку U1 и выберите вариант R_AB (5/10/50/100 кОм) в «Потенциометр → Схема включения».",
    "Выясните, каким напряжением питается кристалл (3.3 или 5 В), и задайте VDD — от него зависят допустимые напряжения на выводах.",
    "Соедините мост и click: GND, 3.3V, SCK, MOSI, CS (MISO не нужен). Если click стоит в плате MikroE — её микроконтроллер не должен работать с шиной одновременно.",
    "«Подключение» → выберите порт → «Подключить». Нажмите «Прочитать всё» в «Регистрах»: STATUS = 1E0h, TCON = 1FFh, движок = NV_WIPER0 (с завода 080h).",
    "Подайте на A и B напряжения в пределах 0…VDD и проверьте V_W мультиметром на кодах 0, 128, 256.",
];

/// Bridge wiring: (MCU pin, direction, board pin, signal, note).
pub const WIRING: &[(&str, &str, &str, &str, &str)] = &[
    ("GND", "—", "GND", "GND", "общий провод — первым"),
    ("3V3", "—", "3.3V", "VDD", "или 5V, если кристалл питается от 5 В (проверьте)"),
    ("SCK", "→", "SCK", "SCK", ""),
    ("MOSI", "↔", "MOSI", "SDI/SDO", "запись — от моста; при чтении мост отпускает линию"),
    ("D10", "→", "CS", "CS", "активный 0"),
    ("MISO", "—", "—", "NC", "на click не разведён, не подключать"),
];

/// Hardware safety notes. Keep them in the UI and in docs (CLAUDE.md rule).
pub const SAFETY: &[&str] = &[
    "Уровни логики SPI равны VDD кристалла: при питании 3.3 В не подавайте 5-вольтовые сигналы (Arduino Uno/Mega) без преобразователя уровней.",
    "Если click установлен в отладочную плату MikroE (Clicker, Fusion, EasyPIC…), её микроконтроллер тоже мастер SPI: остановите его программу или выньте click, прежде чем подключать внешний мост.",
    "Напряжения на A, W, B — только в пределах 0…VDD (абсолютный максимум −0.3…VDD + 0.3 В), ток через любой вывод — не больше 2.5 мА.",
    "Соединяйте землю моста и платы до сигнальных проводов.",
];

// ───────────── Academy ─────────────

pub struct Lesson {
    pub title: &'static str,
    /// Paragraphs; "§ text" — sub-heading, "$ text" — formula, "- text" — bullet.
    pub body: &'static [&'static str],
    /// (button label, view, block)
    pub try_it: Option<(&'static str, View, Option<&'static str>)>,
}

pub const LESSONS: &[Lesson] = &[
    Lesson {
        title: "Что такое цифровой потенциометр",
        body: &[
            "Обычный потенциометр — резистивная дорожка и скользящий контакт. В MCP4161 дорожка заменена цепочкой из 256 одинаковых резисторов между выводами A и B, а скользящий контакт — набором ключей: замкнут ровно один из 257 отводов.",
            "§ Сопротивление между W и B",
            "$ R_S = R_AB / 256        R_WB = R_AB · N / 256 + R_W",
            "N — код движка 0…256, R_W ≈ 75 Ом — сопротивление открытого ключа. Поэтому даже при N = 0 между W и B остаётся десятки ом, а не ноль.",
            "- Код хранится в регистре WIPER0 (адрес 00h).",
            "- Коды выше 100h (256) тоже дают полную шкалу: W соединён с A.",
        ],
        try_it: Some(("Покрутить движок", View::Pot, None)),
    },
    Lesson {
        title: "Реостат и делитель напряжения",
        body: &[
            "Один и тот же кристалл работает двумя способами.",
            "§ Реостат (переменный резистор)",
            "Используются два вывода, W и B. Сопротивление растёт с кодом, но зависит от R_AB конкретного экземпляра (±20 %) и от R_W.",
            "§ Делитель (потенциометр)",
            "На A и B подаются напряжения, с W снимается результат:",
            "$ V_W = V_B + (V_A − V_B) · N / 256",
            "Здесь важно только отношение сопротивлений, поэтому разброс R_AB и температура почти не влияют (15 ppm/°C против 50 ppm/°C у реостата). Но выход W имеет внутреннее сопротивление — нагрузка проседает, если она не намного больше R_AB / 4.",
        ],
        try_it: Some(("Задать нагрузку и посмотреть просадку", View::Pot, None)),
    },
    Lesson {
        title: "Команды SPI",
        body: &[
            "Каждая команда начинается с 4 бит адреса и 2 бит кода операции:",
            "$ [AD3:AD0] [C1 C0] [D9:D0]      C = 00 запись · 01 инкремент · 10 декремент · 11 чтение",
            "- Запись и чтение — 16 бит: 00 80 пишет в движок код 080h; 20 80 — то же в EEPROM (адрес 02h).",
            "- Инкремент и декремент — 8 бит: 04 сдвигает движок на шаг вверх, 08 — вниз. Кристалл сам не даст выйти за 0 и 256.",
            "- Пока CS = 0, команды можно слать подряд: 04 04 04 — три шага за один кадр.",
            "- Бит D9 кристалл не использует: в этой позиции он выдаёт CMDERR (1 — команда принята).",
            "Откройте журнал шины (⌘J) и нажмите «+1» на экране потенциометра — увидите однобайтовую команду.",
        ],
        try_it: Some(("Шаги ± в журнале", View::Pot, None)),
    },
    Lesson {
        title: "Чтение по одной линии (MISO не разведён)",
        body: &[
            "У MCP41X1 один вывод данных — SDI/SDO. На DIGI POT click он подключён к линии MOSI, а MISO помечен NC. Значит, обычное полнодуплексное чтение не работает: мост и кристалл будут одновременно управлять одной линией.",
            "§ 3-проводная схема (Figure 6-1)",
            "- Мост выдаёт 6 бит: адрес и команду чтения (11).",
            "- После шестого фронта мост отпускает линию (вход с подтяжкой), кристалл выдаёт CMDERR и 9 бит данных.",
            "- Без внешней подтяжки частота чтения — не выше 250 кГц: у вывода открытый сток со слабой «умной» подтяжкой.",
            "Прошивка моста делает это командой T <hex> <биты>; приложение — кадром Spi3. Ответ 1111 111n nnnn nnnn: старшие единицы — это отпущенная линия.",
        ],
        try_it: Some(("Подключение и прошивка моста", View::Hardware, None)),
    },
    Lesson {
        title: "EEPROM и включение питания",
        body: &[
            "При включении питания кристалл копирует NV_WIPER0 (адрес 02h) в WIPER0 и ставит TCON = 1FFh. Поэтому положение «запоминается» только если записать его в EEPROM.",
            "- Запись в EEPROM длится tWC = 5 мс (до 10 мс); всё это время STATUS.EEWA = 1 и принимаются только команды к адресам 00h, 01h, 04h, 05h.",
            "- Ресурс — около миллиона циклов. Запись раз в секунду исчерпает его примерно за 12 суток.",
            "- Библиотека MikroE для этого click при каждой установке движка пишет и WIPER0, и NV_WIPER0 (кадр 00 nn 20 nn) — удобно, но расходует ресурс. Сохраняйте в EEPROM только итоговое значение.",
        ],
        try_it: Some(("Открыть память", View::Memory, None)),
    },
    Lesson {
        title: "TCON: ключи выводов и отключение",
        body: &[
            "Регистр TCON (04h) содержит по ключу на каждый вывод: R0A, R0W, R0B. Разомкнутый ключ отсоединяет вывод от цепочки.",
            "R0HW = 0 — режим отключения: вывод A разомкнут, движок соединён с B, ток через цепочку не течёт. Код в WIPER0 при этом сохраняется и вернётся при R0HW = 1.",
            "У MCP4161 нет вывода SHDN, так что отключение — только через TCON.",
        ],
        try_it: Some(("Ключи TCON на блок-схеме", View::Overview, Some("tcon"))),
    },
    Lesson {
        title: "Допуск, температура и калибровка",
        body: &[
            "R_AB каждого экземпляра может отличаться от номинала на ±20 %, а R_W зависит от напряжения питания (75 Ом типично, до 300 Ом при 2.7 В).",
            "$ R_AB(T) ≈ R_AB(25 °C) · (1 + 50·10⁻⁶ · (T − 25))",
            "- В режиме делителя это почти не важно: результат — отношение сопротивлений.",
            "- В режиме реостата для точности нужна калибровка: измерьте R_WB на двух кодах и пересчитайте R_AB и R_W.",
            "В симуляторе «этот экземпляр» отличается от номинала (настройка «Отклонение R_AB»): сравните мультиметр с моделью.",
        ],
        try_it: Some(("Сравнить с мультиметром", View::Pot, None)),
    },
    Lesson {
        title: "Пределы: ток, напряжения, полоса",
        body: &[
            "- Напряжение на A, W, B — только от 0 до VDD. Выход за пределы открывает защитные диоды.",
            "- Ток через любой вывод — не больше 2.5 мА. Для варианта 5 кОм при 5 В ток A→B уже 1 мА, а нагрузка на W в крайнем положении добавляет ещё.",
            "- Полоса по −3 дБ: 2 МГц (5 кОм) … 100 кГц (100 кОм) — для звука хватает, для ВЧ — нет.",
            "Приложение проверяет эти пределы на каждом изменении и показывает предупреждения на блок-схеме.",
        ],
        try_it: Some(("Проверить свою схему", View::Pot, None)),
    },
];
