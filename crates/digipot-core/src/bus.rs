//! SPI command format of the MCP4161 (DS22059B §7, Figure 7-1, Table 7-2).
//!
//! ```text
//! 16-bit Write / Read:       [AD3 AD2 AD1 AD0][C1 C0][D9 D8 | D7 … D0]
//! 8-bit Increment/Decrement: [AD3 AD2 AD1 AD0][C1 C0][x x]
//! C1:C0 = 00 write data, 01 increment, 10 decrement, 11 read data
//! ```
//! D9 is ignored by the device (memory is 9 bits wide, §7.2) and sits where the
//! chip drives its CMDERR bit, so the host always sends D9 = 0. Several commands
//! may follow each other while CS stays low ("continuous commands", §7.4).
//! SPI modes 0,0 and 1,1, up to 10 MHz (§6.1.4, §6.2).
//!
//! On the DIGI POT click the chip's single SDI/SDO pin goes to the mikroBUS MOSI
//! line only (MISO is marked NC on the board). Writes work with any SPI master.
//! Reads use the 3-wire scheme of Figure 6-1: the host drives the 6 address and
//! command bits, releases the line and clocks in the remaining 10 bits that the
//! chip drives (open drain with a weak "smart" pull-up: ≤ 250 kHz, Table 6-1).
//! That is `Frame::Spi3`; the bridge firmware implements it with bit-banging.

pub const SPI_MODE: u8 = 0;
pub const SPI_MAX_HZ: u32 = 10_000_000;
/// Read clock on the multiplexed SDI/SDO pin without an external pull-up (Table 6-1).
pub const SPI3_MAX_HZ: u32 = 250_000;
/// Bits the host drives in a 3-wire read: AD3:AD0 + C1:C0.
pub const CMD_BITS: u32 = 6;

/// One device command (Table 7-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Write { addr: u8, data: u16 },
    Read { addr: u8 },
    Inc { addr: u8 },
    Dec { addr: u8 },
}

impl Cmd {
    pub fn encode(&self) -> Vec<u8> {
        match *self {
            Cmd::Write { addr, data } => (((addr as u16 & 0xF) << 12) | (data & 0x1FF)).to_be_bytes().to_vec(),
            // data bits are driven by the chip; the host leaves the line high
            Cmd::Read { addr } => (((addr as u16 & 0xF) << 12) | (0b11 << 10) | 0x3FF).to_be_bytes().to_vec(),
            Cmd::Inc { addr } => vec![((addr & 0xF) << 4) | 0b0100],
            Cmd::Dec { addr } => vec![((addr & 0xF) << 4) | 0b1000],
        }
    }

    pub fn describe(&self) -> String {
        match *self {
            Cmd::Write { addr, data } => format!("запись {:02X}h ← {:03X}h", addr, data),
            Cmd::Read { addr } => format!("чтение {:02X}h", addr),
            Cmd::Inc { addr } => format!("инкремент {:02X}h", addr),
            Cmd::Dec { addr } => format!("декремент {:02X}h", addr),
        }
    }
}

/// Split the bytes of one CS-low frame into commands. `false` in the second
/// element means the frame ended in the middle of a 16-bit command (the device
/// ignores such a partial command, §7.3.1).
pub fn parse(tx: &[u8]) -> (Vec<Cmd>, bool) {
    let mut v = vec![];
    let mut i = 0;
    while i < tx.len() {
        let b = tx[i];
        let addr = b >> 4;
        match (b >> 2) & 0b11 {
            0b01 => {
                v.push(Cmd::Inc { addr });
                i += 1;
            }
            0b10 => {
                v.push(Cmd::Dec { addr });
                i += 1;
            }
            c => {
                if i + 1 >= tx.len() {
                    return (v, false);
                }
                let data = (((b & 0b11) as u16) << 8) | tx[i + 1] as u16;
                v.push(if c == 0b00 { Cmd::Write { addr, data: data & 0x1FF } } else { Cmd::Read { addr } });
                i += 2;
            }
        }
    }
    (v, true)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// Full-duplex SPI, CS low for the whole frame.
    Spi(Vec<u8>),
    /// 3-wire SPI: drive the first `drive_bits` bits of `tx`, then release the
    /// data line and sample it for the rest of the frame.
    Spi3 { tx: Vec<u8>, drive_bits: u32 },
    I2cWrite { addr: u8, data: Vec<u8> },
    I2cRead { addr: u8, wr: Vec<u8>, n: usize },
}

