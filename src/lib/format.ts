// Display formatting only — no engineering math here (that lives in Rust).

const PREFIX: [number, string][] = [
  [1e9, "Г"],
  [1e6, "М"],
  [1e3, "к"],
  [1, ""],
  [1e-3, "м"],
  [1e-6, "мк"],
  [1e-9, "н"],
  [1e-12, "п"],
];

const UNIT_RU: Record<string, string> = { V: "В", A: "А", W: "Вт", Hz: "Гц", s: "с", F: "Ф", Ohm: "Ом", "Ω": "Ом" };

/** Engineering format with SI prefix: fmtEng(0.0025, "V") → "2.5 мВ". Non-SI units are printed as-is. */
export function fmtEng(v: number | null | undefined, unit = "", digits = 4): string {
  if (v == null || !isFinite(v)) return "—";
  const u = UNIT_RU[unit];
  if (u == null) {
    const s = Math.abs(v) >= 1000 ? v.toFixed(0) : trim(v.toPrecision(digits));
    return unit ? `${s} ${unit}` : s;
  }
  if (v === 0) return `0 ${u}`;
  const a = Math.abs(v);
  const [k, p] = PREFIX.find(([k]) => a >= k * 0.9995) ?? PREFIX[PREFIX.length - 1];
  return `${trim((v / k).toPrecision(digits))} ${p}${u}`;
}

function trim(s: string): string {
  if (s.includes("e")) return s;
  return s.includes(".") ? s.replace(/0+$/, "").replace(/\.$/, "") : s;
}

/** Parse "2.5m", "4.7 мк", "10k", "1e-3", "250 мВ" → number (base unit). */
export function parseEng(input: string): number | null {
  const s = input.trim().replace(",", ".").replace(/\s+/g, "");
  const m = s.match(/^([-+]?\d*\.?\d+(?:e[-+]?\d+)?)([a-zA-Zа-яА-Яµ]*)$/);
  if (!m) return null;
  const v = parseFloat(m[1]);
  const u = m[2].replace(/(в|v|гц|hz|а|a|вт|w|ом|ohm|ф|f|с|s)$/i, "");
  const map: Record<string, number> = {
    "": 1, p: 1e-12, "п": 1e-12, n: 1e-9, "н": 1e-9, u: 1e-6, "µ": 1e-6, "мк": 1e-6,
    m: 1e-3, "м": 1e-3, k: 1e3, "к": 1e3, M: 1e6, "М": 1e6, meg: 1e6, G: 1e9, "Г": 1e9,
  };
  if (!(u in map)) return null;
  return v * map[u];
}

export function hexW(v: number | null | undefined, bits = 32): string {
  if (v == null) return "—";
  return "0x" + (v >>> 0).toString(16).toUpperCase().padStart(Math.ceil(bits / 4), "0");
}

export function fmtTime(s: number | null | undefined): string {
  if (s == null || !isFinite(s)) return "—";
  return fmtEng(s, "s", 4);
}

export function clamp(v: number, a: number, b: number) {
  return Math.min(b, Math.max(a, v));
}

/** Sign-extend a two's complement field value for display. */
export function signed(v: number, bits: number): number {
  return v >= 2 ** (bits - 1) ? v - 2 ** bits : v;
}
