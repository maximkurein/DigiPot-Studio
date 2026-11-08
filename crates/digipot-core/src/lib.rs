//! DigiPot Studio core: everything about the Microchip MCP4161 that does not
//! depend on a UI. Both the desktop app (Tauri) and the web server use it
//! through `Engine::handle(cmd, args)`.

pub mod about;
pub mod api;
pub mod bus;
pub mod model;
pub mod regs;
pub mod sim;
pub mod transport;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Link(String),
    #[error("{0}")]
    Invalid(String),
    #[error("неизвестная команда: {0}")]
    UnknownCommand(String),
    #[error("аргументы команды: {0}")]
    Args(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub use api::Engine;
