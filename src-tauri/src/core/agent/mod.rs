//! O agente: entende a frase, age, conversa e aprende.
//!
//! O ciclo é curto de propósito — **uma** chamada ao roteador ([`intent`]) e, quando é
//! papo em vez de comando, mais duas em [`converse`] (responder e extrair memória).
//! Não é o loop de tool use da Anthropic; quando ele entrar, entra ao lado, atrás
//! desta mesma função [`handle`].
//!
//! Saem daqui duas coisas: a resposta que o usuário lê, e o LOG do gatilho — o que foi
//! ouvido, o que o modelo entendeu, no que deu, e o que entrou ou saiu da memória. O
//! log existe porque um assistente que abre programas e guarda fatos sobre você erra
//! em silêncio se ninguém puder auditar o que ele achou que foi pedido.

mod converse;
mod intent;

use std::time::Instant;

use chrono::Utc;

pub use intent::{aquecer, client};

/// Reexportados para `core::vision::ollama`, que fala com o MESMO modelo já quente —
/// em 4 GB de VRAM não cabe um segundo.
pub(crate) use intent::{pedir as pedir_ao_modelo, KEEP_ALIVE};

use intent::Intent;

use crate::config::AppSettings;
use crate::core::automation::{self, AutomationState};
use crate::core::cameras::{self, Catalogo};
use crate::core::casa::chaveiro::{Busca, Chaveiro};
use crate::core::lugar::Localizador;
use crate::core::tempo;
use crate::core::casa::controle::{self, Ajuste};
use crate::core::memory::{Acao, Memoria};
use crate::core::music;
use crate::core::search;
use crate::core::system::{self, MediaKey, SystemError};
use crate::core::vision;

/// Um passo de volume. A tecla do Windows anda ~2%, o que é imperceptível quando
/// alguém fala "aumenta o volume" — em comando de voz um passo precisa ser um passo.
const PASSO_DE_VOLUME: i8 = 10;

/// Quantas mensagens antigas viram resumo de uma vez.
const LOTE_DE_RESUMO: usize = 30;

const NOTA_DO_RESUMO: &str = "resumo das conversas";

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error(
        "não achei o Ollama em {url}. Instale de ollama.com, deixe ele rodando, e baixe o modelo com `ollama pull {model}`"
    )]
    Offline { url: String, model: String },
    #[error("o Ollama não tem o modelo {0} baixado — rode `ollama pull {0}` no terminal")]
    ModeloAusente(String),
    #[error("o Ollama recusou a chamada (HTTP {status}): {corpo}")]
    Recusado { status: u16, corpo: String },
    #[error(
        "o modelo demorou demais para responder — na primeira chamada ele carrega na memória e isso leva mais de um minuto"
    )]
    Demorou,
    #[error("falha de rede ao falar com o Ollama: {0}")]
    Rede(String),
    #[error("o modelo devolveu algo que não é uma ação válida: {0}")]
    NaoEntendi(String),
    #[error(
        "o modelo {0} não enxerga imagem. Troque por um multimodal em Configurações — `ollama pull qwen2.5vl:3b` e ponha `qwen2.5vl:3b` no campo do modelo"
    )]
    SemVisao(String),
    #[error("não consegui pegar a imagem da câmera: {0}")]
    SemCamera(String),
    #[error("não consegui capturar a tela: {0}")]
    SemTela(String),
    /// A visão pela Anthropic falhou. Uma variante só, e não uma por causa (rede, HTTP,
    /// recusa, corpo estranho), porque **este erro nunca chega à tela**: quem o recebe é
    /// o fallback para o modelo local, que já vai responder a pergunta. O texto é para
    /// o stderr de quem está depurando.
    #[error("{0}")]
    VisaoRemota(String),
}

/// Coisa que só a UI sabe fazer.
///
/// A câmera é o caso claro: quem é dono do laço de preview é o `sensorStore`, não o
/// Rust. Abrir o dispositivo aqui deixaria a câmera ligada com o botão apagado e a
/// tela vazia — o estado da UI é que manda. Então o agente PEDE, e a UI faz
/// exatamente o que o botão faria. O widget de música segue a mesma ideia.
///
/// Serializado como `{"tipo":"...", ...}` e espelhado em `src/lib/tauri/events.ts`.
/// A tag INTERNA é o que deixa uma variante carregar dados (a faixa) sem os outros
/// casos ganharem um nível de aninhamento à toa — do lado do TypeScript isso vira uma
/// união discriminada, com o `switch` exaustivo de graça.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum AcaoDeUi {
    WebcamOn,
    WebcamOff,
    /// Abre a janela de câmeras já mostrando uma delas.
    ///
    /// Leva o **id** e não o nome falado: quem casou "garagem" com a câmera certa foi o
    /// catálogo, aqui no Rust, e mandar o nome cru obrigaria a UI a repetir esse
    /// casamento com uma lista que ela conhece pior.
    CameraOn { camera: String },
    CameraOff,
    /// Guarda o rosto de quem está na webcam AGORA sob este nome.
    ///
    /// Pedido à UI pela mesma razão da webcam: quem é dono da câmera é ela. E há uma
    /// segunda razão aqui — o cadastro precisa de uma foto TIRADA NA HORA, e a UI é quem
    /// sabe se o preview já está aberto (aproveita o quadro) ou se precisa acender a luz.
    CadastrarRosto { nome: String },
    /// Abre o widget de "tocando agora" com a faixa que acabou de começar.
    Tocando {
        faixa: music::Faixa,
    },
    /// Abre um endereço numa aba do navegador interno.
    ///
    /// Vem como pedido à UI, e não como coisa feita aqui, porque as abas são webviews do
    /// Tauri e este módulo não o conhece — a mesma razão da webcam. E é o caminho certo
    /// por um segundo motivo: quem abre a janelinha do navegador é a tela, e ela precisa
    /// estar aberta ANTES do webview nascer, para ter um buraco a medir.
    AbrirSite { url: String },
    /// Uma busca numa aba nova.
    Pesquisar { query: String },
}

/// O que [`handle`] devolve: a fala, e o log quando houve comando ou mudança na memória.
pub struct Outcome {
    /// `None` em conversa que não mexeu em nada. Uma caixa de log embaixo de cada
    /// "bom dia" é o ruído que faz o log inteiro passar a ser ignorado.
    pub trace: Option<String>,
    pub reply: String,
    /// `Some` quando o comando só se completa do lado da UI.
    pub ui: Option<AcaoDeUi>,
    /// `Some` quando ainda falta o Jarvis ANOTAR o que aprendeu nesta troca.
    ///
    /// Sai daqui em vez de acontecer dentro do [`handle`] porque **medimos**: destilar o
    /// assunto e escrever a nota custaram 1,29 s de um turno de 4,79 s — 27% do tempo —, e
    /// o usuário esperava por isso calado, antes de ouvir a resposta que já estava pronta.
    ///
    /// Quem executa é a fronteira (`commands::chat`), depois de a resposta já ter saído.
    pub manutencao: Option<Manutencao>,
}

/// O material bruto para o Jarvis anotar o que aprendeu, depois de já ter respondido.
pub struct Manutencao {
    pub dito: String,
    pub resposta: String,
}

/// Cada frase da resposta, entregue enquanto o modelo ainda escreve o resto.
///
/// **É por aqui que a fala começa antes do fim.** Quem chama o [`handle`] recebe as frases
/// na ordem em que foram escritas e faz com elas o que só ele sabe fazer: `commands::chat`
/// manda para o motor de voz e para a tela. O `core` não conhece nem um nem outro.
///
/// Toda resposta passa por aqui, não só a de conversa: comando de PC devolve uma frase
/// fixa, que sai inteira numa chamada só. Assim quem escuta não precisa saber qual caminho
/// o agente tomou para saber o que falar.
pub type AoFalar<'a> = &'a (dyn Fn(&str) + Sync);

