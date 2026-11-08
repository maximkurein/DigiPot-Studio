// DIGI POT click (MikroE MIKROE-923, HW REV 1.00) facts for the Board and
// Connection views. mikroBUS pin names follow the mikroBUS standard; which pins
// the click uses is taken from the board silkscreen (MISO is marked NC).
// Items marked «проверьте» were not confirmed from the photos — check your board.

export interface Hotspot {
  id: string;
  /** Designator as printed on the PCB (J1, JP3, TP5 …). */
  ref: string;
  title: string;
  kind: "connector" | "jumper" | "power" | "testpoint" | "chip" | "led" | "switch";
  desc: string;
  /** Position on the 1000×600 board drawing. */
  x: number;
  y: number;
  w: number;
  h: number;
  /** Optional block id to jump to on the overview. */
  block?: string;
}

export const BOARD = {
  name: "DIGI POT click (MIKROE-923)",
  size: "формат click, разъём mikroBUS",
  supply: "3.3 В и 5 В с mikroBUS",
};

/** mikroBUS rows as drawn on the board view (top to bottom). */
export const MIKROBUS_LEFT = ["AN", "RST", "CS", "SCK", "MISO", "MOSI", "3.3V", "GND"];
export const MIKROBUS_RIGHT = ["PWM", "INT", "RX", "TX", "SCL", "SDA", "5V", "GND"];
/** Pins the click actually uses (others are NC). */
export const USED_PINS = new Set(["CS", "SCK", "MOSI", "3.3V", "5V", "GND"]);

export const HOTSPOTS: Hotspot[] = [
  {
    id: "u1", ref: "U1", title: "MCP4161 (цифровой потенциометр)", kind: "chip",
    desc: "Microchip MCP4161: 8 бит (257 отводов), EEPROM, SPI. Суффикс в маркировке задаёт R_AB: 502 = 5 кОм, 103 = 10 кОм, 503 = 50 кОм, 104 = 100 кОм — сверьте с корпусом и выберите вариант в «Потенциометр».",
    x: 420, y: 230, w: 160, h: 140, block: "wiper",
  },
  {
    id: "mb-l", ref: "mikroBUS ←", title: "Левый ряд mikroBUS", kind: "connector",
    desc: "Используются CS, SCK, MOSI (на кристалле — совмещённый вывод SDI/SDO), 3.3V и GND. MISO на плате не разведён (NC): чтение идёт по линии MOSI по 3-проводной схеме. AN и RST не используются.",
    x: 60, y: 90, w: 110, h: 420, block: "spi",
  },
  {
    id: "mb-r", ref: "mikroBUS →", title: "Правый ряд mikroBUS", kind: "connector",
    desc: "Используются 5V и GND. PWM, INT, RX, TX, SCL, SDA не подключены.",
    x: 830, y: 90, w: 110, h: 420, block: "por",
  },
  {
    id: "term", ref: "P0A · P0W · P0B", title: "Выводы потенциометра", kind: "connector",
    desc: "Четыре контакта у края платы — выводы A, W, B и общий провод (проверьте порядок по шелкографии своей платы). Напряжения — только 0…VDD, ток — до 2.5 мА.",
    x: 330, y: 40, w: 340, h: 90, block: "terminals",
  },
  {
    id: "pwr", ref: "PWR", title: "Питание кристалла", kind: "power",
    desc: "Плата получает 3.3 В и 5 В с mikroBUS. От какого из них питается MCP4161 (перемычка или фиксированно) — проверьте на своей ревизии: VDD задаёт допустимый диапазон напряжений на A, W, B и уровни логики SPI.",
    x: 420, y: 440, w: 160, h: 80, block: "por",
  },
];

/** Step-by-step first power-up checklist (shown on the Board view). */
export const CHECKLIST: string[] = [
  "Прочитайте маркировку U1 и выберите вариант R_AB (5/10/50/100 кОм) в «Потенциометр → Схема включения».",
  "Выясните, каким напряжением питается кристалл (3.3 или 5 В), и задайте VDD — от него зависят допустимые напряжения на выводах.",
  "Соедините мост и click: GND, 3.3V, SCK, MOSI, CS (MISO не нужен). Если click стоит в плате MikroE — её микроконтроллер не должен работать с шиной одновременно.",
  "«Подключение» → выберите порт → «Подключить». Нажмите «Прочитать всё» в «Регистрах»: STATUS = 1E0h, TCON = 1FFh, движок = NV_WIPER0 (с завода 080h).",
  "Подайте на A и B напряжения в пределах 0…VDD и проверьте V_W мультиметром на кодах 0, 128, 256.",
];

/** Bridge wiring: MCU pin → click pin (Connection view and docs). */
export const WIRING: { mcu: string; board: string; signal: string; dir: "→" | "←" | "↔" | "—"; note?: string }[] = [
  { mcu: "GND", board: "GND", signal: "GND", dir: "—", note: "общий провод — первым" },
  { mcu: "3V3", board: "3.3V", signal: "VDD", dir: "—", note: "или 5V, если кристалл питается от 5 В (проверьте)" },
  { mcu: "SCK", board: "SCK", signal: "SCK", dir: "→" },
  { mcu: "MOSI", board: "MOSI", signal: "SDI/SDO", dir: "↔", note: "запись — от моста; при чтении мост отпускает линию" },
  { mcu: "D10", board: "CS", signal: "CS", dir: "→", note: "активный 0" },
  { mcu: "MISO", board: "—", signal: "NC", dir: "—", note: "на click не разведён, не подключать" },
];

/** Hardware safety notes. Keep them in the UI and in docs (CLAUDE.md rule). */
export const SAFETY: string[] = [
  "Уровни логики SPI равны VDD кристалла: при питании 3.3 В не подавайте 5-вольтовые сигналы (Arduino Uno/Mega) без преобразователя уровней.",
  "Если click установлен в отладочную плату MikroE (Clicker, Fusion, EasyPIC…), её микроконтроллер тоже мастер SPI: остановите его программу или выньте click, прежде чем подключать внешний мост.",
  "Напряжения на A, W, B — только в пределах 0…VDD (абсолютный максимум −0.3…VDD + 0.3 В), ток через любой вывод — не больше 2.5 мА.",
  "Соединяйте землю моста и платы до сигнальных проводов.",
];
