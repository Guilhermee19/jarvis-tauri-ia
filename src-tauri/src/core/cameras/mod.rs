//! As câmeras de segurança da casa, e o caminho até a imagem delas.
//!
//! Duas marcas, dois dialetos, e um só jeito de olhar. O **DVR Xiongmai** (o que o app
//! XMEye abre) fala RTSP com Digest numa URL de formato próprio, e é *multi-canal* — um
//! endereço só carrega várias câmeras. A **V380** fala ONVIF sem senha nenhuma e entrega
//! a URL do stream quando perguntada. Nada disso chega à tela sozinho: a webview não
//! decodifica H.264, e o app não embarca decoder.
//!
//! Quem atravessa essa ponte é o **go2rtc**, que sobe como serviço em
//! [`crate::core::services`] igual ao Piper e ao Whisper. Ele recebe RTSP e devolve duas
//! coisas: vídeo que o navegador toca, e JPEG por HTTP. O JPEG é o que faz a visão
//! funcionar de graça — é o mesmo formato que a webcam já produz, então
//! [`crate::core::vision`] não precisou aprender nada novo.
//!
//! ## Por que um catálogo, e não configuração
//!
//! Mesma divisão que o [`crate::core::casa::chaveiro`] faz: configuração é o que você
//! escolhe, catálogo é o que o app precisa saber para funcionar. Uma lista de câmeras
//! com senha de DVR não pertence à tela de preferências, e o arquivo é reescrito por
//! inteiro a cada cadastro — coisa que ninguém quer que aconteça com o `settings.json`.
//!
//! ## Sobre guardar a senha em texto puro
//!
//! A senha do DVR fica legível no `cameras.json`, como a `local_key` da Tuya no
//! `casa.json` e as chaves da Anthropic e do Spotify no `settings.json`. É a política
//! atual do projeto e vale a mesma ressalva escrita nos dois: o lugar certo é o keyring
//! do sistema. Manter os três juntos e migrar de uma vez é melhor que inventar um
//! esquema só para as câmeras — e é o que faz esta linha ser uma dívida registrada em
//! vez de uma surpresa.

pub mod go2rtc;
pub mod onvif;
pub mod varredura;
pub mod vigia;
pub mod xiongmai;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::core::lock;
use crate::core::memory::normalizar;
use crate::storage::StorageError;

/// Que dialeto a câmera fala. Decide como a URL do stream é montada e se dá para
/// mexer nela.
///
/// Não é a MARCA: o que importa aqui é o protocolo. Uma câmera nova de outro fabricante
/// que fale ONVIF entra como [`TipoDeCamera::Onvif`] sem código novo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoDeCamera {
    /// DVR Xiongmai/Sofia — o que o XMEye abre. Multi-canal, RTSP com Digest, URL de
    /// formato proprietário. Sem PTZ por este caminho.
    #[default]
    Dvr,
    /// Câmera ONVIF, como a V380. A própria câmera diz a URL do stream, e costuma
    /// aceitar PTZ.
    Onvif,
}

/// Uma câmera, do jeito que o app precisa conhecê-la.
///
/// `Default` para o `#[serde(default)]` conseguir ler um `cameras.json` escrito por uma
/// versão anterior sem o campo que acabou de nascer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Camera {
    /// Identificador estável, e **é ele que vira o `src` do go2rtc**. Sem espaço nem
    /// acento, porque entra numa query string e num YAML.
    pub id: String,
    /// Como você a chama em voz alta: "garagem", "portão". É por aqui que
    /// [`Catalogo::achar_por_nome`] casa a frase com o aparelho.
    pub nome: String,
    /// IP na rede local. Sem porta: cada protocolo tem a sua e elas moram no código.
    pub host: String,
    pub tipo: TipoDeCamera,
    /// Qual câmera dentro do DVR, começando em 1. Ignorado quando o tipo é
    /// [`TipoDeCamera::Onvif`], onde um endereço é uma câmera só.
    pub canal: u8,
    pub usuario: String,
    pub senha: String,
    /// A URL RTSP crua, quando ela não pode ser derivada.
    ///
    /// **Vazio é o caso normal**, e não um cadastro pela metade: para o DVR a URL sai do
    /// host, canal e credenciais, e guardá-la seria manter dois lugares dizendo a mesma
    /// coisa — com a garantia de que um deles fica velho. Preenchido, ganha do palpite:
    /// é a saída para a câmera cuja URL veio do `GetStreamUri` ou de um firmware
    /// esquisito. Mesma convenção de `cidade` e `ollama_model` nas configurações.
    pub rtsp_url: String,
    /// Fora da grade principal, por escolha sua. Como o `oculto` do chaveiro, é sobre a
    /// tela: ela continua no catálogo e continua atendendo por voz.
    pub oculto: bool,
    /// Vigiar esta câmera em busca de movimento. Ver [`vigia`].
    pub vigiar: bool,
}

