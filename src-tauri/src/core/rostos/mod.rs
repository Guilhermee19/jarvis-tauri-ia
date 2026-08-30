//! Saber QUEM está na frente da webcam.
//!
//! Serve a uma coisa só: o Jarvis abrir e dizer "bom dia, Guilherme" em vez de "bom dia".
//!
//! ## Por que não dá para pedir isso ao modelo de visão
//!
//! O `qwen2.5vl` que o app já usa descreve a cena ("um homem de camiseta escura"), mas
//! não carrega identidades — perguntar "é a mesma pessoa desta foto?" devolve resposta
//! instável, e ele erra com confiança. O modo de falha seria chamar o dono pelo nome do
//! irmão, que é pior do que não saudar.
//!
//! Reconhecer rosto é outra técnica, e é a que o Windows Hello usa: **detectar**
//! ([`deteccao`], modelo YuNet), **alinhar e vetorizar** ([`embedding`], modelo SFace) e
//! **comparar por distância** com os rostos já conhecidos. O nome nunca sai de um modelo:
//! sai do [`Conhecidos`], que é uma lista que o dono escreveu.
//!
//! ## Onde os modelos moram
//!
//! Em `%APPDATA%\com.jarvis.app\rostos\`, no mesmo esquema do Piper e do Whisper — arquivo
//! que o dono põe na pasta, não recurso embutido no binário. São 37 MB que só quem quer a
//! saudação precisa baixar, e mantê-los fora do executável é o que evita cobrar isso de
//! todo mundo.
//!
//! ## Sobre guardar rosto
//!
//! O que fica no disco são **vetores de 128 números**, não fotos. Deles não se reconstrói
//! a imagem de ninguém, e eles só servem contra este mesmo par de modelos. O arquivo é
//! local, como o resto — nada disso sai da máquina.

pub mod deteccao;
pub mod embedding;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use image::DynamicImage;
use ort::session::Session;
use ort::value::Value;
use serde::{Deserialize, Serialize};

use crate::core::lock;
use crate::core::memory::normalizar;
use crate::storage::StorageError;

use deteccao::{Encaixe, Escala, Rosto};

const ARQUIVO: &str = "rostos.json";
const MODELO_DETECCAO: &str = "yunet.onnx";
const MODELO_EMBEDDING: &str = "sface.onnx";

#[derive(Debug, thiserror::Error)]
pub enum RostoError {
    #[error(
        "não achei os modelos de reconhecimento em {0}.\nBaixe os dois do OpenCV Zoo e \
         ponha nessa pasta:\n  \
         face_detection_yunet_2023mar.onnx  → renomeie para yunet.onnx\n  \
         face_recognition_sface_2021dec.onnx → renomeie para sface.onnx\n\
         Estão em github.com/opencv/opencv_zoo, na pasta models/."
    )]
    ModelosAusentes(String),
    #[error("não consegui carregar o modelo {modelo}: {detalhe}")]
    ModeloInvalido { modelo: String, detalhe: String },
    #[error("não consegui olhar a imagem: {0}")]
    Inferencia(String),
    #[error("não vi ninguém na câmera")]
    NenhumRosto,
    #[error("não consegui ler a imagem da câmera: {0}")]
    Imagem(String),
}

/// Uma pessoa que o Jarvis conhece.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pessoa {
    /// Chave estável, derivada do nome. É o que o `rostos.json` usa como índice.
    pub id: String,
    /// Como ele vai te chamar: "Guilherme".
    pub nome: String,
    /// Os vetores de 128 números — **vários por pessoa, e é isso que faz funcionar**.
    ///
    /// Um retrato só te reconhece de dia, sem óculos e no mesmo ângulo. Cada novo
    /// cadastro (de barba, de manhã, de boné) acrescenta um ponto, e o reconhecimento usa
    /// o MELHOR deles — que é como o rosto passa a ser reconhecido em condições que nunca
    /// foram fotografadas.
    pub assinaturas: Vec<Vec<f32>>,
    /// Quando ele te viu pela última vez, em ms. `0` = nunca desde o cadastro.
    pub visto_em: i64,
}

