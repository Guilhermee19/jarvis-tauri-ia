use tauri::{AppHandle, State};

use crate::core::music;
use crate::core::system::{self, MediaKey};
use crate::state::AppState;
use crate::window;

/// Controle de janela exposto à UI. Fica no Rust (e não no `@tauri-apps/api/window`)
/// para que a bandeja, a wake word e a barra de título compartilhem exatamente a
/// mesma lógica de mostrar/esconder.
#[tauri::command]
pub fn show_window(app: AppHandle) -> Result<(), String> {
    window::show(&app)
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> Result<(), String> {
    window::hide(&app)
}

#[tauri::command]
pub fn toggle_window(app: AppHandle) -> Result<(), String> {
    window::toggle(&app)
}

#[tauri::command]
pub fn minimize_window(app: AppHandle) -> Result<(), String> {
    window::minimize(&app)
}

/// Devolve o estado depois de alternar, para o botão trocar de ícone sem uma segunda
/// viagem pelo IPC.
#[tauri::command]
pub fn toggle_maximize_window(app: AppHandle) -> Result<bool, String> {
    window::toggle_maximize(&app)
}

#[tauri::command]
pub fn is_window_maximized(app: AppHandle) -> Result<bool, String> {
    window::is_maximized(&app)
}

/// O que dá para saber do player SEM OAuth: o título da janela do Spotify.
///
/// `"Artista - Música"` enquanto toca, `"Spotify Premium"` quando pausa — medido. É
/// esse par que deixa o widget congelar a barra de progresso na pausa em vez de
/// continuar contando e mentir.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    /// `None` com o Spotify fechado ou parado.
    pub titulo: Option<String>,
    pub tocando: bool,
}

#[tauri::command]
pub fn now_playing() -> NowPlaying {
    match system::titulo_do_spotify() {
        Some(titulo) if !system::esta_parado(&titulo) => NowPlaying {
            titulo: Some(titulo),
            tocando: true,
        },
        // Parado e fechado viram o mesmo estado: em ambos não há o que mostrar
        // tocando, e o widget já tem a faixa que ele mesmo pediu.
        _ => NowPlaying {
            titulo: None,
            tocando: false,
        },
    }
}

/// Capa e duração da música que está tocando, descobertas pelo TÍTULO da janela.
///
/// É o que faz o widget mostrar a arte do álbum de qualquer música — não só das que o
/// Jarvis mandou tocar. Devolve `None` em vez de erro quando não há credencial do
/// Spotify ou a busca não acha: nesse caso o widget cai no texto do título, que já é
/// melhor que nada.
///
/// Quem evita chamar isto a cada segundo é o frontend, que só pergunta quando o título
/// muda.
#[tauri::command]
pub async fn identify_track(
    title: String,
    state: State<'_, AppState>,
) -> Result<Option<music::Faixa>, String> {
    let settings = state.settings();

    let achada = music::buscar(
        &state.http(),
        &title,
        &settings.spotify_client_id,
        &settings.spotify_client_secret,
    )
    .await;

    // Uma linha por identificação, no console do `tauri dev`. "Capa vazia" e "não
    // identificou" são falhas silenciosas na tela — o widget mostra um quadrado
    // cinza e não dá pista nenhuma de qual dos dois aconteceu.
    match &achada {
        Ok(faixa) => eprintln!(
            "[jarvis] identify_track {title:?} -> {} · capa={}",
            faixa.como_texto(),
            faixa.capa.as_deref().unwrap_or("(vazia)")
        ),
        Err(erro) => eprintln!("[jarvis] identify_track {title:?} -> {erro}"),
    }

    Ok(achada.ok())
}

/// Botões de transporte do widget. Mesma tecla de mídia que o agente usa — quem
/// recebe é o player em foco, então funciona com Spotify, YouTube ou qualquer outro.
#[tauri::command]
pub fn press_media_key(key: String) -> Result<(), String> {
    let tecla = match key.as_str() {
        "play-pause" => MediaKey::PlayPause,
        "next" => MediaKey::Next,
        "previous" => MediaKey::Previous,
        outro => return Err(format!("tecla de mídia desconhecida: {outro}")),
    };

    system::press(tecla).map_err(|erro| erro.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