impl Camera {
    /// A URL RTSP que o go2rtc vai consumir, credenciais inclusas.
    ///
    /// Derivada em vez de guardada — ver o porquê no campo [`Camera::rtsp_url`].
    pub fn rtsp(&self) -> String {
        if !self.rtsp_url.trim().is_empty() {
            return onvif::com_credenciais(self.rtsp_url.trim(), &self.usuario, &self.senha);
        }

        match self.tipo {
            TipoDeCamera::Dvr => xiongmai::rtsp(&self.host, self.canal, &self.usuario, &self.senha),
            // Sem URL guardada, o palpite da ONVIF é o caminho que a V380 devolveu no
            // `GetStreamUri`. Erra num firmware diferente — e é exatamente por isso que
            // o cadastro pode gravar a URL de verdade em vez de depender disto.
            TipoDeCamera::Onvif => onvif::rtsp_provavel(&self.host, &self.usuario, &self.senha),
        }
    }

    /// Se dá para mandar esta câmera virar. Só o ONVIF tem esse caminho aqui.
    pub fn tem_ptz(&self) -> bool {
        matches!(self.tipo, TipoDeCamera::Onvif)
    }
}

const ARQUIVO: &str = "cameras.json";

/// As câmeras conhecidas, e o disco onde elas moram.
///
/// Registrado com `app.manage` como capacidade própria, ao lado do `Chaveiro` e pela
/// mesma razão: as configurações são de um dono, o que o app precisa saber para
/// funcionar é de outro.
pub struct Catalogo {
    path: PathBuf,
    /// `BTreeMap` para o arquivo sair em ordem estável entre gravações — um JSON que
    /// se embaralha sozinho é ilegível no diff de quem versiona a pasta de config.
    cameras: Mutex<BTreeMap<String, Camera>>,
}

impl Catalogo {
    /// Lê o que houver no disco. Falha de leitura **não** impede o app de subir: sem
    /// catálogo o painel de câmeras fica vazio, que é um app pior mas inteiro.
    pub fn new(config_dir: &Path) -> Self {
        let path = config_dir.join(ARQUIVO);
        let cameras = carregar(&path).unwrap_or_else(|erro| {
            eprintln!("[jarvis] não consegui ler o {ARQUIVO} ({erro}); seguindo sem câmeras");
            BTreeMap::new()
        });

        Self {
            path,
            cameras: Mutex::new(cameras),
        }
    }

    pub fn todas(&self) -> Vec<Camera> {
        lock(&self.cameras).values().cloned().collect()
    }

    pub fn de(&self, id: &str) -> Option<Camera> {
        lock(&self.cameras).get(id).cloned()
    }

    pub fn vazio(&self) -> bool {
        lock(&self.cameras).is_empty()
    }

    /// Cadastra ou atualiza. O `id` é a chave; o resto vem por cima.
    pub fn guardar(&self, camera: Camera) -> Result<(), StorageError> {
        let mut mapa = lock(&self.cameras);
        mapa.insert(camera.id.clone(), camera);
        gravar(&self.path, &mapa)
    }

    pub fn remover(&self, id: &str) -> Result<(), StorageError> {
        let mut mapa = lock(&self.cameras);
        mapa.remove(id);
        gravar(&self.path, &mapa)
    }

