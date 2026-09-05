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
//! ## A regra da thread, que custou um travamento inteiro
//!
//! Um `#[tauri::command]` **sem `async` roda na thread principal**, dentro do callback de
//! mensagem do WebView2 que trouxe a chamada. Criar um webview dali trava o app: o wry
//! espera o controlador do WebView2 ficar pronto com `webview2_com::wait_with_pump`, que é
//! um `GetMessage`/`DispatchMessage` — ou seja, **um message loop aninhado dentro de um
//! handler do WebView2**. O app congela e não volta.
//!
//! Daí as três regras que este arquivo e o `commands/navegador.rs` seguem:
//!
//! 1. Todo comando que **cria ou destrói** um webview é `#[tauri::command(async)]`, para
//!    sair da thread principal. Aí o `send_user_message` do Tauri posta um evento no laço
//!    em vez de executar em linha, e o webview nasce no laço normal, sem aninhamento.
//! 2. **Nenhum `Mutex` daqui é segurado durante uma chamada ao Tauri.** `set_position` e
//!    companhia atravessam para a thread principal e ESPERAM por ela; segurar um cadeado
//!    nessa espera trava o app se a thread principal estiver, no mesmo instante, pedindo o
//!    mesmo cadeado dentro de um `browser_bounds` — que a tela dispara a cada quadro de
//!    arrasto. É por isso que `redesenhar` monta um plano antes de mexer em qualquer janela.
//! 3. **Os callbacks do webview não criam nada** — eles avisam a tela, e ela pede. É a
//!    mesma razão da regra 1: um `add_child` de dentro do `on_new_window` seria o
//!    aninhamento de novo, agora por outra porta.
//!
//! Este módulo mora fora de `core/` de propósito: ele **é** Tauri da primeira à última
//! linha, e a regra de `core/` é justamente não conhecê-lo.

use std::sync::Mutex;

use serde::Serialize;
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl};
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

/// A página nasce reduzida.
///
/// A janelinha é bem menor que uma janela de navegador, e um site desenhado para 1280 px
/// dentro de 600 px vira barra de rolagem horizontal e menu sanfonado.
///
/// **O número sai de uma conta, não de gosto**, e a conta desmente o valor anterior: o
/// zoom multiplica a largura CSS que a página enxerga, então num painel de 600 px os 80%
/// entregavam 750 px — ainda longe dos 1280 e ainda no menu sanfonado que este comentário
/// reclamava. A 70% são 857 px, que caem na faixa de tablet, onde a maioria dos layouts
/// responsivos ainda serve o conteúdo em coluna única em vez de cair na versão de celular.
///
/// Não desce mais que isso porque o piso é a leitura: a 70% um corpo de texto de 16 px sai
/// com 11 px, que ainda se lê de relance. Os 1200 px que o parágrafo acima sugere sairiam
/// a 50%, com o texto em 8 px — a página caberia inteira e não daria para ler nada.
const ZOOM: f64 = 0.7;

/// A aba mudou de endereço sozinha — clique num link, redirecionamento, rota de SPA.
///
/// Precisa ser evento: a navegação acontece DENTRO da página, sem passar por comando
/// nenhum, então a barra de endereço não tem como saber sem alguém contar.
const EVENTO_URL: &str = "jarvis://browser-url";

/// A página pediu uma janela nova (clique do meio, `target="_blank"`, `window.open`).
///
/// Vai como pedido à tela em vez de virar aba aqui mesmo, e não é preciosismo: abrir o
/// webview de dentro deste callback é o message loop aninhado da regra 1 lá em cima.
const EVENTO_ABA_NOVA: &str = "jarvis://browser-new-tab";

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

        let avisa_url = {
            let app = app.clone();
            let id = id.clone();

            move |destino: &Url| {
                // Guarda no estado E avisa a tela. O estado é o que `browser_state`
                // devolve; o evento é o que move a barra de endereço na hora.
                let navegador = app.state::<Navegador>();
                let destino = destino.to_string();

                if let Some(aba) = lock(&navegador.abas).iter_mut().find(|aba| aba.id == id) {
                    aba.titulo = Url::parse(&destino).as_ref().map_or_else(
                        |_| destino.clone(),
                        titulo_de,
                    );
                    aba.url = destino.clone();
                }

                let _ = app.emit(EVENTO_URL, MudouDeEndereco { id: &id, url: &destino });
                true
            }
        };

        let pede_aba_nova = {
            let app = app.clone();

            move |destino: Url, _: tauri::webview::NewWindowFeatures| {
                // NUNCA `add_child` aqui: este callback roda na thread principal, dentro
                // do WebView2, e criar webview daqui é o travamento da regra 1. A tela
                // recebe o pedido e chama o comando `async`, que roda fora dela.
                let _ = app.emit(EVENTO_ABA_NOVA, destino.to_string());
                tauri::webview::NewWindowResponse::Deny
            }
        };

        let webview = janela
            .add_child(
                WebviewBuilder::new(&id, WebviewUrl::External(url.clone()))
                    .on_navigation(avisa_url)
                    // Sem isto, um link com `target="_blank"` abriria uma JANELA nova do
                    // Tauri, sem barra de abas e sem como fechar. Aqui ela vira aba.
                    .on_new_window(pede_aba_nova),
                LONGE,
                LogicalSize::new(area.largura.max(1.0), area.altura.max(1.0)),
            )
            .map_err(|erro| self::erro(&format!("não consegui abrir a aba: {erro}")))?;

        // Erro ignorado: zoom é conforto, e uma aba em 100% é melhor que nenhuma aba.
        let _ = webview.set_zoom(ZOOM);

        let aba = Aba {
            titulo: titulo_de(url),
            url: url.to_string(),
            id: id.clone(),
        };

        lock(&self.abas).push(aba.clone());
        self.ativar(app, &id)?;

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
        // O plano sai PRIMEIRO, e os cadeados caem antes da primeira chamada ao Tauri —
        // ver a regra 2 no topo do arquivo. `None` na área quer dizer "esconde esta".
        let plano: Vec<(String, Option<Area>)> = {
            let area = *lock(&self.area);
            let ativa = lock(&self.ativa).clone();

            lock(&self.abas)
                .iter()
                .map(|aba| {
                    let na_frente = ativa.as_deref() == Some(aba.id.as_str());
                    (aba.id.clone(), area.filter(|_| na_frente))
                })
                .collect()
        };

        for (id, area) in plano {
            let Some(webview) = app.get_webview(&id) else {
                continue;
            };

            match area {
                Some(area) => {
                    let _ = webview.set_position(LogicalPosition::new(area.x, area.y));
                    let _ = webview
                        .set_size(LogicalSize::new(area.largura.max(1.0), area.altura.max(1.0)));
                    let _ = webview.show();
                }
                None => {
                    let _ = webview.hide();
                }
            }
        }
    }
}

/// Carga do [`EVENTO_URL`]. Espelhado em `src/lib/tauri/events.ts`.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MudouDeEndereco<'a> {
    id: &'a str,
    url: &'a str,
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
