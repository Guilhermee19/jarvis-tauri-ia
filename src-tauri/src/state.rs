//! Estado compartilhado da aplicação, injetado nos comandos via `tauri::State`.
//!
//! Configurações e o cliente HTTP do intérprete. O histórico de conversa saiu daqui
//! e foi para `core::memory`, junto com o resto do que o Jarvis lembra — ele não era
//! estado de aplicação, era memória, e agora persiste em disco.

use std::sync::Mutex;

use crate::config::AppSettings;
use crate::core::lock;
use crate::storage::{SettingsStore, StorageError};

pub struct AppState {
    settings: Mutex<AppSettings>,
    store: Box<dyn SettingsStore>,
    /// Cliente do intérprete. Fica aqui pelo mesmo motivo que o do TTS fica no
    /// `VoiceState`: o pool de conexões é caro para recriar por chamada, e o timeout
    /// dele é longo de propósito (carregar o modelo na primeira vez leva minutos).
    http: reqwest::Client,
}

impl AppState {
    pub fn new(store: Box<dyn SettingsStore>) -> Self {
        let mut settings = store.load().unwrap_or_else(|error| {
            eprintln!("[jarvis] não consegui ler as configurações ({error}); usando os padrões");
            AppSettings::default()
        });

        // Uma correção, não uma preferência: o porquê (com o número) está no
        // `config::endereco_direto`. Aqui e não no `save` porque o arquivo de quem já usa
        // o app tem `localhost` escrito, e ninguém vai abrir as Configurações para trocar
        // um endereço que parece igual.
        settings.ollama_url = crate::config::endereco_direto(&settings.ollama_url);

        Self {
            settings: Mutex::new(settings),
            store,
            http: crate::core::agent::client(),
        }
    }

    pub fn settings(&self) -> AppSettings {
        lock(&self.settings).clone()
    }

    /// O clone compartilha o pool de conexões — é a forma barata prevista pelo reqwest.
    pub fn http(&self) -> reqwest::Client {
        self.http.clone()
    }

    /// Grava em disco ANTES de atualizar a memória: se o disco falhar, o estado
    /// em memória continua batendo com o que está persistido.
    pub fn save_settings(&self, next: AppSettings) -> Result<(), StorageError> {
        self.store.save(&next)?;
        *lock(&self.settings) = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeStore {
        saved: Mutex<Option<AppSettings>>,
    }

    impl SettingsStore for FakeStore {
        fn load(&self) -> Result<AppSettings, StorageError> {
            Ok(lock(&self.saved).clone().unwrap_or_default())
        }

        fn save(&self, settings: &AppSettings) -> Result<(), StorageError> {
            *lock(&self.saved) = Some(settings.clone());
            Ok(())
        }
    }

    #[test]
    fn carrega_as_settings_do_store_ao_iniciar() {
        let store = FakeStore::default();
        store
            .save(&AppSettings {
                anthropic_api_key: "sk-teste".to_owned(),
                persona: crate::config::Persona::Ultron,
                ..AppSettings::default()
            })
            .expect("salva");

        let state = AppState::new(Box::new(store));

        assert_eq!(state.settings().anthropic_api_key, "sk-teste");
        assert_eq!(state.settings().persona, crate::config::Persona::Ultron);
    }

    #[test]
    fn salvar_settings_persiste_e_atualiza_a_memoria() {
        let state = AppState::new(Box::new(FakeStore::default()));

        state
            .save_settings(AppSettings {
                anthropic_api_key: "sk-nova".to_owned(),
                ..AppSettings::default()
            })
            .expect("salva");

        assert_eq!(state.settings().anthropic_api_key, "sk-nova");
    }
}