// Nove parâmetros pelo mesmo motivo do `send_message`: cada capacidade que o agente
// alcança entra como um `&` próprio, e agrupá-las numa struct só para agradar o lint
// criaria um tipo que existe por causa do lint.
#[allow(clippy::too_many_arguments)]
pub async fn handle(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    automation: &AutomationState,
    chaveiro: &Chaveiro,
    catalogo: &Catalogo,
    localizador: &Localizador,
    dito: &str,
    ao_falar: AoFalar<'_>,
) -> Result<Outcome, AgentError> {
    // Modelo vazio desliga o intérprete e volta ao mock. É a saída de emergência sem
    // precisar de um booleano — mesmo padrão do `tts_voice_id` ("vazio = padrão").
    if settings.ollama_model.trim().is_empty() {
        let reply = crate::core::chat::mock_reply_text(&settings.assistant_name, dito);
        ao_falar(&reply);

        return Ok(Outcome {
            trace: None,
            reply,
            ui: None,
            manutencao: None,
        });
    }

    // Você pode ter editado as notas no Obsidian com o app aberto.
    memoria.recarregar();

    let url = &settings.ollama_url;
    let model = &settings.ollama_model;
    let nome = &settings.assistant_name;

    let relogio = Instant::now();
    let acao = intent::interpret(http, url, model, nome, &memoria.apelidos(), dito).await?;
    let pensou = relogio.elapsed();

    let mut log = Log::novo(dito, model, &acao, pensou, settings.log_detalhado);
    let mut ui = None;
    // Preenchido só no caminho de CONVERSA: é o único que gera nota. Comando de PC e casa
    // não ensinam nada que valha uma nota, e o `destilar` já os descartava.
    let mut manutencao = None;

    // O caminho de conversa é o único que entrega a resposta em pedaços — os outros
    // devolvem uma frase pronta, que sai inteira lá embaixo. Sem esta marca, a conversa
    // seria falada duas vezes.
    let mut ja_falou = false;

    let reply = match &acao {
        Intent::Reply {} => {
            // A resposta sai agora; as notas ficam para depois dela. O `resposta` é
            // preenchido no fim do `handle`, quando o texto final já existe.
            manutencao = Some(Manutencao {
                dito: dito.to_owned(),
                resposta: String::new(),
            });

            ja_falou = true;
            conversar(http, settings, memoria, dito, ao_falar).await?
        }

        // Pergunta sobre o mundo: dá uma olhada na internet, GUARDA o que achou, e
        // responde conversando. A aba do navegador só abre quando ele mandou pesquisar
        // — abrir Google no meio de um papo é interromper, não ajudar.
        Intent::WebSearch { query } => {
            log.acao(&acao);

            if pediu_a_aba(dito) {
                ui = Some(AcaoDeUi::Pesquisar {
                    query: query.clone(),
                });
            }

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: true,
            });
            memoria.atualizar_rotinas();

            pesquisar_e_responder(http, settings, memoria, query, &mut log).await
        }

        // Tocar uma música nomeada. Sobe o Spotify se estiver fechado — o `spotify:`
        // é registrado pelo app, então o ShellExecute resolve.
        Intent::PlayMusic { query } => {
            log.acao(&acao);

            let relogio = Instant::now();
            let tocando = music::tocar(
                http,
                query,
                &settings.spotify_client_id,
                &settings.spotify_client_secret,
            )
            .await;
            let levou = relogio.elapsed().as_millis();

            log.linhas.push(format!(
                "MÚSICA     spotify · {}",
                music::modo(&settings.spotify_client_id, &settings.spotify_client_secret)
            ));
            log.desfecho(tocando.as_ref().err().map(ToString::to_string), levou);

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: tocando.is_ok(),
            });
            memoria.atualizar_rotinas();

            match tocando {
                Ok(tocando) => match tocando.faixa {
                    Some(faixa) => {
                        let frase = format!("Tocando {}.", faixa.como_texto());
                        ui = Some(AcaoDeUi::Tocando { faixa });
                        frase
                    }
                    // Sem credencial não há faixa, e sem faixa não há widget: mostrar
                    // uma capa vazia com "?" seria pior que não mostrar nada.
                    None => format!(
                        "Abri a busca por \"{query}\" no Spotify — é só dar play. Para eu \
                         tocar direto, ponha as credenciais do Spotify em Configurações."
                    ),
                },
                Err(erro) => erro.to_string(),
            }
        }

        // A câmera é da UI. O agente registra a ação e pede; quem liga é o
        // `sensorStore`, pelo mesmo caminho do botão da barra de ícones.
        Intent::WebcamOn {} | Intent::WebcamOff {} => {
            log.acao(&acao);

            let ligar = matches!(acao, Intent::WebcamOn {});
            ui = Some(if ligar {
                AcaoDeUi::WebcamOn
            } else {
                AcaoDeUi::WebcamOff
            });

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: true,
            });
            memoria.atualizar_rotinas();

            if ligar {
                "Ligando a câmera.".to_owned()
            } else {
                "Câmera desligada.".to_owned()
            }
        }

        // "eu sou o Guilherme". O rosto é guardado pela UI, que é dona da câmera — aqui
        // se resolve só o nome e a frase de volta.
        Intent::SouEu { pessoa } => {
            log.acao(&acao);

            let nome = pessoa.trim();
            if nome.is_empty() {
                "Não peguei o nome. Pode repetir?".to_owned()
            } else {
                ui = Some(AcaoDeUi::CadastrarRosto {
                    nome: nome.to_owned(),
                });

                memoria.registrar_acao(Acao {
                    quando: Utc::now().timestamp_millis(),
                    acao: verbo(&acao),
                    alvo: argumentos(&acao),
                    ok: true,
                });

                format!("Prazer, {nome}. Vou lembrar do seu rosto.")
            }
        }

        // As câmeras de segurança. Como a webcam, a janela é da UI: o agente resolve
        // QUAL câmera (que é a parte que só o catálogo sabe fazer) e pede.
        Intent::CameraOn { camera } => {
            log.acao(&acao);

            match catalogo.achar_por_nome(camera) {
                cameras::Busca::Uma(achada) => {
                    ui = Some(AcaoDeUi::CameraOn {
                        camera: achada.id.clone(),
                    });

                    memoria.registrar_acao(Acao {
                        quando: Utc::now().timestamp_millis(),
                        acao: verbo(&acao),
                        alvo: argumentos(&acao),
                        ok: true,
                    });

                    format!("Mostrando a {}.", achada.nome)
                }
                // Frase, e não erro: a tela mostraria "Comando falhou" no lugar de algo
                // que ensina o que fazer. Mesma política da casa inteligente.
                cameras::Busca::Nenhuma => nenhuma_camera(catalogo, camera),
                cameras::Busca::Varias(nomes) => {
                    format!("Tenho mais de uma com esse nome: {}. Qual delas?", nomes.join(", "))
                }
            }
        }

        Intent::CameraOff {} => {
            log.acao(&acao);
            ui = Some(AcaoDeUi::CameraOff);

            "Fechando as câmeras.".to_owned()
        }

        // "tem alguém na garagem?" — o `look` da câmera de rede. A imagem vem do go2rtc,
        // já em JPEG, e daí em diante é o mesmo caminho da webcam.
        Intent::LookCamera { camera } => {
            log.acao(&acao);

            match catalogo.achar_por_nome(camera) {
                cameras::Busca::Uma(achada) => {
                    let relogio = Instant::now();
                    let resultado =
                        olhar_camera(http, settings, memoria, &achada, dito, &mut log).await;
                    log.desfecho(
                        resultado.as_ref().err().map(ToString::to_string),
                        relogio.elapsed().as_millis(),
                    );

                    memoria.registrar_acao(Acao {
                        quando: Utc::now().timestamp_millis(),
                        acao: verbo(&acao),
                        alvo: argumentos(&acao),
                        ok: resultado.is_ok(),
                    });

                    resultado?
                }
                cameras::Busca::Nenhuma => nenhuma_camera(catalogo, camera),
                cameras::Busca::Varias(nomes) => {
                    format!("Em qual delas? Tenho {}.", nomes.join(", "))
                }
            }
        }

        Intent::CameraMove { camera, direcao } => {
            log.acao(&acao);

            match catalogo.achar_por_nome(camera) {
                cameras::Busca::Uma(achada) => {
                    let resultado = crate::commands::cameras::mover(http, &achada, *direcao).await;

                    memoria.registrar_acao(Acao {
                        quando: Utc::now().timestamp_millis(),
                        acao: verbo(&acao),
                        alvo: argumentos(&acao),
                        ok: resultado.is_ok(),
                    });

                    match resultado {
                        Ok(()) => format!(
                            "Virei a {} para a {}.",
                            achada.nome,
                            direcao.como_texto()
                        ),
                        // A recusa da câmera sem PTZ já vem escrita e explica o motivo —
                        // repetir a frase aqui daria duas redações da mesma causa.
                        Err(erro) => erro,
                    }
                }
                cameras::Busca::Nenhuma => nenhuma_camera(catalogo, camera),
                cameras::Busca::Varias(nomes) => {
                    format!("Qual delas eu viro? Tenho {}.", nomes.join(", "))
                }
            }
        }

        // Olhar — para a câmera ou para a tela. A captura vem PRIMEIRO e o pedido de
        // mostrar depois: se fosse ao contrário, a UI e o Rust disputariam a abertura do
        // dispositivo. `capture_webcam_frame` abre e fecha sozinho quando a câmera está
        // desligada — foi desenhado para exatamente este caso.
        Intent::Look { fonte } => {
            // `Auto` vira uma fonte concreta AQUI, e não no prompt: quem sabe se a
            // câmera está aberta é o `AutomationState`, e adivinhar isso num 3B seria
            // trocar um fato por um palpite.
            let fonte = fonte.resolver(automation.is_webcam_open());
            let acao = Intent::Look { fonte };
            log.acao(&acao);

            let relogio = Instant::now();
            let resultado = olhar(http, settings, memoria, automation, dito, fonte, &mut log).await;
            log.desfecho(
                resultado.as_ref().err().map(ToString::to_string),
                relogio.elapsed().as_millis(),
            );

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: resultado.is_ok(),
            });
            memoria.atualizar_rotinas();

            match resultado {
                Ok(resposta) => {
                    // Só mostra a câmera se deu certo — ligar a webcam para em seguida
                    // dizer "não consegui ver" é o pior dos dois mundos. E olhar a tela
                    // não liga câmera nenhuma.
                    if fonte == vision::Fonte::Webcam {
                        ui = Some(AcaoDeUi::WebcamOn);
                    }
                    resposta
                }
                Err(erro) => erro.to_string(),
            }
        }

        // ---- memória explícita: o caminho confiável ----------------------
        Intent::Remember { fact } => {
            log.acao(&acao);
            if memoria.lembrar(&assunto_de(fact), fact) {
                log.memoria('+', &assunto_de(fact));
                "Guardado.".to_owned()
            } else {
                "Isso eu já sabia.".to_owned()
            }
        }
        Intent::Forget { about } => {
            log.acao(&acao);
            let apagadas = memoria.esquecer(about);
            if apagadas.is_empty() {
                format!("Não achei nada sobre \"{about}\" na memória.")
            } else {
                for nome in &apagadas {
                    log.memoria('-', nome);
                }
                format!("Esqueci {}.", apagadas.join(", "))
            }
        }
        Intent::Alias { nickname, target } => {
            log.acao(&acao);
            if memoria.apelidar(nickname, target) {
                log.memoria('+', nickname);
                format!("Combinado: \"{nickname}\" é o {target}.")
            } else {
                "Isso eu já sabia.".to_owned()
            }
        }

        // ---- o tempo lá fora ----------------------------------------------
        Intent::Weather {} | Intent::WeatherAt { .. } => {
            log.acao(&acao);
            let relogio = Instant::now();

            let pedido = match &acao {
                Intent::WeatherAt { local } => local.as_str(),
                _ => "",
            };
            let frase = ver_o_tempo(http, settings, localizador, pedido).await;
            log.desfecho(None, relogio.elapsed().as_millis());

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: true,
            });
            memoria.atualizar_rotinas();

            frase
        }

        // ---- o navegador interno ------------------------------------------
        //
        // Abrir um site deixou de sair para o navegador do sistema: agora vira uma aba
        // aqui dentro. O caminho de fora continua existindo no botão da barra de
        // endereço — senha salva, extensão e impressão ainda precisam dele.
        Intent::OpenSite { url } => {
            log.acao(&acao);
            ui = Some(AcaoDeUi::AbrirSite { url: url.clone() });

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: true,
            });
            memoria.atualizar_rotinas();

            format!("Abrindo {url}.")
        }

        // ---- a casa inteligente -------------------------------------------
        Intent::SmartHome { .. } | Intent::SmartColor { .. } | Intent::SmartBright { .. } => {
            log.acao(&acao);
            let relogio = Instant::now();
            let resultado = casa(chaveiro, &acao);
            log.desfecho(
                resultado.as_ref().err().cloned(),
                relogio.elapsed().as_millis(),
            );

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: resultado.is_ok(),
            });
            memoria.atualizar_rotinas();

            match resultado {
                Ok(frase) => frase,
                Err(erro) => erro,
            }
        }

        // ---- comandos do PC ----------------------------------------------
        _ => {
            log.acao(&acao);
            let relogio = Instant::now();
            let resultado = execute(&acao);
            log.resultado(&resultado, relogio.elapsed().as_millis());

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: resultado.is_ok(),
            });
            memoria.atualizar_rotinas();

            // Falha de execução responde como FRASE, não como erro de IPC: "não achei
            // o Spotify" é uma conversa, não o backend caindo.
            match resultado {
                Ok(frase) => frase,
                Err(erro) => erro.to_string(),
            }
        }
    };

    // A troca só é anotável depois de a resposta existir — daí o preenchimento aqui, e
    // não lá dentro do braço da conversa.
    if let Some(servico) = manutencao.as_mut() {
        servico.resposta.clone_from(&reply);
    }

    // Comando e busca chegam aqui com o texto inteiro na mão: falam de uma vez, que é o
    // que sempre fizeram. Só a conversa é que já foi saindo pelo caminho.
    if !ja_falou {
        ao_falar(&reply);
    }

    Ok(Outcome {
        trace: log.render(),
        reply,
        ui,
        manutencao,
    })
}

