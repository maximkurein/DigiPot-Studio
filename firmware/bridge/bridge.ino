/*
  DigiPot Studio — USB-serial bridge (SPI + I2C + GPIO) for the MCP4161
  ---------------------------------------------------------------------
  Works on any Arduino-core board with 3.3 V I/O: Raspberry Pi Pico (arduino-pico),
  STM32 Nucleo (STM32duino, via ST-LINK VCP), ESP32, Arduino Zero/Due/Nano 33 ...

  !!! Check the chip's interface voltage in the datasheet. Never drive 1.8/3.3 V
  !!! pins from a 5 V board (Uno/Mega/Nano classic) without level shifters.

  Protocol (115200 8N1, one command per line, replies end with \n, '#' = comment):
     ?                    -> BRIDGE 1 <board> spi,spi3,i2c,gpio
     CFG <hz> <mode>      -> OK                  SPI clock and mode (0-3)
     S <hex>              -> OK <hex>            SPI full duplex, CS low for the whole frame
     T <hex> <bits>       -> OK <hex>            3-wire SPI read, bit-banged mode 0 at ~100 kHz:
                                                 drive the first <bits> bits on MOSI, release MOSI
                                                 (input with pull-up) right after the rising edge of
                                                 the last driven bit and sample it for the rest.
                                                 MCP41X1 has one SDI/SDO pin and the DIGI POT click
                                                 wires it to mikroBUS MOSI only (MISO is NC).
     IW <aa> <hex>        -> OK | ERR NACK       I2C write, 7-bit address in hex
     IR <aa> <hex|-> <n>  -> OK <hex> | ERR NACK I2C write (optional) + repeated start + read n
     P <pin> 0|1          -> OK                  drive a named output
     G <pin>              -> OK 0|1              read a named pin

  Wiring to the DIGI POT click: SCK -> SCK, MOSI -> MOSI (SDI/SDO), D10 -> CS, GND, 3.3V.
  The click has no control/status pins, so PIN_TABLE is empty (transport.rs PINS = []).
*/
#include <SPI.h>
#include <Wire.h>

#ifndef PIN_CS
#define PIN_CS 10
#endif
#ifndef PIN_SCK
#define PIN_SCK SCK
#endif
#ifndef PIN_MOSI
#define PIN_MOSI MOSI
#endif
// Half period of the bit-banged 3-wire read, µs (5 µs -> ~100 kHz; the chip allows
// 250 kHz on SDI/SDO without an external pull-up, DS22059B Table 6-1).
#ifndef SPI3_HALF_US
#define SPI3_HALF_US 5
#endif

struct NamedPin {
  const char* name;
  int pin;
  bool output;
  bool idle;
};

// name, MCU pin, output?, idle level. The DIGI POT click has none; the entry
// below is a placeholder and N_PINS = 0 keeps it unused.
static NamedPin PIN_TABLE[] = {
  {"-", -1, false, false},
};
static const int N_PINS = 0;

#ifdef ARDUINO_BOARD
#define BOARD_NAME ARDUINO_BOARD
#else
#define BOARD_NAME "arduino"
#endif

static uint32_t spiHz = 1000000UL;
static uint8_t spiMode = SPI_MODE0;
static char line[300];
static uint16_t len = 0;
static uint8_t buf[128];

static void reply(const char* s) {
  Serial.print(s);
  Serial.print('\n');
}

