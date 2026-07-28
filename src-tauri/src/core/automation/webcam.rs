//! Webcam via `nokhwa`.
//!
//! A `Camera` do nokhwa não é `Send`, então ela não pode morar no estado
//! compartilhado do Tauri. A sessão é uma thread dona da câmera, e as capturas
//! viajam por canal — mesmo desenho do stream do microfone, pelo mesmo motivo.

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

use super::{AutomationError, CapturedImage};

type Reply = mpsc::Sender<Result<CapturedImage, AutomationError>>;

enum Request {
    Grab(Reply),
}

pub struct Session {
    /// `Option` para poder ser solto no `Drop` ANTES do `join`: é o fim do canal
    /// que faz a thread sair do laço e liberar a câmera.
    requests: Option<mpsc::Sender<Request>>,
    worker: Option<JoinHandle<()>>,
}

impl Session {
    pub fn open() -> Result<Self, AutomationError> {
        let (requests, inbox) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::channel();

        let worker = thread::spawn(move || {
            let mut camera = match open_camera() {
                Ok(camera) => {
                    let _ = ready_tx.send(Ok(()));
                    camera
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };

            while let Ok(Request::Grab(reply)) = inbox.recv() {
                let _ = reply.send(grab(&mut camera));
            }
        });

        ready_rx
            .recv()
            .map_err(|_| AutomationError::Camera("a thread da webcam morreu ao abrir".into()))??;

        Ok(Self {
            requests: Some(requests),
            worker: Some(worker),
        })
    }

    pub fn grab(&self) -> Result<CapturedImage, AutomationError> {
        let (reply, answer) = mpsc::channel();

        self.requests
            .as_ref()
            .ok_or(AutomationError::CameraClosed)?
            .send(Request::Grab(reply))
            .map_err(|_| AutomationError::CameraClosed)?;

        answer.recv().map_err(|_| AutomationError::CameraClosed)?
    }
}

impl Drop for Session {
    /// Espera a thread terminar de propósito: sem o `join`, "fechar a webcam"
    /// voltaria antes de a câmera ser solta e a luz continuaria acesa.
    fn drop(&mut self) {
        drop(self.requests.take());

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn open_camera() -> Result<Camera, AutomationError> {
    // Perguntar a lista antes de abrir separa "não tem câmera" de "tem, mas deu
    // erro" — dois problemas com soluções bem diferentes para o usuário.
    let cameras = nokhwa::query(ApiBackend::Auto).map_err(classify)?;
    if cameras.is_empty() {
        return Err(AutomationError::NoCamera);
    }

    // Maior taxa de quadros disponível: para o preview, fluidez importa mais que
    // resolução, e a captura de frame único aproveita o mesmo stream já aberto.
    let format = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(CameraIndex::Index(0), format).map_err(classify)?;
    camera.open_stream().map_err(classify)?;

    Ok(camera)
}

fn grab(camera: &mut Camera) -> Result<CapturedImage, AutomationError> {
    let buffer = camera.frame().map_err(classify)?;
    let resolution = buffer.resolution();
    let rgb = buffer.decode_image::<RgbFormat>().map_err(classify)?;

    CapturedImage::from_webcam(resolution.width_x, resolution.height_y, rgb.into_raw())
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
