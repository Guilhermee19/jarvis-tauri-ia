use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::agent::{self, AgentError};
use crate::core::automation::AutomationState;
use crate::core::cameras::Catalogo;
use crate::core::casa::chaveiro::Chaveiro;
use crate::core::chat::{ChatMessage, ChatResponse, Role};
use crate::core::memory::Memoria;
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

    // O `core` não conhece Tauri, então quem emite é a fronteira. Falha de emissão é
    // engolida: a resposta já está composta, e sem UI escutando não há o que fazer.
    if let Some(acao) = outcome.ui {
        eprintln!("[jarvis] ui-action {acao:?}");
        let _ = app.emit(UI_ACTION_EVENT, acao);
    }

    let reply = ChatMessage::new(Role::Assistant, outcome.reply);
    memoria.push_message(reply.clone());

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
            crate::core::agent::manter_memoria(&http, &settings, &memoria, &servico).await;
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

fn stringify(error: AgentError) -> String {
    error.to_string()
}