    /// Procura uma câmera pelo nome dito em voz alta.
    ///
    /// Mesma regra do [`crate::core::casa::chaveiro::Chaveiro::achar_por_nome`]: TODAS
    /// as palavras do nome da câmera têm que aparecer na frase. O caminho inverso
    /// casaria "a câmera" com qualquer uma da casa.
    ///
    /// A diferença é o desempate: aqui, **uma câmera só cadastrada ganha o empate
    /// vazio**. "mostra a câmera" com uma única câmera na casa é um pedido sem
    /// ambiguidade, e obrigar a dizer o nome dela seria burocracia.
    pub fn achar_por_nome(&self, dito: &str) -> Busca {
        let frase = normalizar(dito);
        let mapa = lock(&self.cameras);

        let palavras: Vec<&str> = frase.split(' ').filter(|p| !p.is_empty()).collect();
        let mut candidatos: Vec<(usize, Camera)> = Vec::new();

        for camera in mapa.values() {
            let nome = normalizar(&camera.nome);
            if nome.is_empty() {
                continue;
            }

            let do_nome: Vec<&str> = nome.split(' ').filter(|p| !p.is_empty()).collect();
            if do_nome.iter().all(|palavra| palavras.contains(palavra)) {
                candidatos.push((do_nome.len(), camera.clone()));
            }
        }

        if candidatos.is_empty() {
            // Ninguém pelo nome. Com uma câmera só na casa, ela É a resposta.
            let visiveis: Vec<&Camera> = mapa.values().filter(|c| !c.oculto).collect();
            return match visiveis.as_slice() {
                [unica] => Busca::Uma(Box::new((*unica).clone())),
                _ => Busca::Nenhuma,
            };
        }

        // Nome mais específico ganha: com "portão" e "portão fundos" cadastrados, "olha
        // o portão dos fundos" casa com os dois, e o certo é o de duas palavras.
        let melhor = candidatos
            .iter()
            .map(|(quantas, _)| *quantas)
            .max()
            .unwrap_or(0);
        candidatos.retain(|(quantas, _)| *quantas == melhor);

        match candidatos.len() {
            1 => Busca::Uma(Box::new(candidatos.remove(0).1)),
            // Empate de verdade: perguntar é melhor que mostrar a câmera errada.
            _ => Busca::Varias(candidatos.into_iter().map(|(_, c)| c.nome).collect()),
        }
    }
}

/// O resultado de procurar uma câmera por nome.
///
/// Três casos e não um `Option`, pela mesma razão da [`crate::core::casa::chaveiro::Busca`]:
/// "não achei" e "achei três" pedem respostas diferentes do assistente.
pub enum Busca {
    Uma(Box<Camera>),
    Nenhuma,
    Varias(Vec<String>),
}

fn carregar(path: &Path) -> Result<BTreeMap<String, Camera>, StorageError> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let raw = fs::read_to_string(path)?;

    // Mesmo cuidado com o BOM do `json_store`: quem abre o arquivo no Bloco de Notas
    // para conferir uma senha e salva sem querer põe três bytes invisíveis na frente.
    Ok(serde_json::from_str(raw.trim_start_matches('\u{feff}'))?)
}

