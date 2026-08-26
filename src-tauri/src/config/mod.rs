//! Configuração do app persistida em disco.
//!
//! Preferências futuras (atalho da wake word, voz do TTS, modelo do Claude) entram
//! aqui como campos novos — `#[serde(default)]` garante que arquivos antigos continuem
//! carregando sem migração.

use serde::{Deserialize, Serialize};

pub const DEFAULT_ASSISTANT_NAME: &str = "Jarvis";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Escolhido medindo, não por reputação: contra o `llama3.2:3b` ele acertou 12 de 12
/// comandos falados em português (o llama errou "próxima faixa") e ainda devolveu a
/// busca com os acentos corrigidos. Latência com o modelo quente: ~0,4 s.
pub const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5:3b";

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
    /// Onde o Ollama escuta. Local por padrão; o campo existe para apontar para outra
    /// máquina da rede, que é como um notebook fraco usa o desktop de casa.
    pub ollama_url: String,
    /// Modelo que interpreta os comandos. VAZIO DESLIGA o intérprete e volta às
    /// respostas simuladas — é a saída de emergência, no mesmo padrão do
    /// `tts_voice_id`, sem precisar de um booleano só para isso.
    pub ollama_model: String,
    /// Pasta da memória (markdown, formato Obsidian). Vazio = a pasta `memoria/` do
    /// projeto em desenvolvimento, ou a de dados do usuário num app instalado.
    pub memoria_path: String,
    /// Chave do Brave Search (grátis, 2000 buscas/mês). VAZIO usa a Wikipedia, que não
    /// precisa de chave e responde bem "quem foi X" e "o que é Y" — mas não sabe preço,
    /// clima nem notícia. É a chave que transforma isso em busca web de verdade.
    pub brave_api_key: String,
    /// Credenciais do Spotify (*client credentials*, sem OAuth). VAZIAS fazem "toque X"
    /// abrir a busca dentro do app em vez de tocar a faixa — achar o ID exato da música
    /// não tem caminho sem credencial, e isso foi medido.
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            anthropic_api_key: String::new(),
            assistant_name: DEFAULT_ASSISTANT_NAME.to_owned(),
            eleven_labs_api_key: String::new(),
            tts_voice_id: String::new(),
            ollama_url: DEFAULT_OLLAMA_URL.to_owned(),
            ollama_model: DEFAULT_OLLAMA_MODEL.to_owned(),
            memoria_path: String::new(),
            brave_api_key: String::new(),
            spotify_client_id: String::new(),
            spotify_client_secret: String::new(),
        }
    }
}
