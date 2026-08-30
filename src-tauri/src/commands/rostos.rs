//! Quem está na frente da webcam, do lado que o frontend alcança.
//!
//! Regra da casa: nada de lógica de negócio. O que existe aqui é resolver o `data_dir`
//! (que só o Tauri sabe), tirar a foto e traduzir erro para `String`.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::core::automation::AutomationState;
use crate::core::rostos::{Conhecidos, Pessoa, Reconhecedor, RostoError};
use crate::core::vision::so_o_base64;

/// O que o boot recebe quando pergunta "quem está aí?".
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuemEsta {
    /// O nome, quando reconhecido. **Vazio quando não** — e é o estado que interessa,
    /// porque é ele que faz o Jarvis saudar sem arriscar nome nenhum.
    pub nome: String,
    pub id: String,
    /// De 0 a 1. Zero quando não reconheceu ninguém.
    pub semelhanca: f32,
    /// Havia alguém na frente da câmera, mesmo que desconhecido.
    ///
    /// Separa dois silêncios que o `nome` vazio junta: "não tem ninguém aí" e "tem
    /// alguém que eu não conheço". Só o segundo merece perguntar quem é.
    pub tem_alguem: bool,
}

/// As pessoas cadastradas, sem os vetores.
///
/// As assinaturas não saem daqui: são centenas de números por pessoa que a tela não usa
/// para nada, e mandá-las pelo IPC a cada listagem seria pagar caro por nada.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PessoaConhecida {
    pub id: String,
    pub nome: String,
    /// Quantas condições diferentes foram cadastradas (de óculos, de manhã, de barba).
    pub cadastros: usize,
    pub visto_em: i64,
}

impl From<Pessoa> for PessoaConhecida {
    fn from(pessoa: Pessoa) -> Self {
        Self {
            id: pessoa.id,
            nome: pessoa.nome,
            cadastros: pessoa.assinaturas.len(),
            visto_em: pessoa.visto_em,
        }
    }
}

#[tauri::command]
pub fn list_people(catalogo: State<'_, Conhecidos>) -> Vec<PessoaConhecida> {
    catalogo.todas().into_iter().map(Into::into).collect()
}

#[tauri::command]
pub fn forget_person(id: String, catalogo: State<'_, Conhecidos>) -> Result<(), String> {
    catalogo.esquecer(&id).map_err(|erro| erro.to_string())
}

/// Olha pela webcam e diz quem está lá.
///
/// **Tira a foto e fecha a câmera**: com a webcam desligada o `capture_webcam_frame` abre,
/// captura e solta o dispositivo sozinho — a luz pisca em vez de ficar acesa. Com o
/// preview já ligado ele aproveita a sessão aberta e não interfere nela.
///
/// Nunca devolve `Err` por não reconhecer ninguém: cena vazia e rosto desconhecido são
/// respostas, não falhas. Erro aqui é só o que impede de tentar — modelo ausente, câmera
/// ocupada por outro programa.
#[tauri::command]
pub async fn who_is_there(
    app: AppHandle,
    automation: State<'_, AutomationState>,
    reconhecedor: State<'_, Reconhecedor>,
    catalogo: State<'_, Conhecidos>,
) -> Result<QuemEsta, String> {
    let assinatura = match olhar(&app, &automation, &reconhecedor) {
        Ok(assinatura) => assinatura,
        // Ninguém na frente da câmera não é erro: é a resposta "não vi ninguém", e o
        // boot precisa dela para saudar sem nome em vez de mostrar uma falha na tela.
        Err(RostoError::NenhumRosto) => {
            return Ok(QuemEsta {
                nome: String::new(),
                id: String::new(),
                semelhanca: 0.0,
                tem_alguem: false,
            })
        }
        Err(erro) => return Err(erro.to_string()),
    };

    let Some(quem) = catalogo.quem_e(&assinatura) else {
        // Tem alguém, mas não sei quem. É este caso que faz o Jarvis perguntar.
        return Ok(QuemEsta {
            nome: String::new(),
            id: String::new(),
            semelhanca: 0.0,
            tem_alguem: true,
        });
    };

    catalogo.marcar_visto(&quem.id);

    Ok(QuemEsta {
        nome: quem.nome,
        id: quem.id,
        semelhanca: quem.semelhanca,
        tem_alguem: true,
    })
}

/// Guarda o rosto que está na câmera AGORA sob um nome.
///
/// Chamado quando a pessoa responde "sou o Guilherme", e também pelo cadastro manual.
/// Cadastrar de novo com o mesmo nome **acrescenta** uma condição em vez de substituir —
/// é o que faz reconhecer de óculos sem parar de reconhecer sem.
#[tauri::command]
pub async fn enroll_face(
    nome: String,
    app: AppHandle,
    automation: State<'_, AutomationState>,
    reconhecedor: State<'_, Reconhecedor>,
    catalogo: State<'_, Conhecidos>,
) -> Result<PessoaConhecida, String> {
    let nome = nome.trim().to_owned();
    if nome.is_empty() {
        return Err("preciso de um nome para guardar o rosto".to_owned());
    }

    let assinatura = olhar(&app, &automation, &reconhecedor).map_err(|erro| erro.to_string())?;

    catalogo
        .aprender(&nome, assinatura)
        .map(Into::into)
        .map_err(|erro| erro.to_string())
}

/// Tira a foto e devolve o vetor do rosto principal.
///
/// Síncrono de propósito, dentro de comandos `async`: a captura e a inferência somam
/// pouco mais de um segundo com a câmera fria e ~50 ms com ela aberta, e um
/// `spawn_blocking` aqui só acrescentaria uma troca de thread — é o mesmo julgamento
/// que o `olhar` do agente já fez para a visão.
fn olhar(
    app: &AppHandle,
    automation: &AutomationState,
    reconhecedor: &Reconhecedor,
) -> Result<Vec<f32>, RostoError> {
    let pasta = app
        .path()
        .app_data_dir()
        .map_err(|erro| RostoError::Imagem(erro.to_string()))?
        .join("rostos");

    // Carrega os modelos antes da foto: se eles não estiverem instalados, o erro sai sem
    // acender a luz da câmera à toa.
    let modelos = reconhecedor.modelos(&pasta)?;

    let quadro = automation
        .capture_webcam_frame(None, None)
        .map_err(|erro| RostoError::Imagem(erro.to_string()))?;

    let jpeg = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(so_o_base64(&quadro.data_url))
            .map_err(|erro| RostoError::Imagem(erro.to_string()))?
    };

    modelos.assinatura(&jpeg)
}
