// Mirrors of the Rust types serialized by digipot-core (api.rs / regs.rs / model.rs).
// Keep in sync when adding RPC commands or snapshot fields.

export type Access = "rw" | "ro" | "sticky" | "fixed";

export interface RegDef {
  addr: number;
  name: string;
  title: string;
  reset: number;
}

export interface FieldDef {
  name: string;
  reg: number;
  msb: number;
  lsb: number;
  access: Access;
  default: number;
  title: string;
  desc: string;
  options: [number, string][];
  block: string;
  unit: string;
  signed: boolean;
}

export interface PinDef {
  name: string;
  dir: "in" | "out";
  idle: boolean;
  title: string;
}

export interface ChipInfo {
  name: string;
  vendor: string;
  board: string;
  bus: string;
  data_bits: number;
  can_read: boolean;
  commit_reg: number | null;
  full_scale: number;
}

/** Сведения о программе (digipot-core `about.rs`). */
export interface AppInfo {
  name: string;
  version: string;
  /** DD.MM.YYYY */
  release_date: string;
  author: string;
  license: string;
  repository: string;
  /** Зеркало репозитория (Forgejo). */
  mirror: string;
}

export interface Meta {
  regs: RegDef[];
  fields: FieldDef[];
  defaults: number[];
  pins: PinDef[];
  /** true for EEPROM-backed registers (written only by nv_write). */
  nv: boolean[];
  /** [part suffix, RAB Ω] */
  rab_options: [string, number][];
  version: string;
  app: AppInfo;
  chip: ChipInfo;
}

export interface Metric {
  key: string;
  label: string;
  value: number;
  unit: string;
  block: string;
  top: boolean;
}

export interface Issue {
  level: "error" | "warn" | "info";
  block: string;
  text: string;
}

export interface Derived {
  metrics: Metric[];
  issues: Issue[];
  calc: Record<string, number | boolean>;
}

/** Board-level settings (model.rs `Settings`). */
export interface Settings {
  rab_nominal: number;
  vdd: number;
  va: number;
  vb: number;
  load_ohm: number;
  rw_ohm: number;
  temp_c: number;
  sim_rab_dev_pct: number;
}

/** Virtual multimeter on the simulated part (its RAB differs from nominal). */
export interface Meter {
  rab: number;
  rwb: number;
  rwa: number;
  vw: number;
}

/** model.rs `Curve`: transfer characteristic over codes 0…256. */
export interface Curve {
  code: number[];
  rwb: number[];
  rwa: number[];
  vw_ideal: number[];
  vw: number[];
  inl_lo: number[];
  inl_hi: number[];
}

export interface LinkInfo {
  kind: "sim" | "serial";
  name: string;
  detail: string;
  readback: boolean;
}

export interface SimStatus {
  state: "ok" | "busy" | "warn" | "fault" | "off";
  text: string;
  signals: { name: string; value: number; unit: string }[];
  pins: [string, boolean][];
}

export interface Snapshot {
  regs: number[];
  desired: number[];
  dev_regs: number[] | null;
  dirty: boolean[];
  fields: Record<string, number>;
  derived: Derived;
  settings: Settings;
  link: LinkInfo;
  link_error: string | null;
  auto_apply: boolean;
  pins: Record<string, boolean>;
  inputs: Record<string, boolean | null>;
  log_head: number;
  readback: { words: (number | null)[]; matches: (boolean | null)[] };
  sim: SimStatus | null;
  meter: Meter | null;
  ee_writes: number;
}

export interface Status {
  sim: SimStatus | null;
  meter: Meter | null;
  inputs: Record<string, boolean | null>;
  log_head: number;
  link_error: string | null;
}

export interface Txn {
  id: number;
  t_ms: number;
  op: "write" | "read" | "raw" | "pin" | "note";
  reg: number | null;
  tx: string | null;
  rx: string | null;
  value: number | null;
  changes: [string, number, number][];
  note: string;
  ok: boolean;
}

export interface PortInfo {
  path: string;
  kind: string;
  vid: number | null;
  pid: number | null;
  manufacturer: string | null;
  product: string | null;
  serial: string | null;
}

export interface UsbDev {
  vid: number;
  pid: number;
  manufacturer: string | null;
  product: string | null;
  serial: string | null;
  bus: number;
  address: number;
  hint: string | null;
  likely_bridge: boolean;
}

export type ViewId = "overview" | "pot" | "memory" | "board" | "registers" | "hardware" | "learn" | "about";