fn gravar(path: &Path, mapa: &BTreeMap<String, Camera>) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, serde_json::to_string_pretty(mapa)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uma pasta limpa por teste.
    ///
    /// **Só apaga na primeira chamada de cada teste**, e isso não é detalhe: o teste de
    /// ida-e-volta ao disco abre DOIS catálogos na mesma pasta, e um `remove_dir_all` no
    /// segundo apagaria justamente o arquivo que ele foi conferir.
    fn pasta(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jarvis-cameras-{nome}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    /// Pasta limpa, para quem começa do zero.
    fn pasta_nova(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jarvis-cameras-{nome}"));
        let _ = fs::remove_dir_all(&dir);
        pasta(nome)
    }

    fn camera(id: &str, nome: &str) -> Camera {
        Camera {
            id: id.to_owned(),
            nome: nome.to_owned(),
            host: "192.168.18.249".to_owned(),
            tipo: TipoDeCamera::Dvr,
            canal: 1,
            usuario: "admin".to_owned(),
            ..Camera::default()
        }
    }

    #[test]
    fn guarda_e_le_de_volta() {
        let catalogo = Catalogo::new(&pasta_nova("ida-e-volta"));
        assert!(catalogo.vazio());

        catalogo.guardar(camera("garagem", "garagem")).unwrap();
        assert_eq!(catalogo.de("garagem").unwrap().nome, "garagem");

        // O que importa é sobreviver ao disco: um catálogo novo lê o mesmo arquivo.
        let outro = Catalogo::new(&pasta("ida-e-volta"));
        assert_eq!(outro.todas().len(), 1);
    }

    #[test]
    fn nome_mais_especifico_ganha() {
        let catalogo = Catalogo::new(&pasta_nova("especifico"));
        catalogo.guardar(camera("portao", "portão")).unwrap();
        catalogo
            .guardar(camera("portao-fundos", "portão fundos"))
            .unwrap();

        // As duas casam com a frase; a de duas palavras é a certa.
        match catalogo.achar_por_nome("olha o portão dos fundos") {
            Busca::Uma(c) => assert_eq!(c.id, "portao-fundos"),
            _ => panic!("devia ter achado a dos fundos"),
        }
    }

    /// "mostra a câmera" com uma câmera só é um pedido sem ambiguidade — exigir o nome
    /// dela seria burocracia, e é o caso mais comum de quem tem uma câmera só.
    #[test]
    fn com_uma_camera_so_nao_precisa_dizer_o_nome() {
        let catalogo = Catalogo::new(&pasta_nova("unica"));
        catalogo.guardar(camera("garagem", "garagem")).unwrap();

        match catalogo.achar_por_nome("mostra a câmera") {
            Busca::Uma(c) => assert_eq!(c.id, "garagem"),
            _ => panic!("com uma só, ela é a resposta"),
        }
    }

    /// Com duas, o mesmo pedido vago passa a ser ambíguo de verdade.
    #[test]
    fn com_duas_cameras_o_pedido_vago_vira_pergunta() {
        let catalogo = Catalogo::new(&pasta_nova("ambigua"));
        catalogo.guardar(camera("garagem", "garagem")).unwrap();
        catalogo.guardar(camera("quintal", "quintal")).unwrap();

        assert!(matches!(
            catalogo.achar_por_nome("mostra a câmera"),
            Busca::Nenhuma
        ));
    }

    /// Acento e caixa não podem separar "Garagem" de "garagem" — quem fala não digita.
    #[test]
    fn acha_apesar_do_acento_e_da_caixa() {
        let catalogo = Catalogo::new(&pasta_nova("acento"));
        catalogo.guardar(camera("portao", "Portão")).unwrap();
        catalogo.guardar(camera("garagem", "garagem")).unwrap();

        match catalogo.achar_por_nome("abre o PORTAO") {
            Busca::Uma(c) => assert_eq!(c.id, "portao"),
            _ => panic!("normalizar devia ter resolvido isso"),
        }
    }

    /// A URL derivada é a do DVR; a guardada ganha dela. Sem isso, cadastrar a URL que
    /// o `GetStreamUri` devolveu não teria efeito nenhum.
    #[test]
    fn a_url_guardada_ganha_da_derivada() {
        let derivada = camera("garagem", "garagem").rtsp();
        assert!(derivada.contains("channel=1"));

        let mut fixa = camera("v380", "quintal");
        fixa.tipo = TipoDeCamera::Onvif;
        fixa.rtsp_url = "rtsp://192.168.18.179/live/ch00_0".to_owned();

        // O caminho é o guardado, e a credencial cadastrada entra nele: a câmera diz
        // ONDE está o stream, não como entrar. Sem essa injeção o go2rtc levaria 401
        // numa URL que veio da própria câmera.
        assert_eq!(fixa.rtsp(), "rtsp://admin:@192.168.18.179/live/ch00_0");
    }

    /// A V380 não tem senha nenhuma, e é o caso real desta casa: sem usuário, a URL
    /// guardada passa intacta em vez de ganhar um `@` solto que a inutilizaria.
    #[test]
    fn url_guardada_de_camera_sem_credencial_passa_intacta() {
        let mut aberta = camera("v380", "quintal");
        aberta.tipo = TipoDeCamera::Onvif;
        aberta.usuario = String::new();
        aberta.senha = String::new();
        aberta.rtsp_url = "rtsp://192.168.18.179/live/ch00_0".to_owned();

        assert_eq!(aberta.rtsp(), "rtsp://192.168.18.179/live/ch00_0");
    }

    #[test]
    fn so_a_onvif_tem_ptz() {
        assert!(!camera("garagem", "garagem").tem_ptz());

        let mut v380 = camera("v380", "quintal");
        v380.tipo = TipoDeCamera::Onvif;
        assert!(v380.tem_ptz());
    }
}
