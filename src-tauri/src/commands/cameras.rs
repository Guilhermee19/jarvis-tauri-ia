//! As câmeras de segurança, do lado que o frontend alcança.
//!
//! Regra da casa vale aqui como em todo `commands/`: nada de lógica de negócio. O que
//! existe neste arquivo é resolver o `data_dir` (que só o Tauri sabe), garantir o
//! serviço de pé e traduzir erro para `String`.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::cameras::onvif::{self, Direcao};
use crate::core::cameras::varredura::{self, Achado};
use crate::core::cameras::vigia::{self, Sentinela, Vigilancia};
use crate::core::cameras::{go2rtc, Camera, Catalogo};
use crate::core::memory::{Acao, Memoria};
use crate::core::services::Services;
use crate::core::vision;
use crate::state::AppState;

/// Movimento numa câmera vigiada, com o que o modelo viu. Escutado pelo
/// `src/hooks/useSensorEvents.ts`.
const CAMERA_ALERT_EVENT: &str = "jarvis://camera-alert";

/// O que a tela recebe quando algo se mexe.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alerta {
    pub camera: String,
    /// O nome falado, não o id: quem lê o aviso é uma pessoa.
    pub nome: String,
    /// O que o modelo disse ter visto.
    pub resposta: String,
    pub quando: i64,
}

/// O que a tela precisa saber para tocar os streams.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ligado {
    /// A base do go2rtc (`http://127.0.0.1:8646`). A UI monta a partir dela tanto a
    /// `<img>` do quadro quanto a tag do player.
    pub base_url: String,
    pub cameras: Vec<Camera>,
}

/// O que uma sondagem ONVIF descobriu sobre um endereço.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sondagem {
    /// "IPCAM IPCAM (HS-Camera_No1)" — o suficiente para confirmar que é a câmera certa.
    pub descricao: String,
    /// A URL que a própria câmera disse, já pronta para o cadastro. É ela que faz o
    /// campo `rtspUrl` valer mais que qualquer palpite.
    pub rtsp_url: String,
    pub perfis: Vec<String>,
}

/// O que já se conhece, sem encostar na rede — responde na hora.
///
/// Existe para o painel ter o que mostrar no instante em que abre, no mesmo espírito do
/// `known_devices` da casa.
#[tauri::command]
pub fn list_cameras(catalogo: State<'_, Catalogo>) -> Vec<Camera> {
    catalogo.todas()
}

#[tauri::command]
pub fn save_camera(camera: Camera, catalogo: State<'_, Catalogo>) -> Result<(), String> {
    if camera.id.trim().is_empty() {
        return Err("a câmera precisa de um identificador".to_owned());
    }
    if camera.host.trim().is_empty() {
        return Err("a câmera precisa de um endereço na rede".to_owned());
    }

    catalogo.guardar(camera).map_err(|erro| erro.to_string())
}

#[tauri::command]
pub fn remove_camera(id: String, catalogo: State<'_, Catalogo>) -> Result<(), String> {
    catalogo.remover(&id).map_err(|erro| erro.to_string())
}

/// Sobe o go2rtc (se ninguém atender) e devolve por onde falar com ele.
///
/// Chamado quando a janela de câmeras abre. É aqui que a subida preguiçosa acontece —
/// quem nunca abre o painel não paga um processo a mais, que é a mesma política do
/// Whisper e dos motores de voz.
#[tauri::command]
pub async fn start_cameras(
    app: AppHandle,
    state: State<'_, AppState>,
    services: State<'_, Services>,
    catalogo: State<'_, Catalogo>,
) -> Result<Ligado, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|erro| format!("sem diretório de dados para achar o go2rtc: {erro}"))?;

    let cameras = catalogo.todas();
    let base_url = services
        .ensure_go2rtc(&state.http(), &data_dir, &cameras)
        .await
        .map_err(|erro| erro.to_string())?;

    // A ronda começa junto com o serviço, e sobrevive à janela fechar: vigilância que só
    // funciona com o painel aberto não é vigilância. A `Sentinela` garante que ela nasce
    // uma vez só, por mais vezes que este comando seja chamado.
    if app.state::<Sentinela>().ligar_uma_vez() {
        rondar(app.clone());
    }

    Ok(Ligado { base_url, cameras })
}

