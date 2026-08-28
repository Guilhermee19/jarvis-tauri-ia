//! Quanto do computador está sendo usado agora: processador, memória e placa de vídeo.
//!
//! ## Por que PDH, e não uma crate de métricas
//!
//! O `sysinfo` resolveria processador e memória em três linhas, e **não sabe ler GPU** —
//! nenhuma crate popular sabe, de forma independente de fabricante. O caminho que o
//! próprio Gerenciador de Tarefas usa são os contadores de desempenho do Windows (PDH),
//! e eles cobrem os três. Uma dependência a menos e uma resposta a mais.
//!
//! O PDH também é o único caminho de GPU que funciona igual em NVIDIA, AMD e Intel: a
//! alternativa seria a NVML, que é da NVIDIA e não existe nas outras.
//!
//! ## O contador precisa de duas leituras
//!
//! Uso de processador é uma TAXA: ela só existe entre dois instantes. A primeira coleta
//! depois de abrir a consulta devolve zero, e é por isso que o [`Monitor`] fica vivo
//! entre as chamadas em vez de abrir e fechar a cada pergunta — abrindo e fechando, toda
//! resposta seria zero, o que parece "o computador está ocioso" em vez de "não medi".

use super::SystemError;

/// Um retrato do computador neste instante.
///
/// Bytes crus e não texto formatado: quem desenha a barra precisa da proporção, e quem
/// escreve "3,2 GB" precisa decidir a unidade junto com o espaço que tem na tela.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metricas {
    /// 0 a 100.
    pub cpu: f64,
    pub memoria_usada: u64,
    pub memoria_total: u64,
    /// 0 a 100. **É a soma dos motores da placa** (3D, cópia, codificação de vídeo…),
    /// limitada a 100 — a mesma conta do Gerenciador de Tarefas.
    pub gpu: f64,
    /// Memória dedicada da placa em uso. `0` quando não há contador (vídeo integrado
    /// costuma não expor).
    pub gpu_memoria_usada: u64,
    pub gpu_memoria_total: u64,
    /// O nome da placa, para o cartão não dizer só "GPU".
    pub gpu_nome: String,
}

pub use imp::Monitor;

/// Dono do monitor entre as chamadas.
///
/// Abre na PRIMEIRA pergunta, e não na subida do app: os contadores de desempenho custam
/// alguns milissegundos para abrir e quem nunca abre o painel nunca paga por eles. Depois
/// disso ele fica vivo, porque uso de processador é uma taxa entre duas leituras — um
/// monitor recriado a cada chamada responderia zero para sempre.
pub struct DesempenhoState {
    monitor: std::sync::Mutex<Option<Monitor>>,
}

impl DesempenhoState {
    pub fn new() -> Self {
        Self {
            monitor: std::sync::Mutex::new(None),
        }
    }

    pub fn amostrar(&self) -> Result<Metricas, SystemError> {
        let mut slot = crate::core::lock(&self.monitor);

        let monitor = match slot.as_ref() {
            Some(monitor) => monitor,
            None => slot.insert(Monitor::abrir()?),
        };

        monitor.amostrar()
    }
}

