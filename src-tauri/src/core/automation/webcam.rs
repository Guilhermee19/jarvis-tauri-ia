//! Webcam via `nokhwa`.
//!
//! A `Camera` do nokhwa não é `Send`, então ela não pode morar no estado
//! compartilhado do Tauri. A sessão é uma thread dona da câmera — mesmo desenho do
//! stream do microfone, pelo mesmo motivo.
//!
//! A thread puxa quadros SEM PARAR e guarda só o mais recente. Isso não é
//! desperdício: `camera.frame()` devolve o próximo quadro da fila do driver, não o
//! atual. Puxando mais devagar do que a câmera produz, a fila cresce e o preview
//! passa a mostrar o passado — que é exatamente a sensação de travamento. Consumindo
//! no ritmo da câmera, a fila nunca acumula e quem lê pega sempre o quadro de agora.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
    Resolution,
};
use nokhwa::{Buffer, Camera};
use serde::Serialize;

use super::{AutomationError, CapturedImage};
use crate::core::lock;

/// Alvo do preview: ~640×480 porque o destino é uma janela de ~600px de largura —
/// resolução maior só engorda o quadro no caminho até a UI.
///
/// PONTO DE TROCA: quando a v0.2 quiser uma foto de qualidade para o modelo ler,
/// é aqui (ou numa sessão paralela) que o alvo sobe.
const TARGET_PIXELS: i64 = 640 * 480;
/// Acima disso, a thread de captura só queima CPU: o preview não mostra mais nada.
const TARGET_FPS: i32 = 30;

/// Quanto [`Session::open`] espera o primeiro quadro. Generoso porque a partida a
/// frio do MediaFoundation costuma levar 1–2 s: `open_stream` volta na hora, mas o
/// dispositivo ainda está acordando.
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(5);

/// Quanto [`Session::grab`] espera com a câmera JÁ transmitindo. Aqui a espera é só
/// para engasgo: em regime, a thread sempre tem um quadro pronto.
const FRAME_TIMEOUT: Duration = Duration::from_millis(1200);

struct Frame {
    bytes: Vec<u8>,
    format: FrameFormat,
    resolution: Resolution,
}