/// Quem foi reconhecido, e com quanta certeza.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reconhecido {
    pub id: String,
    pub nome: String,
    /// De 0 a 1. Acima de [`embedding::LIMIAR`] é considerado a mesma pessoa.
    pub semelhanca: f32,
}

/// Os modelos carregados, prontos para responder.
///
/// Carregar custa centenas de milissegundos e acontece **uma vez**, na primeira pergunta.
/// Guardar as sessões é o que faz a saudação do boot não pagar isso de novo a cada
/// reconhecimento — e é o mesmo raciocínio do `keep_alive` do modelo de linguagem.
pub struct Modelos {
    detector: Mutex<Session>,
    reconhecedor: Mutex<Session>,
}

impl Modelos {
    /// Carrega os dois ONNX da pasta.
    pub fn carregar(pasta: &Path) -> Result<Self, RostoError> {
        let deteccao = pasta.join(MODELO_DETECCAO);
        let reconhecimento = pasta.join(MODELO_EMBEDDING);

        if !deteccao.is_file() || !reconhecimento.is_file() {
            return Err(RostoError::ModelosAusentes(pasta.display().to_string()));
        }

        Ok(Self {
            detector: Mutex::new(sessao(&deteccao, MODELO_DETECCAO)?),
            reconhecedor: Mutex::new(sessao(&reconhecimento, MODELO_EMBEDDING)?),
        })
    }

    /// Acha o rosto principal da imagem e devolve o vetor dele.
    pub fn assinatura(&self, jpeg: &[u8]) -> Result<Vec<f32>, RostoError> {
        let imagem =
            image::load_from_memory(jpeg).map_err(|erro| RostoError::Imagem(erro.to_string()))?;

        let rosto = deteccao::principal(self.detectar(&imagem)?)?;
        let recorte = embedding::alinhar(&imagem, &rosto);

        self.vetorizar(&recorte)
    }

    /// Os rostos da imagem, em coordenadas dela.
    fn detectar(&self, imagem: &DynamicImage) -> Result<Vec<Rosto>, RostoError> {
        let encaixe = Encaixe::novo(imagem.width(), imagem.height());
        let entrada = tensor_da_deteccao(imagem, encaixe);

        let sessao = lock(&self.detector);
        let valor = Value::from_array((
            [1_i64, 3, deteccao::ENTRADA as i64, deteccao::ENTRADA as i64],
            entrada,
        ))
        .map_err(|erro| RostoError::Inferencia(erro.to_string()))?;

        let saidas = sessao
            .run(
                ort::inputs!["input" => valor]
                    .map_err(|e| RostoError::Inferencia(e.to_string()))?,
            )
            .map_err(|erro| RostoError::Inferencia(erro.to_string()))?;

        // Os doze tensores, buscados por nome. Extrair aqui e decodificar no `deteccao`
        // é o que mantém a matemática do decode testável sem um modelo carregado.
        let mut escalas = Vec::new();
        let mut dados = Vec::new();
        for passo in [8_usize, 16, 32] {
            for grandeza in ["cls", "obj", "bbox", "kps"] {
                let chave = format!("{grandeza}_{passo}");
                let tensor = saidas[chave.as_str()]
                    .try_extract_tensor::<f32>()
                    .map_err(|erro| RostoError::Inferencia(erro.to_string()))?;

                // `iter()` e não `as_slice()`: a view pode não ser contígua, e o
                // `as_slice` devolveria `None` num caso que não dá para prever daqui.
                dados.push(tensor.iter().copied().collect::<Vec<f32>>());
            }
        }

        for (i, passo) in [8_usize, 16, 32].into_iter().enumerate() {
            escalas.push(Escala {
                passo,
                cls: &dados[i * 4],
                obj: &dados[i * 4 + 1],
                bbox: &dados[i * 4 + 2],
                kps: &dados[i * 4 + 3],
            });
        }

        Ok(deteccao::decodificar(&escalas, encaixe))
    }

