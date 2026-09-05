use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::agent::{self, AcaoDeUi, AgentError};
use crate::core::automation::AutomationState;
use crate::core::cameras::Catalogo;
use crate::core::casa::chaveiro::Chaveiro;
use crate::core::chat::{ChatMessage, ChatResponse, Role};
use crate::core::memory::{Avaliacao, Erro, Memoria, Veredito};
use crate::core::lugar::Localizador;
use crate::core::services::Services;
use crate::state::AppState;

/// Recebe a mensagem do usuário, deixa o agente entender e agir, e devolve a resposta.
///
/// Uma jogada pode empurrar DUAS mensagens no histórico: o log do gatilho e a
/// resposta. A assinatura não muda por causa disso — o frontend recarrega o histórico
/// depois de enviar, que é o que mantém o espelho fiel sem inventar um segundo canal.
/// Pedido do agente para a UI fazer algo que só ela sabe fazer — hoje, ligar e
/// desligar a câmera. Escutado por `src/hooks/useSensorEvents.ts`.
const UI_ACTION_EVENT: &str = "jarvis://ui-action";

/// Uma frase da resposta, assim que ela fica pronta — antes de o modelo escrever o resto.
///
/// É evento, e não o retorno do comando, porque o retorno é UM só e chega no fim: quem
/// espera por ele lê a resposta inteira depois de já a ter ouvido inteira. Com isto a
/// bolha cresce no mesmo passo da fala. Escutado por `src/hooks/useSensorEvents.ts`.
const REPLY_CHUNK_EVENT: &str = "jarvis://reply-chunk";

/// As notas mudaram — o grafo do conhecimento que está aberto precisa se redesenhar.
///
/// Sai só quando mudou de verdade (a `Memoria::versao` diz), e não a cada mensagem: um
/// "bom dia" não muda nota nenhuma, e recarregar o grafo por causa dele seria trabalho
/// para desenhar exatamente a mesma coisa.
///
/// Vazio de propósito: o que a tela faz com isto é reler o grafo inteiro, e mandar o que
/// mudou junto seria um segundo formato para manter em sincronia com o primeiro.
const MEMORIA_EVENT: &str = "jarvis://memoria-mudou";

/// Uma frase, com o crachá do turno que a gerou.
///
/// O `turno` existe por causa da interrupção: mandar uma pergunta nova enquanto ele
/// responde a anterior **corta a fala**, mas não o texto — a resposta velha continua
/// chegando do modelo por alguns segundos. Sem o crachá, aquelas frases entrariam na bolha
/// da pergunta nova. Quem o gera é a tela, que é quem sabe qual turno está desenhado.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Pedaco<'a> {
    turno: &'a str,
    frase: &'a str,
}

/// Manda um pedido do agente para a tela.
///
/// Existe como função porque agora há DOIS momentos que pedem: o meio do turno, pelo
/// `ao_pedir_ui` (a aba da busca, que precisa abrir enquanto ele pesquisa), e o fim dele,
/// pelo `Outcome::ui`. Os dois têm que emitir o mesmo evento com o mesmo log, senão a
/// tela passa a se comportar diferente conforme o caminho que o agente tomou.
///
/// Falha de emissão é engolida: não há UI escutando, e não há o que fazer a respeito.
fn pedir_a_ui(app: &AppHandle, acao: AcaoDeUi) {
    eprintln!("[jarvis] ui-action {acao:?}");
    let _ = app.emit(UI_ACTION_EVENT, acao);
}

