//! Hardware links: the virtual chip, a USB-serial bridge (firmware in
//! /firmware/bridge) and USB / serial-port discovery used to identify the board.
//!
//! Bridge text protocol (one command per line, 115200 8N1, replies end with \n,
//! lines starting with '#' are comments and ignored):
//!   ?                    → BRIDGE <ver> <board> <caps>      caps: spi,spi3,i2c,gpio
//!   CFG <hz> <mode>      → OK                               SPI clock and mode 0-3
//!   S <hex>              → OK <hex>                         SPI full duplex, CS low for the frame
//!   T <hex> <bits>       → OK <hex>                         3-wire SPI (bit-banged, mode 0, ≤ 100 kHz):
//!                                                           drive the first <bits> bits on MOSI, release
//!                                                           MOSI (input with pull-up) and sample it for the
//!                                                           rest of the frame — the MCP41X1 SDI/SDO read
//!   IW <aa> <hex>        → OK | ERR NACK                    I²C write (7-bit address, hex)
//!   IR <aa> <hex|-> <n>  → OK <hex> | ERR NACK              I²C write-then-read (repeated start)
//!   P <pin> 0|1          → OK                               drive a named output pin
//!   G <pin>              → OK 0|1                           read a named pin

use crate::bus;
use crate::sim::VirtualChip;
use crate::{Error, Result};
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

/// Control / status pins of the chip that the bridge can reach.
/// The DIGI POT click routes only CS, SCK and SDI/SDO to mikroBUS (MCP41X1 has
/// no SHDN / WP pins), so there are none.
#[derive(Serialize, Clone, Copy, Debug)]
pub struct PinDef {
    pub name: &'static str,
    /// "out" (host drives) | "in" (chip drives)
    pub dir: &'static str,
    /// Level after connect (outputs).
    pub idle: bool,
    pub title: &'static str,
}

pub const PINS: &[PinDef] = &[];

#[derive(Serialize, Clone, Debug)]
pub struct LinkInfo {
    /// "sim" | "serial"
    pub kind: &'static str,
    pub name: String,
    pub detail: String,
    pub readback: bool,
}

pub trait Link: Send {
    fn info(&self) -> LinkInfo;
    /// One SPI frame, CS asserted for its duration. Returns bytes seen on MISO.
    fn spi(&mut self, tx: &[u8]) -> Result<Vec<u8>>;
    /// 3-wire SPI frame: the host drives the first `drive_bits` bits, then
    /// listens on the same data line. Returns the bits sampled over the whole frame.
    fn spi3(&mut self, _tx: &[u8], _drive_bits: u32) -> Result<Vec<u8>> {
        Err(Error::Link("3-проводной SPI не поддерживается этим подключением".into()))
    }
    fn i2c_write(&mut self, _addr: u8, _data: &[u8]) -> Result<()> {
        Err(Error::Link("I²C не поддерживается этим подключением".into()))
    }
    fn i2c_read(&mut self, _addr: u8, _wr: &[u8], _n: usize) -> Result<Vec<u8>> {
        Err(Error::Link("I²C не поддерживается этим подключением".into()))
    }
    fn set_pin(&mut self, pin: &str, high: bool) -> Result<()>;
    fn get_pin(&mut self, pin: &str) -> Result<Option<bool>>;
    fn as_sim(&mut self) -> Option<&mut VirtualChip> {
        None
    }
}

impl Link for VirtualChip {
    fn info(&self) -> LinkInfo {
        LinkInfo {
            kind: "sim",
            name: "Виртуальный MCP4161".into(),
            detail: format!("Поведенческая модель кристалла · {}", bus::describe()),
            readback: true,
        }
    }
    fn spi(&mut self, tx: &[u8]) -> Result<Vec<u8>> {
        Ok(VirtualChip::spi(self, tx))
    }
    fn spi3(&mut self, tx: &[u8], drive_bits: u32) -> Result<Vec<u8>> {
        Ok(VirtualChip::spi3(self, tx, drive_bits))
    }
    fn i2c_write(&mut self, _addr: u8, data: &[u8]) -> Result<()> {
        if VirtualChip::i2c_write(self, data) {
            Ok(())
        } else {
            Err(Error::Link("NACK".into()))
        }
    }
    fn i2c_read(&mut self, _addr: u8, wr: &[u8], _n: usize) -> Result<Vec<u8>> {
        Ok(VirtualChip::i2c_read(self, wr))
    }
    fn set_pin(&mut self, pin: &str, high: bool) -> Result<()> {
        VirtualChip::set_pin(self, pin, high);
        Ok(())
    }
    fn get_pin(&mut self, pin: &str) -> Result<Option<bool>> {
        Ok(self.pin(pin))
    }
    fn as_sim(&mut self) -> Option<&mut VirtualChip> {
        Some(self)
    }
}

