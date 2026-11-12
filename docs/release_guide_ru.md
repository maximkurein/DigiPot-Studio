# Выпуск релиза DigiPot Studio

Связанное: [VERSION_MANAGEMENT.md](../VERSION_MANAGEMENT.md) · [BUILD.md](../BUILD.md) · [CHANGELOG.md](../CHANGELOG.md)

## Чек-лист

1. **Версия**
   ```bash
   python3 tools/update_version.py bump patch     # или minor / major
   ```
2. **CHANGELOG.md** — `## [Unreleased]` → `## [X.Y.Z] - YYYY-MM-DD`, добавить новый пустой `## [Unreleased]` сверху.
3. **Проверка**
   ```bash
   python3 tools/update_version.py status         # все строки OK
   make check
   ```
4. **Ручная проверка** — `npm run app` и `make egui`, в симуляторе: «Потенциометр» (ползунок, шаги ±1/±10, графики R_WB/R_WA и V_W), «EEPROM» (запись с подтверждением → «Выключить и включить питание» → движок берётся из NV_WIPER0), «Регистры» (прочитать всё → readback ✓), журнал шины (⌘/Ctrl + J), экспорт/импорт. С платой — то же через USB-мост, R_WB мультиметром. Снимки egui — `make egui-shots`.
5. **Коммит**
   ```bash
   git commit -am "Краткое описание для пользователя; bump to X.Y.Z"
   git push
   ```
6. **Сборка**
   - локально: `python3 build_macos.py --universal --with-server` (Mac) и `.\build_windows.ps1 -WithServer` (Windows) → `release/vX.Y.Z/` + `checksum.txt`;
   - нативный интерфейс: `python3 build_macos.py --egui --universal` — только локально, workflow **Release** его не собирает; файлы приложите к черновику релиза вручную, `checksum.txt` дополняется;
   - или в GitHub: `git tag vX.Y.Z && git push origin vX.Y.Z` → workflow **Release** соберёт `.dmg` (universal), `.exe`, `.msi` и создаст черновик релиза.
7. **Публикация** — проверьте черновик релиза, вставьте раздел из CHANGELOG и контрольные суммы, опубликуйте.

## Именование артефактов

| Платформа | Файл |
|---|---|
| macOS | `DigiPot-Studio-vX.Y.Z-macos-<arm64\|x64\|universal>.dmg`, `….app.zip` |
| macOS, egui | `DigiPot-Studio-egui-vX.Y.Z-macos-<arm64\|x64\|universal>.dmg`, `….app.zip` |
| Windows | `DigiPot-Studio-Setup-vX.Y.Z-x64.exe`, `DigiPot-Studio-vX.Y.Z-x64.msi` |
| Сервер | `digipot-server-vX.Y.Z-<платформа>` |
| Контрольные суммы | `checksum.txt` (SHA-256) |
