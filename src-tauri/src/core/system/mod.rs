//! Controle do PC: abrir sites e programas, volume e mídia.
//!
//! Separado de [`crate::core::automation`] de propósito — aquele módulo é sobre
//! PERCEBER o ambiente (webcam, tela) e reserva o input sintético com trava de
//! segurança para a v0.4. Aqui é ação sobre o sistema operacional: nunca depende de
//! qual janela está em foco, nunca precisa de confirmação, e a fronteira de confiança
//! é a *string* que chega, não o gesto.
//!
//! Quem chama isto é o intérprete de intenção. Assuma que TODA `&str` que entra veio
//! de um modelo de linguagem interpretando fala do usuário — por isso [`target`]
//! existe, e é a única parte do módulo com teste.

mod audio;
mod janela;
mod target;

pub use audio::{nudge_volume, press, set_volume, toggle_mute, MediaKey};
pub use janela::{esta_parado, titulo_do_spotify};

#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("não vou abrir \"{0}\" — só aceito endereços http e https")]
    UrlInvalida(String),
    #[error(
        "\"{0}\" não é um nome de programa válido — diga só o nome, como \"spotify\" ou \"notepad\""
    )]
    ProgramaInvalido(String),
    #[error("não entendi o que você quer pesquisar")]
    BuscaVazia,
    #[error(
        "não encontrei \"{0}\" — confira o nome, ou abra o programa uma vez pelo menu Iniciar para o Windows registrar onde ele mora"
    )]
    NaoEncontrado(String),
    #[error("o Windows negou a abertura de \"{0}\" — pode ser o UAC ou uma política do sistema")]
    AcessoNegado(String),
    #[error("falha ao abrir \"{alvo}\": {detalhe}")]
    Shell { alvo: String, detalhe: String },
    #[error("nenhum dispositivo de saída de áudio — conecte um fone ou uma caixa de som")]
    SemSaidaDeAudio,
    #[error(
        "o Windows bloqueou a tecla de mídia — normalmente é uma janela aberta como administrador em primeiro plano"
    )]
    TeclaBloqueada,
    #[error("falha ao falar com o mixer do Windows: {0}")]
    Com(String),
}

/// Abre um endereço no navegador padrão. Aceita `youtube.com` e `https://youtube.com`.
pub fn open_url(raw: &str) -> Result<(), SystemError> {
    shell_open(target::site(raw)?.as_str())
}

/// Serviços que são SITE, não programa — e que o roteador insiste em mandar como
/// programa. Medido: com a regra explícita no prompt, `abre o youtube` passou a
/// acertar, mas `abre o gmail` e `abre a netflix` continuaram vindo como `open_app`.
///
/// Isto é consultado só DEPOIS de o Windows não achar o programa, então quem tem o
/// app da Netflix instalado continua abrindo o app. É fallback, não desvio.
///
/// ponytail: lista curta e à mão. A cauda longa já tem solução melhor — o usuário
/// ensina ("netflix é o site netflix.com") e o apelido entra no prompt do roteador.
const SERVICOS_WEB: [(&str, &str); 10] = [
    ("gmail", "https://mail.google.com"),
    ("youtube", "https://www.youtube.com"),
    ("netflix", "https://www.netflix.com"),
    ("instagram", "https://www.instagram.com"),
    ("chatgpt", "https://chatgpt.com"),
    ("github", "https://github.com"),
    ("linkedin", "https://www.linkedin.com"),
    ("drive", "https://drive.google.com"),
    ("maps", "https://maps.google.com"),
    ("outlook", "https://outlook.live.com"),
];

/// Abre um programa pelo nome, deixando o Windows resolver onde ele mora — PATH e a
/// chave `App Paths` do registro, que é o mesmo caminho do Win+R.
pub fn open_app(raw: &str) -> Result<(), SystemError> {
    let nome = target::app(raw)?;

    match shell_open(&nome) {
        // Só o "não encontrei" cai para o site. Acesso negado e erro de DLL são
        // problemas do programa que EXISTE, e mascará-los com uma aba de navegador
        // esconderia a causa real.
        Err(SystemError::NaoEncontrado(_)) => match site_conhecido(&nome) {
            Some(url) => shell_open(url),
            None => Err(SystemError::NaoEncontrado(nome)),
        },
        outro => outro,
    }
}

fn site_conhecido(nome: &str) -> Option<&'static str> {
    let nome = nome.trim().trim_end_matches(".exe");
    SERVICOS_WEB
        .iter()
        .find(|(servico, _)| servico.eq_ignore_ascii_case(nome))
        .map(|(_, url)| *url)
}

