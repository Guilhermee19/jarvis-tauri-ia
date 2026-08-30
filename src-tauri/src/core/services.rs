//! Serviços locais que o Jarvis usa e, se preciso, sobe sozinho.
//!
//! São até quatro processos fora do app: o **Ollama** (que interpreta os comandos), o
//! **whisper-server** (que transcreve a fala) e **dois motores de voz** — o Piper (rápido,
//! voz de catálogo) e o Chatterbox (lento, clona a voz do dono). Só um dos dois de voz sobe:
//! quem manda é a configuração. O Ollama se instala como serviço do Windows e normalmente
//! já está de pé; os outros são programa numa pasta, e sem isto aqui o usuário teria que
//! abrir um terminal antes de falar com o assistente.
//!
//! Os três juntos são o motivo de o Jarvis não depender de nenhuma API paga para ouvir,
//! pensar e responder: tudo acontece nesta máquina.
//!
//! O ciclo é sempre o mesmo: bate na porta, e só sobe o processo se ninguém atender.
//! Isso torna a operação idempotente e faz o app conviver com um servidor que o
//! usuário já tinha aberto na mão.
//!
//! A subida é **preguiçosa** de propósito — acontece na primeira transcrição, não no
//! `setup` do app. Quem nunca usa a voz não paga nada, e não existe um caminho de
//! falha no boot por causa de um arquivo que talvez nem tenha sido baixado.

use std::fs::File;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::core::lock;

/// Sem isto, uma janela preta de console pisca na tela toda vez que um serviço sobe.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Onde a saída de cada serviço é despejada, dentro da pasta dele.
///
/// Um por serviço, sobrescrito a cada subida: é material de diagnóstico da vez, não
/// histórico — e o que interessa é sempre por que ele não subiu AGORA.
const ARQUIVO_DE_LOG: &str = "servico.log";

/// Porta do whisper-server. Alta e sem uso conhecido — o 8080 do padrão dele colide
/// com metade dos projetos web que alguém possa ter rodando.
const PORTA_WHISPER: u16 = 8642;

/// Nome do modelo. Trocar por `ggml-large-v3-turbo-q5_0.bin` é a única mudança
/// necessária para subir a qualidade, ao custo de latência.
const MODELO_WHISPER: &str = "ggml-small-q5_1.bin";

/// Porta do servidor do Chatterbox — a padrão dele, a mesma que está no `config.yaml`
/// que acompanha o projeto. Mudar aqui exige mudar lá também.
const PORTA_CHATTERBOX: u16 = 8004;

/// O que o `ensure_chatterbox` procura na pasta para decidir se está instalado.
///
/// O `server.py` sozinho não bastaria: ele existe assim que o repositório é clonado, e o
/// que demora (e o que costuma faltar) é o ambiente Python com o torch dentro.
///
/// **`venv`, sem ponto** — é o nome que o `start.py` do servidor usa (`VENV_FOLDER`), e
/// escrever `.venv` aqui faria o app dizer "não achei o servidor de voz" numa instalação
/// perfeitamente correta.
const PYTHON_DO_CHATTERBOX: &str = "venv/Scripts/python.exe";

/// O carimbo que o instalador do servidor deixa quando termina — é ELE que diz "está
/// pronto", e não a existência do interpretador.
///
/// A diferença não é teórica: instalar leva vários minutos, e durante todos eles o
/// `python.exe` já existe com o ambiente pela metade. Sem este carimbo, uma tentativa de
/// falar no meio da instalação sobe um servidor que morre no primeiro `import`.
const CARIMBO_DE_INSTALACAO: &str = "venv/.install_complete";

/// Porta do Piper. Vizinha da do Whisper, e **não** a 5000 que ele usa por padrão — essa
/// colide com metade dos projetos web que alguém possa ter rodando.
const PORTA_PIPER: u16 = 8645;

/// O Python do ambiente do Piper. Aqui é `venv` porque é o nome que o passo de instalação
/// do README cria — não há instalador próprio ditando o nome, como no Chatterbox.
const PYTHON_DO_PIPER: &str = "venv/Scripts/python.exe";