pub fn describe() -> String {
    format!("SPI режим {}, 16 бит: адрес 4 + команда 2 + данные 10; чтение по 3-проводной схеме (SDI/SDO)", SPI_MODE)
}

pub fn can_read() -> bool {
    true
}

fn addr_of(reg: usize) -> u8 {
    crate::regs::REGS[reg].addr as u8
}

pub fn write_frame(reg: usize, value: u32) -> Frame {
    Frame::Spi(Cmd::Write { addr: addr_of(reg), data: value as u16 }.encode())
}

pub fn read_frame(reg: usize) -> Option<Frame> {
    Some(Frame::Spi3 { tx: Cmd::Read { addr: addr_of(reg) }.encode(), drive_bits: CMD_BITS })
}

/// `n` increment (`up`) or decrement commands in one CS window.
pub fn step_frame(reg: usize, up: bool, n: usize) -> Frame {
    let c = if up { Cmd::Inc { addr: addr_of(reg) } } else { Cmd::Dec { addr: addr_of(reg) } };
    Frame::Spi((0..n).flat_map(|_| c.encode()).collect())
}

/// Value from the 16 bits sampled during a read: `None` when the CMDERR bit
/// (bit 9) is low, i.e. the chip rejected the address/command (§7.3).
pub fn parse_read(rx: &[u8]) -> Option<u32> {
    if rx.len() < 2 {
        return None;
    }
    let w = u16::from_be_bytes([rx[0], rx[1]]);
    (w & (1 << 9) != 0).then_some((w & 0x1FF) as u32)
}

/// Simulator side: bits the chip drives on SDI/SDO during a read
/// (Table 7-2, MISO column: `1111 111n nnnn nnnn`; CMDERR low on error).
pub fn read_reply(value: Option<u32>) -> Vec<u8> {
    let w: u16 = match value {
        Some(v) => 0xFE00 | (v as u16 & 0x1FF),
        None => 0xFC00,
    };
    w.to_be_bytes().to_vec()
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02X}", x)).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    let s: String = s.chars().filter(|c| !c.is_whitespace() && *c != '_').collect();
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(&s);
    if s.is_empty() || !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regs;

    #[test]
    fn table_7_2_bit_strings() {
        // MOSI strings from Table 7-2
        assert_eq!(Cmd::Write { addr: 0x0, data: 0x100 }.encode(), vec![0b0000_0001, 0x00]);
        assert_eq!(Cmd::Write { addr: 0x2, data: 0x080 }.encode(), vec![0b0010_0000, 0x80]);
        assert_eq!(Cmd::Read { addr: 0x5 }.encode()[0] >> 2, 0b01_0111);
        assert_eq!(Cmd::Inc { addr: 0x0 }.encode(), vec![0b0000_0100]);
        assert_eq!(Cmd::Dec { addr: 0x0 }.encode(), vec![0b0000_1000]);
    }

    #[test]
    fn write_then_parse() {
        for r in 0..regs::NUM_REGS {
            let Frame::Spi(tx) = write_frame(r, 0x1A5) else { panic!() };
            assert_eq!(parse(&tx), (vec![Cmd::Write { addr: regs::REGS[r].addr as u8, data: 0x1A5 }], true));
        }
    }

    #[test]
    fn continuous_commands_and_partial_frame() {
        // MikroE library frame: write the volatile wiper, then the NV wiper, in one CS window
        let (c, ok) = parse(&[0x00, 0x7F, 0x20, 0x7F]);
        assert!(ok);
        assert_eq!(c, vec![Cmd::Write { addr: 0, data: 0x7F }, Cmd::Write { addr: 2, data: 0x7F }]);
        let (c, ok) = parse(&[0x04, 0x04, 0x00]);
        assert_eq!(c, vec![Cmd::Inc { addr: 0 }, Cmd::Inc { addr: 0 }]);
        assert!(!ok);
    }

    #[test]
    fn read_reply_and_cmderr() {
        assert_eq!(parse_read(&read_reply(Some(0x080))), Some(0x080));
        assert_eq!(parse_read(&read_reply(None)), None);
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(unhex("0x01ab FF"), Some(vec![0x01, 0xAB, 0xFF]));
        assert_eq!(hex(&[0x01, 0xAB]), "01AB");
        assert_eq!(unhex("123"), None);
    }
}
