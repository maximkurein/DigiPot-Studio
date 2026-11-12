# Нативный интерфейс на egui

Как устроен `crates/digipot-egui` — второй настольный интерфейс DigiPot Studio, написанный целиком на Rust, без WebView, Node и React. Документ для тех, кто будет его читать, править или собирать. Все примеры кода взяты из проекта (иногда сокращены — пропуски отмечены `…`).

- [Зачем второй интерфейс](#зачем-второй-интерфейс)
- [Что такое egui: immediate mode](#что-такое-egui-immediate-mode)
- [Место в проекте](#место-в-проекте)
- [Точка входа](#точка-входа)
- [Главный цикл: кадр за кадром](#главный-цикл-кадр-за-кадром)
- [Экраны не трогают движок: `Shared` и `Act`](#экраны-не-трогают-движок-shared-и-act)
- [Движок в отдельном потоке](#движок-в-отдельном-потоке)
- [Пример экрана: ползунок движка](#пример-экрана-ползунок-движка)
- [Безопасность EEPROM в интерфейсе](#безопасность-eeprom-в-интерфейсе)
- [Графики (egui_plot) и схемы (Painter)](#графики-egui_plot-и-схемы-painter)
- [Тема и цвета](#тема-и-цвета)
- [Диалоги файлов и настройки](#диалоги-файлов-и-настройки)
- [Снимки экранов для проверки](#снимки-экранов-для-проверки)
- [Сборка .app и .dmg](#сборка-app-и-dmg)
- [Как добавить новую функцию](#как-добавить-новую-функцию)
- [Частые ловушки](#частые-ловушки)

## Зачем второй интерфейс

| | Tauri (`src-tauri` + `src/`) | egui (`crates/digipot-egui`) |
|---|---|---|
| Язык интерфейса | TypeScript, React 19, Tailwind 4 | Rust |
| Отрисовка | системный WebView | OpenGL (glow), сам рисует каждый кадр |
| Сборка | нужен Node + npm | только `cargo` |
| Связь с движком | RPC `rpc` (JSON через IPC) | прямой вызов `Engine::handle` в своём потоке |
| Размер `.dmg` (arm64) | больше (WebView-обвязка, фронтенд) | ≈ 3,7 МБ |
| Веб-версия | да (`digipot-server`) | нет |

Оба интерфейса пользуются одним движком `digipot-core` и **одними и теми же правилами** (EEPROM только через подтверждённый `nv_write`, автозапись только в энергозависимые регистры). Tauri и веб-режим остаются основными; egui — лёгкая нативная альтернатива.

## Что такое egui: immediate mode

В React описывают дерево компонентов, а библиотека следит за изменениями. egui устроен иначе — **immediate mode**: каждый кадр (до 60 раз в секунду, но только когда что-то изменилось) функция `update` заново «проговаривает» весь интерфейс, и вызов виджета сразу возвращает, что с ним произошло.

```rust
// одна строка = нарисовать кнопку И узнать, нажали ли её в этом кадре
if ui.button("Прочитать").clicked() {
    // …
}
```

Отсюда три следствия, которые видны во всём крейте:

1. **Нет отдельного состояния виджетов** — значение ползунка лежит в обычной переменной, которую виджет меняет по `&mut`.
2. **Рисовать нужно быстро** — в `update` нельзя ждать железо. Поэтому движок живёт в отдельном потоке (см. ниже).
3. **Состояние, переживающее кадр** (выбранная вкладка, черновик ввода), хранится в структурах приложения (`ViewState`), а не в виджетах.

## Место в проекте

```text
crates/
  digipot-core/      ← модель, регистры, симулятор, транспорт, Engine::handle (общее)
  digipot-server/    ← HTTP-сервер для веб-режима
  digipot-egui/      ← этот интерфейс
    src/
      main.rs        точка входа, параметры окна
      app.rs         оболочка: навигация, верхняя панель, журнал шины, тосты, Act → Job
      worker.rs      поток движка: очередь заданий, публикация состояния
      theme.rs       палитра (те же цвета, что CSS-переменные веб-версии) и стиль egui
      widgets.rs     карточки, кнопки, поля, таблицы — общие элементы
      canvas.rs      рисование в координатах «как в SVG viewBox»
      content.rs     тексты блоков, платы, уроков (зеркало blocks.ts / board.ts / lessons.ts)
      fmt.rs         форматирование чисел: 2.5 мВ, 10 кОм, 0x1FF
      shots.rs       автоматический обход экранов со снимками
      views/*.rs     восемь экранов: pot, overview, memory, registers, board, hardware, learn, about
src-tauri/           ← Tauri-оболочка (не зависит от egui)
```

Зависимости (`crates/digipot-egui/Cargo.toml`):

```toml
[dependencies]
digipot-core = { path = "../digipot-core" }
serde_json = "1"
# 0.33 / 0.34: the newest line that builds on Rust 1.88+ (0.34+ needs 1.92)
eframe = { version = "0.33", default-features = false, features = ["default_fonts", "glow", "persistence", "wayland", "x11"] }
egui_plot = "0.34"
egui_extras = "0.33"
rfd = "0.15"
```

- **eframe** — окно, цикл событий, OpenGL-бэкенд (`glow`) и сохранение настроек (`persistence`).
- **egui_plot** — графики R_WB/R_WA, V_W, INL.
- **egui_extras** — таблицы (журнал шины, карта регистров).
- **rfd** — системные диалоги «Открыть / Сохранить».

Запуск:

```bash
make egui                          # = cargo run --release -p digipot-egui
cargo test -p digipot-egui         # тесты форматирования и потока движка
```

## Точка входа

`src/main.rs` целиком умещается на экран:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DigiPot Studio — MCP4161")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("DigiPot Studio", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
```

`windows_subsystem = "windows"` убирает консольное окно в релизной сборке под Windows. `App::new` загружает шрифты, тему и запускает поток движка:

```rust
pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
    theme::install_fonts(&cc.egui_ctx);
    let get = |k: &str| cc.storage.and_then(|s| s.get_string(k));
    let dark = get("theme").map(|t| t != "light").unwrap_or(true);
    // …
    let (worker, st, log) = Worker::spawn(Engine::new(config_path()), cc.egui_ctx.clone());
    // …
}
```

## Главный цикл: кадр за кадром

`impl eframe::App for App` — сердце интерфейса (`src/app.rs`). Каждый кадр проходит одни и те же шаги:

```rust
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.refresh(ctx);            // 1. забрать то, что опубликовал поток движка
    self.shortcuts(ctx);          // 2. ⌘/Ctrl + 1…8, ⌘/Ctrl + J

    let sh = &self.sh;            // 3. нарисовать всё; экраны получают только &Shared
    top_bar(ctx, sh);
    sidebar(ctx, sh);
    // …
    match sh.view {
        View::Pot => views::pot::show(ui, sh, &mut vs.pot),
        View::Memory => views::memory::show(ui, sh, &mut vs.memory),
        // …
    }
    self.show_toasts(ctx);

    // 4. после кадра — выполнить действия, которые экраны поставили в очередь
    let acts: Vec<Act> = self.sh.acts.borrow_mut().drain(..).collect();
    for a in acts {
        self.run(ctx, a);
    }
}
```

```text
 ┌──────────── UI-поток (кадр) ────────────┐          ┌──── поток движка ────┐
 │ refresh() ← state, log, replies ◄───────┼──inbox───┤ Engine::state()      │
 │ draw views (читают &Shared)             │          │ Engine::handle(...)  │
 │ views → sh.act(Act::…)                  │          │ последовательный     │
 │ run(Act) → worker.send(Job) ────────────┼─channel─►│ порт / симулятор     │
 └─────────────────────────────────────────┘          └──────────────────────┘
```

## Экраны не трогают движок: `Shared` и `Act`

Экран получает `&Shared` — неизменяемый снимок всего, что нужно для отрисовки, — и не может напрямую ничего изменить. Чтобы что-то сделать, он **ставит действие в очередь**:

```rust
/// Something a view asks the shell to do after the frame.
pub enum Act {
    Rpc(&'static str, Value),
    Go(View, Option<Option<&'static str>>),
    Toast(Tone, String),
    Export,
    Import,
    Raw(String, Option<u32>),
    ReadNv,
    // …
}

/// What every view may read, plus the action queue.
pub struct Shared {
    pub st: State,          // последний типизированный снимок движка
    pub log: Vec<Txn>,      // журнал шины
    pub pal: Pal,           // цвета текущей темы
    pub busy: bool,         // идёт обмен с мостом
    // …
    acts: RefCell<Vec<Act>>,
}
```

Почему `RefCell`: экран держит `&Shared` (общую ссылку), а положить действие в очередь — это изменение. `RefCell` позволяет это безопасно в одном потоке. Для удобства есть короткие методы:

```rust
impl Shared {
    pub fn act(&self, a: Act) { self.acts.borrow_mut().push(a); }
    pub fn rpc(&self, cmd: &'static str, args: Value) { self.act(Act::Rpc(cmd, args)); }
    pub fn set_field(&self, name: &str, value: u32) {
        self.rpc("set_field", json!({ "name": name, "value": value }));
    }
    pub fn field(&self, name: &str) -> u32 {
        self.st.fields.get(name).copied().unwrap_or(0)
    }
}
```

`State` — это структура из `digipot-core` (`api::State`), тот же снимок, который Tauri-версия получает по RPC `get_state` в виде JSON. egui читает её напрямую, без сериализации:

```rust
pub struct State {
    pub regs: Words,
    pub fields: BTreeMap<&'static str, u32>,
    pub derived: Derived,          // метрики и предупреждения модели
    pub settings: Settings,
    pub link: transport::LinkInfo,
    pub sim: Option<SimStatus>,
    // …
}
```

Оболочка превращает `Act` в задание для потока движка (`App::run`):

```rust
match a {
    Act::Rpc(cmd, args) => self.send(Job::Rpc { cmd, args, ok: None }),
    Act::Export => self.send(Job::Export),
    Act::ReadNv => self.send(Job::ReadNv),
    Act::Go(v, b) => { self.sh.view = v; /* … */ }
    // …
}
```

**Главное правило:** все изменения идут через `Engine::handle(cmd, args)` — тот же RPC, что у Tauri и веб-сервера. Поэтому правила безопасности проверяются в одном месте, в `digipot-core/src/api.rs`, и интерфейс не может их обойти.

## Движок в отдельном потоке

Обмен с USB-мостом идёт по последовательному порту, а запись EEPROM ждёт до 10 мс (tWC). Если делать это в `update`, окно замирает. Поэтому `Engine` переезжает в поток `digipot-engine` (`src/worker.rs`):

```rust
pub enum Job {
    /// `Engine::handle(cmd, args)`; `ok` is toasted on success.
    Rpc { cmd: &'static str, args: Value, ok: Option<String> },
    Raw { hex: String, drive: Option<u32> },
    ReadNv,
    Ports,
    Export,
}
```

Цикл потока: ждать задание не дольше 350 мс, забрать всё, что накопилось, выполнить, опубликовать результат и разбудить окно.

```rust
loop {
    let first = match rx.recv_timeout(IDLE_REFRESH) {      // 350 мс
        Ok(j) => Some(j),
        Err(RecvTimeoutError::Timeout) => None,            // простой: просто обновить статус
        Err(RecvTimeoutError::Disconnected) => break,      // окно закрыто
    };
    let mut jobs: Vec<Job> = first.into_iter().collect();
    jobs.extend(rx.try_iter());                            // всё, что пришло за время обмена
    for job in coalesce(jobs) {
        reset |= run(&mut engine, job, &mut replies);
    }
    let state = engine.state();
    // … положить state, новые записи журнала и ответы во inbox под Mutex
    ctx.request_repaint();                                 // попросить egui перерисовать кадр
}
```

Окно в начале каждого кадра забирает всё разом (`Worker::take`), не дожидаясь потока:

```rust
fn refresh(&mut self, ctx: &egui::Context) {
    let u = self.worker.take();
    if let Some(st) = u.state {
        self.sh.st = st;
    }
    self.sh.log.extend(u.log);
    for r in u.replies {
        match r {
            Reply::Toast(t, s) => self.toast(t, s),
            Reply::ExportText(text) => self.save_export(text),
            Reply::Ports(ports, usb) => { /* … */ }
        }
    }
    // индикатор «обмен…» — только если задание длится дольше 150 мс
}
```

### Слияние заданий

Когда пользователь тянет ползунок через медленный мост, за время одного обмена может прийти десяток `set_field WIPER0`. Отправлять все — бессмысленно, важно только последнее значение. `coalesce` оставляет последнее из подряд идущих одинаковых заданий, но **не переставляет** задания другого типа: `step` между двумя записями — это барьер.

```rust
#[test]
fn coalesces_slider_runs_but_keeps_barriers() {
    let jobs = vec![
        set("WIPER0", 1),
        set("WIPER0", 2),
        set("R0A", 0),
        set("WIPER0", 3),
        Job::Rpc { cmd: "step", args: json!({ "up": true }), ok: None },
        set("WIPER0", 4),
        set("WIPER0", 5),
    ];
    assert_eq!(describe(&coalesce(jobs)), ["set_field:0", "set_field:3", "step:null", "set_field:5"]);
}
```

## Пример экрана: ползунок движка

Фрагмент `views/pot.rs` показывает типичный экран: прочитать из `Shared`, нарисовать, поставить действие.

```rust
w::card(ui, &p, "Движок", "WIPER0 · 257 положений (0…256)", |ui| {
    let mut draft = ps.draft.unwrap_or(eff);
    // …
    let r = ui.add(egui::Slider::new(&mut draft, 0..=FS).show_value(false));
    // keep the dragged value until the engine thread has caught up,
    // otherwise the handle jumps back for a frame after release
    if r.dragged() {
        ps.draft = Some(draft);
    } else if !sh.busy || ps.draft == Some(eff) {
        ps.draft = None;
    }
    if r.changed() && draft != eff {
        sh.set_field("WIPER0", draft);
    }
});
```

Здесь видна особенность асинхронного движка: пока поток не применил значение, `sh.st` ещё старое. Черновик `ps.draft` (в `PotState`, переживает кадр) не даёт ручке ползунка «прыгнуть» назад.

Кнопки шагов — однобайтовые команды инкремента/декремента MCP4161:

```rust
if w::button(ui, &p, label, Btn::Default).on_hover_text(tip).clicked() {
    sh.rpc("step", json!({ "up": up, "n": n }));
}
```

## Безопасность EEPROM в интерфейсе

Каждая запись EEPROM расходует один из ~1 млн циклов, поэтому ни ползунки, ни автозапись туда не пишут. На экране «EEPROM» запись — двухшаговая кнопка: первый щелчок «взводит» её на 5 секунд, второй выполняет.

```rust
/// Two-step button: the first click arms it (5 s), the second performs the write.
fn confirm_button(ui: &mut Ui, sh: &Shared, ms: &mut MemoryState, key: &'static str,
                  label: &str, confirm: &str, enabled: bool) -> bool {
    let armed = matches!(ms.armed, Some((k, t)) if k == key && t > Instant::now());
    if !armed {
        if ui.add_enabled(enabled, w::btn(&p, label, Btn::Default)).clicked() {
            ms.armed = Some((key, Instant::now() + Duration::from_secs(5)));
        }
        return false;
    }
    // … кнопки «Записать N» (красная) и «Отмена»
    go
}

fn nv_write(sh: &Shared, reg: usize, value: u32) {
    sh.rpc("nv_write", json!({ "index": reg, "value": value, "confirm": true }));
}
```

Даже если интерфейс ошибётся, движок откажет: `set_field` для `NV_WIPER0` возвращает ошибку, и поток показывает её тостом. Это проверяет тест в `worker.rs`:

```rust
// errors come back as toasts, the EEPROM rule still holds
w.send(set("NV_WIPER0", 1));
let u = settle(&w);
assert!(matches!(u.replies.first(), Some(Reply::Toast(Tone::Err, _))));
```

## Графики (egui_plot) и схемы (Painter)

**Графики.** Точки кривых считает Rust-модель (`Engine::curve`), интерфейс их только рисует:

```rust
pu.line(Line::new("R_WB", pts(&curve.rwb)).color(p.sig).width(2.0));
pu.line(Line::new("R_WA", pts(&curve.rwa)).color(p.clk).width(2.0));
pu.vline(VLine::new("", eff as f64).color(p.accent).style(LineStyle::Dashed { length: 6.0 }));
```

**Схемы** (блок-схема обзора, схема TCON, плата) рисуются вручную через `egui::Painter`. Чтобы переносить координаты из SVG веб-версии без пересчёта, есть помощник `Canvas` (`src/canvas.rs`): он задаёт «viewBox» и переводит его координаты в пиксели окна.

```rust
/// Allocate the full available width; height follows the view box aspect.
pub fn new(ui: &mut Ui, vb: [f32; 4], max_h: f32) -> (Canvas, Response) {
    let w = ui.available_width();
    let mut k = w / vb[2];
    if vb[3] * k > max_h {
        k = max_h / vb[3];
    }
    // …
}

pub fn p(&self, x: f32, y: f32) -> Pos2 {
    self.rect.min + egui::vec2((x - self.vx) * self.k, (y - self.vy) * self.k)
}
```

## Тема и цвета

`src/theme.rs` содержит палитру `Pal` с теми же значениями, что CSS-переменные веб-интерфейса (`bg`, `panel`, `accent`, цвета сигналов `sig`/`clk`/`pwr`…), и переводит её в стиль egui:

```rust
pub fn apply(ctx: &egui::Context, p: &Pal) {
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.override_text_color = Some(p.fg);
    v.slider_trailing_fill = true;      // иначе на тёмном фоне не видно дорожку ползунка
    // …
    // фиксируем тему, иначе eframe переключается вслед за системой
    ctx.set_theme(if p.dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.all_styles_mut(|s| { /* отступы, размеры шрифтов */ });
}
```

## Диалоги файлов и настройки

- **Диалоги** (`rfd`) открываются только в UI-потоке — этого требует macOS. Поэтому экспорт идёт в два шага: поток движка готовит текст (`Job::Export` → `Reply::ExportText`), а окно показывает «Сохранить как»:

  ```rust
  Act::Import => {
      // the dialog stays on the UI thread (macOS requires it)
      let Some(path) = rfd::FileDialog::new().add_filter("Текст", &["txt", "cfg", "csv"]).pick_file()
      else { return; };
      // … прочитать файл и отправить Job::Rpc { cmd: "import_config", … }
  }
  ```

- **Настройки платы** хранятся в том же файле, что у Tauri-приложения, — оба интерфейса видят одну конфигурацию:

  ```rust
  fn config_path() -> Option<PathBuf> {
      const ID: &str = "com.digipotstudio.desktop";
      // macOS: ~/Library/Application Support/com.digipotstudio.desktop/digipot-studio.json
      // Windows: %APPDATA%\com.digipotstudio.desktop\…, Linux: $XDG_CONFIG_HOME/…
  }
  ```

- **Тема и открытый журнал** запоминаются средствами eframe (`persistence`):

  ```rust
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
      storage.set_string("theme", if self.sh.pal.dark { "dark".into() } else { "light".into() });
      storage.set_string("console", if self.sh.console_open { "1".into() } else { "0".into() });
  }
  ```

## Снимки экранов для проверки

Чтобы проверить вёрстку без ручного кликанья, приложение умеет само обойти все экраны (тёмная тема, затем несколько в светлой) и сохранить снимки:

```bash
make egui-shots                    # = DIGIPOT_SHOTS=target/egui-shots cargo run -p digipot-egui
sips -s format png target/egui-shots/01_Pot_dark.bmp --out /tmp/pot.png   # при желании в PNG
```

`src/shots.rs` ждёт 12 кадров (чтобы улеглись раскладка и анимации), просит у egui снимок `ViewportCommand::Screenshot` и пишет его как 24-битный BMP — без дополнительных зависимостей.

## Сборка .app и .dmg

```bash
python3 build_macos.py --egui                  # архитектура этого Mac
python3 build_macos.py --egui --universal      # Apple Silicon + Intel
make build-mac-egui                            # то же самое
```

Node не нужен. Скрипт:

1. проверяет синхронизацию версий (`tools/update_version.py status`);
2. запускает тесты `digipot-core` и `digipot-egui`;
3. собирает `cargo build --release -p digipot-egui` (для universal — две цели и `lipo -create`);
4. сам собирает пакет — у egui нет сборщика, как у Tauri:

   ```text
   DigiPot Studio egui.app/Contents/
     Info.plist          CFBundleIdentifier = com.digipotstudio.egui, версия из config/version.json
     PkgInfo
     MacOS/digipot-egui
     Resources/icon.icns (из src-tauri/icons)
   ```

5. подписывает его ad-hoc: `codesign --force --deep --sign -`;
6. делает `.dmg` через `hdiutil` (внутри приложение и ярлык «Программы») и `.app.zip` через `ditto`;
7. дописывает SHA-256 в `release/vX.Y.Z/checksum.txt`, не стирая строки других сборок.

Отдельный bundle id (`com.digipotstudio.egui`) нужен, чтобы приложение ставилось рядом с Tauri-версией (`com.digipotstudio.desktop`) и macOS их не путала. Подробнее — [BUILD.md](BUILD.md).

## Как добавить новую функцию

1. **Логика** — новая команда в `Engine::handle` (`crates/digipot-core/src/api.rs`) и тест к ней. Физика и правила — только здесь.
2. **Данные для экрана** — если нужно новое поле состояния, добавьте его в `api::State`; Tauri получит его в `get_state` автоматически (не забудьте `src/lib/types.ts`).
3. **Экран egui** — читайте из `sh.st`, вызывайте `sh.rpc("новая_команда", json!({ … }))`. Если ответ команды нужен окну (как текст экспорта), заведите вариант `Job` и `Reply` в `worker.rs`.
4. **Тексты** — русские, названия регистров на английском; если текст есть и в веб-версии, держите `content.rs` синхронным с `blocks.ts` / `board.ts` / `lessons.ts`.
5. **Проверка** — `cargo test -p digipot-egui`, `cargo clippy -p digipot-egui -- -D warnings`, `make egui-shots` и посмотреть снимки.

## Частые ловушки

| Симптом | Причина | Решение в проекте |
|---|---|---|
| Окно подвисает при обмене с мостом | вызов движка в `update` | только через `Job` в поток `digipot-engine` |
| Ручка ползунка прыгает назад после отпускания | состояние ещё не пришло из потока | черновик `PotState::draft`, пока `sh.busy` |
| Текст или фон графиков «не той» темы | eframe следует системной теме | `ctx.set_theme(...)` + `set_visuals_of` для обеих тем |
| Иконка-символ показывается квадратом | глифа нет в шрифтах egui | брать символы, которые есть в `default_fonts` (например, 📈, Ω) |
| Кнопки в строке стоят в обратном порядке | родитель с раскладкой справа налево | `Layout::left_to_right` для группы |
| Правая колонка обрезана | жёсткий `set_max_width` | `ui.available_width().min(1600.0)` |
| Диалог файла падает на macOS | вызван не из главного потока | `rfd` — только в UI-потоке |
| Новая версия eframe не собирается | 0.34+ требует Rust 1.92 | держим eframe 0.33 / egui_plot 0.34 |