/// As quatro vozes brasileiras do catálogo do Piper.
///
/// Mora aqui, e não no frontend, porque é a mesma lista que a mensagem de erro de
/// instalação precisa citar — duas cópias divergiriam no dia em que uma quinta voz
/// aparecesse.
///
/// **Repare no `edresson`:** ele é `low`, e os outros três são `medium`. Deduzir o
/// sufixo daria um id que não existe, e o servidor do Piper responde a voz inexistente
/// caindo em SILÊNCIO na voz padrão — o sintoma seria "escolhi o edresson e saiu o faber".
pub const VOZES_PIPER: [&str; 4] = [
    "pt_BR-cadu-medium",
    "pt_BR-edresson-low",
    "pt_BR-faber-medium",
    "pt_BR-jeff-medium",
];

/// A voz que sobe com o servidor. Ele carrega as outras sob demanda, então esta é só a
/// primeira a ficar quente — não uma trava.
const VOZ_INICIAL_DO_PIPER: &str = "pt_BR-faber-medium";

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(
        "não achei o Whisper em {0}.\nBaixe whisper-blas-bin-x64.zip de \
         github.com/ggml-org/whisper.cpp/releases e o modelo {1} de \
         huggingface.co/ggerganov/whisper.cpp, e descompacte os dois nessa pasta."
    )]
    WhisperAusente(String, &'static str),
    #[error(
        "não achei o servidor de voz em {0}.\nClone github.com/devnen/Chatterbox-TTS-Server \
         nessa pasta e rode `start.bat` UMA vez à mão: ele monta o ambiente Python (que \
         exige a versão 3.10) e baixa alguns GB do modelo. Essa primeira vez é decisão do \
         dono da máquina, não do app."
    )]
    ChatterboxAusente(String),
    #[error("não consegui iniciar o {servico}: {detalhe}")]
    NaoSubiu { servico: String, detalhe: String },
    #[error(
        "não achei o Piper em {0}.\nCrie o ambiente e baixe as vozes:\n  \
         py -3.10 -m venv <pasta>\\venv\n  \
         <pasta>\\venv\\Scripts\\python -m pip install \"piper-tts[http]\"\n  \
         <pasta>\\venv\\Scripts\\python -m piper.download_voices --data-dir <pasta> \
         pt_BR-cadu-medium pt_BR-edresson-low pt_BR-faber-medium pt_BR-jeff-medium"
    )]
    PiperAusente(String),
    #[error(
        "o servidor de voz ainda está sendo instalado em {0}.\nDeixe o `start.bat` \
         terminar — ele baixa alguns GB e leva vários minutos. A pasta já existe, mas o \
         ambiente Python está pela metade."
    )]
    ChatterboxIncompleto(String),
    #[error(
        "não achei o go2rtc em {0}.\nBaixe o `go2rtc_win64.zip` de \
         github.com/AlexxIT/go2rtc/releases e descompacte o executável nessa pasta. \
         Ele é um arquivo só, sem instalador — é o que traduz o vídeo das câmeras \
         para um formato que o app consegue mostrar."
    )]
    Go2rtcAusente(String),
    #[error(
        "o {servico} subiu e morreu na hora. O motivo está em {pasta}\\servico.log."
    )]
    MorreuAoSubir { servico: String, pasta: String },
    #[error("o {0} subiu mas não respondeu a tempo — veja se outro programa está usando a porta")]
    NaoRespondeu(String),
}

/// Quantas threads dar ao Whisper.
///
/// **Medido nesta máquina** (Ryzen 7 5700X, 8 núcleos / 16 threads, áudio de 18,5 s):
///
/// | threads | tempo |
/// | --- | --- |
/// | 4 (o valor antigo, fixo) | 2,29 s |
/// | **8** | **1,66 s** |
/// | 16 | 2,38 s |
///
/// Ou seja: subir até os núcleos FÍSICOS ganha 27%, e passar deles perde tudo de volta.
/// Transcrição é computação pura, e duas threads disputando a mesma unidade de execução
/// custam mais em troca de contexto do que rendem.
///
/// O teto de 8 não é a contagem desta máquina virando regra: é onde o `small` para de
/// escalar. Numa máquina de 4 núcleos isto devolve 4, que era o valor fixo de antes — ele
/// tinha sido escrito para o laptop de 4 núcleos onde o projeto nasceu.
fn threads_do_whisper() -> usize {
    std::thread::available_parallelism()
        .map(|total| threads_para(total.get()))
        .unwrap_or(4)
}

