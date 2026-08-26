//! Serviços locais que o Jarvis usa e, se preciso, sobe sozinho.
//!
//! São dois processos fora do app: o **Ollama** (que interpreta os comandos) e o
//! **whisper-server** (que transcreve a fala). O Ollama se instala como serviço do
//! Windows e normalmente já está de pé; o Whisper é só um `.exe` numa pasta, e sem
//! isto aqui o usuário teria que abrir um terminal antes de falar com o assistente.
//!
//! O ciclo é sempre o mesmo: bate na porta, e só sobe o processo se ninguém atender.
//! Isso torna a operação idempotente e faz o app conviver com um servidor que o
//! usuário já tinha aberto na mão.
//!
//! A subida é **preguiçosa** de propósito — acontece na primeira transcrição, não no
//! `setup` do app. Quem nunca usa a voz não paga nada, e não existe um caminho de
//! falha no boot por causa de um arquivo que talvez nem tenha sido baixado.

use std::path::Path;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

use crate::core::lock;

/// Sem isto, uma janela preta de console pisca na tela toda vez que um serviço sobe.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Porta do whisper-server. Alta e sem uso conhecido — o 8080 do padrão dele colide
/// com metade dos projetos web que alguém possa ter rodando.
const PORTA_WHISPER: u16 = 8642;

/// Nome do modelo. Trocar por `ggml-large-v3-turbo-q5_0.bin` é a única mudança
/// necessária para subir a qualidade, ao custo de latência.
const MODELO_WHISPER: &str = "ggml-small-q5_1.bin";

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(
        "não achei o Whisper em {0}.\nBaixe whisper-blas-bin-x64.zip de \
         github.com/ggml-org/whisper.cpp/releases e o modelo {1} de \
         huggingface.co/ggerganov/whisper.cpp, e descompacte os dois nessa pasta."
    )]
    WhisperAusente(String, &'static str),
    #[error("não consegui iniciar o {servico}: {detalhe}")]
    NaoSubiu { servico: String, detalhe: String },
    #[error("o {0} subiu mas não respondeu a tempo — veja se outro programa está usando a porta")]
    NaoRespondeu(String),
}

pub fn whisper_url() -> String {
    format!("http://127.0.0.1:{PORTA_WHISPER}")
}

/// Dono dos processos que o app subiu, para poder derrubá-los ao sair.
///
/// Só os que ELE subiu: se o usuário já tinha um whisper-server aberto, `ensure` não
/// spawna nada e o `shutdown` não tem o que matar — não é o app que derruba servidor
/// dos outros.
#[derive(Default)]
pub struct Services {
    filhos: Mutex<Vec<Child>>,
}

impl Services {
    pub fn new() -> Self {
        Self::default()
    }

    /// Garante que existe um whisper-server atendendo, e devolve a URL dele.
    pub async fn ensure_whisper(
        &self,
        http: &reqwest::Client,
        data_dir: &Path,
    ) -> Result<String, ServiceError> {
        let url = whisper_url();
        if responde(http, &url).await {
            return Ok(url);
        }

        let pasta = data_dir.join("whisper");
        let exe = pasta.join("whisper-server.exe");
        let modelo = pasta.join(MODELO_WHISPER);

        if !exe.is_file() || !modelo.is_file() {
            return Err(ServiceError::WhisperAusente(
                pasta.display().to_string(),
                MODELO_WHISPER,
            ));
        }

        let mut comando = Command::new(&exe);
        comando
            .arg("-m")
            .arg(&modelo)
            // Fixar o idioma: sem isto o Whisper detecta pelos primeiros segundos e,
            // em comando curto, às vezes chuta espanhol.
            .args(["-l", "pt"])
            .args(["-t", "4"])
            .args(["--host", "127.0.0.1"])
            .args(["--port", &PORTA_WHISPER.to_string()])
            // As DLLs (ggml, openblas) ficam ao lado do exe.
            .current_dir(&pasta);

        self.spawn("whisper-server", comando)?;

        // Carregar o modelo leva alguns segundos na primeira vez.
        esperar(http, &url, Duration::from_secs(60))
            .await
            .map_err(|()| ServiceError::NaoRespondeu("whisper-server".to_owned()))?;

        Ok(url)
    }

    /// Garante que o Ollama atende. Ele quase sempre já está de pé — instalar o
    /// Ollama no Windows deixa um serviço rodando — então isto é a rede de segurança
    /// para quem o desligou.
    pub async fn ensure_ollama(&self, http: &reqwest::Client, url: &str) -> bool {
        if responde(http, url).await {
            return true;
        }

        // Pelo nome: quem instalou o Ollama tem ele no PATH. Se não tiver, não há o
        // que fazer aqui — o erro bom vem do `AgentError::Offline`, que já diz onde
        // baixar e qual comando rodar.
        let mut comando = Command::new("ollama");
        comando.arg("serve");

        if self.spawn("ollama", comando).is_err() {
            return false;
        }

        esperar(http, url, Duration::from_secs(30)).await.is_ok()
    }

    fn spawn(&self, servico: &str, mut comando: Command) -> Result<(), ServiceError> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            comando.creation_flags(CREATE_NO_WINDOW);
        }

        let filho = comando.spawn().map_err(|error| ServiceError::NaoSubiu {
            servico: servico.to_owned(),
            detalhe: error.to_string(),
        })?;

        lock(&self.filhos).push(filho);
        Ok(())
    }

    /// Derruba o que subimos. Chamado quando o app encerra de verdade.
    ///
    /// ponytail: processo pode ficar órfão se o app morrer de forma anormal (crash,
    /// Gerenciador de Tarefas). O jeito à prova disso no Windows é um Job Object com
    /// `KILL_ON_JOB_CLOSE` — vale a pena se aparecer whisper-server sobrando.
    pub fn shutdown(&self) {
        for mut filho in lock(&self.filhos).drain(..) {
            let _ = filho.kill();
            let _ = filho.wait();
        }
    }
}

/// Qualquer resposta HTTP conta como vivo, inclusive 404: a pergunta é "tem alguém
/// atendendo nessa porta", não "essa rota existe".
async fn responde(http: &reqwest::Client, url: &str) -> bool {
    http.get(url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

async fn esperar(http: &reqwest::Client, url: &str, limite: Duration) -> Result<(), ()> {
    let comeco = std::time::Instant::now();

    while comeco.elapsed() < limite {
        if responde(http, url).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    Err(())
}