    fn vetorizar(&self, recorte: &image::RgbImage) -> Result<Vec<f32>, RostoError> {
        let entrada = embedding::tensor(recorte);

        let sessao = lock(&self.reconhecedor);
        let valor = Value::from_array((
            [
                1_i64,
                3,
                embedding::ENTRADA as i64,
                embedding::ENTRADA as i64,
            ],
            entrada,
        ))
        .map_err(|erro| RostoError::Inferencia(erro.to_string()))?;

        let saidas = sessao
            .run(ort::inputs!["data" => valor].map_err(|e| RostoError::Inferencia(e.to_string()))?)
            .map_err(|erro| RostoError::Inferencia(erro.to_string()))?;

        let tensor = saidas["fc1"]
            .try_extract_tensor::<f32>()
            .map_err(|erro| RostoError::Inferencia(erro.to_string()))?;

        Ok(tensor.iter().copied().collect())
    }
}

fn sessao(caminho: &Path, nome: &str) -> Result<Session, RostoError> {
    Session::builder()
        .and_then(|construtor| construtor.commit_from_file(caminho))
        .map_err(|erro| RostoError::ModeloInvalido {
            modelo: nome.to_owned(),
            detalhe: erro.to_string(),
        })
}

/// A imagem no formato do detector: `[1, 3, 640, 640]`, proporcional e centralizada.
fn tensor_da_deteccao(imagem: &DynamicImage, encaixe: Encaixe) -> Vec<f32> {
    let lado = deteccao::ENTRADA as u32;
    let largura = (imagem.width() as f32 * encaixe.escala).round().max(1.0) as u32;
    let altura = (imagem.height() as f32 * encaixe.escala).round().max(1.0) as u32;

    let reduzida = imagem
        .resize_exact(largura, altura, image::imageops::FilterType::Triangle)
        .to_rgb8();

    let pixels = (lado * lado) as usize;
    let mut dados = vec![0.0_f32; pixels * 3];

    for (x, y, pixel) in reduzida.enumerate_pixels() {
        let destino = ((y + encaixe.deslocamento_y as u32) * lado
            + (x + encaixe.deslocamento_x as u32)) as usize;
        if destino >= pixels {
            continue;
        }

        // NCHW, e o YuNet do OpenCV Zoo recebe o pixel cru — como o SFace.
        dados[destino] = f32::from(pixel[0]);
        dados[pixels + destino] = f32::from(pixel[1]);
        dados[pixels * 2 + destino] = f32::from(pixel[2]);
    }

    dados
}

/// Os modelos, carregados só quando alguém precisa deles.
///
/// **Preguiçoso pela mesma razão dos serviços de voz**: carregar os dois ONNX custa
/// centenas de milissegundos e 37 MB de memória, e quem nunca liga a saudação não deve
/// pagar isso no boot. A primeira pergunta paga; as seguintes reaproveitam.
///
/// Registrado com `app.manage`, como as outras capacidades.
#[derive(Default)]
pub struct Reconhecedor {
    modelos: Mutex<Option<std::sync::Arc<Modelos>>>,
}

impl Reconhecedor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Os modelos prontos, carregando-os se for a primeira vez.
    ///
    /// Devolve um `Arc` para o chamador poder soltar o mutex antes de rodar a inferência
    /// — que leva dezenas de milissegundos e travaria qualquer outra chamada.
    pub fn modelos(&self, pasta: &Path) -> Result<std::sync::Arc<Modelos>, RostoError> {
        let mut guarda = lock(&self.modelos);

        if let Some(modelos) = guarda.as_ref() {
            return Ok(modelos.clone());
        }

        let modelos = std::sync::Arc::new(Modelos::carregar(pasta)?);
        *guarda = Some(modelos.clone());

        Ok(modelos)
    }
}

/// As pessoas que o Jarvis conhece, e o disco onde elas moram.
///
/// Ao lado do `casa.json` e do `cameras.json`, pela mesma razão: é dado que o app
/// aprendeu, não configuração que alguém escolheu numa tela.
pub struct Conhecidos {
    path: PathBuf,
    pessoas: Mutex<BTreeMap<String, Pessoa>>,
}

impl Conhecidos {
    pub fn new(config_dir: &Path) -> Self {
        let path = config_dir.join(ARQUIVO);
        let pessoas = carregar(&path).unwrap_or_else(|erro| {
            eprintln!("[jarvis] não consegui ler o {ARQUIVO} ({erro}); seguindo sem rostos");
            BTreeMap::new()
        });

        Self {
            path,
            pessoas: Mutex::new(pessoas),
        }
    }