/// A conta separada do sistema, para poder ser testada sem depender da máquina.
///
/// Divide por dois para estimar os núcleos físicos a partir dos lógicos, com **piso de 4**:
/// numa máquina de 4 núcleos sem hyperthreading a metade seria 2, o que é pior que o valor
/// fixo que existia antes — a otimização não pode piorar hardware nenhum.
fn threads_para(logicos: usize) -> usize {
    (logicos / 2).clamp(4, 8)
}

pub fn whisper_url() -> String {
    format!("http://127.0.0.1:{PORTA_WHISPER}")
}

pub fn chatterbox_url() -> String {
    format!("http://127.0.0.1:{PORTA_CHATTERBOX}")
}

pub fn piper_url() -> String {
    format!("http://127.0.0.1:{PORTA_PIPER}")
}

/// O executável do go2rtc dentro da pasta dele.
///
/// O nome que vem no zip oficial. Windows-only como o resto do app.
const EXECUTAVEL_DO_GO2RTC: &str = "go2rtc.exe";

/// O nome alternativo, que é como o arquivo sai do release quando baixado direto.
///
/// Duas tentativas porque renomear um executável baixado é justamente o passo que se
/// esquece, e o erro resultante ("não achei o go2rtc") mandaria procurar um arquivo que
/// está lá na frente do usuário.
const EXECUTAVEL_DO_GO2RTC_ALT: &str = "go2rtc_win64.exe";

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
            .args(["-t", &threads_do_whisper().to_string()])
            .args(["--host", "127.0.0.1"])
            .args(["--port", &PORTA_WHISPER.to_string()])
            // As DLLs (ggml, openblas) ficam ao lado do exe.
            .current_dir(&pasta);

        let filho = self.spawn("whisper-server", comando, Some(&pasta))?;

        // Carregar o modelo leva alguns segundos na primeira vez.
        match self.esperar(http, &url, filho, Duration::from_secs(60)).await {
            Espera::Atendeu => Ok(url),
            Espera::Morreu => Err(ServiceError::MorreuAoSubir {
                servico: "whisper-server".to_owned(),
                pasta: pasta.display().to_string(),
            }),
            Espera::Demorou => Err(ServiceError::NaoRespondeu("whisper-server".to_owned())),
        }
    }

    /// Garante que existe um servidor do Chatterbox atendendo, e devolve a URL dele.
    ///
    /// Mesmo ciclo do Whisper — bate na porta, sobe se ninguém atender — com duas
    /// diferenças que vêm de ser um processo Python com um modelo grande atrás:
    ///
    /// - O que se executa é o **Python do ambiente virtual**, e não o do sistema. O
    ///   servidor exige 3.10 exato, e a máquina de quem usa quase certamente tem outro.
    /// - A espera é de **três minutos**, não de um. Importar o torch e subir meio bilhão
    ///   de parâmetros para a VRAM não se compara a carregar um `ggml` pequeno; um limite
    ///   curto aqui viraria "não respondeu a tempo" num servidor que estava só subindo.
    /// - Estar instalado é o **carimbo do instalador**, não a existência do interpretador.
    pub async fn ensure_chatterbox(
        &self,
        http: &reqwest::Client,
        data_dir: &Path,
    ) -> Result<String, ServiceError> {
        let url = chatterbox_url();
        if responde(http, &url).await {
            return Ok(url);
        }

        let pasta = data_dir.join("chatterbox");
        let python = pasta.join(PYTHON_DO_CHATTERBOX);
        let servidor = pasta.join("server.py");

        if !python.is_file() || !servidor.is_file() {
            return Err(ServiceError::ChatterboxAusente(pasta.display().to_string()));
        }

        // Instalação pela metade tem interpretador e não tem dependência. Sem esta
        // checagem, quem clica em "falar" enquanto o `start.bat` roda sobe um servidor que
        // morre num `ModuleNotFoundError` — e a mensagem que sobra culpa o lugar errado.
        if !pasta.join(CARIMBO_DE_INSTALACAO).is_file() {
            return Err(ServiceError::ChatterboxIncompleto(
                pasta.display().to_string(),
            ));
        }

        let mut comando = Command::new(&python);
        comando
            .arg(&servidor)
            // Pelo mesmo motivo das DLLs do Whisper: o servidor lê `config.yaml`,
            // `voices/` e `reference_audio/` relativos ao diretório de trabalho, e de
            // outro lugar ele sobe sem achar nem os clipes de voz.
            .current_dir(&pasta);

        let filho = self.spawn("chatterbox", comando, Some(&pasta))?;

        match self.esperar(http, &url, filho, Duration::from_secs(180)).await {
            Espera::Atendeu => Ok(url),
            Espera::Morreu => Err(ServiceError::MorreuAoSubir {
                servico: "servidor de voz".to_owned(),
                pasta: pasta.display().to_string(),
            }),
            Espera::Demorou => Err(ServiceError::NaoRespondeu("servidor de voz".to_owned())),
        }
    }

    /// Garante que existe um Piper atendendo, e devolve a URL dele.
    ///
    /// É o irmão rápido do [`Self::ensure_chatterbox`], e as diferenças todas vêm de o
    /// Piper ser pequeno:
    ///
    /// - **Espera 30 s, não 180.** Um `.onnx` de ~60 MB não se compara a meio bilhão de
    ///   parâmetros indo para a VRAM.
    /// - **Não tem carimbo de instalação** como o `.install_complete` do Chatterbox,
    ///   porque não há instalador próprio para deixá-lo. O que se checa é o interpretador
    ///   **e pelo menos uma voz**: um `pip install` que terminou sem o `download_voices`
    ///   deixa exatamente esse estado, e sem a segunda checagem o servidor subiria mudo.
    /// - **Não usa a GPU.** O Piper roda em CPU e deixa a placa inteira para o Ollama, que
    ///   é metade da razão de ele existir aqui.
    pub async fn ensure_piper(
        &self,
        http: &reqwest::Client,
        data_dir: &Path,
    ) -> Result<String, ServiceError> {
        let url = piper_url();
        if responde(http, &url).await {
            return Ok(url);
        }

        let pasta = data_dir.join("piper");
        let python = pasta.join(PYTHON_DO_PIPER);
        let tem_voz = VOZES_PIPER
            .iter()
            .any(|voz| pasta.join(format!("{voz}.onnx")).is_file());

        if !python.is_file() || !tem_voz {
            return Err(ServiceError::PiperAusente(pasta.display().to_string()));
        }

        let mut comando = Command::new(&python);
        comando
            .args(["-m", "piper.http_server"])
            .args(["-m", VOZ_INICIAL_DO_PIPER])
            // O `--data-dir` é o que deixa UM servidor atender as quatro vozes: ele carrega
            // sob demanda o `.onnx` que a requisição pedir, e guarda em cache.
            .arg("--data-dir")
            .arg(&pasta)
            .args(["--host", "127.0.0.1"])
            .args(["--port", &PORTA_PIPER.to_string()])
            .current_dir(&pasta);

        let filho = self.spawn("piper", comando, Some(&pasta))?;

        match self.esperar(http, &url, filho, Duration::from_secs(30)).await {
            Espera::Atendeu => Ok(url),
            Espera::Morreu => Err(ServiceError::MorreuAoSubir {
                servico: "Piper".to_owned(),
                pasta: pasta.display().to_string(),
            }),
            Espera::Demorou => Err(ServiceError::NaoRespondeu("Piper".to_owned())),
        }
    }

    /// Garante que o go2rtc atende, e devolve a URL dele.
    ///
    /// **Reescreve a configuração antes de subir**, e é o que separa este dos outros
    /// serviços: os demais têm config estática, e o go2rtc tem uma lista de câmeras que
    /// muda a cada cadastro. Gerar o YAML aqui é o que faz uma câmera nova aparecer sem
    /// ninguém editar arquivo.
    ///
    /// A escrita acontece mesmo quando o serviço já está de pé, e isso é de propósito:
    /// o arquivo passa a valer na próxima subida, e a alternativa (escrever só quando
    /// spawna) deixaria a configuração velha para sempre em quem nunca fecha o app.
    pub async fn ensure_go2rtc(
        &self,
        http: &reqwest::Client,
        data_dir: &Path,
        cameras: &[crate::core::cameras::Camera],
    ) -> Result<String, ServiceError> {
        let url = crate::core::cameras::go2rtc::url();
        let pasta = data_dir.join("go2rtc");

        let config =
            crate::core::cameras::go2rtc::escrever_config(&pasta, cameras).map_err(|erro| {
                ServiceError::NaoSubiu {
                    servico: "go2rtc".to_owned(),
                    detalhe: erro.to_string(),
                }
            })?;

        if responde(http, &url).await {
            return Ok(url);
        }

        let executavel = [EXECUTAVEL_DO_GO2RTC, EXECUTAVEL_DO_GO2RTC_ALT]
            .iter()
            .map(|nome| pasta.join(nome))
            .find(|caminho| caminho.is_file())
            .ok_or_else(|| ServiceError::Go2rtcAusente(pasta.display().to_string()))?;

        let mut comando = Command::new(&executavel);
        // `-config` explícito: sem ele o go2rtc procura o YAML no diretório de trabalho,
        // que não é necessariamente o dele.
        comando.arg("-config").arg(&config).current_dir(&pasta);

        let filho = self.spawn("go2rtc", comando, Some(&pasta))?;

        // 15 s e não os 30 dos servidores de voz: o go2rtc é um binário Go que abre a
        // porta em menos de um segundo. Ele NÃO conecta nas câmeras na subida — isso é
        // preguiçoso, no primeiro quadro pedido —, então uma câmera offline não segura
        // esta espera.
        match self
            .esperar(http, &url, filho, Duration::from_secs(15))
            .await
        {
            Espera::Atendeu => Ok(url),
            Espera::Morreu => Err(ServiceError::MorreuAoSubir {
                servico: "go2rtc".to_owned(),
                pasta: pasta.display().to_string(),
            }),
            Espera::Demorou => Err(ServiceError::NaoRespondeu("go2rtc".to_owned())),
        }
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

        // Sem pasta: o Ollama não é nosso, não sabemos onde ele mora, e o erro bom dele
        // vem do `AgentError::Offline` de qualquer forma.
        let Ok(filho) = self.spawn("ollama", comando, None) else {
            return false;
        };

        matches!(
            self.esperar(http, url, filho, Duration::from_secs(30)).await,
            Espera::Atendeu
        )
    }

    /// Sobe o processo e devolve **onde ele ficou** na lista de filhos, para o
    /// [`Self::esperar`] poder vigiar aquele exato processo.
    ///
    /// O índice, e não o último da lista: dois `ensure_*` concorrentes intercalariam os
    /// `push`, e vigiar "o último" faria um esperar pela morte do outro.
    fn spawn(
        &self,
        servico: &str,
        mut comando: Command,
        pasta: Option<&Path>,
    ) -> Result<usize, ServiceError> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            comando.creation_flags(CREATE_NO_WINDOW);
        }

        // **Sem isto, um serviço em Python morre antes de abrir a porta.**
        //
        // O `CREATE_NO_WINDOW` tira o console, e com ele o stdout do filho deixa de ser
        // válido. Um `.exe` nativo como o whisper-server não liga; o Flask do Piper imprime
        // o banner de inicialização assim que sobe, e morre em
        // `OSError: [Errno 9] Bad file descriptor` — sem nunca escutar na porta, e sem
        // deixar rastro, porque o rastro ia justamente para o descritor quebrado.
        //
        // O arquivo, e não `Stdio::null()`, porque foi exatamente a INVISIBILIDADE que
        // custou caro: a mensagem de erro do app mandava rodar o servidor à mão para
        // descobrir o motivo. Agora o motivo fica no disco.
        comando.stdin(Stdio::null());

        match pasta.map(|pasta| pasta.join(ARQUIVO_DE_LOG)).map(File::create) {
            Some(Ok(saida)) => {
                let erros = saida.try_clone().map_err(|error| ServiceError::NaoSubiu {
                    servico: servico.to_owned(),
                    detalhe: error.to_string(),
                })?;
                comando.stdout(saida).stderr(erros);
            }
            // Sem pasta (ou sem permissão de escrita nela) o serviço ainda tem que subir:
            // perder o log é ruim, não subir é pior.
            _ => {
                comando.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }

        let filho = comando.spawn().map_err(|error| ServiceError::NaoSubiu {
            servico: servico.to_owned(),
            detalhe: error.to_string(),
        })?;

        let mut filhos = lock(&self.filhos);
        filhos.push(filho);

        Ok(filhos.len() - 1)
    }

    /// Espera o serviço atender, **desistindo cedo se o processo morrer**.
    ///
    /// Sem o `try_wait` no meio, um servidor que morre no primeiro `import` deixa o app
    /// parado o timeout inteiro para no fim dizer "não respondeu a tempo" — o que é
    /// verdade, e manda procurar exatamente no lugar errado (a porta, e não o processo).
    async fn esperar(
        &self,
        http: &reqwest::Client,
        url: &str,
        filho: usize,
        limite: Duration,
    ) -> Espera {
        let comeco = std::time::Instant::now();

        while comeco.elapsed() < limite {
            if responde(http, url).await {
                return Espera::Atendeu;
            }

            if self.morreu(filho) {
                return Espera::Morreu;
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        Espera::Demorou
    }

    /// `true` quando aquele processo já terminou. Erro ao consultar conta como vivo: se
    /// nem dá para perguntar, esperar o timeout é o palpite menos errado.
    fn morreu(&self, filho: usize) -> bool {
        lock(&self.filhos)
            .get_mut(filho)
            .and_then(|filho| filho.try_wait().ok().flatten())
            .is_some()
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


/// Como terminou a espera por um serviço. Três casos e não um `bool` porque "não
/// atendeu" e "morreu" mandam procurar em lugares diferentes.
enum Espera {
    Atendeu,
    Morreu,
    Demorou,
}

#[cfg(test)]
mod tests {
    use super::threads_para;

    /// Sobe um serviço de verdade pelo caminho do app e diz se ele atendeu.
    ///
    /// Existe por causa de um bug que só aparece AQUI: o `CREATE_NO_WINDOW` tira o console
    /// do filho, e um serviço em Python morre ao imprimir o banner de inicialização —
    /// `OSError: [Errno 9] Bad file descriptor` — sem nunca abrir a porta. O
    /// `fala_de_verdade` não pega isso, porque ele conversa com um servidor já de pé.
    ///
    /// ```text
    /// JARVIS_SERVICO=piper cargo test --lib -- --ignored --nocapture sobe_o_servico_de_verdade
    /// ```
    #[test]
    #[ignore]
    fn sobe_o_servico_de_verdade() {
        let qual = std::env::var("JARVIS_SERVICO").unwrap_or_else(|_| "piper".to_owned());
        let data_dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
            .join("com.jarvis.app");

        let servicos = super::Services::new();
        let http = reqwest::Client::new();

        let subiu = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                match qual.as_str() {
                    "whisper" => servicos.ensure_whisper(&http, &data_dir).await,
                    "chatterbox" => servicos.ensure_chatterbox(&http, &data_dir).await,
                    _ => servicos.ensure_piper(&http, &data_dir).await,
                }
            });

        match subiu {
            Ok(url) => println!("{qual} atendeu em {url}"),
            Err(erro) => println!("{qual} não subiu: {erro}"),
        }

        servicos.shutdown();
    }

    /// A regra de threads do Whisper, nos três formatos de máquina que importam.
    #[test]
    fn as_threads_seguem_os_nucleos_fisicos_sem_nunca_piorar() {
        // Esta máquina: 8 núcleos / 16 threads. Medido: 8 é o melhor, 16 é pior que 4.
        assert_eq!(threads_para(16), 8);
        // O laptop de 4 núcleos / 8 threads onde o projeto nasceu — devolve o mesmo 4 que
        // estava fixo no código, então a mudança não altera nada lá.
        assert_eq!(threads_para(8), 4);
        // Sem hyperthreading, a metade daria 2. O piso impede a otimização de piorar.
        assert_eq!(threads_para(4), 4);
        // Máquina enorme: o `small` não escala além disso, e mais threads só disputam.
        assert_eq!(threads_para(64), 8);
    }
}