/// Papo: responde com histórico e memória, e depois tenta aprender algo com o que foi
/// dito. As duas chamadas são separadas porque um 3B não faz as duas coisas numa só —
/// o porquê, com os números, está no topo de [`converse`].
async fn conversar(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    dito: &str,
    ao_falar: AoFalar<'_>,
) -> Result<String, AgentError> {
    // O tema entra SÓ na conversa, e por causa do tom. O roteador e a busca recebem
    // apenas o nome: classificar um verbo e resumir uma busca não mudam com o jeito de
    // falar — e mexer no prompt do roteador é o que quebra o app.
    let resposta = converse::responder(
        http,
        settings,
        &memoria.contexto(dito),
        &memoria.recentes(converse::JANELA),
        dito,
        ao_falar,
    )
    .await?;

    Ok(resposta)
}

/// Anota o que a troca ensinou: destila o assunto, reescreve a nota, e resume se for hora.
///
/// **Roda DEPOIS de o usuário já ter a resposta**, e é essa a única diferença em relação a
/// como isto funcionava antes. Nada aqui muda o que ele ouve — são as duas a quatro
/// chamadas ao Ollama que mantêm a memória, e fazê-lo esperar por elas era 27% do turno.
///
/// Best-effort do começo ao fim, como já era: perder uma nota é recuperável ("lembra
/// que...", que passa pelo roteador), travar a resposta não.
pub async fn manter_memoria(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    servico: &Manutencao,
) {
    // O log de memória ficava no `trace` da resposta. Agora que isto roda depois, ele não
    // tem para onde ir — e um `Log` de mentira aqui é mais honesto que fingir que as linhas
    // ainda cabem naquela caixa, que já foi entregue.
    let mut log = Log::mudo();

    destilar(
        http,
        settings,
        memoria,
        &servico.dito,
        &servico.resposta,
        &mut log,
    )
    .await;
    talvez_resumir(http, settings, memoria).await;
}

/// Entende do que a troca tratou e escreve a nota de conhecimento sobre aquele assunto.
///
/// Duas chamadas, e a divisão é o que faz a nota virar documento: a primeira só decide
/// O TEMA, a segunda recebe o que já estava escrito e devolve a nota inteira reescrita.
/// Numa chamada só, sem o texto anterior na frente, o modelo não teria como fundir — e
/// o resultado seria a pilha de frases coladas que a nota não deve ser.
///
/// BEST-EFFORT do começo ao fim: qualquer falha aqui é engolida, porque a resposta ao
/// usuário já foi composta e perder uma nota é recuperável ("lembra que...", que passa
/// pelo roteador e é confiável).
async fn destilar(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    dito: &str,
    resposta: &str,
    log: &mut Log,
) {
    let url = &settings.ollama_url;
    let model = &settings.ollama_model;

    // A troca inteira, não só a frase do usuário: metade do conhecimento aparece na
    // explicação que o assistente deu, e a versão anterior jogava isso fora.
    let troca = format!("Usuário: {dito}\nAssistente: {resposta}");
    let indice = memoria.nomes_das_notas();

    let Ok(Some(assunto)) = converse::destilar_assunto(http, url, model, &indice, &troca).await
    else {
        return;
    };

    let atual = memoria.corpo_da_nota(&assunto);
    let Ok(nota) =
        converse::escrever_nota(http, url, model, &assunto, &atual, &troca, &indice).await
    else {
        return;
    };

    // Nota igual à anterior é o caso de "a conversa não acrescentou nada" — o próprio
    // prompt manda devolver como está. Regravar só sujaria o `atualizado` e o git.
    if nota.trim().is_empty() || nota.trim() == atual.trim() {
        return;
    }

    memoria.escrever_conhecimento(&assunto, &nota);
    log.memoria(if atual.is_empty() { '+' } else { '~' }, &assunto);
}

