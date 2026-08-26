//! Ler o título da janela do Spotify — o jeito de saber se ele está tocando.
//!
//! Existe porque abrir `spotify:track:<id>` **navega até a faixa mas não dá play** —
//! medido. A correção é apertar a tecla de mídia depois, e a tecla é um TOGGLE: mandar
//! ela quando já está tocando PAUSA a música, que é o oposto do que o usuário pediu.
//!
//! Daí a leitura do título, que é o sinal barato e confiável:
//!
//! | título da janela      | estado    |
//! | --------------------- | --------- |
//! | `Spotify Premium`     | parado    |
//! | `Spotify Free`        | parado    |
//! | `Spotify`             | parado    |
//! | `Charlie Brown Jr. — Só os Loucos Sabem` | tocando |
//!
//! O casamento é por PROCESSO, não só por título: uma aba do navegador chamada
//! "Spotify — Web Player" enganaria a busca por texto, e aí o Jarvis pausaria o
//! desktop achando que estava parado.

/// Nomes que a janela assume quando NÃO há nada tocando. Tocando, o título vira
/// "Artista — Música", que não casa com nenhum deles.
const PARADO: [&str; 3] = ["spotify", "spotify premium", "spotify free"];

pub fn esta_parado(titulo: &str) -> bool {
    let titulo = titulo.trim().to_lowercase();
    PARADO.iter().any(|nome| titulo == *nome)
}

#[cfg(windows)]
mod imp {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, MAX_PATH};

    /// Continua enumerando. (`TRUE`/`FALSE` não existem como constantes no crate.)
    const CONTINUA: BOOL = BOOL(1);
    /// Para a enumeração.
    const PARA: BOOL = BOOL(0);
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    /// Título da janela principal do Spotify, ou `None` se ele não estiver aberto.
    pub fn titulo_do_spotify() -> Option<String> {
        let mut achado: Option<String> = None;

        // SAFETY: `EnumWindows` chama `visitar` sinceramente com o ponteiro que
        // passamos, e ele vive até o fim desta função. O `_ =` é porque a enumeração
        // devolve erro quando o callback pede para parar, que é o caminho de sucesso.
        let _ = unsafe {
            EnumWindows(
                Some(visitar),
                LPARAM(std::ptr::addr_of_mut!(achado) as isize),
            )
        };

        achado
    }

    unsafe extern "system" fn visitar(janela: HWND, saida: LPARAM) -> BOOL {
        // SAFETY: o ponteiro veio de `titulo_do_spotify`, que ainda está na pilha.
        let destino = unsafe { &mut *(saida.0 as *mut Option<String>) };

        // Janela invisível é o monte de janelas-mensagem que todo app de Electron
        // cria; nenhuma delas tem título útil.
        if !unsafe { IsWindowVisible(janela) }.as_bool() {
            return CONTINUA;
        }
        if !e_do_spotify(janela) {
            return CONTINUA;
        }

        let comprimento = unsafe { GetWindowTextLengthW(janela) };
        if comprimento <= 0 {
            return CONTINUA;
        }

        let mut buffer = vec![0_u16; comprimento as usize + 1];
        let escritos = unsafe { GetWindowTextW(janela, &mut buffer) };
        if escritos <= 0 {
            return CONTINUA;
        }

        *destino = Some(String::from_utf16_lossy(&buffer[..escritos as usize]));
        // Para a enumeração: a primeira janela visível com título já é a principal.
        PARA
    }

    fn e_do_spotify(janela: HWND) -> bool {
        let mut pid = 0_u32;
        // SAFETY: `pid` é um `u32` vivo; a API só escreve nele.
        unsafe { GetWindowThreadProcessId(janela, Some(&mut pid)) };
        if pid == 0 {
            return false;
        }

        // SAFETY: handle fechado pelo `Drop` do `OwnedHandle` que o wrapper devolve.
        let Ok(processo) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
        else {
            return false;
        };

        let mut buffer = [0_u16; MAX_PATH as usize];
        let mut tamanho = buffer.len() as u32;

        // SAFETY: `buffer` e `tamanho` casam, e o handle é válido aqui.
        let ok = unsafe {
            QueryFullProcessImageNameW(
                processo,
                PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(buffer.as_mut_ptr()),
                &mut tamanho,
            )
        }
        .is_ok();

        if !ok {
            return false;
        }

        String::from_utf16_lossy(&buffer[..tamanho as usize])
            .rsplit('\\')
            .next()
            .is_some_and(|nome| nome.eq_ignore_ascii_case("Spotify.exe"))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn titulo_do_spotify() -> Option<String> {
        None
    }
}

pub use imp::titulo_do_spotify;

#[cfg(test)]
mod tests {
    use super::*;

    /// Errar isto é pausar a música do usuário achando que ia tocá-la.
    #[test]
    fn reconhece_a_janela_parada() {
        for parado in [
            "Spotify",
            "Spotify Premium",
            "Spotify Free",
            "  spotify premium  ",
        ] {
            assert!(esta_parado(parado), "devia ser parado: {parado:?}");
        }

        for tocando in [
            "Charlie Brown Jr. — Só os Loucos Sabem",
            "Queen - Bohemian Rhapsody",
            // Título que CONTÉM "Spotify" mas não é o estado parado.
            "Spotify — Web Player: Music for everyone",
        ] {
            assert!(!esta_parado(tocando), "devia ser tocando: {tocando:?}");
        }
    }

    /// Precisa do Spotify aberto, então fica fora do `cargo test` normal:
    ///
    /// ```text
    /// cargo test le_o_titulo_do_spotify -- --ignored --nocapture
    /// ```
    ///
    /// É o único jeito de provar o `EnumWindows` + casamento por processo. Se isto
    /// devolver `None` com o Spotify aberto, a tecla de play nunca seria enviada e
    /// "toque tal música" abriria a faixa parada — em silêncio.
    #[test]
    #[ignore = "precisa do Spotify aberto"]
    fn le_o_titulo_do_spotify() {
        let titulo = titulo_do_spotify().expect("o Spotify precisa estar aberto para este teste");

        println!("título: {titulo:?}  ->  parado = {}", esta_parado(&titulo));
        assert!(!titulo.trim().is_empty());
    }
}
