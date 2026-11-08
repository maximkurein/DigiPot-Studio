//! Сведения о программе для экрана «О программе» и заголовков.
//!
//! Значения синхронизируются из `config/version.json`:
//! `python3 tools/update_version.py sync` — вручную файл не правят.

/// Название приложения.
pub const NAME: &str = "DigiPot Studio";
/// Версия приложения (совпадает с `CARGO_PKG_VERSION`, см. тест ниже).
pub const VERSION: &str = "0.1.1";
/// Дата выпуска, DD.MM.YYYY.
pub const RELEASE_DATE: &str = "08.11.2026";
/// Разработчик.
pub const AUTHOR: &str = "Kurein M N";
/// Лицензия исходного кода.
pub const LICENSE: &str = "MIT";
/// Основной репозиторий проекта.
pub const REPOSITORY: &str = "https://github.com/maximkurein/DigiPot-Studio";
/// Зеркало репозитория (Forgejo, локальная сеть).
pub const MIRROR: &str = "http://100.64.0.12:3000/mxm/DigiPot-Studio";

/// «DigiPot Studio v0.1.1 · Kurein M N» — для журналов и заголовков окон.
pub fn title() -> String {
    format!("{NAME} v{VERSION} · {AUTHOR}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_cargo() {
        // Рассинхрон = забыли `python3 tools/update_version.py sync`.
        assert_eq!(super::VERSION, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn title_has_author() {
        assert!(super::title().contains(super::AUTHOR));
    }
}