/// Largura máxima da tela mandada ao modelo.
///
/// Sem teto, 1080p em PNG vira 1–4 MB de base64 no corpo da requisição — contra ~530 KB
/// de um quadro de câmera. E reduzir a tela não custa o que custaria reduzir a webcam: o
/// que se lê numa captura é texto grande de interface, não um rótulo minúsculo a meio
/// metro da lente.
///
/// ponytail: 1568 é o ponto onde uma imagem custa ~1600 tokens. O Claude lê até 2576 px
/// (e cobra até ~4784 tokens por imagem) — subir é trocar este número, se algum dia um
/// cartaz sair ilegível numa tela 4K.
const LARGURA_DA_TELA: u32 = 1568;

/// Tira uma imagem — da câmera ou da tela —, responde a pergunta olhando para ela, e
/// pesquisa quando a resposta não está na imagem.
///
/// **A busca aqui não é a mesma coisa que a busca de antes.** Ela era best-effort com a
/// descrição inteira como consulta — uma pergunta longa e ruim, disparada sempre. Agora
/// quem decide é o modelo de visão: ele devolve `buscar` preenchido quando identificou a
/// coisa mas a resposta está fora da imagem (a data dos ingressos não está no cartaz).
/// Vazio, a imagem já respondeu e não se gasta uma ida à internet.
/// A frase para quando o nome dito não casou com câmera nenhuma.
///
/// Duas redações porque as causas são diferentes e as correções também: sem nenhuma
/// câmera cadastrada, o que falta é o cadastro; com câmeras cadastradas, o que falhou
/// foi o nome — e dizer QUAIS existem é o que transforma o erro em instrução.
fn nenhuma_camera(catalogo: &Catalogo, pedida: &str) -> String {
    if catalogo.vazio() {
        return "Não tenho câmera nenhuma cadastrada ainda. Adicione uma no painel de \
                câmeras, com o endereço dela na rede."
            .to_owned();
    }

    let nomes: Vec<String> = catalogo
        .todas()
        .into_iter()
        .map(|camera| camera.nome)
        .collect();

    let pedida = pedida.trim();
    if pedida.is_empty() {
        return format!("Qual câmera? Tenho {}.", nomes.join(", "));
    }

    format!("Não tenho nenhuma câmera chamada \"{pedida}\". Tenho {}.", nomes.join(", "))
}

/// Olha uma câmera de segurança e responde a pergunta.
///
/// Gêmeo do [`olhar`], e separado dele de propósito: a origem da imagem é outra (o
/// go2rtc, não o `nokhwa`) e não há `Fonte` a resolver — quem escolhe a câmera é o
/// catálogo, antes de chegar aqui. O que os dois compartilham é o que importa: daqui
/// para a frente é [`vision::ver`] igual, com o mesmo par modelo-local/Claude e o mesmo
/// campo `buscar`.
async fn olhar_camera(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    camera: &cameras::Camera,
    pergunta: &str,
    log: &mut Log,
) -> Result<String, AgentError> {
    let imagem = cameras::go2rtc::frame_data_url(http, &camera.id)
        .await
        .map_err(|erro| AgentError::SemCamera(erro.to_string()))?;

    let visao = vision::ver(
        http,
        settings,
        &vision::Imagem::do_data_url(&imagem),
        pergunta,
        vision::Fonte::Camera,
    )
    .await?;

    let termo = visao.buscar.trim();
    if termo.is_empty() {
        return Ok(visao.resposta);
    }

    // Mesmo desfecho do [`olhar`]: o modelo identificou a coisa, mas a resposta está
    // fora da imagem. Vale para o modelo de um carro parado na frente tanto quanto para
    // um cartaz na webcam.
    let consulta = format!("{termo} {}", pergunta.trim());
    Ok(pesquisar_e_responder(http, settings, memoria, consulta.trim(), log).await)
}

async fn olhar(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    automation: &AutomationState,
    pergunta: &str,
    fonte: vision::Fonte,
    log: &mut Log,
) -> Result<String, AgentError> {
    // ponytail: chamadas bloqueantes dentro de `async`. Com a câmera já aberta o `grab`
    // volta em milissegundos; fechada, o `Session::open` custa algumas centenas, e a
    // captura de tela fica na mesma ordem. Vira `spawn_blocking` se travar a UI de verdade.
    let imagem = match fonte {
        // A mesma resolução configurada para o preview, e sem teto de largura: o modelo
        // lê o quadro INTEIRO. Reduzir pelo tamanho da janela é o oposto do que se quer
        // — é na resolução cheia que ele tem chance de ler um rótulo ou reconhecer um
        // objeto pequeno.
        vision::Fonte::Webcam | vision::Fonte::Auto => automation
            .capture_webcam_frame(settings.webcam_target(), None)
            .map_err(|erro| AgentError::SemCamera(erro.to_string()))?,
        vision::Fonte::Tela => automation::capture_screen(None, Some(LARGURA_DA_TELA))
            .map_err(|erro| AgentError::SemTela(erro.to_string()))?,
        // Câmera de rede não passa por aqui: ela não tem dispositivo local para abrir, e
        // quem a atende é o [`olhar_camera`], que já recebeu a câmera resolvida. Só se
        // chega neste braço com um `Intent::Look { fonte: "camera" }` — que o schema não
        // deixa o modelo emitir, porque `fonte` é um enum de três valores e este não
        // está entre eles. É erro em vez de `unreachable!`: uma frase estranha do modelo
        // não pode derrubar o app.
        vision::Fonte::Camera => {
            return Err(AgentError::SemCamera(
                "para olhar uma câmera de segurança eu preciso saber qual — diga o nome dela"
                    .to_owned(),
            ))
        }
    };

    let visao = vision::ver(
        http,
        settings,
        &vision::Imagem::do_data_url(&imagem.data_url),
        pergunta,
        fonte,
    )
    .await?;

    let termo = visao.buscar.trim();
    if termo.is_empty() {
        return Ok(visao.resposta);
    }

    // A autoridade INVERTE aqui, e é por isso que este caminho é outro:
    // `responder_sobre_o_que_viu` manda ignorar a busca quando ela falar de outra coisa
    // — certo para "que mouse é esse", e exatamente errado para "quando são os
    // ingressos", onde a resposta só existe na busca. A imagem identificou a coisa; o
    // resto é o caminho normal de pergunta sobre o mundo, com as fontes e a nota.
    let consulta = format!("{termo} {}", pergunta.trim());
    Ok(pesquisar_e_responder(http, settings, memoria, consulta.trim(), log).await)
}

/// Frases que pedem a aba do navegador. O resto é pergunta no meio da conversa, e
/// abrir o Google nessas seria interromper.
const ORDENS_DE_BUSCA: [&str; 5] = ["pesquis", "procur", "busca ", "no google", "abre o google"];

fn pediu_a_aba(dito: &str) -> bool {
    let normalizado = crate::core::memory::normalizar(dito);
    ORDENS_DE_BUSCA
        .iter()
        .any(|ordem| normalizado.contains(ordem.trim_end()))
}

/// Dá uma olhada na internet, guarda o que achou e responde conversando.
///
/// Nunca devolve `Err`: sem internet, dizer isso na conversa é melhor que derrubar a
/// mensagem inteira com um erro de IPC.
async fn pesquisar_e_responder(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    consulta: &str,
    log: &mut Log,
) -> String {
    let fonte = search::fonte(&settings.brave_api_key);
    let relogio = Instant::now();

    let achados = match search::pesquisar(http, consulta, &settings.brave_api_key).await {
        Ok(achados) => achados,
        Err(erro) => {
            log.busca(fonte, 0, relogio.elapsed());
            return erro.to_string();
        }
    };
    log.busca(fonte, achados.len(), relogio.elapsed());

    // Guarda ANTES de responder: se o modelo falhar na hora de falar, o conhecimento
    // fica na pasta do mesmo jeito. É isso que faz ele não pesquisar a mesma coisa
    // duas vezes — na próxima, a nota já está no contexto da conversa.
    memoria.aprender(consulta, &converse::nota_da_busca(&achados));
    log.memoria('+', &crate::core::memory::slug(consulta));

    let resposta = converse::responder_com_busca(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        &settings.assistant_name,
        consulta,
        &achados,
    )
    .await;

    match resposta {
        Ok(texto) if !texto.is_empty() => texto,
        // Ollama fora do ar: o trecho cru ainda responde melhor que uma mensagem de erro.
        _ => achados[0].trecho.chars().take(400).collect(),
    }
}