/// O laço que vigia as câmeras marcadas.
///
/// Vive pelo resto da execução do app. Não há caminho de parada porque não há o que
/// parar: sem câmera marcada ele dorme e não pede quadro nenhum, e o custo disso é um
/// `sleep` a cada quatro segundos.
///
/// **Os estados são pegos a cada volta, não capturados uma vez.** É o que faz marcar uma
/// câmera para vigiar valer na hora, sem reiniciar o app — e o que evita segurar um
/// `State` atravessando um `await`, que não compilaria.
fn rondar(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut vigilancia = Vigilancia::new();

        loop {
            tokio::time::sleep(vigia::INTERVALO).await;

            // Bloco síncrono: os guards de `State` morrem aqui, antes de qualquer
            // `await`. O que sai são cópias.
            let (vigiadas, settings, http) = {
                let catalogo = app.state::<Catalogo>();
                let state = app.state::<AppState>();
                let vigiadas: Vec<Camera> = catalogo
                    .todas()
                    .into_iter()
                    .filter(|camera| camera.vigiar)
                    .collect();

                (vigiadas, state.settings(), state.http())
            };

            // Descadastrar uma câmera, ou desmarcá-la, tem que apagar o quadro guardado:
            // comparar com uma cena de horas atrás alertaria por causa da luz que mudou,
            // no exato momento em que alguém religou a vigilância.
            let ids: Vec<String> = vigiadas.iter().map(|camera| camera.id.clone()).collect();
            vigilancia.reter(&ids);

            for camera in vigiadas {
                let Ok(jpeg) = go2rtc::frame_jpeg(&http, &camera.id).await else {
                    // Câmera fora do ar não é evento. Ficar quieto é o certo: um alerta
                    // por quadro perdido treinaria o usuário a ignorar os avisos.
                    continue;
                };

                let Ok(quadro) = vigia::assinatura(&jpeg) else {
                    continue;
                };

                if vigilancia.deve_olhar(&camera.id, quadro) != vigia::Veredito::Olhar {
                    continue;
                }

                olhar_o_movimento(&app, &http, &settings, &camera, &jpeg).await;
            }
        }
    });
}

/// O segundo estágio: mexeu, mas foi gente ou foi a árvore?
///
/// Só chega aqui o que passou pelo portão barato do [`vigia`], que é o desenho inteiro —
/// esta função ocupa a GPU por segundos, e chamá-la a cada quadro deixaria o assistente
/// surdo enquanto vigia.
async fn olhar_o_movimento(
    app: &AppHandle,
    http: &reqwest::Client,
    settings: &crate::config::AppSettings,
    camera: &Camera,
    jpeg: &[u8],
) {
    use base64::Engine;
    let base64 = base64::engine::general_purpose::STANDARD.encode(jpeg);

    let visao = match vision::ver(
        http,
        settings,
        &vision::Imagem {
            base64: &base64,
            mime: "image/jpeg",
        },
        vigia::PERGUNTA,
        vision::Fonte::Camera,
    )
    .await
    {
        Ok(visao) => visao,
        Err(erro) => {
            eprintln!(
                "[jarvis] a vigilância da câmera {} falhou: {erro}",
                camera.id
            );
            return;
        }
    };

    // A rede de segurança contra "uma garagem tranquila ao entardecer" virar notificação.
    if !vigia::vale_avisar(&visao.resposta) {
        return;
    }

    let alerta = Alerta {
        camera: camera.id.clone(),
        nome: camera.nome.clone(),
        resposta: visao.resposta,
        quando: chrono::Utc::now().timestamp_millis(),
    };

    // Fica no histórico de ações, ao lado do resto que o Jarvis fez. É o que permite
    // perguntar depois "o que aconteceu na garagem hoje" e ter o que responder.
    app.state::<Memoria>().registrar_acao(Acao {
        quando: alerta.quando,
        acao: "camera_alerta".to_owned(),
        alvo: format!("{} · {}", alerta.nome, alerta.resposta),
        ok: true,
    });

    // Falha de emissão é engolida: sem UI escutando não há o que fazer, e o alerta já
    // está na memória de qualquer forma.
    let _ = app.emit(CAMERA_ALERT_EVENT, alerta);
}

/// Um quadro da câmera, como `data:` URL.
///
/// É o mesmo formato do `capture_webcam_frame`, e de propósito: a tela mostra os dois
/// numa `<img>` pelo mesmo caminho, e o quadro serve de degradação graciosa quando o
/// player de vídeo não carrega.
#[tauri::command]
pub async fn camera_snapshot(id: String, state: State<'_, AppState>) -> Result<String, String> {
    go2rtc::frame_data_url(&state.http(), &id)
        .await
        .map_err(|erro| erro.to_string())
}

