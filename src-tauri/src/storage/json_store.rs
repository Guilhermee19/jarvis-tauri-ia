//! Persistência das configurações em um `settings.json` na pasta de config do app
//! (`%APPDATA%\com.jarvis.app` no Windows).

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::AppSettings;
use crate::storage::{SettingsStore, StorageError};

const SETTINGS_FILE: &str = "settings.json";

pub struct JsonSettingsStore {
    path: PathBuf,
}

impl JsonSettingsStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join(SETTINGS_FILE),
        }
    }
}

impl SettingsStore for JsonSettingsStore {
    fn load(&self) -> Result<AppSettings, StorageError> {
        // Primeira execução: sem arquivo, vale o default — não é erro.
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }

        let raw = fs::read_to_string(&self.path)?;

        // O BOM tem que sair ANTES do parse: o `serde_json` recusa com um
        // "expected value at line 1 column 1" que não diz nada sobre bytes invisíveis,
        // e o `AppState` cai nos padrões — perdendo TODAS as chaves de uma vez, em
        // silêncio. Custou uma investigação inteira: a capa do álbum sumiu do widget
        // porque este arquivo tinha três bytes a mais no começo.
        //
        // E o caminho é comum: Bloco de Notas, `Set-Content -Encoding utf8` do
        // PowerShell 5.1 e vários editores gravam UTF-8 COM BOM por padrão. Quem abre
        // o `settings.json` para colar uma chave na mão cai nisso.
        Ok(serde_json::from_str(raw.trim_start_matches('\u{feff}'))?)
    }

    fn save(&self, settings: &AppSettings) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.path, serde_json::to_string_pretty(settings)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jarvis-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// O Bloco de Notas e o `Set-Content -Encoding utf8` do PowerShell 5.1 gravam
    /// UTF-8 COM BOM. Sem tolerar isso, editar o arquivo à mão apaga silenciosamente
    /// todas as chaves — o app volta aos padrões e não reclama em lugar nenhum que o
    /// usuário veja.
    #[test]
    fn arquivo_com_bom_ainda_carrega() {
        let dir = temp_dir("bom");
        fs::create_dir_all(&dir).expect("cria a pasta");

        let com_bom = format!(
            "\u{feff}{}",
            r#"{"spotifyClientId":"abc","assistantName":"Sexta-feira"}"#
        );
        fs::write(dir.join(SETTINGS_FILE), com_bom).expect("escreve");

        let carregado = JsonSettingsStore::new(&dir)
            .load()
            .expect("carrega mesmo com BOM");

        assert_eq!(carregado.spotify_client_id, "abc");
        assert_eq!(carregado.assistant_name, "Sexta-feira");
    }

    #[test]
    fn sem_arquivo_devolve_os_padroes() {
        let store = JsonSettingsStore::new(&temp_dir("vazio"));

        assert_eq!(store.load().expect("carrega").assistant_name, "Jarvis");
    }

    #[test]
    fn ida_e_volta_pelo_disco() {
        let store = JsonSettingsStore::new(&temp_dir("roundtrip"));
        let settings = AppSettings {
            anthropic_api_key: "sk-disco".to_owned(),
            assistant_name: "Sexta-feira".to_owned(),
            persona: crate::config::Persona::Ultron,
            ..AppSettings::default()
        };

        // `save` cria a pasta de config: na primeira execução ela não existe.
        store.save(&settings).expect("salva");
        let loaded = store.load().expect("carrega");

        assert_eq!(loaded.anthropic_api_key, "sk-disco");
        // Nome e tema são campos independentes: o tema sugere o nome ao trocar, mas não
        // o sobrescreve. Um Ultron chamado "Sexta-feira" tem que sobreviver ao disco.
        assert_eq!(loaded.assistant_name, "Sexta-feira");
        assert_eq!(loaded.persona, crate::config::Persona::Ultron);
    }

    /// `#[serde(default)]` é o que evita migração quando um campo novo entra.
    #[test]
    fn arquivo_com_campo_faltando_usa_o_padrao() {
        let dir = temp_dir("parcial");
        fs::create_dir_all(&dir).expect("cria pasta");
        fs::write(
            dir.join(SETTINGS_FILE),
            r#"{"anthropicApiKey":"sk-parcial"}"#,
        )
        .expect("escreve");

        let loaded = JsonSettingsStore::new(&dir).load().expect("carrega");

        assert_eq!(loaded.anthropic_api_key, "sk-parcial");
        assert_eq!(loaded.assistant_name, "Jarvis");
    }
}
