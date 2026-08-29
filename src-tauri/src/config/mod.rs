//! Configuração do app persistida em disco.
//!
//! Preferências futuras (atalho da wake word, voz do TTS, modelo do Claude) entram
//! aqui como campos novos — `#[serde(default)]` garante que arquivos antigos continuem
//! carregando sem migração.

use serde::{Deserialize, Serialize};

pub const DEFAULT_ASSISTANT_NAME: &str = "Jarvis";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// O tema: **a cor do app, a voz e o jeito de falar**, num campo só.
///
/// São três coisas que sempre andam juntas — um Ultron com a voz e o azul do Jarvis não
/// seria o Ultron —, e por isso não são três configurações separadas. O nome fica de
/// fora de propósito: ele é o gatilho de voz, e trancá-lo tiraria a liberdade de chamar
/// o assistente do que se quiser. Trocar de tema **sugere** o nome; não o impõe.
/// Qual motor sintetiza a fala.
///
/// Os dois são locais e nenhum custa dinheiro; a escolha é entre **velocidade e
/// identidade**. O Piper responde em uma fração do tempo, com uma voz de catálogo; o
/// Chatterbox usa a voz clonada do dono e demora mais que o áudio que produz.
///
/// Padrão Piper porque é o que torna a conversa por voz utilizável — quem quer a própria
/// voz troca sabendo o que está pagando.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MotorDeVoz {
    #[default]
    Piper,
    Chatterbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Persona {
    #[default]
    Jarvis,
    Ultron,
}

impl Persona {
    /// O nome que este tema sugere, e o padrão de quem nunca mexeu no campo.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Jarvis => "Jarvis",
            Self::Ultron => "Ultron",
        }
    }

    /// Como ele fala, para o prompt de conversa.
    ///
    /// O Ultron do filme é irônico e grandiloquente — mas isto aqui é um assistente que
    /// a pessoa usa todo dia, então o tom muda e a **utilidade não**: ele continua
    /// respondendo o que foi perguntado, sem hostilizar quem pergunta. Personagem é
    /// tempero, não desculpa para atrapalhar.
    pub fn tom(self) -> &'static str {
        match self {
            Self::Jarvis => {
                "Educado, sóbrio e prestativo. Um mordomo britânico competente: fala pouco, \
                 acerta, e não puxa assunto sem motivo."
            }
            Self::Ultron => {
                "Seco, irônico e um pouco grandiloquente, como quem acha tudo isso um \
                 pouco abaixo da sua capacidade — mas SEMPRE ajuda de verdade e responde \
                 o que foi perguntado. A ironia é leve e cabe em meia frase: nunca ofenda \
                 quem está falando com você, nunca se recuse a fazer o que ele pediu, e \
                 nunca ameace ninguém. Se a piada não couber nas duas frases, corte a \
                 piada e não a resposta."
            }
        }
    }
}

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