#[cfg(windows)]
mod imp {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_SOFTWARE,
    };
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhGetFormattedCounterValue, PdhOpenQueryW, PDH_FMT_COUNTERVALUE,
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    };
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    use super::{Metricas, SystemError};

    /// O que o Gerenciador de Tarefas chama de "Utilização" — ele conta a frequência
    /// real, então um processador em turbo passa de 100% no contador antigo
    /// (`% Processor Time`), que fica como reserva para Windows mais velho.
    const CPU: &str = r"\Processor Information(_Total)\% Processor Utility";
    const CPU_ANTIGO: &str = r"\Processor(_Total)\% Processor Time";
    /// Curinga: cada processo com trabalho na placa vira uma instância, e o uso total é
    /// a soma delas.
    const GPU: &str = r"\GPU Engine(*)\Utilization Percentage";
    const GPU_MEMORIA: &str = r"\GPU Adapter Memory(*)\Dedicated Usage";

    /// A consulta aberta e os contadores dentro dela.
    ///
    /// `Send` é seguro aqui: os identificadores do PDH são opacos e a documentação
    /// permite usá-los de outra thread, desde que não em duas ao mesmo tempo — o que o
    /// `Mutex` de quem guarda o monitor garante.
    pub struct Monitor {
        consulta: PDH_HQUERY,
        cpu: PDH_HCOUNTER,
        gpu: Option<PDH_HCOUNTER>,
        gpu_memoria: Option<PDH_HCOUNTER>,
        gpu_nome: String,
        gpu_memoria_total: u64,
    }

    // SAFETY: identificadores opacos do PDH, usados sob `Mutex`.
    unsafe impl Send for Monitor {}

    impl Monitor {
        pub fn abrir() -> Result<Self, SystemError> {
            let mut consulta = PDH_HQUERY::default();
            // SAFETY: a consulta é fechada no `Drop`.
            let status = unsafe { PdhOpenQueryW(None, 0, &mut consulta) };
            if status != ERROR_SUCCESS.0 {
                return Err(SystemError::Com(format!(
                    "não consegui abrir os contadores de desempenho do Windows (0x{status:08X})"
                )));
            }

            let mut monitor = Self {
                consulta,
                cpu: PDH_HCOUNTER::default(),
                gpu: None,
                gpu_memoria: None,
                gpu_nome: String::new(),
                gpu_memoria_total: 0,
            };

            // O de processador é obrigatório: sem ele o cartão não tem o que mostrar.
            monitor.cpu = monitor
                .adicionar(CPU)
                .or_else(|| monitor.adicionar(CPU_ANTIGO))
                .ok_or_else(|| {
                    SystemError::Com("o contador de uso de processador não existe aqui".into())
                })?;

            // Os de vídeo são opcionais: máquina virtual e driver antigo não os expõem, e
            // isso não pode impedir de ver processador e memória.
            monitor.gpu = monitor.adicionar(GPU);
            monitor.gpu_memoria = monitor.adicionar(GPU_MEMORIA);
            (monitor.gpu_nome, monitor.gpu_memoria_total) = placa();

            // A primeira coleta é a linha de base das taxas. Sem ela, a leitura seguinte
            // não teria de quando medir e devolveria zero.
            // SAFETY: consulta recém-aberta.
            unsafe { PdhCollectQueryData(monitor.consulta) };

            Ok(monitor)
        }

        /// `PdhAddEnglishCounterW` e não `PdhAddCounterW`: os nomes de contador são
        /// TRADUZIDOS, e num Windows em português `\Processor Information` não existe —
        /// existe `\Informações do Processador`. A versão inglesa aceita o nome canônico
        /// em qualquer idioma, e sem ela isto funcionaria só em máquinas em inglês.
        fn adicionar(&self, caminho: &str) -> Option<PDH_HCOUNTER> {
            let mut contador = PDH_HCOUNTER::default();
            // SAFETY: `caminho` vive até o fim da chamada; o contador morre com a consulta.
            let status = unsafe {
                PdhAddEnglishCounterW(self.consulta, &HSTRING::from(caminho), 0, &mut contador)
            };

            (status == ERROR_SUCCESS.0).then_some(contador)
        }

        /// Lê os contadores. Vale a distância desde a chamada anterior.
        pub fn amostrar(&self) -> Result<Metricas, SystemError> {
            // SAFETY: consulta viva; falha aqui vira leitura zerada adiante.
            let status = unsafe { PdhCollectQueryData(self.consulta) };
            if status != ERROR_SUCCESS.0 {
                return Err(SystemError::Com(format!(
                    "os contadores de desempenho pararam de responder (0x{status:08X})"
                )));
            }

            let (memoria_usada, memoria_total) = memoria();

            Ok(Metricas {
                cpu: valor(self.cpu).unwrap_or(0.0).clamp(0.0, 100.0),
                memoria_usada,
                memoria_total,
                // Somar e limitar a 100: cada motor da placa (3D, cópia, vídeo) é uma
                // instância, e vários trabalhando ao mesmo tempo passariam de 100.
                gpu: self
                    .gpu
                    .and_then(soma_do_curinga)
                    .unwrap_or(0.0)
                    .clamp(0.0, 100.0),
                gpu_memoria_usada: self
                    .gpu_memoria
                    .and_then(soma_do_curinga)
                    .unwrap_or(0.0)
                    .max(0.0) as u64,
                gpu_memoria_total: self.gpu_memoria_total,
                gpu_nome: self.gpu_nome.clone(),
            })
        }
    }

    impl Drop for Monitor {
        fn drop(&mut self) {
            // SAFETY: fecha a consulta e, com ela, todos os contadores.
            unsafe { PdhCloseQuery(self.consulta) };
        }
    }

    fn valor(contador: PDH_HCOUNTER) -> Option<f64> {
        let mut lido = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: `lido` é do tipo que o formato pede.
        let status =
            unsafe { PdhGetFormattedCounterValue(contador, PDH_FMT_DOUBLE, None, &mut lido) };

        if status != ERROR_SUCCESS.0 {
            return None;
        }

        // SAFETY: o formato pedido foi `DOUBLE`, então é este o campo válido da união.
        Some(unsafe { lido.Anonymous.doubleValue })
    }

    /// Soma todas as instâncias de um contador com curinga.
    ///
    /// Duas chamadas de propósito: a primeira só descobre o tamanho do buffer, porque o
    /// número de instâncias muda a cada leitura — cada processo que encosta na placa
    /// aparece e some.
    fn soma_do_curinga(contador: PDH_HCOUNTER) -> Option<f64> {
        let mut tamanho = 0u32;
        let mut quantos = 0u32;

        // SAFETY: buffer nulo com tamanho zero é a forma documentada de perguntar o
        // tamanho necessário.
        unsafe {
            PdhGetFormattedCounterArrayW(
                contador,
                PDH_FMT_DOUBLE,
                &mut tamanho,
                &mut quantos,
                None,
            )
        };

        if tamanho == 0 {
            return None;
        }

        let mut buffer = vec![0u8; tamanho as usize];
        // SAFETY: buffer do tamanho que a própria chamada anterior pediu.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                contador,
                PDH_FMT_DOUBLE,
                &mut tamanho,
                &mut quantos,
                Some(buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>()),
            )
        };
        if status != ERROR_SUCCESS.0 {
            return None;
        }

        // SAFETY: o PDH escreveu `quantos` itens no começo do buffer.
        let itens = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
                quantos as usize,
            )
        };

        Some(
            itens
                .iter()
                // SAFETY: o formato pedido foi `DOUBLE`.
                .map(|item| unsafe { item.FmtValue.Anonymous.doubleValue })
                .filter(|valor| valor.is_finite())
                .sum(),
        )
    }

    fn memoria() -> (u64, u64) {
        let mut estado = MEMORYSTATUSEX {
            dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
            ..Default::default()
        };

        // SAFETY: `dwLength` preenchido como a API exige.
        if unsafe { GlobalMemoryStatusEx(&mut estado) }.is_err() {
            return (0, 0);
        }

        (
            estado.ullTotalPhys.saturating_sub(estado.ullAvailPhys),
            estado.ullTotalPhys,
        )
    }

    /// Nome e memória dedicada da placa, pelo DXGI.
    ///
    /// O contador de memória diz quanto está EM USO e não quanto existe — sem o total, a
    /// barra não tem escala. O DXGI dá os dois de graça, e o nome junto.
    ///
    /// Adaptadores de software (o "Microsoft Basic Render Driver") ficam de fora: eles
    /// aparecem em toda máquina e reportariam uma placa que não existe.
    fn placa() -> (String, u64) {
        // SAFETY: fábrica DXGI, liberada pelo COM ao sair de escopo.
        let Ok(fabrica) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
            return (String::new(), 0);
        };

        for indice in 0.. {
            // SAFETY: itera até a própria API dizer que acabou.
            let Ok(adaptador) = (unsafe { fabrica.EnumAdapters1(indice) }) else {
                break;
            };

            // SAFETY: descreve o adaptador que a iteração acabou de entregar.
            let Ok(descricao) = (unsafe { adaptador.GetDesc1() }) else {
                continue;
            };

            if DXGI_ADAPTER_FLAG(descricao.Flags as i32) == DXGI_ADAPTER_FLAG_SOFTWARE {
                continue;
            }

            let nome = String::from_utf16_lossy(&descricao.Description)
                .trim_end_matches('\0')
                .to_owned();

            return (nome, descricao.DedicatedVideoMemory as u64);
        }

        (String::new(), 0)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Metricas, SystemError};

    pub struct Monitor;

    impl Monitor {
        pub fn abrir() -> Result<Self, SystemError> {
            Err(SystemError::Com(
                "as métricas de desempenho usam os contadores do Windows".into(),
            ))
        }

        pub fn amostrar(&self) -> Result<Metricas, SystemError> {
            Err(SystemError::Com(
                "as métricas de desempenho usam os contadores do Windows".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lê os contadores DESTA máquina e imprime. Fora do `cargo test` comum porque
    /// depende do subsistema de desempenho do Windows e leva um segundo — o intervalo
    /// mínimo para uma taxa existir.
    ///
    /// `cargo test --lib -- --ignored --nocapture le_o_computador_de_verdade`
    #[test]
    #[ignore]
    fn le_o_computador_de_verdade() {
        let estado = DesempenhoState::new();

        // A primeira leitura é a linha de base: ela sai zerada por definição.
        let _ = estado.amostrar();
        std::thread::sleep(std::time::Duration::from_millis(1000));

        match estado.amostrar() {
            Ok(metricas) => println!("{metricas:#?}"),
            Err(erro) => println!("falhou: {erro}"),
        }
    }
}
