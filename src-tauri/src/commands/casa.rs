use serde::Serialize;
use tauri::State;

use crate::core::casa::chaveiro::Chaveiro;
use crate::core::casa::controle::{self, Ajuste, Detalhe, Estado};
use crate::core::casa::nuvem::{Controle, Tecla};
use crate::core::casa::{conhecidos, endereco_de, descobrir_com, nuvem, Aparelho, CasaError, Varredura};
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
    let endereco = endereco_de(&chaveiro, &id, &ip, &versao)
        .ok_or_else(|| controle::ControleError::SemChave.to_string())?;

    controle::ligar(&endereco.alvo(), ligado).map_err(|erro| erro.to_string())
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
    let endereco = endereco_de(&chaveiro, &id, &ip, &versao)
        .ok_or_else(|| controle::ControleError::SemChave.to_string())?;

    controle::detalhar(&endereco.alvo()).map_err(|erro| erro.to_string())
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
    let endereco = endereco_de(&chaveiro, &id, &ip, &versao)
        .ok_or_else(|| controle::ControleError::SemChave.to_string())?;

    controle::ajustar(&endereco.alvo(), &ajuste).map_err(|erro| erro.to_string())
}

/// Tira um aparelho da lista principal, ou devolve para ela.
///
/// Só a tela muda: ele continua sendo varrido, continua com a chave guardada e continua
/// obedecendo por voz. É sobre não disputar espaço com o que você usa todo dia.
#[tauri::command]
pub fn set_device_hidden(
    id: String,
    oculto: bool,
    chaveiro: State<'_, Chaveiro>,
) -> Result<(), String> {
    chaveiro.ocultar(&id, oculto).map_err(|erro| erro.to_string())
}

/// As teclas de um controle de infravermelho — as da TV, as do ar-condicionado.
///
/// Vem da nuvem porque é lá que elas moram: o emissor guarda zero códigos, ele só emite o
/// que mandarem. É a mesma razão de a TV não aparecer na varredura da rede.
#[tauri::command]
pub async fn ir_keys(
    emissor: String,
    remoto: String,
    state: State<'_, AppState>,
) -> Result<Controle, String> {
    let settings = state.settings();
    let (client_id, client_secret) = settings
        .tuya()
        .ok_or_else(|| nuvem::NuvemError::SemCredencial.to_string())?;

    nuvem::teclas(
        &state.http(),
        client_id,
        client_secret,
        &settings.tuya_regiao,
        &emissor,
        &remoto,
    )
    .await
    .map_err(|erro| erro.to_string())
}

/// Aperta uma tecla do controle.
///
/// **É o único comando do app que precisa de internet.** O código infravermelho de
/// "ligar a TV" mora na biblioteca da Tuya, não no emissor — ele não tem o que mandar até
/// alguém contar qual é o código.
#[tauri::command]
pub async fn send_ir_key(
    emissor: String,
    remoto: String,
    categoria: i64,
    tecla: Tecla,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings = state.settings();
    let (client_id, client_secret) = settings
        .tuya()
        .ok_or_else(|| nuvem::NuvemError::SemCredencial.to_string())?;

    nuvem::apertar(
        &state.http(),
        client_id,
        client_secret,
        &settings.tuya_regiao,
        &emissor,
        &remoto,
        categoria,
        &tecla,
    )
    .await
    .map_err(|erro| erro.to_string())
}

/// Liga ou desliga UMA das chaves do aparelho.
///
/// Existe porque um aparelho pode ter várias: a tomada dupla desta casa responde `1` e
/// `2`, e o botão do cartão só alcança a primeira. Aqui a tela diz qual.
#[tauri::command(async)]
pub fn set_device_dp(
    id: String,
    ip: String,
    versao: String,
    dp: String,
    ligado: bool,
    chaveiro: State<'_, Chaveiro>,
) -> Result<Detalhe, String> {
    let endereco = endereco_de(&chaveiro, &id, &ip, &versao)
        .ok_or_else(|| controle::ControleError::SemChave.to_string())?;

    controle::enviar_dps(
        &endereco.alvo(),
        [(dp, serde_json::Value::Bool(ligado))].into_iter().collect(),
    )
    .map_err(|erro| erro.to_string())
}
