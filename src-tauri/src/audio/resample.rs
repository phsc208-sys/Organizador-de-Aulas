use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use super::TARGET_RATE;
use crate::error::{AppError, AppResult};

/// Reamostrador em streaming (mono) para 16 kHz. Se o microfone já grava a 16 kHz, só repassa as amostras.
pub struct Resampler16k {
    inner: Option<SincFixedIn<f32>>,
    pendente: Vec<f32>,
}

impl Resampler16k {
    pub fn novo(taxa_origem: u32) -> AppResult<Self> {
        if taxa_origem == TARGET_RATE {
            return Ok(Self { inner: None, pendente: Vec::new() });
        }
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let razao = TARGET_RATE as f64 / taxa_origem as f64;
        let inner = SincFixedIn::<f32>::new(razao, 2.0, params, 1024, 1)
            .map_err(|e| AppError::Audio(format!("não foi possível criar o reamostrador: {e}")))?;
        Ok(Self { inner: Some(inner), pendente: Vec::new() })
    }

    /// Recebe amostras na taxa de origem e devolve as já convertidas para 16 kHz.
    pub fn processar(&mut self, entrada: &[f32]) -> AppResult<Vec<f32>> {
        let Some(r) = self.inner.as_mut() else {
            return Ok(entrada.to_vec());
        };
        self.pendente.extend_from_slice(entrada);
        let mut saida = Vec::new();
        loop {
            let necessario = r.input_frames_next();
            if self.pendente.len() < necessario {
                break;
            }
            let bloco: Vec<f32> = self.pendente.drain(..necessario).collect();
            let convertido = r
                .process(&[bloco], None)
                .map_err(|e| AppError::Audio(format!("falha ao reamostrar: {e}")))?;
            saida.extend_from_slice(&convertido[0]);
        }
        Ok(saida)
    }

    /// Descarrega o que sobrou no buffer ao fim da gravação.
    pub fn finalizar(&mut self) -> AppResult<Vec<f32>> {
        let Some(r) = self.inner.as_mut() else {
            return Ok(Vec::new());
        };
        if self.pendente.is_empty() {
            return Ok(Vec::new());
        }
        let resto = [std::mem::take(&mut self.pendente)];
        let convertido = r
            .process_partial(Some(&resto[..]), None)
            .map_err(|e| AppError::Audio(format!("falha ao reamostrar o final: {e}")))?;
        Ok(convertido.into_iter().next().unwrap_or_default())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mesma_taxa_repassa_as_amostras() {
        let mut r = Resampler16k::novo(16_000).unwrap();
        let entrada = vec![0.1, 0.2, 0.3];
        assert_eq!(r.processar(&entrada).unwrap(), entrada);
    }

    #[test]
    fn de_48k_para_16k_gera_um_terco_das_amostras() {
        let mut r = Resampler16k::novo(48_000).unwrap();
        let entrada = vec![0.0f32; 48_000];
        let mut saida = r.processar(&entrada).unwrap();
        saida.extend(r.finalizar().unwrap());
        let esperado = 16_000i64;
        assert!((saida.len() as i64 - esperado).abs() < 600, "saiu {}", saida.len());
    }
}