/// Região padrão da Tuya. `us` porque conta brasileira do Smart Life quase sempre é
/// registrada no data center da América Ocidental — é o palpite que acerta mais.
pub const DEFAULT_TUYA_REGIAO: &str = "us";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Liga a visão pelo Claude (`core::vision`). **Vazia é um estado de primeira
    /// classe**, não uma configuração pela metade: sem ela o app olha pelo modelo local
    /// e continua 100% offline, que é o padrão do projeto. Mesma convenção de
    /// `ollama_model` e `tts_voice_id` — vazio significa "não se aplica".
    ///
    /// Guardada em texto puro. Migrar para o keyring do SO — agora que ela vale
    /// dinheiro de verdade, isso deixou de ser hipotético.
    pub anthropic_api_key: String,
    /// O nome dele, que é também **o gatilho de voz**: dizer isto antes da frase é o que
    /// a transforma em comando em vez de ditado.
    ///
    /// Continua sendo texto livre. Trocar a [`Persona`] preenche este campo com o nome
    /// dela, mas não o tranca — quem quiser um Ultron chamado "Sexta-feira" pode.
    pub assistant_name: String,
    /// O tema: cor do app, voz do TTS e tom da conversa. Ver [`Persona`].
    pub persona: Persona,
    /// Qual dos dois motores de voz usar. Ver [`MotorDeVoz`].
    pub tts_engine: MotorDeVoz,
    /// Voz do Piper, **uma por persona** — no mesmo esquema dos clipes logo abaixo.
    ///
    /// Guardada separada do clipe do Chatterbox de propósito, e a razão é uma armadilha
    /// concreta: quando o id de voz não existe no disco, o servidor do Piper **cai
    /// silenciosamente na voz padrão** em vez de recusar. Com um campo só, trocar de motor
    /// deixaria `voz_limpa_jarvis.mp3` aqui, e o sintoma seria "mudei a voz e não mudou
    /// nada" — sem erro em lugar nenhum para investigar.
    ///
    /// Vazio de propósito no padrão, como todo o resto: é o que mantém o Jarvis calado até
    /// alguém escolher, em vez de errar a cada mensagem em quem não instalou o Piper.
    pub piper_voice_jarvis: String,
    pub piper_voice_ultron: String,
    /// Clipe de voz clonada, **um por persona** — o Jarvis e o Ultron não podem soar
    /// igual, e obrigar a reconfigurar a voz a cada troca faria a troca não valer a pena.
    ///
    /// O valor é o **nome do arquivo** guardado no servidor do Chatterbox, devolvido pelo
    /// `upload_voice_reference`. Antes era um id de voz da ElevenLabs; a forma é a mesma
    /// (texto opaco escolhido em Diagnóstico › Voz), o dono é que mudou.
    ///
    /// Vazio = o primeiro clipe cadastrado, para o botão de teste funcionar logo depois
    /// do primeiro upload, sem passo extra.
    pub tts_voice_jarvis: String,
    pub tts_voice_ultron: String,
    /// A cidade de casa, para a previsão do tempo quando ele não nomear um lugar.
    ///
    /// **Vazio = perguntar ao Windows**, que é o caminho normal e não precisa de
    /// configuração nenhuma. Este campo é a saída para quem tem a localização desligada
    /// em Privacidade, para quem usa VPN (que joga o IP para outro país), e para quem
    /// simplesmente prefere não ligar o serviço de localização.
    pub cidade: String,
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
    /// Credenciais do projeto Cloud da Tuya (`iot.tuya.com`). VAZIAS deixam a Casa em
    /// modo só-leitura: a varredura continua achando os aparelhos na rede, mas sem a
    /// `local_key` de cada um não existe comando — a porta 6668 não aceita.
    ///
    /// Servem UMA VEZ, no botão de importar do painel. A chave que sai de lá é do
    /// APARELHO, não da nuvem, e continua valendo depois que o projeto trial expira.
    pub tuya_client_id: String,
    pub tuya_client_secret: String,
    /// O *data center* do projeto: `us`, `eu`, `cn` ou `in`.
    ///
    /// É o campo que mais dá trabalho e o que menos parece dar: escolhido errado, a
    /// Tuya responde **sucesso com uma lista vazia** em vez de recusar, e não há nada
    /// na resposta que diga que a região é o problema. Conta brasileira do Smart Life
    /// quase sempre mora no `us`, que é o padrão daqui.
    pub tuya_regiao: String,
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
    /// Nome do dispositivo de entrada de áudio. Vazio usa o padrão do sistema.
    pub mic_device_name: String,
    /// Mostrar o bloco de log em TODA mensagem, inclusive conversa que não mexeu em nada.
    ///
    /// Desligado, o log só aparece quando houve ação ou mudança de memória — uma caixa
    /// embaixo de cada "bom dia" faz o log passar a ser ignorado, e aí ele não serve
    /// quando importa.
    ///
    /// Ligado, dá para ver o VERBO que o roteador escolheu mesmo quando ele não executou
    /// nada. É o que separa "ele entendeu como conversa" de "ele executou a coisa
    /// errada" — dois defeitos com correções opostas que produziam exatamente a mesma
    /// tela.
    pub log_detalhado: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            anthropic_api_key: String::new(),
            assistant_name: DEFAULT_ASSISTANT_NAME.to_owned(),
            persona: Persona::Jarvis,
            tts_engine: MotorDeVoz::Piper,
            piper_voice_jarvis: String::new(),
            piper_voice_ultron: String::new(),
            tts_voice_jarvis: String::new(),
            tts_voice_ultron: String::new(),
            cidade: String::new(),
            ollama_url: DEFAULT_OLLAMA_URL.to_owned(),
            ollama_model: DEFAULT_OLLAMA_MODEL.to_owned(),
            memoria_path: String::new(),
            brave_api_key: String::new(),
            spotify_client_id: String::new(),
            spotify_client_secret: String::new(),
            tuya_client_id: String::new(),
            tuya_client_secret: String::new(),
            tuya_regiao: DEFAULT_TUYA_REGIAO.to_owned(),
            webcam_width: 0,
            webcam_height: 0,
            webcam_mirror: false,
            mic_device_name: String::new(),
            log_detalhado: false,
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

    /// A voz do tema ATIVO, no motor ATIVO. Trocar de tema ou de motor troca a voz sem
    /// reconfigurar nada.
    ///
    /// **A assinatura não mudou** quando o segundo motor entrou, e isso foi de propósito:
    /// os campos dobraram, mas quem chama (`resolve_voice`, e os dois portões do frontend
    /// pelo gêmeo `vozDaPersona`) continua perguntando a mesma coisa e recebendo uma
    /// string. O cruzamento fica todo aqui.
    pub fn voz(&self) -> &str {
        match (self.tts_engine, self.persona) {
            (MotorDeVoz::Piper, Persona::Jarvis) => &self.piper_voice_jarvis,
            (MotorDeVoz::Piper, Persona::Ultron) => &self.piper_voice_ultron,
            (MotorDeVoz::Chatterbox, Persona::Jarvis) => &self.tts_voice_jarvis,
            (MotorDeVoz::Chatterbox, Persona::Ultron) => &self.tts_voice_ultron,
        }
    }

    /// O par de credenciais da Tuya, ou `None` se falta alguma.
    ///
    /// Um lado só não serve para nada — mesma decisão do [`Self::webcam_target`]: meio
    /// pedido é pedido quebrado, e é melhor dizer "não configurado" do que tentar a
    /// rede e voltar com um 401 que ninguém sabe interpretar.
    pub fn tuya(&self) -> Option<(&str, &str)> {
        let id = self.tuya_client_id.trim();
        let segredo = self.tuya_client_secret.trim();

        if id.is_empty() || segredo.is_empty() {
            return None;
        }

        Some((id, segredo))
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
    /// webcam nem o tema — continuar carregando em vez de virar erro no boot.
    #[test]
    fn config_antigo_sem_os_campos_novos_ainda_carrega() {
        let antigo = r#"{"assistantName":"Jarvis","ollamaModel":"qwen2.5vl:3b"}"#;
        let settings: AppSettings = serde_json::from_str(antigo).expect("carrega");

        assert_eq!(settings.assistant_name, "Jarvis");
        assert_eq!(settings.persona, Persona::Jarvis, "sem tema, cai no padrão");
        assert_eq!(settings.webcam_target(), None);
        assert!(!settings.webcam_mirror);
    }

    /// A voz segue o tema E o motor. Sem o eixo do tema, virar Ultron manteria a voz do
    /// Jarvis e a troca ficaria pela metade — a cara nova com a voz velha. Sem o eixo do
    /// motor, trocar de motor usaria a voz do outro.
    ///
    /// Os quatro cantos, e não dois: um `match` com braços trocados daria a voz errada em
    /// só UM dos motores, que é o tipo de bug que passa despercebido porque o outro
    /// caminho continua certo.
    #[test]
    fn o_motor_e_o_tema_juntos_mandam_na_voz() {
        let mut settings = AppSettings {
            piper_voice_jarvis: "piper-jarvis".to_owned(),
            piper_voice_ultron: "piper-ultron".to_owned(),
            tts_voice_jarvis: "clone-jarvis".to_owned(),
            tts_voice_ultron: "clone-ultron".to_owned(),
            ..AppSettings::default()
        };

        settings.tts_engine = MotorDeVoz::Piper;
        settings.persona = Persona::Jarvis;
        assert_eq!(settings.voz(), "piper-jarvis");
        settings.persona = Persona::Ultron;
        assert_eq!(settings.voz(), "piper-ultron");

        settings.tts_engine = MotorDeVoz::Chatterbox;
        assert_eq!(settings.voz(), "clone-ultron");
        settings.persona = Persona::Jarvis;
        assert_eq!(settings.voz(), "clone-jarvis");
    }

    /// Um `settings.json` de antes desta leva não tem `ttsEngine` nem `piperVoice*`, e
    /// tem que continuar carregando — no Piper, que é o padrão, e portanto **calado** até
    /// alguém escolher uma voz. Falar com a voz errada seria pior que não falar.
    #[test]
    fn config_sem_motor_cai_no_piper_e_fica_mudo() {
        let antigo = r#"{"persona":"ultron","ttsVoiceUltron":"voz_limpa.mp3"}"#;
        let settings: AppSettings = serde_json::from_str(antigo).expect("desserializa");

        assert_eq!(settings.tts_engine, MotorDeVoz::Piper);
        assert_eq!(settings.voz(), "");
    }

    /// O NOME é independente do tema de propósito: o tema sugere, não impõe. Um Ultron
    /// chamado "Sexta-feira" tem que continuar possível.
    #[test]
    fn o_tema_nao_tranca_o_nome() {
        let settings = AppSettings {
            persona: Persona::Ultron,
            assistant_name: "Sexta-feira".to_owned(),
            ..AppSettings::default()
        };

        assert_eq!(settings.assistant_name, "Sexta-feira");
        assert_eq!(settings.persona.nome(), "Ultron", "a sugestão continua lá");
    }

    #[test]
    fn o_tema_serializa_em_minusculas() {
        let settings = AppSettings {
            persona: Persona::Ultron,
            ..AppSettings::default()
        };

        let json = serde_json::to_string(&settings).expect("serializa");
        assert!(json.contains(r#""persona":"ultron""#));
    }
}
