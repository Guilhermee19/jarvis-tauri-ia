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
    /// Duas tentativas: primeiro no formato que a política do preview escolheu,
    /// depois no que a própria câmera trouxe por padrão. Algumas webcams aceitam um
    /// formato no `set` e simplesmente não transmitem nele — e é melhor um preview
    /// grande demais do que nenhum.
    pub fn open() -> Result<Self, AutomationError> {
        match Self::start(FormatPolicy::Preview) {
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

    pub fn grab(&self) -> Result<CapturedImage, AutomationError> {
        self.wait_for_frame(FRAME_TIMEOUT)?;

        match lock(&self.latest).as_ref() {
            Some(frame) => encode(frame),
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

/// MJPEG já É um JPEG: os bytes vão direto para a UI. Decodificar para RGB e
/// re-codificar custava dezenas de milissegundos por quadro para chegar ao mesmo
/// lugar — era a maior parte do custo do preview.
///
/// Câmeras que só falam YUYV/NV12 caem no caminho lento, que é o único jeito de
/// virar imagem exibível.
fn encode(frame: &Frame) -> Result<CapturedImage, AutomationError> {
    if frame.format == FrameFormat::MJPEG {
        return Ok(CapturedImage::from_jpeg(
            frame.resolution.width_x,
            frame.resolution.height_y,
            &frame.bytes,
        ));
    }

    let buffer = Buffer::new(frame.resolution, &frame.bytes, frame.format);
    let rgb = buffer.decode_image::<RgbFormat>().map_err(classify)?;

    CapturedImage::from_rgb(
        frame.resolution.width_x,
        frame.resolution.height_y,
        rgb.into_raw(),
    )
}

#[derive(Clone, Copy)]
enum FormatPolicy {
    /// Ajusta para o formato leve do preview.
    Preview,
    /// Não mexe: fica com o que a câmera trouxe ao abrir.
    CameraDefault,
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

    if let (FormatPolicy::Preview, Some(format)) = (policy, pick_preview_format(&mut camera)) {
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

/// Política do preview, em ordem: MJPEG (evita re-codificar cada quadro), depois a
/// resolução mais perto de [`TARGET_PIXELS`], e por fim o fps mais perto de
/// [`TARGET_FPS`] — 60 quadros por segundo só gastariam CPU sem mostrar mais nada.
fn pick_preview_format(camera: &mut Camera) -> Option<CameraFormat> {
    let formats = camera.compatible_camera_formats().ok()?;

    formats.into_iter().max_by_key(|format| {
        let resolution = format.resolution();
        let pixels = i64::from(resolution.width_x) * i64::from(resolution.height_y);
        let fps_distance = (format.frame_rate() as i32 - TARGET_FPS).abs();

        (
            u8::from(format.format() == FrameFormat::MJPEG),
            -(pixels - TARGET_PIXELS).abs(),
            -fps_distance,
        )
    })
}

/// Mesma heurística do microfone: a crate não tem um erro próprio para "permissão
/// negada", e a mensagem do backend é a única pista de que o problema é o painel de
/// privacidade do sistema, não a câmera.
fn classify(error: nokhwa::NokhwaError) -> AutomationError {
    let message = error.to_string();
    let lowered = message.to_lowercase();

    if lowered.contains("denied") || lowered.contains("permission") || lowered.contains("0x80070005")
    {
        AutomationError::CameraDenied
    } else if lowered.contains("not found") || lowered.contains("no device") {
        AutomationError::NoCamera
    } else {
        AutomationError::Camera(message)
    }
}