/// Destila o que já saiu da janela do prompt, em lotes.
///
/// Silencioso de propósito: falhar em resumir não pode atrapalhar a conversa que
/// acabou de acontecer, e não há o que o usuário faça a respeito.
async fn talvez_resumir(http: &reqwest::Client, settings: &AppSettings, memoria: &Memoria) {
    let pendentes = memoria.pendentes_de_resumo(converse::JANELA, LOTE_DE_RESUMO);
    if pendentes.len() < LOTE_DE_RESUMO {
        return;
    }

    let anterior = memoria
        .notas()
        .into_iter()
        .find(|nota| nota.nome == crate::core::memory::slug(NOTA_DO_RESUMO))
        .map(|nota| nota.corpo)
        .unwrap_or_default();

    let resumido = converse::resumir(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        &pendentes,
        &anterior,
    )
    .await;

    if let Ok(texto) = resumido {
        if !texto.is_empty() {
            memoria.guardar_resumo(NOTA_DO_RESUMO, &texto);
            memoria.marcar_resumidas(pendentes.len());
        }
    }
}

/// Título curto para um fato que veio sem assunto (o caminho explícito manda só a
/// frase). As primeiras palavras que carregam significado servem bem.
fn assunto_de(fato: &str) -> String {
    const VAZIAS: [&str; 12] = [
        "que", "de", "do", "da", "em", "no", "na", "um", "uma", "para", "com", "ele",
    ];

    let normalizado = crate::core::memory::normalizar(fato);
    let palavras: Vec<&str> = normalizado
        .split(' ')
        .filter(|palavra| palavra.len() > 2 && !VAZIAS.contains(palavra))
        .take(4)
        .collect();

    if palavras.is_empty() {
        "anotacao".to_owned()
    } else {
        palavras.join(" ")
    }
}

/// Executa e já devolve a frase de confirmação — um `match` só, e a frase pode usar o
/// valor que a ação produziu ("Volume em 60%") em vez de repetir o que foi pedido.
///
/// Frase fixa, e não uma segunda ida ao modelo: é instantânea, determinística, e o
/// TTS tem o que falar sem esperar nada.
fn execute(acao: &Intent) -> Result<String, SystemError> {
    Ok(match acao {
        Intent::OpenApp { name } => {
            system::open_app(name)?;
            format!("Abrindo o {name}.")
        }
        Intent::VolumeUp { steps } => {
            let nivel = system::nudge_volume(passos(*steps))?;
            format!("Volume em {nivel}%.")
        }
        Intent::VolumeDown { steps } => {
            let nivel = system::nudge_volume(-passos(*steps))?;
            format!("Volume em {nivel}%.")
        }
        Intent::VolumeSet { level } => {
            let nivel = (*level).min(100);
            system::set_volume(nivel)?;
            format!("Volume em {nivel}%.")
        }
        Intent::VolumeMute {} => {
            if system::toggle_mute()? {
                "Mudo.".to_owned()
            } else {
                "Som de volta.".to_owned()
            }
        }
        Intent::MediaPlayPause {} => {
            system::press(MediaKey::PlayPause)?;
            "Feito.".to_owned()
        }
        Intent::MediaNext {} => {
            system::press(MediaKey::Next)?;
            "Próxima.".to_owned()
        }
        Intent::MediaPrevious {} => {
            system::press(MediaKey::Previous)?;
            "Anterior.".to_owned()
        }
        // Tratados em `handle`, antes de chegar aqui.
        Intent::Reply {}
        | Intent::WebSearch { .. }
        | Intent::PlayMusic { .. }
        | Intent::WebcamOn {}
        | Intent::WebcamOff {}
        | Intent::Look { .. }
        | Intent::CameraOn { .. }
        | Intent::CameraOff {}
        | Intent::LookCamera { .. }
        | Intent::CameraMove { .. }
        | Intent::SouEu { .. }
        | Intent::Remember { .. }
        | Intent::Forget { .. }
        | Intent::Alias { .. }
        | Intent::OpenSite { .. }
        | Intent::Weather { .. }
        | Intent::WeatherAt { .. }
        | Intent::SmartHome { .. }
        | Intent::SmartColor { .. }
        | Intent::SmartBright { .. } => String::new(),
    })
}

/// Descobre ONDE e responde que tempo faz lá.
///
/// **Nunca devolve `Err`**, pela mesma razão da casa inteligente: "não achei nenhum lugar
/// chamado Xique-Xique" é uma resposta de conversa, não o backend caindo. Cada erro do
/// caminho vira a frase que ele fala.
///
/// A ordem das três fontes de "onde" não é arbitrária:
///
/// 1. **O lugar que ele nomeou.** Perguntar do tempo em Lisboa não pode responder de casa.
/// 2. **A cidade das configurações.** Quem preencheu esse campo o fez para vencer a
///    detecção — VPN, localização desligada, ou preferência.
/// 3. **O Windows.** O caminho normal, e o único que não precisa de configuração.
async fn ver_o_tempo(
    http: &reqwest::Client,
    settings: &AppSettings,
    localizador: &Localizador,
    local: &str,
) -> String {
    let pedido = local.trim();
    let casa = settings.cidade.trim();

    let por_nome = if pedido.is_empty() { casa } else { pedido };

    let (onde, nome) = if por_nome.is_empty() {
        match localizador.onde_estou(&settings.assistant_name) {
            Ok(coordenadas) => (coordenadas, None),
            Err(erro) => return erro.to_string(),
        }
    } else {
        match tempo::procurar(http, por_nome).await {
            Ok(lugar) => (lugar.coordenadas, Some(lugar.completo())),
            Err(erro) => return erro.to_string(),
        }
    };

    match tempo::consultar(http, onde).await {
        Ok(previsao) => previsao.frase(nome.as_deref()),
        Err(erro) => erro.to_string(),
    }
}

/// Acha o aparelho pelo nome dito e manda o comando.
///
/// **Todo erro daqui vira frase**, e nunca um `Err` de IPC: "não achei nenhuma luz com
/// esse nome" é uma resposta de conversa, não o backend caindo. É a mesma decisão que o
/// braço dos comandos do PC já toma logo acima.
///
/// Bloqueia por alguns segundos dentro de uma função `async` — a mesma licença que o
/// `execute` toma para as teclas de mídia. O teto é o timeout de 5 s do `controle`, e é
/// uma conversa dentro da própria rede.
fn casa(chaveiro: &Chaveiro, acao: &Intent) -> Result<String, String> {
    let dito = match acao {
        Intent::SmartHome { aparelho, .. }
        | Intent::SmartColor { aparelho, .. }
        | Intent::SmartBright { aparelho, .. } => aparelho.as_str(),
        _ => return Err("isso nao e um pedido para a casa".to_owned()),
    };

    let aparelho = match chaveiro.achar_por_nome(dito) {
        Busca::Um(achado) => *achado,

        // Os três "não achei" pedem respostas diferentes, e é por isso que a busca não
        // devolve um `Option`: mandar procurar por nome quem nunca importou nada seria
        // um conselho inútil.
        Busca::Nenhum if chaveiro.vazio() => {
            return Err(
                "ainda não sei quais aparelhos você tem. Abre o painel Casa, procura os                  aparelhos e importa os nomes da nuvem — aí eu passo a saber."
                    .to_owned(),
            )
        }
        Busca::Nenhum => {
            let conhecidos: Vec<String> = chaveiro
                .todos()
                .into_iter()
                .filter(|aparelho| !aparelho.nome.trim().is_empty())
                .map(|aparelho| aparelho.nome)
                .collect();

            return Err(format!(
                "não achei nenhum aparelho chamado \"{dito}\". Os que eu conheço: {}.",
                conhecidos.join(", ")
            ));
        }
        Busca::Varios(nomes) => {
            return Err(format!(
                "isso serve para mais de um aparelho: {}. Qual deles?",
                nomes.join(", ")
            ))
        }
    };

    // Nome sem endereço é aparelho importado da nuvem que a rede nunca anunciou — ou
    // que só anunciou antes deste chaveiro existir.
    if aparelho.ultimo_ip.trim().is_empty() {
        return Err(format!(
            "sei quem é {}, mas não sei onde está na rede. Procura os aparelhos no              painel Casa uma vez e eu passo a alcançar.",
            aparelho.nome
        ));
    }

    // Pelo `endereco_de` e não montando o alvo à mão: num subaparelho ZigBee o endereço,
    // o protocolo e a chave são do GATEWAY, e só ele sabe disso.
    let endereco = crate::core::casa::endereco_de(
        chaveiro,
        &aparelho.id,
        &aparelho.ultimo_ip,
        &aparelho.versao,
    )
    .ok_or_else(|| controle::ControleError::SemChave.to_string())?;

    executar(&endereco, acao, &aparelho.nome)
}