/// Põe uma fala do Jarvis no histórico, sem ter havido pergunta.
///
/// Existe para a saudação de quando o app abre — a única coisa que ele diz por conta
/// própria. **Vai para a `Memoria` e não só para a tela**: o histórico mora no backend, e
/// uma mensagem empurrada só no frontend sumiria no `loadHistory` seguinte, deixando a
/// conversa começar com a resposta do usuário a uma pergunta que não está mais lá.
///
/// Não passa pelo agente de propósito: não há nada a interpretar numa frase que o próprio
/// app compôs, e mandá-la ao roteador gastaria uma chamada ao modelo para nada.
#[tauri::command]
pub fn announce(content: String, memoria: State<'_, Memoria>) -> Result<(), String> {
    let content = content.trim().to_owned();
    if content.is_empty() {
        return Err("mensagem vazia".to_owned());
    }

    memoria.push_message(ChatMessage::new(Role::Assistant, content));
    Ok(())
}

#[tauri::command]
// Dez parâmetros porque no Tauri a lista de argumentos É o mecanismo de injeção: cada
// `State` que o comando precisa entra como um parâmetro, e agrupá-los numa struct só
// para agradar o lint criaria um tipo que existe por causa do lint.
#[allow(clippy::too_many_arguments)]
pub async fn send_message(
    content: String,
    turno: String,
    app: AppHandle,
    state: State<'_, AppState>,
    memoria: State<'_, Memoria>,
    services: State<'_, Services>,
    automation: State<'_, AutomationState>,
    chaveiro: State<'_, Chaveiro>,
    catalogo: State<'_, Catalogo>,
    localizador: State<'_, Localizador>,
) -> Result<ChatResponse, String> {
    let content = content.trim().to_owned();
    if content.is_empty() {
        return Err("mensagem vazia".to_owned());
    }

    memoria.push_message(ChatMessage::new(Role::User, content.clone()));

    let settings = state.settings();
    let http = state.http();

    // Sobe o Ollama se ninguém atender. O resultado é ignorado de propósito: se não
    // subir, quem dá a mensagem boa (com o link e o comando do `ollama pull`) é o
    // `AgentError::Offline`, logo abaixo — reportar aqui seria pior e em dobro.
    if !settings.ollama_model.trim().is_empty() {
        let _ = services.ensure_ollama(&http, &settings.ollama_url).await;
    }

    // O retrato das notas ANTES do turno. Comparado no fim, ele diz se a busca aprendeu
    // alguma coisa — e é o que decide se a tela precisa saber.
    let notas_antes = memoria.versao();

    // A fila da fala abre ANTES do modelo pensar, e é de propósito: subir o servidor de
    // voz leva segundos na primeira vez do dia, e eles cabem inteiros dentro do tempo que
    // o Ollama leva para escrever a primeira frase. Depois disso ela fica só esperando.
    let (frases, recebidas) = tokio::sync::mpsc::unbounded_channel::<String>();
    let fala = tauri::async_runtime::spawn(super::voice::falar_em_fila(app.clone(), recebidas));

    // Cada frase pronta vai para dois lugares ao mesmo tempo: a boca e a tela. É essa
    // dupla que faz a resposta ser ouvida e lida no mesmo passo, em vez de aparecer
    // inteira no fim.
    let outcome = {
        let ao_falar = |frase: &str| {
            let _ = app.emit(
                REPLY_CHUNK_EVENT,
                Pedaco {
                    turno: &turno,
                    frase,
                },
            );
            let _ = frases.send(frase.to_owned());
        };

        // O gêmeo do `ao_falar` para a TELA: fecha `Sync` sem esforço porque só captura o
        // `&AppHandle`. É por ele que a aba da busca abre no começo do turno, e não depois
        // da resposta pronta — ver `agent::AoPedirUi`.
        let ao_pedir_ui = |acao: AcaoDeUi| pedir_a_ui(&app, acao);

        agent::handle(
            &http,
            &settings,
            memoria.inner(),
            automation.inner(),
            chaveiro.inner(),
            catalogo.inner(),
            localizador.inner(),
            &content,
            &ao_falar,
            &ao_pedir_ui,
        )
        .await
    };

    // Fechar a ponta de escrita é o que diz à fila "não vem mais frase". Sem isto ela
    // esperaria para sempre — inclusive quando o turno termina em erro.
    drop(frases);
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(erro) => {
            let _ = fala.await;
            return Err(stringify(erro));
        }
    };

    // Ordem importa: o log entra ANTES da resposta, então a conversa se lê como
    // usuário → o que ele entendeu, fez e guardou → o que ele respondeu.
    if let Some(trace) = outcome.trace {
        memoria.push_message(ChatMessage::new(Role::System, trace));
    }

    // O que o turno decidiu só no fim — hoje, os dois caminhos da visão. O que pede a
    // tela no MEIO do turno já saiu pelo `ao_pedir_ui` lá em cima.
    if let Some(acao) = outcome.ui {
        pedir_a_ui(&app, acao);
    }

    let reply = ChatMessage::new(Role::Assistant, outcome.reply);
    memoria.push_message(reply.clone());

    // O que a busca guardou e o que o "esquece X" apagou já aconteceram: quem escreve
    // durante o turno é o `handle`. O aviso sai aqui, e o da manutenção sai depois dela,
    // que é quando a nota da conversa fica pronta.
    if memoria.versao() != notas_antes {
        let _ = app.emit(MEMORIA_EVENT, ());
    }

    // **A resposta sai primeiro; o Jarvis anota depois.**
    //
    // Destilar o assunto e reescrever a nota são de duas a três chamadas ao Ollama, e
    // foram medidas em 1,29 s de um turno de 4,79 s — 27% do tempo, que o usuário passava
    // esperando calado por um trabalho que não muda uma vírgula do que ele vai ouvir.
    //
    // O `spawn` mora AQUI, e não no `core`: a tarefa precisa ser `'static`, e o que o
    // `core` recebe são referências emprestadas do estado do Tauri. Aqui há `AppHandle`,
    // e de dentro dela dá para pegar o estado de novo.
    if let Some(servico) = outcome.manutencao {
        let app = app.clone();
        let http = http.clone();
        let settings = settings.clone();

        tauri::async_runtime::spawn(async move {
            let memoria = app.state::<Memoria>();
            let antes = memoria.versao();

            crate::core::agent::manter_memoria(&http, &settings, &memoria, &servico).await;

            // A nota da conversa nasce aqui, segundos depois de a resposta ter saído. É o
            // caso que mais aparece na tela: você conversa, e o grafo ganha um nó sozinho.
            if memoria.versao() != antes {
                let _ = app.emit(MEMORIA_EVENT, ());
            }
        });
    }

    // **Só volta quando ele calou.** O modelo já terminou de escrever, mas a última frase
    // ainda está no ar — e é neste retorno que o modo conversa reabre o microfone, então
    // voltar antes seria ele ouvindo a si mesmo.
    //
    // Depois do `spawn` da manutenção, para as notas serem escritas ENQUANTO ele fala.
    let _ = fala.await;

    Ok(ChatResponse::new(reply))
}