static int hexval(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

// Parse hex bytes ("81 00 00" or "810000") into out; returns count or -1.
static int parseHex(const char* s, uint8_t* out, int maxn) {
  int n = 0, hi = -1;
  for (; *s; ++s) {
    if (*s == ' ') continue;
    int v = hexval(*s);
    if (v < 0) return -1;
    if (hi < 0) hi = v;
    else {
      if (n >= maxn) return -1;
      out[n++] = (uint8_t)((hi << 4) | v);
      hi = -1;
    }
  }
  return hi < 0 ? n : -1;
}

static void printHex(const uint8_t* b, int n) {
  static const char* H = "0123456789ABCDEF";
  Serial.print("OK ");
  for (int i = 0; i < n; ++i) {
    Serial.print(H[b[i] >> 4]);
    Serial.print(H[b[i] & 15]);
  }
  Serial.print('\n');
}

// 3-wire SPI (mode 0): the host drives `drive` bits, then listens on the same line.
static void spi3(uint8_t* b, int n, int drive) {
  SPI.end();  // take SCK/MOSI back as GPIO
  pinMode(PIN_SCK, OUTPUT);
  digitalWrite(PIN_SCK, LOW);
  pinMode(PIN_MOSI, OUTPUT);
  digitalWrite(PIN_CS, LOW);
  delayMicroseconds(SPI3_HALF_US);
  int bit = 0;
  for (int i = 0; i < n; ++i) {
    uint8_t in = 0;
    for (int k = 7; k >= 0; --k, ++bit) {
      if (bit < drive) digitalWrite(PIN_MOSI, (b[i] >> k) & 1);
      delayMicroseconds(SPI3_HALF_US);
      digitalWrite(PIN_SCK, HIGH);  // chip latches SDI / SDO is valid on the rising edge
      in = (uint8_t)((in << 1) | (digitalRead(PIN_MOSI) ? 1 : 0));
      if (bit == drive - 1) pinMode(PIN_MOSI, INPUT_PULLUP);  // release before the chip drives
      delayMicroseconds(SPI3_HALF_US);
      digitalWrite(PIN_SCK, LOW);
    }
    b[i] = in;
  }
  delayMicroseconds(SPI3_HALF_US);
  digitalWrite(PIN_CS, HIGH);
  pinMode(PIN_MOSI, OUTPUT);
  SPI.begin();
}

static NamedPin* findPin(const char* name) {
  for (int i = 0; i < N_PINS; ++i)
    if (!strcmp(PIN_TABLE[i].name, name)) return &PIN_TABLE[i];
  return nullptr;
}

static void handle(char* l) {
  while (*l == ' ') ++l;
  if (l[0] == '?') {
    Serial.print("BRIDGE 1 ");
    Serial.print(BOARD_NAME);
    Serial.print(" spi,spi3,i2c,gpio\n");
  } else if (!strncmp(l, "CFG ", 4)) {
    char* p;
    uint32_t hz = strtoul(l + 4, &p, 10);
    int mode = (int)strtol(p, nullptr, 10);
    static const uint8_t MODES[] = {SPI_MODE0, SPI_MODE1, SPI_MODE2, SPI_MODE3};
    if (hz < 1000UL || hz > 50000000UL || mode < 0 || mode > 3) return reply("ERR range");
    spiHz = hz;
    spiMode = MODES[mode];
    reply("OK");
  } else if (l[0] == 'S' && l[1] == ' ') {
    int n = parseHex(l + 2, buf, sizeof(buf));
    if (n <= 0) return reply("ERR hex");
    SPI.beginTransaction(SPISettings(spiHz, MSBFIRST, spiMode));
    digitalWrite(PIN_CS, LOW);
    delayMicroseconds(1);
    for (int i = 0; i < n; ++i) buf[i] = SPI.transfer(buf[i]);
    delayMicroseconds(1);
    digitalWrite(PIN_CS, HIGH);
    SPI.endTransaction();
    printHex(buf, n);
  } else if (l[0] == 'T' && l[1] == ' ') {
    char* sp = strrchr(l + 2, ' ');
    if (!sp) return reply("ERR args");
    *sp = 0;
    int drive = atoi(sp + 1);
    int n = parseHex(l + 2, buf, sizeof(buf));
    if (n <= 0 || drive < 0 || drive > n * 8) return reply("ERR hex");
    spi3(buf, n, drive);
    printHex(buf, n);
  } else if (!strncmp(l, "IW ", 3)) {
    char* p;
    uint8_t addr = (uint8_t)strtoul(l + 3, &p, 16);
    while (*p == ' ') ++p;
    int n = parseHex(p, buf, sizeof(buf));
    if (n < 0) return reply("ERR hex");
    Wire.beginTransmission(addr);
    Wire.write(buf, n);
    reply(Wire.endTransmission() == 0 ? "OK" : "ERR NACK");
  } else if (!strncmp(l, "IR ", 3)) {
    char* p;
    uint8_t addr = (uint8_t)strtoul(l + 3, &p, 16);
    while (*p == ' ') ++p;
    char* wr = p;
    while (*p && *p != ' ') ++p;
    if (*p) *p++ = 0;
    int cnt = (int)strtol(p, nullptr, 10);
    if (cnt <= 0 || cnt > (int)sizeof(buf)) return reply("ERR range");
    if (strcmp(wr, "-")) {
      int n = parseHex(wr, buf, sizeof(buf));
      if (n < 0) return reply("ERR hex");
      Wire.beginTransmission(addr);
      Wire.write(buf, n);
      if (Wire.endTransmission(false) != 0) return reply("ERR NACK");
    }
    int got = Wire.requestFrom((int)addr, cnt);
    if (got != cnt) return reply("ERR NACK");
    for (int i = 0; i < cnt; ++i) buf[i] = Wire.read();
    printHex(buf, cnt);
  } else if (l[0] == 'P' && l[1] == ' ') {
    char* name = l + 2;
    char* sp = strchr(name, ' ');
    if (!sp) return reply("ERR args");
    *sp = 0;
    NamedPin* np = findPin(name);
    if (!np || !np->output) return reply("ERR pin");
    digitalWrite(np->pin, sp[1] == '1' ? HIGH : LOW);
    reply("OK");
  } else if (l[0] == 'G' && l[1] == ' ') {
    NamedPin* np = findPin(l + 2);
    if (!np) return reply("ERR pin");
    reply(digitalRead(np->pin) ? "OK 1" : "OK 0");
  } else if (l[0] == 0) {
    // empty line
  } else {
    reply("ERR unknown");
  }
}

void setup() {
  pinMode(PIN_CS, OUTPUT);
  digitalWrite(PIN_CS, HIGH);
  for (int i = 0; i < N_PINS; ++i) {
    if (PIN_TABLE[i].output) {
      pinMode(PIN_TABLE[i].pin, OUTPUT);
      digitalWrite(PIN_TABLE[i].pin, PIN_TABLE[i].idle ? HIGH : LOW);
    } else {
      pinMode(PIN_TABLE[i].pin, INPUT_PULLUP);
    }
  }
  SPI.begin();
  Wire.begin();
  Serial.begin(115200);
}

void loop() {
  while (Serial.available()) {
    char c = (char)Serial.read();
    if (c == '\r') continue;
    if (c == '\n') {
      line[len] = 0;
      handle(line);
      len = 0;
    } else if (len < sizeof(line) - 1) {
      line[len++] = c;
    }
  }
}
