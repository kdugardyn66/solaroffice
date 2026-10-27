//! SolarOffice core: the module registry, the recent/starred library, settings and persistence.
//! No UI code lives here, so every piece is unit-tested without a window.

pub mod apps;
pub mod fmt;
pub mod folders;
pub mod library;
pub mod settings;

pub use apps::AppKind;

/// The SolarOffice version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Now, in Unix milliseconds.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