pub struct Session {
    /// Só o quadro mais recente: quem chega atrasado quer o de agora, não o da fila.
    latest: Arc<Mutex<Option<Frame>>>,
    /// Erro que matou o laço (câmera desconectada, por exemplo).
    failure: Arc<Mutex<Option<AutomationError>>>,
    /// O formato que a câmera realmente aceitou, para as mensagens de erro dizerem
    /// com o que estamos lidando em vez de só "falhou".
    negotiated: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Session {
    /// Duas tentativas: primeiro no formato que a política escolheu, depois no que a
    /// própria câmera trouxe por padrão. Algumas webcams aceitam um formato no `set`
    /// e simplesmente não transmitem nele — e é melhor um preview grande demais do
    /// que nenhum.
    ///
    /// `target` é a resolução das configurações. `None` = automático, que continua
    /// mirando [`TARGET_PIXELS`].
    pub fn open(target: Option<(u32, u32)>) -> Result<Self, AutomationError> {
        match Self::start(FormatPolicy::Preview { target }) {
            Err(AutomationError::CameraSilent { format, .. }) => {
                eprintln!(
                    "[jarvis] a webcam não transmitiu em {format}; tentando com o padrão dela"
                );
                Self::start(FormatPolicy::CameraDefault)
            }
            other => other,
        }
    }

    fn start(policy: FormatPolicy) -> Result<Self, AutomationError> {
        let latest = Arc::new(Mutex::new(None));
        let failure = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel();

        let worker = thread::spawn({
            let latest = Arc::clone(&latest);
            let failure = Arc::clone(&failure);
            let stop = Arc::clone(&stop);

            move || {
                let mut camera = match open_camera(policy) {
                    Ok(camera) => {
                        let format = camera.camera_format();
                        let resolution = format.resolution();
                        let _ = ready_tx.send(Ok(format!(
                            "{}×{} {} a {} fps",
                            resolution.width_x,
                            resolution.height_y,
                            format.format(),
                            format.frame_rate()
                        )));
                        camera
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };

                // `frame()` bloqueia até a câmera entregar, então este laço se
                // regula sozinho no fps do dispositivo — não precisa de sleep.
                while !stop.load(Ordering::Relaxed) {
                    match camera.frame() {
                        Ok(buffer) => *lock(&latest) = Some(Frame::from(&buffer)),
                        Err(error) => {
                            *lock(&failure) = Some(classify(error));
                            return;
                        }
                    }
                }
            }
        });

        let negotiated = ready_rx
            .recv()
            .map_err(|_| AutomationError::Camera("a thread da webcam morreu ao abrir".into()))??;

        let session = Self {
            latest,
            failure,
            negotiated,
            stop,
            worker: Some(worker),
        };

        // Esperar aqui, e não na primeira captura: `open_stream` volta antes de o
        // dispositivo estar transmitindo, e quem paga essa espera tem que ser o
        // "ligar a webcam" (que já mostra estado ocupado), não o laço do preview —
        // lá o atraso viraria um erro e desligaria a câmera recém-aberta.
        session.wait_for_frame(FIRST_FRAME_TIMEOUT)?;

        Ok(session)
    }

    /// `max_width` limita a largura da imagem ENTREGUE, não a da captura.
    ///
    /// É a separação que faz 1080p ser utilizável: a câmera continua capturando em
    /// 1080p (é o que o modelo vai ler), mas a prévia recebe o tamanho da janela.
    /// `None` entrega o quadro inteiro.
    pub fn grab(&self, max_width: Option<u32>) -> Result<CapturedImage, AutomationError> {
        self.wait_for_frame(FRAME_TIMEOUT)?;

        match lock(&self.latest).as_ref() {
            Some(frame) => encode(frame, max_width),
            None => Err(self.no_frame_error(FRAME_TIMEOUT)),
        }
    }

    fn wait_for_frame(&self, timeout: Duration) -> Result<(), AutomationError> {
        let deadline = Instant::now() + timeout;

        loop {
            if let Some(error) = lock(&self.failure).take() {
                return Err(error);
            }

            if lock(&self.latest).is_some() {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(self.no_frame_error(timeout));
            }

            thread::sleep(Duration::from_millis(20));
        }
    }

    /// A causa mais comum aqui não é defeito nenhum: é outro programa (Teams, Zoom,
    /// o navegador) segurando a câmera. O MediaFoundation abre o dispositivo assim
    /// mesmo e simplesmente nunca entrega quadro, então a mensagem precisa dizer
    /// isso — e o formato negociado, para o caso de ser outra coisa.
    fn no_frame_error(&self, waited: Duration) -> AutomationError {
        AutomationError::CameraSilent {
            format: self.negotiated.clone(),
            seconds: waited.as_secs_f32(),
        }
    }
}

impl Drop for Session {
    /// Espera a thread terminar de propósito: sem o `join`, "fechar a webcam"
    /// voltaria antes de a câmera ser solta e a luz continuaria acesa.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Frame {
    fn from(buffer: &Buffer) -> Self {
        Self {
            bytes: buffer.buffer().to_vec(),
            format: buffer.source_frame_format(),
            resolution: buffer.resolution(),
        }
    }
}

/// MJPEG já É um JPEG: quando o tamanho serve, os bytes vão direto para a UI.
/// Decodificar para RGB e re-codificar custa dezenas de milissegundos por quadro, e
/// era a maior parte do custo do preview.
///
/// O passthrough continua valendo sempre que o quadro JÁ cabe em `max_width` — que é
/// o caso do modo automático (640×480 numa janela de ~620px). O caminho lento só
/// entra quando há mesmo o que reduzir, e aí ele PAGA POR SI: um quadro 1080p custa
/// ~530 KB de base64 atravessando o IPC 25×/s, e reduzir para a largura da janela
/// corta isso em quase dez vezes. Sem essa conta, escolher 1080p travava a prévia.
///
/// Câmeras que só falam YUYV/NV12 caem no caminho lento de qualquer jeito, que é o
/// único modo de virar imagem exibível.
fn encode(frame: &Frame, max_width: Option<u32>) -> Result<CapturedImage, AutomationError> {
    let largura = frame.resolution.width_x;
    let precisa_reduzir = max_width.is_some_and(|teto| largura > teto);

    if frame.format == FrameFormat::MJPEG && !precisa_reduzir {
        return Ok(CapturedImage::from_jpeg(
            largura,
            frame.resolution.height_y,
            &frame.bytes,
        ));
    }

    let buffer = Buffer::new(frame.resolution, &frame.bytes, frame.format);
    let rgb = buffer.decode_image::<RgbFormat>().map_err(classify)?;

    CapturedImage::from_rgb(
        largura,
        frame.resolution.height_y,
        rgb.into_raw(),
        if precisa_reduzir { max_width } else { None },
    )
}

#[derive(Clone, Copy)]
enum FormatPolicy {
    /// Ajusta o formato. `target` vem das configurações; `None` mira o padrão leve.
    Preview { target: Option<(u32, u32)> },
    /// Não mexe: fica com o que a câmera trouxe ao abrir.
    CameraDefault,
}

/// Uma resolução que a câmera declara suportar, para a tela de configurações poder
/// oferecer o que EXISTE em vez de uma lista fixa que o dispositivo talvez recuse.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebcamResolution {
    pub width: u32,
    pub height: u32,
    /// Maior taxa oferecida NESTA resolução — é o que muda entre 720p e 1080p na
    /// mesma câmera, e o que explica por que a maior nem sempre é a melhor escolha.
    pub max_fps: u32,
}

/// Resoluções distintas que a câmera aceita, da maior para a menor.
///
/// Abre o dispositivo só para perguntar e fecha em seguida: é uma tela de
/// configuração, não um preview, e segurar a câmera aberta aqui brigaria com a
/// sessão do preview pelo mesmo dispositivo.
pub fn list_resolutions() -> Result<Vec<WebcamResolution>, AutomationError> {
    let cameras = nokhwa::query(ApiBackend::Auto).map_err(classify)?;
    if cameras.is_empty() {
        return Err(AutomationError::NoCamera);
    }

    let default = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(CameraIndex::Index(0), default).map_err(classify)?;
    let formats = camera.compatible_camera_formats().map_err(classify)?;

    // A mesma resolução aparece uma vez por formato de pixel (MJPEG, YUYV) e por
    // taxa. O que interessa na tela é a resolução; de fps, o teto dela.
    let mut melhores: Vec<WebcamResolution> = Vec::new();
    for format in formats {
        let resolution = format.resolution();
        let fps = format.frame_rate();

        match melhores
            .iter_mut()
            .find(|r| r.width == resolution.width_x && r.height == resolution.height_y)
        {
            Some(existente) => existente.max_fps = existente.max_fps.max(fps),
            None => melhores.push(WebcamResolution {
                width: resolution.width_x,
                height: resolution.height_y,
                max_fps: fps,
            }),
        }
    }

    melhores
        .sort_by_key(|r| std::cmp::Reverse((u64::from(r.width) * u64::from(r.height), r.width)));
    Ok(melhores)
}

fn open_camera(policy: FormatPolicy) -> Result<Camera, AutomationError> {
    // Perguntar a lista antes de abrir separa "não tem câmera" de "tem, mas deu
    // erro" — dois problemas com soluções bem diferentes para o usuário.
    let cameras = nokhwa::query(ApiBackend::Auto).map_err(classify)?;
    if cameras.is_empty() {
        return Err(AutomationError::NoCamera);
    }

    // Não usamos `RequestedFormatType::Closest`: ele acha a resolução mais próxima
    // e depois procura os fps filtrando pela resolução PEDIDA, não pela que achou
    // (nokhwa-core, `types.rs`). Numa câmera que não ofereça o valor exato, a lista
    // sai vazia e a abertura falha inteira. Abrimos com o padrão e escolhemos o
    // formato na mão, que também é onde a política do preview fica explícita.
    let default = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(CameraIndex::Index(0), default).map_err(classify)?;

    let escolhido = match policy {
        FormatPolicy::Preview { target } => pick_preview_format(&mut camera, target),
        FormatPolicy::CameraDefault => None,
    };

    if let Some(format) = escolhido {
        // `Exact` é seguro aqui, ao contrário de `Closest`: o formato saiu da
        // própria lista de compatíveis da câmera, então existe.
        let request = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(format));

        // Não é fatal: sem conseguir ajustar, seguimos com o formato que a câmera
        // já deu. O passthrough de MJPEG e o laço de quadro mais recente continuam
        // valendo — só o tamanho do quadro é que não fica no ideal.
        if let Err(error) = camera.set_camera_requset(request) {
            eprintln!(
                "[jarvis] não consegui ajustar o formato da webcam ({error}); seguindo com o padrão"
            );
        }
    }