/// Sobe o Ollama e deixa o modelo carregado antes de alguém perguntar qualquer coisa.
///
/// Chamado uma vez, no `setup` do app, e sem ninguém esperando pelo resultado. **Os
/// números que justificam isto estão no `agent::aquecer`**: 8,5 s de modelo indo para a
/// VRAM mais 0,8 s de prompt do roteador que a primeira mensagem pagava sozinha.
///
/// Falha em silêncio de propósito. Ollama não instalado, modelo não baixado, máquina sem
/// GPU — nada disso vira aviso aqui, porque aqui ninguém pediu nada: a mensagem boa (com
/// o link e o `ollama pull`) continua saindo na primeira pergunta, que é onde ela ajuda.
pub fn aquecer(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let settings = app.state::<AppState>().settings();
        if settings.ollama_model.trim().is_empty() {
            return;
        }

        let http = app.state::<AppState>().http();
        if !app
            .state::<Services>()
            .ensure_ollama(&http, &settings.ollama_url)
            .await
        {
            return;
        }

        // Os apelidos entram no prompt, então eles têm que entrar aqui também: sem eles o
        // prefixo aquecido não é o mesmo que a primeira pergunta vai mandar, e o cache não
        // serve para nada.
        let apelidos = app.state::<Memoria>().apelidos();

        let relogio = std::time::Instant::now();
        match agent::aquecer(
            &http,
            &settings.ollama_url,
            &settings.ollama_model,
            &settings.assistant_name,
            &apelidos,
        )
        .await
        {
            Ok(()) => println!(
                "[jarvis] modelo quente em {:.1} s",
                relogio.elapsed().as_secs_f32()
            ),
            Err(erro) => eprintln!("[jarvis] não consegui aquecer o modelo: {erro}"),
        }
    });
}

