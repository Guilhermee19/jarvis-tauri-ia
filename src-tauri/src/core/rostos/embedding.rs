//! Transformar um rosto num vetor de 128 números, e comparar dois desses.
//!
//! Usa o **SFace**, o reconhecedor do OpenCV Zoo. Ele não sabe nomes: o que ele faz é
//! mapear um rosto para um ponto num espaço de 128 dimensões, de tal forma que dois
//! retratos da MESMA pessoa caiam perto e de pessoas diferentes caiam longe. O nome vem
//! depois, do catálogo — e é por isso que reconhecer alguém é uma conta de distância, não
//! uma classificação.
//!
//! ## Alinhar antes é metade do resultado
//!
//! O modelo espera um recorte de 112×112 com os olhos em posições fixas. Entregar a caixa
//! crua do detector funciona mal: a mesma pessoa com a cabeça inclinada 15° produz um
//! vetor visivelmente diferente, e a distância que separa "é você" de "é outra pessoa"
//! encolhe até não separar mais nada.
//!
//! Por isso o [`alinhar`] gira e escala o rosto pelos DOIS OLHOS antes de recortar. É uma
//! transformação de similaridade por dois pontos — mais simples que o ajuste de cinco
//! pontos que o OpenCV faz, e captura o que importa, que é a inclinação da cabeça.

use image::{imageops::FilterType, DynamicImage, RgbImage};

use super::deteccao::Rosto;

/// O lado do recorte que o SFace espera.
pub const ENTRADA: u32 = 112;

/// Onde os olhos precisam cair no recorte de 112×112.
///
/// São as posições do template do ArcFace, que é o alinhamento com que o SFace foi
/// treinado. Mudar estes números "para centralizar melhor" degrada o reconhecimento
/// inteiro sem dar nenhum sinal de erro.
const OLHO_DIREITO: (f32, f32) = (38.29, 51.69);
const OLHO_ESQUERDO: (f32, f32) = (73.53, 51.50);

/// Acima desta semelhança, é a mesma pessoa.
///
/// 0,363 é o limiar que o OpenCV publica para o SFace com distância de cosseno, medido
/// no LFW. **Subir isto é a correção certa quando ele confunde duas pessoas**; descer é o
/// que faz ele deixar de te reconhecer com barba por fazer. O erro caro é o primeiro:
/// chamar alguém pelo nome errado incomoda mais do que não ser reconhecido.
pub const LIMIAR: f32 = 0.363;

/// Recorta o rosto já girado e escalado, pronto para o modelo.
///
/// A transformação leva os olhos detectados exatamente para [`OLHO_DIREITO`] e
/// [`OLHO_ESQUERDO`]. O resto do rosto acompanha, e o que sobrar fora da imagem original
/// vira preto — o que é melhor que esticar, porque o modelo viu bordas assim no treino.
pub fn alinhar(imagem: &DynamicImage, rosto: &Rosto) -> RgbImage {
    let rgb = imagem.to_rgb8();
    let (direito, esquerdo) = (rosto.pontos[0], rosto.pontos[1]);

    // O vetor entre os olhos define rotação e escala de uma vez: o quanto ele precisa
    // girar para ficar horizontal, e o quanto precisa encolher para ter a distância do
    // template.
    let dx = esquerdo.0 - direito.0;
    let dy = esquerdo.1 - direito.1;
    let distancia = (dx * dx + dy * dy).sqrt();

    // Olhos no mesmo ponto acontece quando o detector erra feio. Cair no recorte simples
    // é melhor que dividir por zero e produzir um NaN que contamina o vetor inteiro.
    if distancia < 1.0 {
        return recorte_simples(&rgb, rosto);
    }

    let alvo = OLHO_ESQUERDO.0 - OLHO_DIREITO.0;
    let escala = alvo / distancia;
    let angulo = dy.atan2(dx);

    // A matriz inversa: para cada pixel do DESTINO, de onde ele vem na origem. É o
    // sentido certo — o direto deixaria buracos no destino.
    let (sin, cos) = angulo.sin_cos();
    let inv_cos = cos / escala;
    let inv_sin = sin / escala;

    let mut recorte = RgbImage::new(ENTRADA, ENTRADA);
    for y in 0..ENTRADA {
        for x in 0..ENTRADA {
            let dx = x as f32 - OLHO_DIREITO.0;
            let dy = y as f32 - OLHO_DIREITO.1;

            let ox = direito.0 + dx * inv_cos - dy * inv_sin;
            let oy = direito.1 + dx * inv_sin + dy * inv_cos;

            if ox >= 0.0 && oy >= 0.0 && (ox as u32) < rgb.width() && (oy as u32) < rgb.height() {
                recorte.put_pixel(x, y, *rgb.get_pixel(ox as u32, oy as u32));
            }
        }
    }

    recorte
}

