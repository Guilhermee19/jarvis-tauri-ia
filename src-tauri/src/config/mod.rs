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
    /// Mesma decisão da key da Anthropic: texto puro por enquanto.
    pub eleven_labs_api_key: String,
    /// Voz do TTS. Vazio = usa a primeira voz da conta, para o botão de teste
    /// funcionar assim que a key é colada, sem passo extra de configuração.
    pub tts_voice_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            anthropic_api_key: String::new(),
            assistant_name: DEFAULT_ASSISTANT_NAME.to_owned(),
            eleven_labs_api_key: String::new(),
            tts_voice_id: String::new(),
        }
    }
}
