//! Configuração do app persistida em disco.
//!
//! Preferências futuras (atalho da wake word, voz do TTS, modelo do Claude) entram
//! aqui como campos novos — `#[serde(default)]` garante que arquivos antigos continuem
//! carregando sem migração.

use serde::{Deserialize, Serialize};

pub const DEFAULT_ASSISTANT_NAME: &str = "Jarvis";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Escolhido medindo, em duas rodadas.
///
/// Primeiro contra o `llama3.2:3b`, para rotear comando: 12 de 12 em português contra
/// 11 (o llama errou "próxima faixa"). Depois, quando a visão entrou, contra o
/// `gemma3:4b` e o `moondream` — e aí o `qwen2.5vl:3b` ganhou de novo, agora nas duas
/// tarefas ao mesmo tempo:
///
/// | modelo         | roteia | enxerga | português | latência da visão |
/// | -------------- | ------ | ------- | --------- | ----------------- |
/// | `qwen2.5:3b`   | 13/15  | não     | sim       | —                 |
/// | `moondream`    | —      | sim     | NÃO       | (troca de modelo) |
/// | `gemma3:4b`    | —      | inventa nome de app | sim | ~17 s      |
/// | `qwen2.5vl:3b` | 15/15  | sim     | sim       | ~2–3,5 s          |
///
/// Ter UM modelo multimodal não é preferência, é necessidade: com 4 GB de VRAM o
/// Ollama não segura dois, e a primeira chamada depois de uma troca levou 67 segundos.
pub const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5vl:3b";

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
    /// Resolução pedida à webcam. `0` em qualquer um dos dois = **automático**, que é
    /// a política antiga: o formato mais perto de 640×480, dimensionado para a janela.
    ///
    /// É um PEDIDO, não uma garantia. A escolha final sai da lista de formatos
    /// compatíveis da própria câmera (`list_webcam_resolutions`), então o valor
    /// salvo aqui só deixa de valer se o dispositivo mudar — outra webcam, ou a
    /// mesma num modo diferente. Nesse caso o mais próximo vence, em vez de falhar.
    pub webcam_width: u32,
    pub webcam_height: u32,
    /// Espelhar a imagem horizontalmente na tela (visão de selfie).
    ///
    /// É preferência de EXIBIÇÃO e por isso mora só na UI: inverter no Rust
    /// obrigaria a decodificar e recodificar cada quadro, matando o passthrough de
    /// MJPEG que é a maior economia do preview. Os bytes continuam na orientação
    /// real — que é o que um modelo tem que receber quando a v0.2 chegar.
    pub webcam_mirror: bool,
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
            webcam_width: 0,
            webcam_height: 0,
            webcam_mirror: false,
        }
    }
}

impl AppSettings {
    /// Resolução pedida, ou `None` para "deixe a política automática escolher".
    ///
    /// Um lado zerado invalida o par inteiro: 1280×0 não é meio pedido, é um pedido
    /// quebrado, e tratar como automático é melhor que procurar o formato mais perto
    /// de zero pixels — que seria o menor de todos.
    pub fn webcam_target(&self) -> Option<(u32, u32)> {
        match (self.webcam_width, self.webcam_height) {
            (0, _) | (_, 0) => None,
            (largura, altura) => Some((largura, altura)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_padrao_deixa_a_webcam_no_automatico() {
        assert_eq!(AppSettings::default().webcam_target(), None);
    }

    #[test]
    fn resolucao_completa_vira_pedido() {
        let settings = AppSettings {
            webcam_width: 1280,
            webcam_height: 720,
            ..AppSettings::default()
        };

        assert_eq!(settings.webcam_target(), Some((1280, 720)));
    }

    /// Um lado zerado é pedido quebrado, não meio pedido: sem isto, o formato mais
    /// perto de zero pixels venceria e a câmera abriria na MENOR resolução que tem.
    #[test]
    fn um_lado_zerado_cai_no_automatico() {
        let so_largura = AppSettings {
            webcam_width: 1280,
            ..AppSettings::default()
        };
        let so_altura = AppSettings {
            webcam_height: 720,
            ..AppSettings::default()
        };

        assert_eq!(so_largura.webcam_target(), None);
        assert_eq!(so_altura.webcam_target(), None);
    }

    /// `#[serde(default)]` é o que faz um settings.json antigo — sem os campos da
    /// webcam — continuar carregando em vez de virar erro no boot.
    #[test]
    fn config_antigo_sem_os_campos_da_webcam_ainda_carrega() {
        let antigo = r#"{"assistantName":"Jarvis","ollamaModel":"qwen2.5vl:3b"}"#;
        let settings: AppSettings = serde_json::from_str(antigo).expect("carrega");

        assert_eq!(settings.assistant_name, "Jarvis");
        assert_eq!(settings.webcam_target(), None);
        assert!(!settings.webcam_mirror);
    }
}
