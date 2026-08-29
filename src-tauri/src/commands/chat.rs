use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::agent::{self, AgentError};
use crate::core::automation::AutomationState;
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

#[tauri::command]
// Oito parâmetros porque no Tauri a lista de argumentos É o mecanismo de injeção: cada
// `State` que o comando precisa entra como um parâmetro, e agrupá-los numa struct só
// para agradar o lint criaria um tipo que existe por causa do lint.
#[allow(clippy::too_many_arguments)]
pub async fn send_message(
    content: String,
    app: AppHandle,
    state: State<'_, AppState>,
    memoria: State<'_, Memoria>,
    services: State<'_, Services>,
    automation: State<'_, AutomationState>,
    chaveiro: State<'_, Chaveiro>,
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

    let outcome = agent::handle(
        &http,
        &settings,
        memoria.inner(),
        automation.inner(),
        chaveiro.inner(),
        localizador.inner(),
        &content,
    )
    .await
    .map_err(stringify)?;

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
