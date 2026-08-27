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
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, RgbImage, RgbaImage};
use serde::Serialize;

pub use screen::{capture_screen, list_monitors, MonitorInfo};
pub use webcam::{list_resolutions as list_webcam_resolutions, WebcamResolution};

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
    /// Separado de [`AutomationError::Camera`] porque é recuperável: quem abre a
    /// câmera tenta de novo com outro formato antes de desistir.
    #[error(
        "a câmera abriu em {format} mas não entregou nenhum quadro em {seconds:.1}s — verifique se outro programa está usando a webcam"
    )]
    CameraSilent { format: String, seconds: f32 },
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
    /// Caminho rápido: os bytes JÁ são um JPEG (MJPEG da webcam), então não há o
    /// que codificar — só embrulhar.
    fn from_jpeg(width: u32, height: u32, jpeg: &[u8]) -> Self {
        Self {
            data_url: data_url(jpeg, "image/jpeg"),
            width,
            height,
        }
    }

    /// Caminho lento, para câmeras que só entregam pixels crus e para quando há o que
    /// reduzir. JPEG e não PNG: o preview roda em laço, e o mesmo quadro em PNG é
    /// várias vezes maior.
    ///
    /// `max_width` reduz mantendo a proporção. Lanczos3 e não o filtro rápido: o
    /// destino é uma prévia que o usuário olha, e reduzir 1920→620 com vizinho mais
    /// próximo serrilha justamente as bordas finas (texto, contorno de objeto) que
    /// fazem a imagem parecer nítida.
    fn from_rgb(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        max_width: Option<u32>,
    ) -> Result<Self, AutomationError> {
        let frame = RgbImage::from_raw(width, height, pixels)
            .ok_or_else(|| AutomationError::Encode("frame com tamanho inesperado".into()))?;
        let mut imagem = DynamicImage::from(frame);

        if let Some(teto) = max_width.filter(|teto| width > *teto) {
            let altura = (u64::from(height) * u64::from(teto) / u64::from(width)).max(1) as u32;
            imagem = imagem.resize_exact(teto, altura, FilterType::Lanczos3);
        }

        Self::encode_jpeg(&imagem)
    }

    /// PNG para a tela: texto de interface vira borrão nos artefatos do JPEG, e é
    /// justamente texto que o modelo vai precisar ler na v0.2.
    fn from_screen(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, AutomationError> {
        let shot = RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| AutomationError::Encode("captura com tamanho inesperado".into()))?;

        Self::encode(&shot.into(), ImageFormat::Png, "image/png")
    }

    /// JPEG com qualidade explícita.
    ///
    /// 85 e não o padrão da crate (75): o quadro já foi reduzido, então o arquivo é
    /// pequeno de qualquer jeito, e a queixa que motivou este caminho era de imagem
    /// pouco nítida — economizar bytes aqui seria economizar na coisa errada.
    fn encode_jpeg(source: &DynamicImage) -> Result<Self, AutomationError> {
        const QUALIDADE: u8 = 85;

        let mut bytes = Cursor::new(Vec::new());
        let encoder = JpegEncoder::new_with_quality(&mut bytes, QUALIDADE);

        source
            .write_with_encoder(encoder)
            .map_err(|error| AutomationError::Encode(error.to_string()))?;

        Ok(Self {
            data_url: data_url(&bytes.into_inner(), "image/jpeg"),
            width: source.width(),
            height: source.height(),
        })
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

        Ok(Self {
            data_url: data_url(&bytes.into_inner(), mime),
            width: source.width(),
            height: source.height(),
        })
    }
}

fn data_url(bytes: &[u8], mime: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:{mime};base64,{encoded}")
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

    /// `target` é a resolução das configurações (`None` = automático).
    ///
    /// Trocar a resolução com a câmera aberta exige FECHAR e reabrir: o formato é
    /// negociado na abertura do stream, e a sessão de pé continuaria entregando o
    /// tamanho antigo. Quem faz isso é o `sensorStore`, que já sabe se o preview
    /// estava ligado — aqui, câmera aberta continua sendo um no-op, para o botão e o
    /// agente poderem chamar sem se coordenar.
    pub fn open_webcam(&self, target: Option<(u32, u32)>) -> Result<(), AutomationError> {
        let mut slot = lock(&self.camera);
        if slot.is_none() {
            *slot = Some(webcam::Session::open(target)?);
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
    /// `max_width` encolhe só a ENTREGA. A captura continua na resolução configurada,
    /// então o laço da prévia pede o tamanho da janela e o agente pede o quadro
    /// inteiro — da mesma câmera, sem reabrir nada.
    pub fn capture_webcam_frame(
        &self,
        target: Option<(u32, u32)>,
        max_width: Option<u32>,
    ) -> Result<CapturedImage, AutomationError> {
        match lock(&self.camera).as_ref() {
            Some(session) => session.grab(max_width),
            // A sessão temporária morre no fim da expressão, e o `Drop` dela fecha
            // a câmera — abrir e fechar fica contido aqui.
            None => webcam::Session::open(target)?.grab(max_width),
        }
    }
}

impl Default for AutomationState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pixels crus de uma imagem RGB do tamanho pedido.
    fn pixels(width: u32, height: u32) -> Vec<u8> {
        vec![128_u8; (width as usize) * (height as usize) * 3]
    }

    /// O caso que motivou o teto: 1080p numa janela de ~620px. Sem reduzir, cada
    /// quadro atravessa o IPC com ~9× os pixels que a tela mostra.
    #[test]
    fn reduzir_respeita_o_teto_e_mantem_a_proporcao() {
        let imagem =
            CapturedImage::from_rgb(1920, 1080, pixels(1920, 1080), Some(620)).expect("codifica");

        assert_eq!(imagem.width, 620);
        // 1080 × 620 / 1920 = 348,75 → 348, truncado.
        assert_eq!(imagem.height, 348);
    }

    /// Quadro que já cabe NÃO pode ser ampliado: esticar 640 para 1280 gastaria banda
    /// para entregar a mesma informação, borrada.
    #[test]
    fn quadro_menor_que_o_teto_passa_intacto() {
        let imagem =
            CapturedImage::from_rgb(640, 480, pixels(640, 480), Some(1280)).expect("codifica");

        assert_eq!((imagem.width, imagem.height), (640, 480));
    }

    #[test]
    fn sem_teto_nada_e_reduzido() {
        let imagem =
            CapturedImage::from_rgb(1920, 1080, pixels(1920, 1080), None).expect("codifica");

        assert_eq!((imagem.width, imagem.height), (1920, 1080));
    }

    /// Uma imagem muito mais larga que alta não pode virar altura zero — `RgbImage`
    /// com 0 de altura não existe, e a codificação falharia em vez de reduzir.
    #[test]
    fn proporcao_extrema_nao_zera_a_altura() {
        let imagem = CapturedImage::from_rgb(2000, 3, pixels(2000, 3), Some(100))
            .expect("codifica mesmo assim");

        assert_eq!(imagem.width, 100);
        assert!(imagem.height >= 1, "altura precisa sobrar pelo menos 1px");
    }

    #[test]
    fn a_imagem_reduzida_sai_como_data_url_de_jpeg() {
        let imagem =
            CapturedImage::from_rgb(1920, 1080, pixels(1920, 1080), Some(320)).expect("codifica");

        assert!(imagem.data_url.starts_with("data:image/jpeg;base64,"));
    }
}
