use tauri::{AppHandle, State};

use crate::core::system::{self, SystemError};
use crate::navegador::{Aba, Area, Navegador};

/// O que a tela precisa saber depois de qualquer mexida: quais abas existem e qual está
/// na frente.
///
/// Devolvido por todo comando que muda alguma coisa, em vez de um `get` separado depois:
/// duas chamadas dariam uma janela em que a tela desenha um estado que já mudou.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoDoNavegador {
    pub abas: Vec<Aba>,
    pub ativa: Option<String>,
}

fn estado(navegador: &Navegador) -> EstadoDoNavegador {
    EstadoDoNavegador {
        abas: navegador.abas(),
        ativa: navegador.ativa(),
    }
}

/// Abre um endereço numa aba nova.
///
/// Aceita o que a pessoa disser — "youtube", "youtube.com", a URL inteira — porque usa a
/// **mesma** normalização do `open_url` que manda para o navegador do sistema. Duas
/// ideias do que é uma URL válida dariam dois comportamentos para a mesma frase.
///
/// **O `(async)` não é enfeite.** Sem ele este comando roda na thread principal, dentro do
/// callback do WebView2, e criar um webview dali abre um message loop aninhado que trava o
/// app inteiro. A explicação completa está no topo de `navegador.rs`.
#[tauri::command(async)]
pub fn browser_open(
    app: AppHandle,
    url: String,
    navegador: State<'_, Navegador>,
) -> Result<EstadoDoNavegador, String> {
    let alvo = system::site(&url).map_err(|erro: SystemError| erro.to_string())?;
    navegador
        .abrir(&app, &alvo)
        .map_err(|erro| erro.to_string())?;

    Ok(estado(&navegador))
}

/// Abre uma busca numa aba nova. `(async)` pelo mesmo motivo do `browser_open`.
#[tauri::command(async)]
pub fn browser_search(
    app: AppHandle,
    query: String,
    navegador: State<'_, Navegador>,
) -> Result<EstadoDoNavegador, String> {
    let alvo = system::search_url(&query).map_err(|erro: SystemError| erro.to_string())?;
    navegador
        .abrir(&app, &alvo)
        .map_err(|erro| erro.to_string())?;

    Ok(estado(&navegador))
}

#[tauri::command]
pub fn browser_state(navegador: State<'_, Navegador>) -> EstadoDoNavegador {
    estado(&navegador)
}

#[tauri::command]
pub fn browser_select(
    app: AppHandle,
    id: String,
    navegador: State<'_, Navegador>,
) -> Result<EstadoDoNavegador, String> {
    navegador
        .ativar(&app, &id)
        .map_err(|erro| erro.to_string())?;

    Ok(estado(&navegador))
}

/// Fecha uma aba. `(async)` porque destruir o controlador do WebView2 de dentro do
/// callback de um webview irmão é a outra metade da armadilha da thread principal.
#[tauri::command(async)]
pub fn browser_close(
    app: AppHandle,
    id: String,
    navegador: State<'_, Navegador>,
) -> EstadoDoNavegador {
    navegador.fechar(&app, &id);

    estado(&navegador)
}

/// Manda uma aba existente para outro endereço — é o que a barra de endereço faz.
#[tauri::command]
pub fn browser_navigate(
    app: AppHandle,
    id: String,
    url: String,
    navegador: State<'_, Navegador>,
) -> Result<EstadoDoNavegador, String> {
    let alvo = system::site(&url).map_err(|erro: SystemError| erro.to_string())?;
    navegador
        .navegar(&app, &id, &alvo)
        .map_err(|erro| erro.to_string())?;

    Ok(estado(&navegador))
}

/// Volta (`-1`) ou avança (`1`) no histórico da aba.
#[tauri::command]
pub fn browser_history(
    app: AppHandle,
    id: String,
    passo: i32,
    navegador: State<'_, Navegador>,
) -> Result<(), String> {
    navegador
        .historico(&app, &id, passo)
        .map_err(|erro| erro.to_string())
}

/// Onde as abas devem ser desenhadas, em pixels lógicos da janela.
///
/// `None` quer dizer "o painel está fechado" e esconde todas. **É obrigatório chamar**:
/// o webview é uma camada nativa acima do HTML, e sem alguém dizer onde ele fica ele
/// nunca aparece — ou pior, continua aparecendo sobre um painel que já fechou.
///
/// Este fica **síncrono de propósito**, ao contrário dos que criam aba: ele não cria nem
/// destrói webview nenhum (só move), e a tela o dispara a cada quadro de arrasto. Na
/// thread principal as chamadas chegam na ordem em que foram feitas; jogadas na piscina
/// de threads, duas medidas poderiam se ultrapassar e a última a valer seria a velha.
#[tauri::command]
pub fn browser_bounds(app: AppHandle, area: Option<Area>, navegador: State<'_, Navegador>) {
    navegador.definir_area(&app, area);
}

/// Manda a página da aba para o navegador de verdade.
///
/// A saída de emergência: login com senha salva, extensão, imprimir, PDF — o navegador
/// embutido é simples de propósito, e o que ele não faz precisa ter para onde ir.
///
/// `(async)` porque o `ShellExecuteW` sobe um navegador inteiro e demora o que quiser —
/// na thread principal isso seria a janela congelando até o Chrome abrir.
#[tauri::command(async)]
pub fn browser_external(id: String, navegador: State<'_, Navegador>) -> Result<(), String> {
    let Some(url) = navegador.url_de(&id) else {
        return Err("essa aba não existe mais".to_owned());
    };

    system::open_url(&url).map_err(|erro: SystemError| erro.to_string())
}