    camera.open_stream().map_err(classify)?;

    Ok(camera)
}

/// Escolhe o formato, em ordem de prioridade.
///
/// Com `target` das configurações, a resolução vem PRIMEIRO: quem pediu 1080p pediu
/// 1080p, e trocar por 720p porque o MJPEG é mais barato ali seria ignorar o ajuste.
/// Sem `target`, vale a política antiga — MJPEG na frente, porque o preview
/// automático é o caminho quente e re-codificar cada quadro custa mais que os pixels
/// que ele economiza.
///
/// O fps mais perto de [`TARGET_FPS`] desempata nos dois casos: 60 quadros por
/// segundo só gastariam CPU sem mostrar mais nada numa janela de 600px.
fn pick_preview_format(camera: &mut Camera, target: Option<(u32, u32)>) -> Option<CameraFormat> {
    escolher_formato(camera.compatible_camera_formats().ok()?, target)
}

/// A pontuação, separada da câmera para poder ser testada sem hardware.
fn escolher_formato(
    formats: Vec<CameraFormat>,
    target: Option<(u32, u32)>,
) -> Option<CameraFormat> {
    let alvo_pixels = match target {
        Some((largura, altura)) => i64::from(largura) * i64::from(altura),
        None => TARGET_PIXELS,
    };

    formats.into_iter().max_by_key(|format| {
        let resolution = format.resolution();
        let pixels = i64::from(resolution.width_x) * i64::from(resolution.height_y);
        let mjpeg = u8::from(format.format() == FrameFormat::MJPEG);
        let fps_distance = (format.frame_rate() as i32 - TARGET_FPS).abs();

        // `exato` separa "é a resolução pedida" de "é perto dela": duas resoluções
        // com a mesma contagem de pixels (640×480 e 800×384) empatariam na distância.
        let exato = u8::from(match target {
            Some((largura, altura)) => {
                resolution.width_x == largura && resolution.height_y == altura
            }
            None => false,
        });

        match target {
            Some(_) => (exato, -(pixels - alvo_pixels).abs(), mjpeg, -fps_distance),
            None => (mjpeg, -(pixels - alvo_pixels).abs(), exato, -fps_distance),
        }
    })
}