/// O plano B: a caixa do detector, redimensionada.
///
/// Só acontece quando os olhos vieram inúteis. Reconhece pior, e ainda assim é melhor que
/// desistir — a pessoa provavelmente está de lado, e uma tentativa fraca que não casa com
/// ninguém tem o mesmo desfecho de não tentar.
fn recorte_simples(rgb: &RgbImage, rosto: &Rosto) -> RgbImage {
    let x = rosto.x.max(0.0) as u32;
    let y = rosto.y.max(0.0) as u32;
    let largura = (rosto.largura as u32)
        .min(rgb.width().saturating_sub(x))
        .max(1);
    let altura = (rosto.altura as u32)
        .min(rgb.height().saturating_sub(y))
        .max(1);

    DynamicImage::ImageRgb8(rgb.clone())
        .crop_imm(x, y, largura, altura)
        .resize_exact(ENTRADA, ENTRADA, FilterType::Triangle)
        .to_rgb8()
}

/// O recorte no formato que o modelo consome: `[1, 3, 112, 112]`, canais separados.
///
/// **Sem normalizar.** O SFace do OpenCV Zoo recebe o pixel cru de 0 a 255 — dividir por
/// 255 aqui, que é o reflexo de quem já treinou rede, produz vetores consistentes entre
/// si e completamente diferentes dos que o modelo deveria dar. E não dá erro nenhum: o
/// reconhecimento simplesmente para de casar.
pub fn tensor(recorte: &RgbImage) -> Vec<f32> {
    let pixels = (ENTRADA * ENTRADA) as usize;
    let mut dados = vec![0.0_f32; pixels * 3];

    for (i, pixel) in recorte.pixels().enumerate() {
        // Os três canais vêm em blocos, não intercalados: é o layout NCHW.
        dados[i] = f32::from(pixel[0]);
        dados[pixels + i] = f32::from(pixel[1]);
        dados[pixels * 2 + i] = f32::from(pixel[2]);
    }

    dados
}