/// As faixas de rede que vale a pena varrer, sem perguntar nada.
///
/// Responde na hora — é a do próprio computador mais a de cada câmera já cadastrada. A
/// tela usa isto para preencher o campo antes de a pessoa digitar, e o caso que motivou
/// isso é o de uma casa com roteador em cascata, onde o PC está numa faixa e as câmeras
/// em outra.
#[tauri::command]
pub fn camera_subnets(catalogo: State<'_, Catalogo>) -> Vec<String> {
    varredura::sugestoes_de_prefixo(&catalogo.todas())
}

/// Varre a faixa e devolve o que parecer câmera, pronto para virar cadastro.
///
/// **Bloqueia por alguns segundos**, como o `discover_devices` da casa: são centenas de
/// sockets, a maioria contra um endereço vazio. Quem chama precisa mostrar que espera.
///
/// Os dois estágios vivem separados no `core` porque um bloqueia e o outro é `async`, e
/// é aqui — na fronteira, que é quem conhece o Tauri — que eles se juntam.
#[tauri::command]
pub async fn scan_cameras(
    prefixo: String,
    state: State<'_, AppState>,
    catalogo: State<'_, Catalogo>,
) -> Result<Vec<Achado>, String> {
    let cadastradas = catalogo.todas();
    let http = state.http();

    // Fora do executor async: são threads que passam a vida bloqueadas num socket, e
    // prendê-las no runtime travaria o resto do app enquanto a varredura corre.
    let candidatos =
        tauri::async_runtime::spawn_blocking(move || varredura::sondar_faixa(&prefixo))
            .await
            .map_err(|erro| format!("a varredura não terminou: {erro}"))?;

    Ok(varredura::identificar_todos(&http, candidatos, &cadastradas).await)
}

/// Pergunta a um endereço, por ONVIF, o que ele é e onde está o stream dele.
///
/// Serve ao cadastro: em vez de o usuário descobrir a URL RTSP por tentativa, a câmera
/// responde. Falha não é fatal — o cadastro manual continua valendo, e é o caminho do
/// DVR, que não fala ONVIF.
#[tauri::command]
pub async fn probe_camera(host: String, state: State<'_, AppState>) -> Result<Sondagem, String> {
    let http = state.http();
    let host = host.trim();

    let descricao = onvif::identificar(&http, host)
        .await
        .map_err(|erro| erro.to_string())?;

    let perfis = onvif::perfis(&http, host)
        .await
        .map_err(|erro| erro.to_string())?;

    // O primeiro perfil é o principal nestas câmeras. O secundário existe e serve para
    // vigiar, mas quem está cadastrando quer ver a imagem boa.
    let rtsp_url = onvif::stream_uri(&http, host, &perfis[0])
        .await
        .map_err(|erro| erro.to_string())?;

    Ok(Sondagem {
        descricao,
        rtsp_url,
        perfis,
    })
}

/// Vira a câmera. Só as ONVIF têm este caminho.
#[tauri::command]
pub async fn move_camera(
    id: String,
    direcao: Direcao,
    state: State<'_, AppState>,
    catalogo: State<'_, Catalogo>,
) -> Result<(), String> {
    let camera = catalogo
        .de(&id)
        .ok_or_else(|| format!("não conheço a câmera \"{id}\""))?;

    mover(&state.http(), &camera, direcao).await
}

/// O movimento em si, compartilhado com o agente.
///
/// Separado do comando porque o roteador de voz precisa exatamente disto e não tem um
/// `State` na mão — e porque a recusa educada de uma câmera sem PTZ é a mesma frase nos
/// dois caminhos.
pub async fn mover(
    http: &reqwest::Client,
    camera: &Camera,
    direcao: Direcao,
) -> Result<(), String> {
    if !camera.tem_ptz() {
        return Err(format!(
            "a câmera \"{}\" não se mexe — ela está num DVR, que não tem esse controle",
            camera.nome
        ));
    }

    // O perfil é perguntado na hora em vez de guardado no cadastro: é uma ida a mais na
    // rede local (milissegundos) e evita um token velho depois de uma troca de firmware,
    // cujo sintoma seria "o PTZ parou de funcionar" sem nada ter mudado aqui.
    let perfis = onvif::perfis(http, &camera.host)
        .await
        .map_err(|erro| erro.to_string())?;

    onvif::mover(http, &camera.host, &perfis[0], direcao)
        .await
        .map_err(|erro| erro.to_string())
}