/// A UI chama isto ao montar. O histórico agora vem do disco, então a conversa
/// sobrevive a fechar o app — não só a esconder a janela.
#[tauri::command]
pub fn get_history(memoria: State<'_, Memoria>) -> Vec<ChatMessage> {
    memoria.historico()
}

/// Limpa a conversa da tela. NÃO apaga as notas nem o diário em `conversas/` — apagar
/// o que ele aprendeu é outro pedido, e tem outro caminho ("esquece X").
#[tauri::command]
pub fn clear_history(memoria: State<'_, Memoria>) {
    memoria.limpar_historico();
}

/// Põe uma nota numa resposta: acertou, passou perto, ou errou.
///
/// **O que ENSINA é a `correcao`, não o veredito.** Um "errou" sozinho não diz a um modelo
/// de 3B o que era esperado — ele não tem como adivinhar. Por isso a tela pede a resposta
/// certa quando você marca erro, e por isso o veredito sem correção só vira registro.
///
/// O que acontece com a correção depende do TIPO, e a distinção é o coração disto:
///
/// - **Errou o FATO** vira nota sobre AQUELE assunto, do [`crate::core::memory::Tipo`]
///   `Corrigido`. Ela volta sozinha quando o assunto voltar, pela mesma busca lexical que
///   já serve a conversa e (desde a mudança do `pesquisar_e_responder`) também a busca na
///   web. O nome do assunto sai do `destilar_assunto`, o mesmo que já nomeia a nota de uma
///   conversa — não há extração nova aqui.
/// - **Respondeu MAL** não tem assunto ao qual se prender: vira regra na nota reservada
///   `jeito-de-responder`, que entra no prompt de toda conversa. Com teto, e o porquê do
///   teto está no `memory::REGRAS_DE_JEITO`.
///
/// Falhar em destilar o assunto NÃO perde a avaliação: ela já foi para o disco antes, e o
/// que se perde é só a nota. É a mesma ordem do `pesquisar_e_responder` — gravar primeiro,
/// falar depois.
#[tauri::command(async)]
pub async fn avaliar_resposta(
    id: String,
    veredito: Veredito,
    tipo: Option<Erro>,
    correcao: Option<String>,
    state: State<'_, AppState>,
    memoria: State<'_, Memoria>,
) -> Result<(), String> {
    let Some((pergunta, resposta)) = memoria.troca_de(&id) else {
        return Err(format!("não achei a resposta \"{id}\" para avaliar"));
    };

    let correcao = correcao
        .map(|texto| texto.trim().to_owned())
        .filter(|texto| !texto.is_empty());

    memoria.registrar_avaliacao(Avaliacao {
        mensagem: id,
        quando: chrono::Utc::now().timestamp_millis(),
        veredito,
        tipo,
        pergunta: pergunta.clone(),
        resposta,
        correcao: correcao.clone(),
    });

    let (Some(correcao), Some(tipo)) = (correcao, tipo) else {
        return Ok(());
    };

    agent::aprender_com_a_correcao(
        &state.http(),
        &state.settings(),
        memoria.inner(),
        &pergunta,
        tipo,
        &correcao,
    )
    .await
    .map_err(stringify)
}

fn stringify(error: AgentError) -> String {
    error.to_string()
}