    pub fn todas(&self) -> Vec<Pessoa> {
        lock(&self.pessoas).values().cloned().collect()
    }

    pub fn vazio(&self) -> bool {
        lock(&self.pessoas).is_empty()
    }

    /// Guarda mais uma assinatura para `nome`, criando a pessoa se ela não existir.
    ///
    /// **Acrescenta em vez de substituir.** Cada cadastro é uma condição diferente (com
    /// óculos, de manhã, com barba), e é a coleção que faz o reconhecimento funcionar
    /// fora do dia em que a primeira foto foi tirada.
    pub fn aprender(&self, nome: &str, assinatura: Vec<f32>) -> Result<Pessoa, StorageError> {
        let nome = nome.trim();
        let id = identificador(nome);

        let mut mapa = lock(&self.pessoas);
        let pessoa = mapa.entry(id.clone()).or_insert_with(|| Pessoa {
            id,
            nome: nome.to_owned(),
            ..Pessoa::default()
        });

        // O nome mais recente ganha: corrigir "guilerme" para "Guilherme" é um caso real,
        // e o id continua o mesmo porque sai do texto normalizado.
        pessoa.nome = nome.to_owned();
        pessoa.assinaturas.push(assinatura);
        pessoa.visto_em = chrono::Utc::now().timestamp_millis();

        let copia = pessoa.clone();
        gravar(&self.path, &mapa)?;

        Ok(copia)
    }

    pub fn esquecer(&self, id: &str) -> Result<(), StorageError> {
        let mut mapa = lock(&self.pessoas);
        mapa.remove(id);
        gravar(&self.path, &mapa)
    }

    /// Quem é este rosto, se for alguém conhecido.
    ///
    /// Compara contra TODAS as assinaturas de cada pessoa e fica com a melhor — é o que
    /// permite reconhecer você de boné depois de ter cadastrado uma foto de boné, sem que
    /// as outras fotos atrapalhem. `None` quando ninguém passa do limiar, e aí o certo é
    /// não arriscar nome nenhum.
    pub fn quem_e(&self, assinatura: &[f32]) -> Option<Reconhecido> {
        let mapa = lock(&self.pessoas);

        let mut melhor: Option<Reconhecido> = None;
        for pessoa in mapa.values() {
            for conhecida in &pessoa.assinaturas {
                let semelhanca = embedding::semelhanca(assinatura, conhecida);
                if semelhanca < embedding::LIMIAR {
                    continue;
                }

                // `match` e não `is_none_or`: aquele só é estável desde o Rust 1.82, e
                // este projeto declara 1.77.2 — o clippy do repo trata isso como erro, e
                // com razão, porque quebraria a compilação de quem está no piso.
                let e_melhor = match melhor.as_ref() {
                    Some(atual) => semelhanca > atual.semelhanca,
                    None => true,
                };

                if e_melhor {
                    melhor = Some(Reconhecido {
                        id: pessoa.id.clone(),
                        nome: pessoa.nome.clone(),
                        semelhanca,
                    });
                }
            }
        }

        melhor
    }

    /// Anota que a pessoa foi vista agora. Falha de gravação não derruba a saudação.
    pub fn marcar_visto(&self, id: &str) {
        let mut mapa = lock(&self.pessoas);
        if let Some(pessoa) = mapa.get_mut(id) {
            pessoa.visto_em = chrono::Utc::now().timestamp_millis();
            if let Err(erro) = gravar(&self.path, &mapa) {
                eprintln!("[jarvis] não consegui anotar a visita no {ARQUIVO}: {erro}");
            }
        }
    }
}

/// O id a partir do nome falado. "Guilherme" e "guilherme " são a mesma pessoa.
fn identificador(nome: &str) -> String {
    let normalizado = normalizar(nome);
    let limpo: String = normalizado
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    limpo.trim_matches('-').to_owned()
}

fn carregar(path: &Path) -> Result<BTreeMap<String, Pessoa>, StorageError> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(raw.trim_start_matches('\u{feff}'))?)
}

