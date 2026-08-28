use serde::Serialize;
use tauri::State;

use crate::core::casa::chaveiro::Chaveiro;
use crate::core::casa::controle::{self, Ajuste, Alvo, Detalhe, Estado};
use crate::core::casa::{conhecidos, descobrir_com, nuvem, Aparelho, CasaError, Varredura};
use crate::state::AppState;

/// `(async)` porque **bloqueia por segundos**: não é uma consulta, é uma janela de
/// escuta, e os aparelhos anunciam quando querem.
#[tauri::command(async)]
pub fn discover_devices(chaveiro: State<'_, Chaveiro>) -> Result<Varredura, String> {
    descobrir_com(&chaveiro).map_err(|erro: CasaError| erro.to_string())
}

/// O que já se conhece, sem encostar na rede — responde na hora.
///
/// Existe para o painel ter o que mostrar no instante em que abre, em vez de dez
/// segundos de tela vazia antes da primeira varredura terminar.
#[tauri::command]
pub fn known_devices(chaveiro: State<'_, Chaveiro>) -> Vec<Aparelho> {
    conhecidos(&chaveiro)
}

/// O que a tela fica sabendo de uma importação.
///
/// A `local_key` **não sai daqui**. Ela é o único segredo que o app guarda que vale
/// dentro da sua casa, e mandá-la para a UI aumentaria a superfície dela — num arquivo
/// de log, num devtools aberto — sem nenhum ganho: quem manda comando é o Rust.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Importado {
    pub id: String,
    pub nome: String,
    pub tem_chave: bool,
}

/// Busca nome e `local_key` na nuvem da Tuya e guarda no chaveiro.
///
/// Devolve a lista de quem ganhou nome, para o painel atualizar os cartões na hora —
/// sem isso a única forma de ver o resultado seria pagar outra varredura de 10 s.
///
/// `semente` é o id de um aparelho que a varredura já viu — a Tuya lista os aparelhos de
/// um USUÁRIO, e o usuário se descobre perguntando por um aparelho conhecido. Quem
/// escolhe é a tela, que já tem a lista na mão; aqui só se verifica que veio alguma
/// coisa.
#[tauri::command]
pub async fn import_tuya_devices(
    semente: String,
    state: State<'_, AppState>,
    chaveiro: State<'_, Chaveiro>,
) -> Result<Vec<Importado>, String> {
    let settings = state.settings();
    let (client_id, client_secret) = settings.tuya().ok_or_else(|| {
        // A mesma frase do `NuvemError::SemCredencial`, e não uma paráfrase: a tela
        // mostra o texto do Rust cru, e duas redações da mesma causa confundem.
        nuvem::NuvemError::SemCredencial.to_string()
    })?;

    let aparelhos = nuvem::importar(
        &state.http(),
        client_id,
        client_secret,
        &settings.tuya_regiao,
        &semente,
    )
    .await
    .map_err(|erro| erro.to_string())?;

    let vista = aparelhos
        .iter()
        .map(|aparelho| Importado {
            id: aparelho.id.clone(),
            nome: aparelho.nome.clone(),
            tem_chave: !aparelho.local_key.trim().is_empty(),
        })
        .collect();

    chaveiro.guardar(aparelhos).map_err(|erro| erro.to_string())?;

    Ok(vista)
}

/// Liga ou desliga um aparelho, e devolve o estado como ele confirmou.
///
/// `ip` e `versao` vêm da tela porque a varredura mais recente está lá — o backend não
/// guarda o retrato da rede, de propósito: um IP guardado envelhece calado quando o
/// roteador redistribui os endereços. A chave, essa sim, sai do chaveiro.
///
/// `(async)` porque abre TCP e espera resposta: são milissegundos numa rede boa e o
/// timeout inteiro numa ruim, e nenhum dos dois pode segurar a thread principal.
#[tauri::command(async)]
pub fn set_device_power(
    id: String,
    ip: String,
    versao: String,
    ligado: bool,
    chaveiro: State<'_, Chaveiro>,
) -> Result<Estado, String> {
    let conhecido = chaveiro
        .de(&id)
        .ok_or_else(|| controle::ControleError::SemChave.to_string())?;

    controle::ligar(
        &Alvo {
            id: &id,
            ip: &ip,
            versao: &versao,
            local_key: &conhecido.local_key,
        },
        ligado,
    )
    .map_err(|erro| erro.to_string())
}

/// O alvo de um comando, montado a partir do que a tela tem e do que o chaveiro guarda.
///
/// Existe para os três comandos de controle não repetirem a mesma busca de chave e o
/// mesmo erro — e para o `clippy` não reclamar de função com sete argumentos.
struct Pedido {
    id: String,
    ip: String,
    versao: String,
    chave: String,
}

impl Pedido {
    fn novo(
        id: String,
        ip: String,
        versao: String,
        chaveiro: &Chaveiro,
    ) -> Result<Self, String> {
        let conhecido = chaveiro
            .de(&id)
            .ok_or_else(|| controle::ControleError::SemChave.to_string())?;

        Ok(Self {
            id,
            ip,
            versao,
            chave: conhecido.local_key,
        })
    }

    fn alvo(&self) -> Alvo<'_> {
        Alvo {
            id: &self.id,
            ip: &self.ip,
            versao: &self.versao,
            local_key: &self.chave,
        }
    }
}

/// Tudo o que o aparelho sabe dizer sobre si: estado, capacidades de luz e os data points
/// crus.
///
/// `(async)` porque abre TCP e espera resposta — são milissegundos numa rede boa e o
/// timeout inteiro numa ruim, e nenhum dos dois pode segurar a thread principal.
#[tauri::command(async)]
pub fn device_state(
    id: String,
    ip: String,
    versao: String,
    chaveiro: State<'_, Chaveiro>,
) -> Result<Detalhe, String> {
    let pedido = Pedido::novo(id, ip, versao, &chaveiro)?;

    controle::detalhar(&pedido.alvo()).map_err(|erro| erro.to_string())
}

/// Muda cor, brilho ou temperatura de uma lâmpada.
#[tauri::command(async)]
pub fn set_light(
    id: String,
    ip: String,
    versao: String,
    ajuste: Ajuste,
    chaveiro: State<'_, Chaveiro>,
) -> Result<Detalhe, String> {
    let pedido = Pedido::novo(id, ip, versao, &chaveiro)?;

    controle::ajustar(&pedido.alvo(), &ajuste).map_err(|erro| erro.to_string())
}