/// A escala da Tuya para brilho, saturação e temperatura vai até 1000; a fala vai até
/// 100. A conversão mora aqui e não no `controle` porque é uma questão de LINGUAGEM —
/// quem diz "trinta por cento" é gente, e quem entende de 0 a 1000 é o aparelho.
const CHEIO: u16 = 1000;

fn executar(
    endereco: &crate::core::casa::Endereco,
    acao: &Intent,
    nome: &str,
) -> Result<String, String> {
    let alvo = endereco.alvo();

    let (ajuste, frase) = match acao {
        Intent::SmartHome { ligar, .. } => {
            return controle::ligar(&alvo, *ligar)
                .map(|_| format!("{} {nome}.", if *ligar { "Liguei" } else { "Desliguei" }))
                .map_err(|erro| erro.to_string())
        }
        Intent::SmartColor { cor, .. } => {
            let Some(ajuste) = tinta_de(cor) else {
                // Recusar e melhor que pintar de uma cor qualquer: quem pediu "turquesa"
                // e recebeu azul nao descobre que a palavra nao foi entendida.
                return Err(format!(
                    "nao sei que cor e {cor:?}. Tente vermelho, laranja, amarelo, verde, ciano, \
                     azul, roxo, rosa, branco, ou \"mais quente\" e \"mais frio\"."
                ));
            };

            (ajuste, format!("Deixei {nome} {cor}."))
        }
        Intent::SmartBright { nivel, .. } => (
            // A fala vai de 0 a 100 e a Tuya de 0 a 1000. A conversao mora aqui e nao no
            // `controle` porque e questao de LINGUAGEM: quem diz "trinta por cento" e
            // gente, e quem entende de 0 a 1000 e o aparelho.
            Ajuste {
                brilho: Some(u16::from((*nivel).min(100)) * CHEIO / 100),
                ..Ajuste::default()
            },
            format!("Brilho de {nome} em {nivel}%."),
        ),
        _ => return Err("isso nao e um pedido para a casa".to_owned()),
    };

    controle::ajustar(&alvo, &ajuste)
        .map(|_| frase)
        .map_err(|erro| erro.to_string())
}

/// De nome de cor para o ajuste que a lampada entende.
///
/// Uma tabela e nao um conversor de CSS: o que chega aqui e fala transcrita, e "azul
/// claro" ou "azulzinho" nao existem em tabela de cor nenhuma. Cobrir as basicas com o
/// nome que as pessoas usam vale mais que aceitar `#0000FF`.
///
/// **Branco nao e uma cor aqui, e um modo.** "Mais quente" e "mais frio" viram
/// temperatura, e nao matiz — pedir branco quente e receber um amarelo saturado e
/// exatamente o erro que sairia se o pedido fosse pelo caminho da cor.
fn tinta_de(cor: &str) -> Option<Ajuste> {
    let nome = crate::core::memory::normalizar(cor);
    let branco = |temperatura: u16| Ajuste {
        temperatura: Some(temperatura),
        ..Ajuste::default()
    };

    // "mais quente" e "branco frio" chegam com duas palavras, e a que decide pode ser
    // qualquer uma delas.
    if nome.split(' ').any(|palavra| palavra == "quente") {
        return Some(branco(0));
    }
    if nome.split(' ').any(|palavra| palavra == "frio" || palavra == "fria") {
        return Some(branco(CHEIO));
    }

    let matiz = match nome.split(' ').next()? {
        "vermelho" | "vermelha" => 0,
        "laranja" => 25,
        "amarelo" | "amarela" => 55,
        "verde" => 120,
        "ciano" | "turquesa" => 180,
        "azul" => 220,
        "roxo" | "roxa" | "lilas" | "violeta" => 280,
        "rosa" | "magenta" | "pink" => 320,
        // Branco puro e o meio da escala de temperatura, e nao um matiz sem saturacao:
        // e assim que a lampada o representa.
        "branco" | "branca" => return Some(branco(CHEIO / 2)),
        _ => return None,
    };

    Some(Ajuste {
        matiz: Some(matiz),
        saturacao: Some(CHEIO),
        ..Ajuste::default()
    })
}

/// Teto em 5 passos: o modelo às vezes inventa um número grande, e ninguém precisa de
/// "aumenta 200".
fn passos(steps: u8) -> i8 {
    i8::try_from(u32::from(steps.clamp(1, 5)) * PASSO_DE_VOLUME as u32).unwrap_or(i8::MAX)
}

/// O log que aparece no chat.
struct Log {
    linhas: Vec<String>,
    /// Sem ação nem mudança de memória, não há log — só houve conversa.
    houve_algo: bool,
    /// Mostra o log em TODA mensagem, inclusive conversa pura. Vem das configurações.
    sempre: bool,
}

impl Log {
    /// A linha `INTERPRETE` carrega **o verbo que o modelo escolheu**, e não só o nome do
    /// modelo e o tempo.
    ///
    /// Sem ele não havia como saber por que uma resposta saiu errada: "salve essa música
    /// nas minhas curtidas" virou `reply` (nada foi executado) e o modelo respondeu que
    /// tinha salvado. O log da época mostrava só `INTERPRETE qwen2.5vl:3b · 3.0 s` — e
    /// "ele entendeu como conversa" e "ele executou a coisa errada" ficavam idênticos.
    fn novo(
        dito: &str,
        model: &str,
        acao: &Intent,
        pensou: std::time::Duration,
        sempre: bool,
    ) -> Self {
        Self {
            linhas: vec![
                format!("GATILHO    {dito}"),
                format!(
                    "INTERPRETE {model} · {:.1} s · {}",
                    pensou.as_secs_f32(),
                    verbo(acao)
                ),
            ],
            houve_algo: false,
            sempre,
        }
    }

    /// Um log que ninguém vai ler, para o caminho que roda depois da resposta.
    ///
    /// A alternativa seria `Option<&mut Log>` espalhado por `destilar` e amigos, para
    /// ganhar linhas que já não têm onde aparecer.
    fn mudo() -> Self {
        Self {
            linhas: Vec::new(),
            houve_algo: false,
            sempre: false,
        }
    }

    fn acao(&mut self, acao: &Intent) {
        self.linhas
            .push(format!("AÇÃO       {} · {}", verbo(acao), argumentos(acao)));
        self.houve_algo = true;
    }

    fn resultado(&mut self, resultado: &Result<String, SystemError>, ms: u128) {
        self.desfecho(resultado.as_ref().err().map(ToString::to_string), ms);
    }

    fn desfecho(&mut self, erro: Option<String>, ms: u128) {
        self.linhas.push(match erro {
            None => format!("RESULTADO  ok · {ms} ms"),
            Some(erro) => format!("RESULTADO  falhou · {erro}"),
        });
    }

    fn memoria(&mut self, sinal: char, nome: &str) {
        self.linhas.push(format!("MEMÓRIA    {sinal} {nome}"));
        self.houve_algo = true;
    }

    /// De onde veio a resposta importa: "wikipedia · 0 resultados" explica sozinho por
    /// que o Jarvis disse que não achou nada.
    fn busca(&mut self, fonte: &str, quantos: usize, levou: std::time::Duration) {
        self.linhas.push(format!(
            "BUSCA      {fonte} · {quantos} resultados · {:.1} s",
            levou.as_secs_f32()
        ));
        self.houve_algo = true;
    }

    /// Sem ação nem memória o log some, porque uma caixa embaixo de cada "bom dia" faz o
    /// log inteiro passar a ser ignorado — e aí ele não serve para nada quando importa.
    ///
    /// `sempre` é a saída para depurar: com ele ligado, dá para ver o verbo escolhido
    /// mesmo numa conversa que não mexeu em nada, que é justamente onde as respostas
    /// erradas se escondiam.
    fn render(self) -> Option<String> {
        (self.houve_algo || self.sempre).then(|| self.linhas.join("\n"))
    }
}

fn verbo(acao: &Intent) -> String {
    campos(acao)
        .get("action")
        .and_then(|valor| valor.as_str())
        .unwrap_or("?")
        .to_owned()
}

/// Formata os argumentos reaproveitando a serialização do enum — variante nova
/// aparece no log sem ninguém tocar aqui.
fn argumentos(acao: &Intent) -> String {
    let campos = campos(acao);
    let lista: Vec<String> = campos
        .iter()
        .filter(|(chave, _)| chave.as_str() != "action")
        .map(|(chave, valor)| match valor.as_str() {
            Some(texto) => format!("{chave}={texto}"),
            None => format!("{chave}={valor}"),
        })
        .collect();

    if lista.is_empty() {
        "sem argumentos".to_owned()
    } else {
        lista.join(" · ")
    }
}

