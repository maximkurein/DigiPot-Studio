//! Display formatting only — no engineering math here (that lives in digipot-core).

const PREFIX: [(f64, &str); 8] =
    [(1e9, "Г"), (1e6, "М"), (1e3, "к"), (1.0, ""), (1e-3, "м"), (1e-6, "мк"), (1e-9, "н"), (1e-12, "п")];

fn unit_ru(unit: &str) -> Option<&'static str> {
    Some(match unit {
        "V" => "В",
        "A" => "А",
        "W" => "Вт",
        "Hz" => "Гц",
        "s" => "с",
        "F" => "Ф",
        "Ohm" | "Ω" => "Ом",
        _ => return None,
    })
}

/// `digits` significant digits, trailing zeros trimmed (like JS `toPrecision` + trim).
pub fn sig(v: f64, digits: usize) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let exp = v.abs().log10().floor() as i32;
    let dec = (digits as i32 - 1 - exp).max(0) as usize;
    let s = format!("{:.*}", dec, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// Engineering format with SI prefix: `eng(0.0025, "V", 4)` → "2.5 мВ".
/// Non-SI units are printed as-is.
pub fn eng(v: f64, unit: &str, digits: usize) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    let Some(u) = unit_ru(unit) else {
        let s = if v.abs() >= 1000.0 { format!("{:.0}", v) } else { sig(v, digits) };
        return if unit.is_empty() { s } else { format!("{} {}", s, unit) };
    };
    if v == 0.0 {
        return format!("0 {}", u);
    }
    let a = v.abs();
    let (k, p) = PREFIX.iter().copied().find(|(k, _)| a >= k * 0.9995).unwrap_or(PREFIX[PREFIX.len() - 1]);
    format!("{} {}{}", sig(v / k, digits), p, u)
}

pub fn eng_opt(v: Option<f64>, unit: &str, digits: usize) -> String {
    v.map(|v| eng(v, unit, digits)).unwrap_or_else(|| "—".into())
}

/// Parse "2.5m", "4.7 мк", "10k", "1e-3", "250 мВ", "10 кОм" → base-unit number.
pub fn parse_eng(input: &str) -> Option<f64> {
    let s: String = input.trim().replace(',', ".").chars().filter(|c| !c.is_whitespace()).collect();
    // longest numeric prefix that parses
    let num_end = s.char_indices().take_while(|(_, c)| c.is_ascii_digit() || "+-.eE".contains(*c)).count();
    let chars: Vec<char> = s.chars().collect();
    let (v, rest) = (1..=num_end).rev().find_map(|n| {
        let head: String = chars[..n].iter().collect();
        head.parse::<f64>().ok().map(|v| (v, chars[n..].iter().collect::<String>()))
    })?;
    let lower = rest.to_lowercase();
    let mut pre = rest.as_str();
    for u in ["°c", "ohm", "ом", "гц", "hz", "вт", "в", "v", "а", "a", "w", "ф", "f", "с", "s"] {
        if lower.ends_with(u) {
            let cut = rest.chars().count() - u.chars().count();
            let idx = rest.char_indices().nth(cut).map(|(i, _)| i).unwrap_or(rest.len());
            pre = &rest[..idx];
            break;
        }
    }
    let k = match pre {
        "" => 1.0,
        "p" | "п" => 1e-12,
        "n" | "н" => 1e-9,
        "u" | "µ" | "мк" => 1e-6,
        "m" | "м" => 1e-3,
        "k" | "к" | "K" | "К" => 1e3,
        "M" | "М" | "meg" => 1e6,
        "G" | "Г" => 1e9,
        _ => return None,
    };
    Some(v * k)
}

/// "0x1FF" with `bits / 4` hex digits.
pub fn hex_w(v: u32, bits: u32) -> String {
    format!("0x{:0w$X}", v, w = bits.div_ceil(4) as usize)
}

/// "1FFh" — three hex digits, datasheet style.
pub fn hex3(v: u32) -> String {
    format!("{:03X}h", v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(eng(0.0025, "V", 4), "2.5 мВ");
        assert_eq!(eng(10_000.0, "Ω", 3), "10 кОм");
        assert_eq!(eng(39.0625, "Ω", 3), "39.1 Ом");
        assert_eq!(eng(f64::NAN, "V", 3), "—");
        assert_eq!(eng(25.0, "°C", 4), "25 °C");
        assert_eq!(hex_w(0x80, 9), "0x080");
    }

    #[test]
    fn parses() {
        assert_eq!(parse_eng("2.5m"), Some(2.5e-3));
        assert_eq!(parse_eng("10 кОм"), Some(10e3));
        assert_eq!(parse_eng("250 мВ"), Some(0.25));
        assert_eq!(parse_eng("3,3"), Some(3.3));
        assert_eq!(parse_eng("1e-3"), Some(1e-3));
        assert_eq!(parse_eng("25 °C"), Some(25.0));
        assert_eq!(parse_eng("abc"), None);
    }
}
