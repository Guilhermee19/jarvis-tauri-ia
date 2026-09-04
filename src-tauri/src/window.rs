//! Ciclo de vida da janela principal.
//!
//! Um lugar só para mostrar/esconder, usado pela bandeja, pela barra de título e
//! (futuramente) pela wake word, que precisa trazer a janela para frente ao ouvir
//! o gatilho.

use tauri::{AppHandle, LogicalSize, Manager, WebviewWindow, Window, WindowEvent};

pub const MAIN_WINDOW_LABEL: &str = "main";

/// Em quantas partes o monitor é dividido para dar o tamanho de abertura.
///
/// Dois, nas DUAS dimensões — metade da largura por metade da altura, que dá um QUARTO
/// da área. É o mesmo retângulo do Win+seta, e é essa a leitura de "um quarto da tela"
/// que a pessoa tem na cabeça ao pedir; um quarto da ÁREA mantendo a proporção daria
/// 0,7 de cada lado, uma janela que parece quase inteira e não é.
const DIVISOR_DA_TELA: f64 = 2.0;

/// Espelham `minWidth`/`minHeight` do `tauri.conf.json`, e existem porque o cálculo
/// acontece ANTES de a janela existir do tamanho final: num monitor pequeno (ou num
/// 1366×768 girado) a metade cai abaixo do mínimo, e o Tauri corrigiria depois — mas
/// aí o `center` já teria centralizado o tamanho errado.
const LARGURA_MINIMA: f64 = 480.0;
const ALTURA_MINIMA: f64 = 540.0;

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| format!("janela \"{MAIN_WINDOW_LABEL}\" não encontrada"))
}

fn to_message(error: tauri::Error) -> String {
    error.to_string()
}

/// Dá à janela o tamanho de abertura e SÓ ENTÃO a mostra.
///
/// **O tamanho não cabe no `tauri.conf.json`.** Lá ele é um par de pixels fixos, e um
/// número que serve num 1080p é uma tarja num ultrawide e não cabe num notebook — a
/// fração do monitor é a mesma nos três, os pixels não. Os 620×700 do arquivo ficam
/// como o que vale quando NENHUM monitor responde, que é o único caso em que não há o
/// que calcular.
///
/// **E a janela nasce escondida** (`"visible": false` no mesmo arquivo). Com ela
/// visível desde o começo, todo start desenhava os 620×700 do config e pulava para o
/// tamanho de verdade um quadro depois — um salto que a pessoa vê em toda abertura.
///
/// Nada aqui devolve erro para cima: falhar em MEDIR o monitor é motivo para abrir do
/// tamanho do config, nunca para não abrir. Por isso o `show` fica fora do caminho de
/// erro — uma janela que não aparece é pior que uma do tamanho errado, e o app inteiro
/// mora dentro dela.
pub fn abrir_dimensionada(app: &AppHandle) {
    // Não achar a janela aqui significa que ela não existe — não há o que mostrar. Mas
    // como ela agora nasce ESCONDIDA, este é o único caminho que termina com o app
    // rodando sem nada na tela, e um app invisível sem uma linha no log é indepurável.
    let window = match main_window(app) {
        Ok(janela) => janela,
        Err(erro) => {
            eprintln!("[jarvis] {erro} — o app subiu sem janela");
            return;
        }
    };

    if let Err(erro) = um_quarto_da_tela(&window) {
        eprintln!("[jarvis] não consegui medir o monitor ({erro}) — abrindo no tamanho do tauri.conf.json");
    }

    let _ = window.show();
    let _ = window.set_focus();
}

/// Metade da largura por metade da altura do monitor em que a janela está, centralizada.
///
/// A conta é em unidades LÓGICAS, e não nos pixels físicos que o monitor reporta: num
/// monitor a 150% os dois números diferem por metade, e comparar um mínimo escrito em
/// lógico contra uma medida em físico deixaria a janela pequena demais exatamente nas
/// telas em que ela mais precisa de espaço.
fn um_quarto_da_tela(window: &WebviewWindow) -> Result<(), String> {
    // O monitor ATUAL primeiro: quem tem duas telas abriu o app na que estava usando, e
    // dimensionar pela primária daria o tamanho da tela errada. O primário é o retorno
    // para quando a janela ainda não caiu em nenhuma.
    let monitor = match window.current_monitor().map_err(to_message)? {
        Some(atual) => atual,
        None => window
            .primary_monitor()
            .map_err(to_message)?
            .ok_or_else(|| "nenhum monitor respondeu".to_owned())?,
    };

    let tela = monitor.size().to_logical::<f64>(monitor.scale_factor());

    window
        .set_size(LogicalSize::new(
            (tela.width / DIVISOR_DA_TELA).max(LARGURA_MINIMA),
            (tela.height / DIVISOR_DA_TELA).max(ALTURA_MINIMA),
        ))
        .map_err(to_message)?;

    // Recentralizar é obrigatório, e não enfeite: o `"center": true` do config rodou
    // com os 620×700, então sem isto a janela nova fica com o canto onde estava o canto
    // da antiga — encostada para cima e para a esquerda do centro.
    window.center().map_err(to_message)
}

pub fn show(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;
    window.unminimize().map_err(to_message)?;
    window.show().map_err(to_message)?;
    window.set_focus().map_err(to_message)?;
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<(), String> {
    main_window(app)?.hide().map_err(to_message)
}

pub fn minimize(app: &AppHandle) -> Result<(), String> {
    main_window(app)?.minimize().map_err(to_message)
}

/// Alterna maximizar/restaurar e devolve o estado NOVO.
///
/// Devolver o estado em vez de `()` poupa a UI de uma segunda chamada só para saber
/// que ícone desenhar — e evita a janela de tempo em que o botão mostra o desenho
/// errado.
pub fn toggle_maximize(app: &AppHandle) -> Result<bool, String> {
    let window = main_window(app)?;

    if window.is_maximized().map_err(to_message)? {
        window.unmaximize().map_err(to_message)?;
        Ok(false)
    } else {
        window.maximize().map_err(to_message)?;
        Ok(true)
    }
}

/// Existe porque o usuário maximiza por fora também — Win+↑, duplo clique na barra,
/// atalho do gerenciador de janelas. Sem consultar, o ícone dessincroniza.
pub fn is_maximized(app: &AppHandle) -> Result<bool, String> {
    main_window(app)?.is_maximized().map_err(to_message)
}

pub fn toggle(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;

    // Minimizada ainda conta como "visível" no Windows, então esconder nesse caso
    // faria o clique na bandeja parecer que não fez nada.
    let is_showing =
        window.is_visible().map_err(to_message)? && !window.is_minimized().map_err(to_message)?;

    if is_showing {
        hide(app)
    } else {
        show(app)
    }
}

/// Fechar no X esconde para a bandeja em vez de encerrar — o Jarvis é feito para
/// ficar sempre rodando em background. Sair de verdade só pelo menu da bandeja.
pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == MAIN_WINDOW_LABEL {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}
