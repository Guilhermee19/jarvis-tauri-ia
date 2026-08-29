//! O navegador de verdade dentro do Jarvis.
//!
//! Cada aba é um **webview filho** da janela principal (`Window::add_child`) — o mesmo
//! motor que desenha o resto do app, e não um `<iframe>`.
//!
//! ## Por que não `<iframe>`
//!
//! Foi medido: Google, YouTube e DuckDuckGo respondem `X-Frame-Options: SAMEORIGIN` e
//! ficariam em branco. O exemplo canônico do roteador de intenção é literalmente "abre o
//! youtube", então um navegador que não abre o YouTube não é um navegador.
//!
//! ## O preço, e ele é real
//!
//! O webview filho é uma camada **nativa**, empilhada acima de todo o HTML da janela. O
//! HUD, a moldura e as outras janelinhas **não conseguem** ser desenhados por cima dele.
//! Duas consequências que moldam o resto do desenho:
//!
//! - A área de conteúdo é um retângulo que a tela precisa **informar** e manter
//!   atualizado — ninguém posiciona isso por CSS. É o `definir_area`.
//! - Fechar a janelinha não basta esconder o HTML: o webview precisa ser escondido
//!   explicitamente, senão ele continua desenhado sobre um painel que não existe mais.
//!
//! Este módulo mora fora de `core/` de propósito: ele **é** Tauri da primeira à última
//! linha, e a regra de `core/` é justamente não conhecê-lo.

use std::sync::Mutex;

use serde::Serialize;
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};
use url::Url;

use crate::core::lock;
use crate::core::system::SystemError;

/// O rótulo do webview de cada aba. O número é só um contador que nunca reinicia —
/// reaproveitar rótulo de aba fechada faria o Tauri encontrar um webview que morreu.
const PREFIXO: &str = "aba-";

/// Onde a aba nasce enquanto a tela não informou a área de verdade.
///
/// Fora da tela, e não em `0,0`: entre criar o webview e a primeira medida do painel
/// passa um quadro, e nesse quadro ele cobriria o canto superior esquerdo da janela.
const LONGE: LogicalPosition<f64> = LogicalPosition::new(-10_000.0, -10_000.0);

/// Uma aba, do jeito que a tela precisa desenhá-la.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aba {
    pub id: String,
    pub url: String,
    /// O que aparece na lingueta. É o host, e não o `<title>` da página: pegar o título
    /// exigiria injetar JavaScript e esperar a resposta, e "youtube.com" já identifica a
    /// aba no instante em que ela nasce.
    pub titulo: String,
}

/// A área da janela onde as abas são desenhadas, em pixels lógicos.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub largura: f64,
    pub altura: f64,
}

/// Dono das abas.
#[derive(Default)]
pub struct Navegador {
    abas: Mutex<Vec<Aba>>,
    ativa: Mutex<Option<String>>,
    /// A última área informada pela tela. Guardada para a aba NOVA já nascer no lugar
    /// certo, em vez de aparecer fora da tela até o próximo redimensionamento.
    area: Mutex<Option<Area>>,
    /// Contador dos rótulos.
    proxima: Mutex<u32>,
}

impl Navegador {
    pub fn new() -> Self {
        Self::default()
    }

    /// Abre uma aba nova e a deixa ativa.
    pub fn abrir(&self, app: &AppHandle, url: &Url) -> Result<Aba, SystemError> {
        let Some(janela) = app.get_window("main") else {
            return Err(erro("a janela principal sumiu"));
        };

        let id = {
            let mut proxima = lock(&self.proxima);
            *proxima += 1;
            format!("{PREFIXO}{proxima}")
        };

        let area = lock(&self.area).unwrap_or_default();
        let webview = janela
            .add_child(
                WebviewBuilder::new(&id, WebviewUrl::External(url.clone()))
                    // Sem isto, um link com `target="_blank"` abriria uma JANELA nova do
                    // Tauri, sem barra de abas e sem como fechar. Dentro da mesma aba é o
                    // que um navegador simples faz.
                    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny),
                LONGE,
                LogicalSize::new(area.largura.max(1.0), area.altura.max(1.0)),
            )
            .map_err(|erro| self::erro(&format!("não consegui abrir a aba: {erro}")))?;

        let aba = Aba {
            titulo: titulo_de(url),
            url: url.to_string(),
            id: id.clone(),
        };

        lock(&self.abas).push(aba.clone());
        self.ativar(app, &id)?;
        let _ = webview;