fn gravar(path: &Path, mapa: &BTreeMap<String, Pessoa>) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, serde_json::to_string_pretty(mapa)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jarvis-rostos-{nome}"));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    /// Um vetor que casa consigo mesmo (cosseno 1) e não casa com o oposto.
    fn vetor(primeiro: f32) -> Vec<f32> {
        let mut v = vec![0.1; 128];
        v[0] = primeiro;
        v
    }

    #[test]
    fn o_id_sai_do_nome_normalizado() {
        assert_eq!(identificador("Guilherme"), "guilherme");
        assert_eq!(identificador("  GUILHERME  "), "guilherme");
        // Acento não pode criar duas pessoas.
        assert_eq!(identificador("Antônio"), identificador("antonio"));
    }

    #[test]
    fn aprende_e_reconhece() {
        let catalogo = Conhecidos::new(&pasta("aprende"));
        assert!(catalogo.vazio());

        catalogo.aprender("Guilherme", vetor(10.0)).unwrap();

        let quem = catalogo.quem_e(&vetor(10.0)).unwrap();
        assert_eq!(quem.nome, "Guilherme");
        assert!(quem.semelhanca > embedding::LIMIAR);
    }

    /// Rosto desconhecido não pode virar palpite: chamar alguém pelo nome errado é o erro
    /// que mais incomoda, e é o que o limiar existe para evitar.
    #[test]
    fn rosto_desconhecido_nao_vira_palpite() {
        let catalogo = Conhecidos::new(&pasta("desconhecido"));
        catalogo.aprender("Guilherme", vetor(10.0)).unwrap();

        // Oposto: cosseno negativo, longe do limiar.
        let estranho: Vec<f32> = vetor(10.0).iter().map(|x| -x).collect();
        assert!(catalogo.quem_e(&estranho).is_none());
    }

    #[test]
    fn catalogo_vazio_nao_reconhece_ninguem() {
        let catalogo = Conhecidos::new(&pasta("vazio"));
        assert!(catalogo.quem_e(&vetor(1.0)).is_none());
    }

    /// **A coleção é o que faz funcionar.** Cadastrar de novo em outra condição não pode
    /// apagar a anterior, senão reconhecer de óculos faria parar de reconhecer sem.
    #[test]
    fn aprender_de_novo_acrescenta_em_vez_de_substituir() {
        let catalogo = Conhecidos::new(&pasta("acrescenta"));
        catalogo.aprender("Guilherme", vetor(10.0)).unwrap();
        let pessoa = catalogo.aprender("Guilherme", vetor(-3.0)).unwrap();

        assert_eq!(pessoa.assinaturas.len(), 2);
        assert_eq!(catalogo.todas().len(), 1, "continua sendo uma pessoa só");

        // As duas condições reconhecem.
        assert!(catalogo.quem_e(&vetor(10.0)).is_some());
        assert!(catalogo.quem_e(&vetor(-3.0)).is_some());
    }

    /// Corrigir a grafia do nome não pode criar uma segunda pessoa.
    #[test]
    fn corrigir_o_nome_mantem_a_mesma_pessoa() {
        let catalogo = Conhecidos::new(&pasta("grafia"));
        catalogo.aprender("guilherme", vetor(10.0)).unwrap();
        catalogo.aprender("Guilherme", vetor(10.0)).unwrap();

        let todas = catalogo.todas();
        assert_eq!(todas.len(), 1);
        assert_eq!(todas[0].nome, "Guilherme", "a grafia mais recente ganha");
    }

    #[test]
    fn sobrevive_ao_disco() {
        let dir = pasta("disco");
        Conhecidos::new(&dir).aprender("Ana", vetor(5.0)).unwrap();

        let outro = Conhecidos::new(&dir);
        assert_eq!(outro.todas().len(), 1);
        assert_eq!(outro.quem_e(&vetor(5.0)).unwrap().nome, "Ana");
    }

    #[test]
    fn esquecer_apaga() {
        let catalogo = Conhecidos::new(&pasta("esquecer"));
        let pessoa = catalogo.aprender("Ana", vetor(5.0)).unwrap();

        catalogo.esquecer(&pessoa.id).unwrap();
        assert!(catalogo.vazio());
        assert!(catalogo.quem_e(&vetor(5.0)).is_none());
    }

    /// Roda o detector contra uma FOTO CONHECIDA, com um rosto grande e bem iluminado.
    ///
    /// Existe para separar dois defeitos que dão o mesmo sintoma ("não achei rosto"): a
    /// câmera não estar vendo ninguém, e a decodificação da saída do modelo estar errada.
    /// Se este teste acha o rosto e o da webcam não, o problema é a cena; se este também
    /// falha, o problema é a matemática.
    ///
    /// `cargo test --lib -- --ignored --nocapture detecta_numa_foto_conhecida`
    #[test]
    #[ignore]
    fn detecta_numa_foto_conhecida() {
        let modelos = match Modelos::carregar(&pasta_dos_modelos()) {
            Ok(modelos) => modelos,
            Err(erro) => {
                println!("pulando: {erro}");
                return;
            }
        };

        let caminho = std::env::temp_dir().join("jarvis-rosto-teste.jpg");
        let Ok(imagem) = image::open(&caminho) else {
            println!("pulando: não achei {}", caminho.display());
            return;
        };
        println!("imagem {}×{}", imagem.width(), imagem.height());

        let achados = modelos.detectar(&imagem).expect("detectar");
        println!("{} rosto(s)", achados.len());
        for rosto in &achados {
            println!(
                "  caixa x={:.0} y={:.0} {:.0}×{:.0} conf={:.2}  olhos dir=({:.0},{:.0}) esq=({:.0},{:.0})",
                rosto.x,
                rosto.y,
                rosto.largura,
                rosto.altura,
                rosto.confianca,
                rosto.pontos[0].0,
                rosto.pontos[0].1,
                rosto.pontos[1].0,
                rosto.pontos[1].1
            );
        }

        if let Ok(rosto) = deteccao::principal(achados) {
            let recorte = embedding::alinhar(&imagem, &rosto);
            let saida = std::env::temp_dir().join("jarvis-alinhado.png");
            let _ = recorte.save(&saida);
            println!("recorte alinhado em {}", saida.display());
        }
    }

    /// A pasta onde os modelos ficam numa instalação de verdade.
    fn pasta_dos_modelos() -> PathBuf {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
            .join("com.jarvis.app")
            .join("rostos")
    }

    /// Roda o pipeline INTEIRO contra a webcam de verdade: tira uma foto, acha o rosto,
    /// gera a assinatura e mede a semelhança de duas capturas seguidas.
    ///
    /// Fora do `cargo test` comum porque precisa dos modelos na pasta e de uma câmera com
    /// alguém na frente — numa máquina de CI não há nem um nem outro. É a única forma de
    /// saber se a matemática do decode e do alinhamento está certa: os testes de mesa
    /// provam que as contas fecham, não que elas encontram um rosto.
    ///
    /// **A segunda medição é a que importa.** Duas fotos suas seguidas têm que passar
    /// folgado do limiar; se elas mal se parecem, o alinhamento está errado e o
    /// reconhecimento nunca vai funcionar por mais fotos que se cadastre.
    ///
    /// `cargo test --lib -- --ignored --nocapture reconhece_de_verdade`
    #[test]
    #[ignore]
    fn reconhece_de_verdade() {
        let modelos = match Modelos::carregar(&pasta_dos_modelos()) {
            Ok(modelos) => modelos,
            Err(erro) => {
                println!("pulando: {erro}");
                return;
            }
        };
        println!("modelos carregados.");

        let camera = crate::core::automation::AutomationState::new();
        let mut assinaturas = Vec::new();

        // Quatro, e não duas: a primeira captura com a câmera fria costuma sair antes de
        // a exposição estabilizar, e essa foto não tem rosto nenhum para achar.
        for tentativa in 1..=4 {
            let relogio = std::time::Instant::now();
            let quadro = match camera.capture_webcam_frame(None, None) {
                Ok(quadro) => quadro,
                Err(erro) => {
                    println!("não consegui usar a webcam: {erro}");
                    return;
                }
            };
            let capturou = relogio.elapsed();

            let jpeg = base64_do_data_url(&quadro.data_url);
            let relogio = std::time::Instant::now();

            // Diagnóstico: o que o detector achou, e o que o reconhecedor vai ver.
            let imagem = image::load_from_memory(&jpeg).expect("jpeg");

            // O quadro cru em disco: sem olhar para ele não dá para separar "a câmera
            // está escura" de "o detector não achou o rosto que estava lá".
            let bruto = std::env::temp_dir().join(format!("jarvis-quadro-{tentativa}.png"));
            let _ = imagem.save(&bruto);

            // O brilho médio conta a mesma história em um número: uma webcam que ainda
            // não ajustou a exposição devolve um quadro quase preto.
            let cinza = imagem.to_luma8();
            let brilho: f64 =
                cinza.pixels().map(|p| f64::from(p[0])).sum::<f64>() / cinza.len() as f64;
            println!(
                "  brilho médio {brilho:.0}/255 · quadro em {}",
                bruto.display()
            );
            let achados = modelos.detectar(&imagem).expect("detectar");
            println!(
                "foto {tentativa}: {}×{}, captura {:.0}ms, {} rosto(s)",
                quadro.width,
                quadro.height,
                capturou.as_millis(),
                achados.len()
            );

            let Ok(rosto) = deteccao::principal(achados) else {
                println!("  (nenhum rosto)");
                continue;
            };
            println!(
                "  caixa x={:.0} y={:.0} {:.0}×{:.0} conf={:.2}",
                rosto.x, rosto.y, rosto.largura, rosto.altura, rosto.confianca
            );
            println!(
                "  olhos: dir=({:.0},{:.0}) esq=({:.0},{:.0})  nariz=({:.0},{:.0})",
                rosto.pontos[0].0,
                rosto.pontos[0].1,
                rosto.pontos[1].0,
                rosto.pontos[1].1,
                rosto.pontos[2].0,
                rosto.pontos[2].1
            );

            // Grava o recorte para poder OLHAR o que o modelo recebe. É a única forma de
            // separar "o alinhamento está torto" de "a matemática do vetor está errada".
            let recorte = embedding::alinhar(&imagem, &rosto);
            let saida = std::env::temp_dir().join(format!("jarvis-rosto-{tentativa}.png"));
            let _ = recorte.save(&saida);
            println!("  recorte em {}", saida.display());

            match modelos.vetorizar(&recorte) {
                Ok(vetor) => {
                    println!(
                        "  vetor de {} números em {:.0}ms",
                        vetor.len(),
                        relogio.elapsed().as_millis()
                    );
                    assinaturas.push(vetor);
                }
                Err(erro) => println!("  {erro}"),
            }
        }

        println!("\n{} de 4 fotos renderam assinatura.", assinaturas.len());

        // Todos os pares, para ver se a semelhança se sustenta e não foi sorte de uma
        // dupla. Duas fotos suas seguidas têm que passar FOLGADO do limiar — se elas mal
        // se parecem, o alinhamento está errado e nenhum cadastro salva.
        let mut piores = f32::MAX;
        for (i, a) in assinaturas.iter().enumerate() {
            for b in assinaturas.iter().skip(i + 1) {
                let semelhanca = embedding::semelhanca(a, b);
                piores = piores.min(semelhanca);
                println!("  par: {semelhanca:.3}");
            }
        }

        if piores < f32::MAX {
            println!(
                "\npior par: {piores:.3}  (limiar {:.3})\n{}",
                embedding::LIMIAR,
                if piores > embedding::LIMIAR {
                    "OK — a mesma pessoa casa consigo mesma com folga."
                } else {
                    "RUIM — duas fotos seguidas não casaram; revisar o alinhamento."
                }
            );
        }
    }

    /// Os bytes do JPEG a partir do `data:` URL que a captura devolve.
    fn base64_do_data_url(data_url: &str) -> Vec<u8> {
        use base64::Engine;

        let so_base64 = crate::core::vision::so_o_base64(data_url);
        base64::engine::general_purpose::STANDARD
            .decode(so_base64)
            .unwrap_or_default()
    }

    /// Com duas pessoas parecidas, ganha a mais parecida — não a primeira da lista.
    #[test]
    fn escolhe_a_pessoa_mais_parecida() {
        let catalogo = Conhecidos::new(&pasta("melhor"));
        catalogo.aprender("Ana", vetor(3.0)).unwrap();
        catalogo.aprender("Bruno", vetor(30.0)).unwrap();

        assert_eq!(catalogo.quem_e(&vetor(30.0)).unwrap().nome, "Bruno");
    }
}
