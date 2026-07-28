//! Configuração do app persistida em disco.
//!
//! Preferências futuras (atalho da wake word, voz do TTS, modelo do Claude) entram
//! aqui como campos novos — `#[serde(default)]` garante que arquivos antigos continuem
//! carregando sem migração.

use serde::{Deserialize, Serialize};

pub const DEFAULT_ASSISTANT_NAME: &str = "Jarvis";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Guardada em texto puro por enquanto. Migrar para o keyring do SO quando a
    /// integração real com a Anthropic entrar.
    pub anthropic_api_key: String,
    /// Usado na UI hoje; vira parte do system prompt quando o agente entrar.
    pub assistant_name: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            anthropic_api_key: String::new(),
            assistant_name: DEFAULT_ASSISTANT_NAME.to_owned(),
        }
    }
}
