//! Percepção do ambiente: webcam e tela.
//!
//! Nesta versão é só captura — nenhum reconhecimento. As duas capacidades entregam
//! o mesmo tipo ([`CapturedImage`]), então a v0.2+ manda qualquer uma delas para o
//! modelo pelo mesmo caminho, e as travas de segurança (confirmação, kill switch)
//! entram aqui quando `input.rs` (mouse e teclado) chegar na v0.4.

mod screen;
mod webcam;

use std::io::Cursor;
use std::sync::Mutex;

use base64::Engine;
use image::{DynamicImage, ImageFormat, RgbImage, RgbaImage};
use serde::Serialize;

pub use screen::{capture_screen, list_monitors, MonitorInfo};

use crate::core::lock;

#[derive(Debug, thiserror::Error)]
pub enum AutomationError {
    #[error("nenhuma webcam encontrada — conecte uma câmera e tente de novo")]
    NoCamera,
    #[error(
        "o sistema negou acesso à câmera. No Windows: Configurações › Privacidade e segurança › Câmera"
    )]
    CameraDenied,
    #[error("falha na webcam: {0}")]
    Camera(String),
    #[error("a webcam não está aberta")]
    CameraClosed,
    #[error("nenhum monitor encontrado")]
    NoMonitor,
    #[error("monitor {0} não existe")]
    UnknownMonitor(u32),
    #[error("falha ao capturar a tela: {0}")]
    Screen(String),
    #[error("falha ao codificar a imagem: {0}")]
    Encode(String),
}

/// Imagem pronta para a UI, já como `data:` URL — o webview mostra direto em uma
/// `<img>`, sem servidor de arquivos nem permissão de leitura de disco.
///
/// O custo é o base64 (+33% de bytes no IPC). Aceitável para diagnóstico; quando a
/// v0.2 mandar isso para um modelo, o caminho é outro (bytes crus, sem passar pela UI).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedImage {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

impl CapturedImage {
    /// JPEG para a webcam: o preview roda em loop, e o mesmo frame em PNG é várias
    /// vezes maior — o peso apareceria direto na fluidez da imagem.
    fn from_webcam(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, AutomationError> {
        let frame = RgbImage::from_raw(width, height, pixels)
            .ok_or_else(|| AutomationError::Encode("frame com tamanho inesperado".into()))?;

        Self::encode(&frame.into(), ImageFormat::Jpeg, "image/jpeg")
    }

    /// PNG para a tela: texto de interface vira borrão nos artefatos do JPEG, e é
    /// justamente texto que o modelo vai precisar ler na v0.2.
    fn from_screen(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, AutomationError> {
        let shot = RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| AutomationError::Encode("captura com tamanho inesperado".into()))?;

        Self::encode(&shot.into(), ImageFormat::Png, "image/png")
    }

    fn encode(
        source: &DynamicImage,
        format: ImageFormat,
        mime: &str,
    ) -> Result<Self, AutomationError> {
        let mut bytes = Cursor::new(Vec::new());
        source
            .write_to(&mut bytes, format)
            .map_err(|error| AutomationError::Encode(error.to_string()))?;

        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());

        Ok(Self {
            data_url: format!("data:{mime};base64,{encoded}"),
            width: source.width(),
            height: source.height(),
        })
    }
}

/// Dono da sessão de webcam. A câmera fica aberta entre capturas porque abrir custa
/// centenas de milissegundos — inviável para um preview ao vivo.
pub struct AutomationState {
    camera: Mutex<Option<webcam::Session>>,
}

impl AutomationState {
    pub fn new() -> Self {
        Self {
            camera: Mutex::new(None),
        }
    }

    pub fn is_webcam_open(&self) -> bool {
        lock(&self.camera).is_some()
    }

    pub fn open_webcam(&self) -> Result<(), AutomationError> {
        let mut slot = lock(&self.camera);
        if slot.is_none() {
            *slot = Some(webcam::Session::open()?);
        }
        Ok(())
    }

    /// Fechar é idempotente: o botão da UI e a limpeza ao sair da tela chamam isso
    /// sem coordenação, e nenhum dos dois precisa saber quem chegou primeiro.
    pub fn close_webcam(&self) {
        lock(&self.camera).take();
    }

    /// Frame atual da webcam. Se ela não estiver aberta, abre e fecha em volta da
    /// captura — é isso que permite o agente (v0.2+) tirar uma foto pontual sem
    /// gerenciar sessão, usando exatamente a mesma função do preview.
    pub fn capture_webcam_frame(&self) -> Result<CapturedImage, AutomationError> {
        match lock(&self.camera).as_ref() {
            Some(session) => session.grab(),
            // A sessão temporária morre no fim da expressão, e o `Drop` dela fecha
            // a câmera — abrir e fechar fica contido aqui.
            None => webcam::Session::open()?.grab(),
        }
    }
}

impl Default for AutomationState {
    fn default() -> Self {
        Self::new()
    }
}