pub struct SerialBridge {
    port: Box<dyn serialport::SerialPort>,
    reader: BufReader<Box<dyn serialport::SerialPort>>,
    path: String,
    ident: String,
}

impl SerialBridge {
    pub fn open(path: &str, baud: u32) -> Result<Self> {
        let port = serialport::new(path, baud)
            .timeout(Duration::from_millis(500))
            .open()
            .map_err(|e| Error::Link(format!("Не удалось открыть {}: {}", path, e)))?;
        let reader = BufReader::new(port.try_clone().map_err(|e| Error::Link(e.to_string()))?);
        let mut b = SerialBridge { port, reader, path: path.to_string(), ident: String::new() };
        // Boards with auto-reset (classic Arduino, some ESP32) need time to boot.
        for attempt in 0..12 {
            if attempt == 1 {
                std::thread::sleep(Duration::from_millis(600));
            }
            let _ = b.port.clear(serialport::ClearBuffer::Input);
            if let Ok(r) = b.raw("?") {
                if r.starts_with("BRIDGE") {
                    b.ident = r;
                    b.cmd(&format!("CFG {} {}", bus::SPI_MAX_HZ.min(4_000_000), bus::SPI_MODE))?;
                    for p in PINS.iter().filter(|p| p.dir == "out") {
                        b.set_pin(p.name, p.idle)?;
                    }
                    return Ok(b);
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Err(Error::Link(format!("{}: нет ответа BRIDGE — прошейте firmware/bridge и проверьте скорость порта", path)))
    }

    fn raw(&mut self, line: &str) -> Result<String> {
        self.port.write_all(format!("{}\n", line).as_bytes()).map_err(|e| Error::Link(e.to_string()))?;
        self.port.flush().ok();
        let mut s = String::new();
        loop {
            s.clear();
            self.reader.read_line(&mut s).map_err(|e| Error::Link(format!("таймаут ответа моста: {}", e)))?;
            let t = s.trim();
            if !t.is_empty() && !t.starts_with('#') {
                return Ok(t.to_string());
            }
        }
    }

    fn cmd(&mut self, line: &str) -> Result<String> {
        let r = self.raw(line)?;
        match r.strip_prefix("OK") {
            Some(rest) => Ok(rest.trim().to_string()),
            None => Err(Error::Link(format!("мост ответил: {}", r))),
        }
    }

    fn hex_reply(&mut self, line: &str) -> Result<Vec<u8>> {
        let r = self.cmd(line)?;
        bus::unhex(&r).ok_or_else(|| Error::Link(format!("неверный ответ моста: {}", r)))
    }
}

impl Link for SerialBridge {
    fn info(&self) -> LinkInfo {
        LinkInfo { kind: "serial", name: format!("Мост {}", self.path), detail: self.ident.clone(), readback: bus::can_read() }
    }
    fn spi(&mut self, tx: &[u8]) -> Result<Vec<u8>> {
        self.hex_reply(&format!("S {}", bus::hex(tx)))
    }
    fn spi3(&mut self, tx: &[u8], drive_bits: u32) -> Result<Vec<u8>> {
        let r = self.hex_reply(&format!("T {} {}", bus::hex(tx), drive_bits));
        if let Err(Error::Link(m)) = &r {
            if m.contains("ERR") {
                return Err(Error::Link(format!("{} — прошивка моста без команды T (3-проводное чтение)? Обновите firmware/bridge", m)));
            }
        }
        r
    }
    fn i2c_write(&mut self, addr: u8, data: &[u8]) -> Result<()> {
        self.cmd(&format!("IW {:02X} {}", addr, bus::hex(data))).map(|_| ())
    }
    fn i2c_read(&mut self, addr: u8, wr: &[u8], n: usize) -> Result<Vec<u8>> {
        let w = if wr.is_empty() { "-".to_string() } else { bus::hex(wr) };
        self.hex_reply(&format!("IR {:02X} {} {}", addr, w, n))
    }
    fn set_pin(&mut self, pin: &str, high: bool) -> Result<()> {
        self.cmd(&format!("P {} {}", pin, high as u8)).map(|_| ())
    }
    fn get_pin(&mut self, pin: &str) -> Result<Option<bool>> {
        Ok(Some(self.cmd(&format!("G {}", pin))?.trim() == "1"))
    }
}

// ───────────── discovery ─────────────

#[derive(Serialize, Clone, Debug)]
pub struct PortInfo {
    pub path: String,
    pub kind: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
}

pub fn list_ports() -> Vec<PortInfo> {
    let mut v: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let mut info = PortInfo { path: p.port_name.clone(), kind: "other".into(), vid: None, pid: None, manufacturer: None, product: None, serial: None };
            match p.port_type {
                serialport::SerialPortType::UsbPort(u) => {
                    info.kind = "usb".into();
                    info.vid = Some(u.vid);
                    info.pid = Some(u.pid);
                    info.manufacturer = u.manufacturer;
                    info.product = u.product;
                    info.serial = u.serial_number;
                }
                serialport::SerialPortType::BluetoothPort => info.kind = "bluetooth".into(),
                serialport::SerialPortType::PciPort => info.kind = "pci".into(),
                serialport::SerialPortType::Unknown => {}
            }
            info
        })
        .collect();
    // macOS lists /dev/tty.* and /dev/cu.* for the same device: keep cu.*
    if cfg!(target_os = "macos") {
        v.retain(|p| !p.path.starts_with("/dev/tty."));
    }
    v
}

#[derive(Serialize, Clone, Debug)]
pub struct UsbDev {
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub bus: u8,
    pub address: u8,
    pub hint: Option<&'static str>,
    pub likely_bridge: bool,
}

/// Vendor hints for USB devices commonly found on or next to evaluation boards.
/// Add the board's own interface chip (vendor ID from its user guide) at the top.
fn vendor_hint(vid: u16, pid: u16) -> (Option<&'static str>, bool) {
    match (vid, pid) {
        (0x0403, _) => (Some("FTDI — типичный USB-SPI/I²C мост (MPSSE: FT232H/FT2232H/FT4232H)"), true),
        (0x04B4, _) => (Some("Cypress/Infineon — USB-мост (FX2LP / CY7C652xx)"), true),
        (0x10C4, 0x87A0) => (Some("Silicon Labs CP2130 — USB-SPI мост"), true),
        (0x10C4, 0xEA90) => (Some("Silicon Labs CP2112 — USB-I²C мост"), true),
        (0x10C4, _) => (Some("Silicon Labs — USB-мост (CP21xx)"), true),
        (0x04D8, 0x00DE) => (Some("Microchip MCP2210 — USB-SPI мост"), true),
        (0x04D8, 0x00DD) => (Some("Microchip MCP2221 — USB-I²C/UART мост"), true),
        (0x04D8, _) => (Some("Microchip — USB-устройство / микроконтроллер"), true),
        (0x0456, _) => (Some("Analog Devices — интерфейс оценочной платы (SDP/ADALM)"), true),
        (0x045B, _) => (Some("Renesas — интерфейс оценочной платы"), true),
        (0x0451, _) => (Some("Texas Instruments — USB-контроллер / USB2ANY"), true),
        (0x1FC9, _) => (Some("NXP — USB-контроллер"), true),
        (0x0483, 0x3748) | (0x0483, 0x374B) | (0x0483, 0x374E) | (0x0483, 0x374F) | (0x0483, 0x3752) | (0x0483, 0x3753) => {
            (Some("ST-LINK — Nucleo можно использовать как мост (firmware/bridge)"), false)
        }
        (0x0483, _) => (Some("STMicroelectronics — USB-устройство"), false),
        (0x2E8A, _) => (Some("Raspberry Pi RP2040/RP2350 — подходит как мост"), false),
        (0x303A, _) => (Some("Espressif ESP32-S2/S3/C3 — подходит как мост"), false),
        (0x2341, _) | (0x2A03, _) => (Some("Arduino"), false),
        (0x239A, _) => (Some("Adafruit"), false),
        (0x067B, _) => (Some("Prolific PL2303 — USB-UART (без SPI)"), false),
        (0x1A86, _) => (Some("WCH CH34x — USB-UART (CH341A умеет SPI/I²C)"), false),
        (0x05AC, _) => (Some("Apple"), false),
        _ => (None, false),
    }
}

pub fn scan_usb() -> Result<Vec<UsbDev>> {
    let list = nusb::list_devices().map_err(|e| Error::Link(format!("USB: {}", e)))?;
    let mut v: Vec<UsbDev> = list
        .filter(|d| d.class() != 0x09) // skip hubs
        .map(|d| {
            let (hint, likely) = vendor_hint(d.vendor_id(), d.product_id());
            UsbDev {
                vid: d.vendor_id(),
                pid: d.product_id(),
                manufacturer: d.manufacturer_string().map(|s| s.to_string()),
                product: d.product_string().map(|s| s.to_string()),
                serial: d.serial_number().map(|s| s.to_string()),
                bus: d.bus_number(),
                address: d.device_address(),
                hint,
                likely_bridge: likely,
            }
        })
        .collect();
    v.sort_by_key(|d| (!d.likely_bridge, d.vid, d.pid));
    Ok(v)
}
