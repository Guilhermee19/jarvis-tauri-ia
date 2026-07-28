//! Camada de persistência local.
//!
//! O resto do app fala com o trait, nunca com a implementação. É isso que permite
//! trocar o JSON por SQLite (memória entre sessões, histórico de conversas) sem
//! encostar nos comandos: basta uma nova impl de `SettingsStore` — e, na mesma pasta,
//! um `HistoryStore` para as mensagens.

pub mod json_store;

use crate::config::AppSettings;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("falha de I/O ao acessar a config: {0}")]
    Io(#[from] std::io::Error),
    #[error("config em JSON inválido: {0}")]
    Serde(#[from] serde_json::Error),
}

pub trait SettingsStore: Send + Sync {
    fn load(&self) -> Result<AppSettings, StorageError>;
    fn save(&self, settings: &AppSettings) -> Result<(), StorageError>;
}