        Ok(aba)
    }

    /// Traz uma aba para a frente e esconde as outras.
    ///
    /// Esconder é obrigatório, e não uma economia: webviews irmãos se empilham na ordem
    /// em que foram criados, então sem esconder os outros a aba "ativa" ficaria atrás das
    /// que vieram depois dela.
    pub fn ativar(&self, app: &AppHandle, id: &str) -> Result<(), SystemError> {
        let existe = lock(&self.abas).iter().any(|aba| aba.id == id);
        if !existe {
            return Err(erro("essa aba não existe mais"));
        }

        *lock(&self.ativa) = Some(id.to_owned());
        self.redesenhar(app);

        Ok(())
    }

    pub fn fechar(&self, app: &AppHandle, id: &str) {
        if let Some(webview) = app.get_webview(id) {
            let _ = webview.close();
        }

        let mut abas = lock(&self.abas);
        let indice = abas.iter().position(|aba| aba.id == id);
        abas.retain(|aba| aba.id != id);

        // A vizinha à esquerda vira a ativa, como em qualquer navegador. Sem isso o
        // painel ficaria vazio com abas abertas ao lado.
        let sucessora = indice
            .and_then(|indice| abas.get(indice.saturating_sub(1)))
            .map(|aba| aba.id.clone());
        drop(abas);

        if lock(&self.ativa).as_deref() == Some(id) {
            *lock(&self.ativa) = sucessora;
        }

        self.redesenhar(app);
    }

    /// Manda a aba ativa para outro endereço.
    pub fn navegar(&self, app: &AppHandle, id: &str, url: &Url) -> Result<(), SystemError> {
        let Some(webview) = app.get_webview(id) else {
            return Err(erro("essa aba não existe mais"));
        };

        webview
            .navigate(url.clone())
            .map_err(|erro| self::erro(&format!("não consegui navegar: {erro}")))?;

        if let Some(aba) = lock(&self.abas).iter_mut().find(|aba| aba.id == id) {
            aba.url = url.to_string();
            aba.titulo = titulo_de(url);
        }

        Ok(())
    }

    /// Anda no histórico da aba. `-1` volta, `1` avança.
    ///
    /// Por `eval` porque o Tauri não expõe histórico: `history.go` é a mesma coisa que o
    /// botão do navegador faz, e evita guardarmos uma pilha própria que sairia de sincronia
    /// com a de verdade no primeiro redirecionamento.
    pub fn historico(&self, app: &AppHandle, id: &str, passo: i32) -> Result<(), SystemError> {
        let Some(webview) = app.get_webview(id) else {
            return Err(erro("essa aba não existe mais"));
        };

        webview
            .eval(format!("history.go({})", passo.clamp(-1, 1)))
            .map_err(|erro| self::erro(&format!("não consegui andar no histórico: {erro}")))
    }

    /// Informa onde as abas devem ser desenhadas. Chamado pela tela a cada mudança de
    /// tamanho ou posição do painel.
    pub fn definir_area(&self, app: &AppHandle, area: Option<Area>) {
        *lock(&self.area) = area;
        self.redesenhar(app);
    }

    pub fn abas(&self) -> Vec<Aba> {
        lock(&self.abas).clone()
    }

    pub fn ativa(&self) -> Option<String> {
        lock(&self.ativa).clone()
    }

    pub fn url_de(&self, id: &str) -> Option<String> {
        lock(&self.abas)
            .iter()
            .find(|aba| aba.id == id)
            .map(|aba| aba.url.clone())
    }

    /// Põe cada webview no lugar: a ativa na área informada, as outras escondidas.
    ///
    /// `area` vazia quer dizer "o painel está fechado" — e aí TODAS somem. Sem isso o
    /// navegador continuaria desenhado sobre um painel que já não existe, e não haveria
    /// como clicar em nada por baixo dele.
    fn redesenhar(&self, app: &AppHandle) {
        let area = *lock(&self.area);
        let ativa = lock(&self.ativa).clone();

        for aba in lock(&self.abas).iter() {
            let Some(webview) = app.get_webview(&aba.id) else {
                continue;
            };

            match (area, ativa.as_deref() == Some(aba.id.as_str())) {
                (Some(area), true) => {
                    let _ = webview.set_position(LogicalPosition::new(area.x, area.y));
                    let _ =
                        webview.set_size(LogicalSize::new(area.largura.max(1.0), area.altura.max(1.0)));
                    let _ = webview.show();
                }
                _ => {
                    let _ = webview.hide();
                }
            }
        }
    }
}

/// O host, sem `www.`, para caber na lingueta.
fn titulo_de(url: &Url) -> String {
    url.host_str()
        .map(|host| host.trim_start_matches("www.").to_owned())
        .unwrap_or_else(|| url.to_string())
}

fn erro(mensagem: &str) -> SystemError {
    SystemError::Com(mensagem.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_titulo_e_o_host_sem_www() {
        let titulo = |bruto: &str| titulo_de(&Url::parse(bruto).expect("url"));

        assert_eq!(titulo("https://www.youtube.com/watch?v=abc"), "youtube.com");
        assert_eq!(titulo("https://pt.wikipedia.org/wiki/Brasil"), "pt.wikipedia.org");
        // Sem host não é caso comum, mas cair num `unwrap` seria pior que uma lingueta feia.
        assert_eq!(titulo("data:text/html,oi"), "data:text/html,oi");
    }
}
