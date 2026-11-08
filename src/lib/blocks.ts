// Blocks of the MCP4161 functional diagram (DS22059B, Figure 2-1 / §4–§5).
// Ids must match `FieldDef.block` (regs.rs) and `Metric.block` / `Issue.block` (model.rs).

export interface BlockDef {
  id: string;
  title: string;
  /** CSS colour token: sig | clk | ref | dig | pwr | ctl */
  tone: "sig" | "clk" | "ref" | "dig" | "pwr" | "ctl";
  desc: string;
  /** Position on the 1200×640 overview SVG. */
  x: number;
  y: number;
  w: number;
  h: number;
}

export const BLOCKS: BlockDef[] = [
  {
    id: "spi",
    title: "SPI",
    tone: "dig",
    desc: "Последовательный интерфейс: 16-битные команды записи/чтения и 8-битные инкремент/декремент. У MCP41X1 один вывод SDI/SDO: чтение идёт по той же линии (3-проводная схема).",
    x: 120, y: 250, w: 150, h: 110,
  },
  {
    id: "por",
    title: "POR / BOR",
    tone: "pwr",
    desc: "Сброс по питанию: при включении движок загружается из EEPROM (NV_WIPER0), TCON = 1FFh. Ниже 1.8 В кристалл держится в сбросе.",
    x: 120, y: 450, w: 150, h: 90,
  },
  {
    id: "eeprom",
    title: "EEPROM",
    tone: "ref",
    desc: "Энергонезависимая память: положение движка при включении и 10 ячеек данных 06h…0Fh. Запись длится до 10 мс (EEWA = 1), ресурс ~1 млн циклов.",
    x: 340, y: 450, w: 170, h: 90,
  },
  {
    id: "wiper",
    title: "Регистр движка",
    tone: "dig",
    desc: "WIPER0 — оперативное положение движка, 0…256 (257 отводов). Коды выше 100h тоже дают полную шкалу.",
    x: 340, y: 250, w: 170, h: 110,
  },
  {
    id: "ladder",
    title: "Резистивная цепочка",
    tone: "sig",
    desc: "256 одинаковых резисторов R_S = R_AB / 256 между A и B и ключ отвода. R_AB: 5, 10, 50 или 100 кОм ±20 %, плюс сопротивление движка R_W ≈ 75 Ом.",
    x: 580, y: 250, w: 190, h: 110,
  },
  {
    id: "tcon",
    title: "TCON",
    tone: "ctl",
    desc: "Ключи выводов: каждый из A, W, B можно отключить, а R0HW = 0 переводит потенциометр в режим отключения (A разомкнут, W соединён с B).",
    x: 580, y: 450, w: 190, h: 90,
  },
  {
    id: "terminals",
    title: "Выводы A · W · B",
    tone: "sig",
    desc: "Аналоговые выводы потенциометра. Напряжение на них — только в пределах 0…VDD, ток через любой вывод — не больше 2.5 мА.",
    x: 860, y: 250, w: 190, h: 290,
  },
];

/** Wires between blocks: [from, to, tone]. */
export const WIRES: [string, string, BlockDef["tone"]][] = [
  ["spi", "wiper", "dig"],
  ["spi", "eeprom", "dig"],
  ["eeprom", "wiper", "ref"],
  ["por", "eeprom", "pwr"],
  ["wiper", "ladder", "dig"],
  ["ladder", "terminals", "sig"],
  ["tcon", "terminals", "ctl"],
];

export function blockById(id: string | null | undefined) {
  return BLOCKS.find((b) => b.id === id) ?? null;
}