/// Pesquisa no Google abrindo o navegador padrão.
pub fn search_web(query: &str) -> Result<(), SystemError> {
    shell_open(target::search_url(query)?.as_str())
}

/// Abre um `spotify:` — a faixa exata, ou a busca dentro do app.
///
/// O esquema `spotify:` é registrado pelo próprio app (inclusive na versão da
/// Microsoft Store, que declara o protocolo no manifesto do pacote), então o
/// `ShellExecuteW` resolve e sobe o Spotify se ele estiver fechado.
///
/// Fronteira de confiança: o termo de busca veio de um modelo interpretando fala. A
/// allowlist de prefixo é o que impede "toque X" de virar `spotify:` com qualquer
/// coisa pendurada atrás.
pub fn abrir_no_spotify(uri: &str) -> Result<(), SystemError> {
    let e_faixa = uri
        .strip_prefix("spotify:track:")
        // ID do Spotify é base62 de 22 caracteres. Qualquer outra coisa não é ID.
        .is_some_and(|id| id.len() == 22 && id.chars().all(|c| c.is_ascii_alphanumeric()));

    let e_busca = uri
        .strip_prefix("spotify:search:")
        .is_some_and(|termo| !termo.trim().is_empty() && termo.len() <= 200);

    // Controle inclui o NUL, e NUL trunca o `PCWSTR`: a mensagem mostraria uma coisa
    // e o Windows abriria outra.
    if (!e_faixa && !e_busca) || uri.chars().any(char::is_control) {
        return Err(SystemError::UrlInvalida(uri.chars().take(80).collect()));
    }

    shell_open(uri)
}

#[cfg(windows)]
fn shell_open(alvo: &str) -> Result<(), SystemError> {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    // `ShellExecuteW` e NUNCA `cmd /C start`: no Windows o `Command` reserializa os
    // argumentos numa linha só e o `cmd.exe` reinterpreta `&`, `|`, `^` e `%VAR%`.
    // O texto aqui veio de um modelo transcrevendo fala — "gatos & cachorros" já
    // bastaria para virar dois comandos. Este recebe o alvo como parâmetro próprio.
    //
    // `lpParameters` é nulo de propósito: o Jarvis abre coisas, não passa argumento
    // para elas. Isso elimina a classe inteira de injeção de argumento.
    let alvo_w = HSTRING::from(alvo);

    // SAFETY: `alvo_w` vive até o fim da função e é terminado em NUL pelo `HSTRING`;
    // os demais ponteiros são nulos, o que a API aceita.
    let resultado = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &alvo_w,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // Contrato da API: o retorno é um HINSTANCE falso, e sucesso é MAIOR que 32.
    // Abaixo disso o número é o código do erro.
    let codigo = resultado.0 as isize;
    if codigo > 32 {
        return Ok(());
    }

    let alvo = alvo.to_owned();
    Err(match codigo {
        // 2 e 3 são arquivo/caminho não encontrado. O 31 (SE_ERR_NOASSOC) entra
        // junto porque é o retorno mais comum quando o nome não está no App Paths —
        // sem isso o usuário recebe "código 31" no lugar de "não encontrei".
        2 | 3 | 31 => SystemError::NaoEncontrado(alvo),
        5 => SystemError::AcessoNegado(alvo),
        11 => SystemError::Shell {
            alvo,
            detalhe: "o arquivo não é um executável válido".into(),
        },
        26 => SystemError::Shell {
            alvo,
            detalhe: "o arquivo está em uso por outro programa".into(),
        },
        32 => SystemError::Shell {
            alvo,
            detalhe: "faltou uma DLL do programa".into(),
        },
        outro => SystemError::Shell {
            alvo,
            detalhe: format!("código {outro} do Windows"),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O roteador manda `gmail` e `netflix` como programa por mais que o prompt peça
    /// o contrário. Errar aqui é o usuário ouvir "não encontrei o programa gmail".
    #[test]
    fn conhece_os_servicos_que_sao_site() {
        assert_eq!(site_conhecido("gmail"), Some("https://mail.google.com"));
        assert_eq!(site_conhecido("NETFLIX"), Some("https://www.netflix.com"));
        // O roteador às vezes acrescenta a extensão.
        assert_eq!(
            site_conhecido("youtube.exe"),
            Some("https://www.youtube.com")
        );

        // Programa de verdade não pode ser sequestrado para o navegador.
        assert_eq!(site_conhecido("spotify"), None);
        assert_eq!(site_conhecido("notepad"), None);
    }
}

#[cfg(not(windows))]
fn shell_open(alvo: &str) -> Result<(), SystemError> {
    Err(SystemError::Shell {
        alvo: alvo.to_owned(),
        detalhe: "abrir programas só existe no Windows".into(),
    })
}
