//! Estado compartilhado da aplicação, injetado nos comandos via `tauri::State`.
//!
//! O histórico de conversa vive AQUI, não no frontend. Essa escolha é o que permite
//! o agente e a memória persistente entrarem depois sem mexer no contrato: hoje o
//! histórico é um `Vec` em memória, amanhã é uma tabela no SQLite atrás de `storage`.

use std::sync::Mutex;

use crate::config::AppSettings;
use crate::core::chat::ChatMessage;
use crate::core::lock;
use crate::storage::{SettingsStore, StorageError};

pub struct AppState {
    history: Mutex<Vec<ChatMessage>>,
    settings: Mutex<AppSettings>,
    store: Box<dyn SettingsStore>,
    /// Cliente do intérprete. Fica aqui pelo mesmo motivo que o do TTS fica no
    /// `VoiceState`: o pool de conexões é caro para recriar por chamada, e o timeout
    /// dele é longo de propósito (carregar o modelo na primeira vez leva minutos).
    http: reqwest::Client,
}

impl AppState {
    pub fn new(store: Box<dyn SettingsStore>) -> Self {
        let settings = store.load().unwrap_or_else(|error| {
            eprintln!("[jarvis] não consegui ler as configurações ({error}); usando os padrões");
            AppSettings::default()
        });

        Self {
            history: Mutex::new(Vec::new()),
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

    pub fn history(&self) -> Vec<ChatMessage> {
        lock(&self.history).clone()
    }

    pub fn push_message(&self, message: ChatMessage) {
        lock(&self.history).push(message);
    }

    pub fn clear_history(&self) {
        lock(&self.history).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::chat::Role;

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
                assistant_name: "Sexta-feira".to_owned(),
                ..AppSettings::default()
            })
            .expect("salva");

        let state = AppState::new(Box::new(store));

        assert_eq!(state.settings().assistant_name, "Sexta-feira");
    }

    #[test]
    fn historico_acumula_e_limpa() {
        let state = AppState::new(Box::new(FakeStore::default()));

        state.push_message(ChatMessage::new(Role::User, "oi"));
        state.push_message(ChatMessage::new(Role::Assistant, "olá"));
        assert_eq!(state.history().len(), 2);

        state.clear_history();
        assert!(state.history().is_empty());
    }

    #[test]
    fn salvar_settings_persiste_e_atualiza_a_memoria() {
        let state = AppState::new(Box::new(FakeStore::default()));

        state
            .save_settings(AppSettings {
                anthropic_api_key: "sk-nova".to_owned(),
                assistant_name: "Jarvis".to_owned(),
                ..AppSettings::default()
            })
            .expect("salva");

        assert_eq!(state.settings().anthropic_api_key, "sk-nova");
    }
}