/// O quanto dois rostos são a mesma pessoa, de -1 a 1.
///
/// Cosseno entre os vetores. Acima de [`LIMIAR`], é a mesma pessoa.
pub fn semelhanca(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut produto = 0.0;
    let mut norma_a = 0.0;
    let mut norma_b = 0.0;

    for (x, y) in a.iter().zip(b.iter()) {
        produto += x * y;
        norma_a += x * x;
        norma_b += y * y;
    }

    let divisor = norma_a.sqrt() * norma_b.sqrt();
    if divisor <= 0.0 {
        return 0.0;
    }

    produto / divisor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_tensor_tem_o_tamanho_que_o_modelo_espera() {
        let recorte = RgbImage::new(ENTRADA, ENTRADA);
        assert_eq!(tensor(&recorte).len(), 3 * 112 * 112);
    }

    /// NCHW: os canais em blocos, não intercalados. Trocar isso não dá erro — só faz o
    /// modelo ver uma imagem com as cores embaralhadas e nunca reconhecer ninguém.
    #[test]
    fn o_tensor_separa_os_canais_em_blocos() {
        let mut recorte = RgbImage::new(ENTRADA, ENTRADA);
        recorte.put_pixel(0, 0, image::Rgb([10, 20, 30]));

        let dados = tensor(&recorte);
        let pixels = (ENTRADA * ENTRADA) as usize;

        assert_eq!(dados[0], 10.0);
        assert_eq!(dados[pixels], 20.0);
        assert_eq!(dados[pixels * 2], 30.0);
    }

    /// O pixel entra CRU. Dividir por 255 aqui quebraria o reconhecimento em silêncio.
    #[test]
    fn o_pixel_entra_sem_normalizar() {
        let mut recorte = RgbImage::new(ENTRADA, ENTRADA);
        recorte.put_pixel(0, 0, image::Rgb([255, 255, 255]));

        assert_eq!(tensor(&recorte)[0], 255.0);
    }

    #[test]
    fn o_cosseno_vai_de_menos_um_a_um() {
        let a = [1.0, 0.0, 0.0];
        assert!((semelhanca(&a, &a) - 1.0).abs() < 0.0001);
        assert!((semelhanca(&a, &[-1.0, 0.0, 0.0]) + 1.0).abs() < 0.0001);
        assert!(semelhanca(&a, &[0.0, 1.0, 0.0]).abs() < 0.0001);
    }

    /// A escala não pode importar: o mesmo rosto mais claro dá um vetor maior, e continua
    /// sendo a mesma pessoa.
    #[test]
    fn o_cosseno_ignora_a_magnitude() {
        let a = [1.0, 2.0, 3.0];
        let dobro = [2.0, 4.0, 6.0];

        assert!((semelhanca(&a, &dobro) - 1.0).abs() < 0.0001);
    }

    /// Vetores de tamanhos diferentes só acontecem se o catálogo tiver sido escrito por
    /// outro modelo. Devolver 0 é o certo: "não é a mesma pessoa" em vez de um pânico.
    #[test]
    fn tamanhos_diferentes_nao_casam() {
        assert_eq!(semelhanca(&[1.0, 2.0], &[1.0, 2.0, 3.0]), 0.0);
        assert_eq!(semelhanca(&[], &[]), 0.0);
    }

    /// Vetor zerado dividiria por zero e contaminaria tudo com NaN.
    #[test]
    fn vetor_zerado_nao_gera_nan() {
        let resultado = semelhanca(&[0.0, 0.0], &[1.0, 1.0]);

        assert!(!resultado.is_nan());
        assert_eq!(resultado, 0.0);
    }

    /// Olhos no mesmo ponto (detector errando feio) dividiria por zero e produziria um
    /// recorte de NaN. O plano B tem que segurar isso.
    #[test]
    fn olhos_colados_caem_no_recorte_simples() {
        let imagem = DynamicImage::ImageRgb8(RgbImage::new(200, 200));
        let rosto = Rosto {
            x: 10.0,
            y: 10.0,
            largura: 50.0,
            altura: 50.0,
            confianca: 0.9,
            pontos: [(20.0, 20.0); 5],
        };

        let recorte = alinhar(&imagem, &rosto);
        assert_eq!(recorte.width(), ENTRADA);
        assert_eq!(recorte.height(), ENTRADA);
    }

    /// O alinhamento tem que devolver sempre 112×112, mesmo com o rosto na borda e
    /// parte da transformação caindo fora da imagem.
    #[test]
    fn o_alinhamento_sempre_devolve_o_tamanho_do_modelo() {
        let imagem = DynamicImage::ImageRgb8(RgbImage::new(320, 240));
        let rosto = Rosto {
            x: 280.0,
            y: 200.0,
            largura: 60.0,
            altura: 60.0,
            confianca: 0.9,
            pontos: [
                (300.0, 210.0),
                (330.0, 214.0),
                (315.0, 225.0),
                (305.0, 235.0),
                (325.0, 237.0),
            ],
        };

        let recorte = alinhar(&imagem, &rosto);
        assert_eq!((recorte.width(), recorte.height()), (ENTRADA, ENTRADA));
    }
}