/// Mesma heurística do microfone: a crate não tem um erro próprio para "permissão
/// negada", e a mensagem do backend é a única pista de que o problema é o painel de
/// privacidade do sistema, não a câmera.
fn classify(error: nokhwa::NokhwaError) -> AutomationError {
    let message = error.to_string();
    let lowered = message.to_lowercase();

    if lowered.contains("denied")
        || lowered.contains("permission")
        || lowered.contains("0x80070005")
    {
        AutomationError::CameraDenied
    } else if lowered.contains("not found") || lowered.contains("no device") {
        AutomationError::NoCamera
    } else {
        AutomationError::Camera(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formato(largura: u32, altura: u32, pixel: FrameFormat, fps: u32) -> CameraFormat {
        CameraFormat::new_from(largura, altura, pixel, fps)
    }

    /// A lista que uma webcam comum oferece: a mesma resolução em MJPEG e em YUYV.
    fn catalogo() -> Vec<CameraFormat> {
        vec![
            formato(640, 480, FrameFormat::MJPEG, 30),
            formato(640, 480, FrameFormat::YUYV, 30),
            formato(1280, 720, FrameFormat::MJPEG, 30),
            formato(1280, 720, FrameFormat::YUYV, 10),
            formato(1920, 1080, FrameFormat::MJPEG, 30),
        ]
    }

    /// Sem pedido do usuário, a política antiga continua: o alvo é ~640×480.
    #[test]
    fn automatico_mira_a_resolucao_do_preview() {
        let escolhido = escolher_formato(catalogo(), None).expect("escolhe algum");

        assert_eq!(escolhido.resolution().width_x, 640);
        assert_eq!(escolhido.resolution().height_y, 480);
    }

    /// O ponto do ajuste: pedir 1080p tem que dar 1080p, e não a resolução que sairia
    /// mais barata. Antes desta mudança não havia como pedir.
    #[test]
    fn a_resolucao_pedida_ganha_do_alvo_padrao() {
        let escolhido = escolher_formato(catalogo(), Some((1920, 1080))).expect("escolhe");

        assert_eq!(escolhido.resolution().width_x, 1920);
        assert_eq!(escolhido.resolution().height_y, 1080);
    }

    /// Empatada a resolução, MJPEG vence — é o passthrough que evita recodificar
    /// cada quadro do preview.
    #[test]
    fn entre_dois_formatos_da_mesma_resolucao_o_mjpeg_ganha() {
        let escolhido = escolher_formato(catalogo(), Some((1280, 720))).expect("escolhe");

        assert_eq!(escolhido.resolution().width_x, 1280);
        assert_eq!(escolhido.format(), FrameFormat::MJPEG);
    }

    /// Uma resolução que a câmera não tem não pode fazer a abertura falhar: cai na
    /// mais próxima. É o caso de trocar de webcam sem mexer nas configurações.
    #[test]
    fn resolucao_inexistente_cai_na_mais_proxima() {
        let escolhido = escolher_formato(catalogo(), Some((1600, 900))).expect("escolhe");

        // 1600×900 = 1,44 Mpx: mais perto de 1280×720 (0,92) que de 1920×1080 (2,07).
        assert_eq!(escolhido.resolution().width_x, 1280);
    }

    /// Duas resoluções podem ter a MESMA contagem de pixels; sem o critério de
    /// igualdade exata, o desempate cairia no formato de pixel e poderia devolver a
    /// outra — com o usuário vendo a resolução que não pediu.
    #[test]
    fn empate_em_pixels_e_desfeito_pela_igualdade_exata() {
        let formatos = vec![
            formato(800, 384, FrameFormat::MJPEG, 30),
            formato(640, 480, FrameFormat::YUYV, 30),
        ];

        let escolhido = escolher_formato(formatos, Some((640, 480))).expect("escolhe");
        assert_eq!(escolhido.resolution().width_x, 640);
    }

    #[test]
    fn lista_vazia_nao_escolhe_nada() {
        assert!(escolher_formato(Vec::new(), None).is_none());
    }
}