fn campos(acao: &Intent) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::to_value(acao) {
        Ok(serde_json::Value::Object(mapa)) => mapa,
        _ => serde_json::Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chaveiro_de_teste(nome: &str) -> Chaveiro {
        let dir = std::env::temp_dir().join(format!("jarvis-agente-casa-{nome}"));
        let _ = std::fs::remove_dir_all(&dir);

        Chaveiro::new(&dir)
    }

    /// **Nada aqui pode virar `Err`.** Uma luz que não foi encontrada é assunto de
    /// conversa; se virasse erro de IPC, a tela mostraria "Comando falhou" no lugar de
    /// uma frase que ensina o que fazer.
    #[test]
    fn a_casa_responde_com_frase_mesmo_quando_nao_da_para_agir() {
        // Sem nada importado, o conselho é importar — e não "não achei esse aparelho",
        // que mandaria procurar um nome numa lista vazia.
        let vazio = chaveiro_de_teste("vazio");
        let apagar = |nome: &str| Intent::SmartHome {
            aparelho: nome.to_owned(),
            ligar: false,
        };

        let resposta = casa(&vazio, &apagar("luz da cozinha")).expect_err("nao tem o que ligar");
        assert!(
            resposta.contains("painel Casa"),
            "a resposta tem que dizer onde resolver: {resposta}"
        );

        // Com aparelho conhecido mas nome que não bate, o conselho é outro: a lista do
        // que ele conhece.
        let cheio = chaveiro_de_teste("cheio");
        cheio
            .guardar(vec![crate::core::casa::chaveiro::Conhecido {
                id: "abc".to_owned(),
                nome: "Luz Cozinha".to_owned(),
                local_key: "0123456789abcdef".to_owned(),
                categoria: "dj".to_owned(),
                online: true,
                ..Default::default()
            }])
            .expect("grava");

        let resposta = casa(&cheio, &apagar("ventilador do quarto")).expect_err("nao conhece");
        assert!(
            resposta.contains("Luz Cozinha"),
            "tem que listar o que ele conhece: {resposta}"
        );

        // Nome certo, mas nunca visto na rede: o que falta é uma varredura, não a nuvem.
        let resposta = casa(&cheio, &apagar("apaga a luz cozinha")).expect_err("sem endereco");
        assert!(
            resposta.contains("onde está na rede"),
            "tem que separar 'não conheço' de 'não sei onde está': {resposta}"
        );
    }

    /// Cronometra um turno inteiro, etapa por etapa, e diz onde o tempo REALMENTE vai.
    ///
    /// Não é um teste: é a ferramenta de medição da latência, no molde do `fala_de_verdade`
    /// e do `bench_filtros`. Existe porque, das ~7 etapas de um turno, só uma tinha número
    /// — e otimizar sem medir é escolher o alvo pelo palpite.
    ///
    /// Ela reproduz o caminho de uma frase de CONVERSA (`Intent::Reply`), que é o mais caro:
    /// transcrever, rotear, responder, destilar o assunto e escrever a nota. As três últimas
    /// são as que o usuário espera hoje sem precisar.
    ///
    /// Precisa de um WAV de fala. O padrão é o que o próprio app deixou na última gravação;
    /// `JARVIS_TURNO_WAV` aponta para outro.
    ///
    /// ```text
    /// cargo test --lib -- --ignored --nocapture turno_de_verdade
    /// ```
    #[test]
    #[ignore]
    fn turno_de_verdade() {
        use std::collections::BTreeMap;
        use std::time::Instant;

        let wav = std::env::var("JARVIS_TURNO_WAV").unwrap_or_else(|_| {
            let cache = std::env::var("LOCALAPPDATA").unwrap_or_default();
            format!("{cache}\\com.jarvis.app\\ultima-gravacao.wav")
        });

        let settings = AppSettings {
            ollama_url: std::env::var("JARVIS_OLLAMA")
                .unwrap_or_else(|_| "http://localhost:11434".to_owned()),
            ollama_model: std::env::var("JARVIS_MODELO")
                .unwrap_or_else(|_| "qwen2.5vl:3b".to_owned()),
            ..AppSettings::default()
        };

        let whisper = std::env::var("JARVIS_WHISPER")
            .unwrap_or_else(|_| crate::core::services::whisper_url());

        let http = crate::core::agent::intent::client();
        let bloco = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        bloco.block_on(async {
            let total = Instant::now();
            let mut etapas: Vec<(&str, f32)> = Vec::new();

            // ---- 1. ouvir -------------------------------------------------
            //
            // `JARVIS_TURNO_FRASE` pula o Whisper e mede o turno de quem DIGITOU. É o
            // recorte certo quando se está mexendo no modelo: são as etapas seguintes que
            // mudam, e subir o whisper-server só para elas seria pedágio.
            let dito = match std::env::var("JARVIS_TURNO_FRASE") {
                Ok(frase) if !frase.trim().is_empty() => frase,
                _ => {
                    let relogio = Instant::now();
                    let ouvido = match crate::core::voice::transcribe(
                        &http,
                        &whisper,
                        std::path::Path::new(&wav),
                    )
                    .await
                    {
                        Ok(texto) => texto,
                        Err(erro) => {
                            println!("transcrição falhou: {erro}");
                            println!("(aponte JARVIS_TURNO_WAV para um WAV com fala, grave um em Diagnóstico › Microfone, ou mande a frase em JARVIS_TURNO_FRASE)");
                            return;
                        }
                    };
                    etapas.push(("ouvir (whisper)", relogio.elapsed().as_secs_f32()));
                    ouvido
                }
            };
            println!("ouviu: {dito:?}\n");

            // ---- 2. rotear ------------------------------------------------
            let relogio = Instant::now();
            let acao = match intent::interpret(
                &http,
                &settings.ollama_url,
                &settings.ollama_model,
                &settings.assistant_name,
                &BTreeMap::new(),
                &dito,
            )
            .await
            {
                Ok(acao) => acao,
                Err(erro) => {
                    println!("interpretação falhou: {erro}");
                    return;
                }
            };
            etapas.push(("rotear (interpret)", relogio.elapsed().as_secs_f32()));
            println!("roteou: {acao:?}\n");

            // ---- 3. responder ---------------------------------------------
            //
            // Duas medidas, e a diferença entre elas é a feature: quanto o modelo levou
            // para escrever TUDO, e quanto levou até a PRIMEIRA frase — que é quando a
            // boca abre, e portanto o único número que o usuário sente.
            let relogio = Instant::now();
            let primeira = std::sync::Mutex::new(None::<f32>);

            let resposta = match converse::responder(&http, &settings, "", &[], &dito, &|frase| {
                let mut primeira = primeira.lock().expect("mutex");
                if primeira.is_none() {
                    *primeira = Some(relogio.elapsed().as_secs_f32());
                    println!("primeira frase: {frase:?}");
                }
            })
            .await
            {
                Ok(resposta) => resposta,
                Err(erro) => {
                    println!("resposta falhou: {erro}");
                    return;
                }
            };
            let respondeu = relogio.elapsed().as_secs_f32();
            let ate_falar = primeira.lock().expect("mutex").unwrap_or(respondeu);
            etapas.push(("responder (o que ele fala)", ate_falar));
            println!(
                "respondeu ({} caracteres) em {respondeu:.2} s, mas começou a falar em {ate_falar:.2} s: {resposta}\n",
                resposta.chars().count()
            );

            // ---- 4 e 5. a manutenção de memória, que hoje vem ANTES da fala ----
            let troca = format!("Usuário: {dito}\nAssistente: {resposta}");

            // O índice REAL, e não uma lista vazia: sem ele o modelo não tem como
            // escolher um assunto que já existe nem como ligar a nota às outras — e o
            // que se mediria seria um caminho que não acontece no app.
            let memoria = crate::core::memory::Memoria::new(std::path::Path::new(
                &std::env::var("JARVIS_MEMORIA").unwrap_or_else(|_| "../memoria".to_owned()),
            ));
            let indice = memoria.nomes_das_notas();

            let relogio = Instant::now();
            let assunto = converse::destilar_assunto(
                &http,
                &settings.ollama_url,
                &settings.ollama_model,
                &indice,
                &troca,
            )
            .await;
            etapas.push(("destilar assunto", relogio.elapsed().as_secs_f32()));

            if let Ok(Some(assunto)) = assunto {
                let relogio = Instant::now();
                let nota = converse::escrever_nota(
                    &http,
                    &settings.ollama_url,
                    &settings.ollama_model,
                    &assunto,
                    &memoria.corpo_da_nota(&assunto),
                    &troca,
                    &indice,
                )
                .await;

                // Quantos `[[links]]` a nota nova traz. É o que diz se o grafo vai crescer
                // sozinho ou continuar sendo pontos soltos ligados por semelhança.
                if let Ok(nota) = &nota {
                    println!("nota sobre {assunto:?}: {} link(s)", nota.matches("[[").count());
                }
                etapas.push(("escrever nota", relogio.elapsed().as_secs_f32()));
            }

            // ---- o veredito -----------------------------------------------
            let soma: f32 = etapas.iter().map(|(_, s)| s).sum();
            println!("{:-<52}", "");
            for (nome, segundos) in &etapas {
                let fatia = (segundos / soma * 40.0) as usize;
                println!(
                    "{nome:<26} {segundos:>6.2} s  {}",
                    "#".repeat(fatia.max(1))
                );
            }
            println!("{:-<52}", "");
            println!("soma das etapas            {soma:>6.2} s");
            println!("turno inteiro              {:>6.2} s", total.elapsed().as_secs_f32());

            // A divisão que importa: o que o usuário espera antes de ouvir, e o que o
            // Jarvis faz depois, sozinho. As duas últimas etapas saíram do caminho crítico
            // — `manter_memoria` roda em `spawn` DEPOIS de a resposta já ter saído.
            let de_fundo: f32 = etapas
                .iter()
                .filter(|(nome, _)| nome.starts_with("destilar") || nome.starts_with("escrever"))
                .map(|(_, s)| s)
                .sum();
            let espera = soma - de_fundo;

            println!("
espera do usuário          {espera:>6.2} s  <- até a fala começar");
            println!(
                "em segundo plano           {de_fundo:>6.2} s  ({:.0}% do trabalho, e ele já não espera por isso)",
                de_fundo / soma * 100.0
            );
        });
    }

    /// Manda o comando na casa de verdade, com o chaveiro real. Fora do `cargo test`
    /// comum porque depende dos aparelhos ligados.
    ///
    /// `JARVIS_FRASE="lâmpada mesa" JARVIS_COR=azul cargo test --lib -- --ignored     ///   --nocapture comanda_a_casa_de_verdade`
    #[test]
    #[ignore]
    fn comanda_a_casa_de_verdade() {
        let dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
            .join("com.jarvis.app");
        let chaveiro = Chaveiro::new(&dir);

        let alvo = std::env::var("JARVIS_FRASE").unwrap_or_default();
        let cor = std::env::var("JARVIS_COR").unwrap_or_default();

        for acao in [
            Intent::SmartColor {
                aparelho: alvo.clone(),
                cor: cor.clone(),
            },
            Intent::SmartBright {
                aparelho: alvo.clone(),
                nivel: 40,
            },
            Intent::SmartColor {
                aparelho: alvo.clone(),
                cor: "branco".to_owned(),
            },
        ] {
            match casa(&chaveiro, &acao) {
                Ok(frase) => println!("ok: {frase}"),
                Err(erro) => println!("erro: {erro}"),
            }
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    }

    /// O log tem que dizer o que foi feito COM QUE ALVO — é a única informação que
    /// permite descobrir por que o comando errado disparou.
    #[test]
    fn o_log_mostra_o_verbo_e_o_alvo() {
        let acao = Intent::OpenApp {
            name: "spotify".to_owned(),
        };
        assert_eq!(verbo(&acao), "open_app");
        assert_eq!(argumentos(&acao), "name=spotify");

        // Ação sem argumento não pode virar uma linha vazia no log.
        assert_eq!(verbo(&Intent::MediaNext {}), "media_next");
        assert_eq!(argumentos(&Intent::MediaNext {}), "sem argumentos");
    }

    /// Conversa que não mexeu em nada não gera bolha de log — senão o log vira ruído
    /// embaixo de cada "bom dia" e o usuário para de ler.
    #[test]
    fn conversa_sem_efeito_nao_gera_log() {
        let vazio = Log::novo(
            "bom dia",
            "qwen2.5:3b",
            &Intent::Reply {},
            std::time::Duration::from_millis(400),
            false,
        );
        assert!(vazio.render().is_none());

        let mut com_memoria = Log::novo(
            "meu gato chama Bidu",
            "qwen2.5:3b",
            &Intent::Reply {},
            std::time::Duration::from_millis(900),
            false,
        );
        com_memoria.memoria('+', "gato bidu");

        let texto = com_memoria.render().expect("tem que gerar log");
        assert!(texto.contains("GATILHO    meu gato chama Bidu"));
        assert!(texto.contains("MEMÓRIA    + gato bidu"));
    }

    /// O caso que motivou isto: "salve essa música nas minhas curtidas" virou `reply`
    /// (nada foi executado) e o modelo respondeu que tinha salvado. O log mostrava só o
    /// nome do modelo e o tempo — "entendeu como conversa" e "executou a coisa errada"
    /// ficavam idênticos na tela, e são defeitos com correções opostas.
    #[test]
    fn o_log_diz_qual_verbo_o_modelo_escolheu() {
        let mut log = Log::novo(
            "salve essa musica nas minhas curtidas",
            "qwen2.5vl:3b",
            &Intent::Reply {},
            std::time::Duration::from_millis(3000),
            true,
        );
        log.memoria('+', "musica curtida");

        let texto = log.render().expect("tem que gerar log");
        assert!(
            texto.contains("reply"),
            "o verbo escolhido tem que aparecer"
        );
        assert!(texto.contains("qwen2.5vl:3b"));
    }

    /// Com o log detalhado ligado, até a conversa que não mexeu em nada aparece — é o
    /// único jeito de ver o verbo quando a resposta saiu errada sem executar nada.
    #[test]
    fn o_log_detalhado_mostra_ate_a_conversa_pura() {
        let log = Log::novo(
            "bom dia",
            "qwen2.5:3b",
            &Intent::Reply {},
            std::time::Duration::from_millis(400),
            true,
        );

        let texto = log.render().expect("com log detalhado, sempre aparece");
        assert!(texto.contains("GATILHO    bom dia"));
        assert!(texto.contains("reply"));
    }

    /// "quem foi santos dumont?" no meio de um papo não pode abrir uma aba do Google
    /// na cara do usuário; "pesquisa isso aí" pode.
    #[test]
    fn so_abre_a_aba_quando_mandaram_pesquisar() {
        for ordem in [
            "pesquisa preço do dólar",
            "procura a receita de pão de queijo",
            "quem foi tesla, pesquisa aí",
            "busca isso no google",
            "abre o google e vê quem descobriu o brasil",
        ] {
            assert!(pediu_a_aba(ordem), "devia abrir a aba: {ordem:?}");
        }

        for papo in [
            "quem foi santos dumont?",
            "o que é uma black hole",
            "qual a capital da austrália",
            "como faz pão de queijo",
        ] {
            assert!(!pediu_a_aba(papo), "não devia abrir a aba: {papo:?}");
        }
    }

    /// O `AcaoDeUi` é espelhado À MÃO em `src/lib/tauri/events.ts`, e nada no build
    /// liga um lado ao outro. Renomear uma variante aqui não quebra nada que apite: o
    /// sintoma seria "pedi para abrir o site e não aconteceu nada", sem erro nenhum,
    /// nem no Rust nem no console. Este teste é o apito.
    #[test]
    fn o_pedido_de_aba_sai_na_forma_que_a_tela_espera() {
        assert_eq!(
            serde_json::to_value(AcaoDeUi::AbrirSite {
                url: "https://www.youtube.com".to_owned(),
            })
            .unwrap(),
            serde_json::json!({ "tipo": "abrir-site", "url": "https://www.youtube.com" }),
        );
        assert_eq!(
            serde_json::to_value(AcaoDeUi::Pesquisar {
                query: "preço do dólar".to_owned(),
            })
            .unwrap(),
            serde_json::json!({ "tipo": "pesquisar", "query": "preço do dólar" }),
        );
    }

    /// O modelo alucina número grande, e "aumenta 200" não pode estourar o `i8` nem
    /// virar um volume absurdo.
    #[test]
    fn passos_ficam_no_teto() {
        assert_eq!(passos(0), PASSO_DE_VOLUME);
        assert_eq!(passos(1), PASSO_DE_VOLUME);
        assert_eq!(passos(3), PASSO_DE_VOLUME * 3);
        assert_eq!(passos(200), PASSO_DE_VOLUME * 5);
    }

    /// O caminho explícito ("lembra que...") manda só a frase, sem título. Um assunto
    /// ruim aqui vira um arquivo com nome ruim na pasta que o usuário vai abrir.
    #[test]
    fn deriva_um_assunto_legivel_do_fato() {
        assert_eq!(
            assunto_de("Acorda 6h30 para a academia."),
            "acorda 6h30 academia"
        );
        assert_eq!(
            assunto_de("Tem um gato chamado Bidu."),
            "tem gato chamado bidu"
        );
        assert_eq!(assunto_de("de um"), "anotacao");
    }
}
