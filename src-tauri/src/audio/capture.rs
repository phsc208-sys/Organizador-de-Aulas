use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample};
use hound::{SampleFormat as WavFormat, WavSpec, WavWriter};
use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{resample::Resampler16k, TARGET_RATE};
use crate::error::{AppError, AppResult};

type Escritor = WavWriter<BufWriter<File>>;

/// Gravação em andamento. A thread de captura grava direto em WAV mono de 16 kHz,
/// então o arquivo já sai no formato que o whisper.cpp espera.
pub struct RecordingHandle {
    stop: Arc<AtomicBool>,
    join: JoinHandle<AppResult<u64>>,
    caminho: PathBuf,
}

impl RecordingHandle {
    /// Encerra a gravação e devolve o caminho do WAV e o total de amostras (a 16 kHz).
    pub fn parar(self) -> AppResult<(PathBuf, u64)> {
        self.stop.store(true, Ordering::SeqCst);
        let amostras = self
            .join
            .join()
            .map_err(|_| AppError::Audio("a thread de captura terminou de forma inesperada".into()))??;
        Ok((self.caminho, amostras))
    }
}

/// Lista os dispositivos de entrada que o cpal enxerga (no Linux, via ALSA/PipeWire).
pub fn listar_dispositivos() -> AppResult<Vec<String>> {
    let host = cpal::default_host();
    let dispositivos = host
        .input_devices()
        .map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(dispositivos.filter_map(|d| d.name().ok()).collect())
}

fn escolher_dispositivo(host: &cpal::Host, preferido: &str) -> AppResult<cpal::Device> {
    let preferido = preferido.trim();
    if !preferido.is_empty() {
        // Se o usuário configurou um dispositivo, só ele serve: não cai silenciosamente no microfone cru.
        let alvo = preferido.to_lowercase();
        if let Ok(dispositivos) = host.input_devices() {
            for d in dispositivos {
                if let Ok(nome) = d.name() {
                    if nome.to_lowercase().contains(&alvo) {
                        return Ok(d);
                    }
                }
            }
        }
        return Err(AppError::Audio(format!(
            "Dispositivo de entrada \"{preferido}\" não encontrado. Confira em Configurações."
        )));
    }
    host.default_input_device()
        .ok_or_else(|| AppError::Audio("Nenhum microfone padrão encontrado.".into()))
}

/// Inicia a gravação. `on_nivel` recebe o pico (0.0 a 1.0) cerca de 10 vezes por segundo.
pub fn iniciar(
    dispositivo: String,
    pulse_source: String,
    destino: PathBuf,
    on_nivel: impl Fn(f32) + Send + 'static,
) -> AppResult<RecordingHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let (pronto_tx, pronto_rx) = mpsc::channel::<Result<(), String>>();
    let caminho = destino.clone();

    // O cpal::Stream não pode trocar de thread em algumas plataformas, então ele
    // nasce, vive e morre dentro da thread de captura.
    let join = thread::Builder::new()
        .name("captura-audio".into())
        .spawn(move || -> AppResult<u64> {
            match executar(&dispositivo, &pulse_source, &destino, stop_thread, &pronto_tx, on_nivel) {
                Ok(total) => Ok(total),
                Err(e) => {
                    let _ = pronto_tx.send(Err(e.to_string()));
                    Err(e)
                }
            }
        })?;

    match pronto_rx.recv_timeout(Duration::from_secs(8)) {
        Ok(Ok(())) => Ok(RecordingHandle { stop, join, caminho }),
        Ok(Err(msg)) => {
            let _ = join.join();
            Err(AppError::Audio(msg))
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            Err(AppError::Audio("Tempo esgotado ao abrir o microfone.".into()))
        }
    }
}

fn executar(
    dispositivo: &str,
    pulse_source: &str,
    destino: &Path,
    stop: Arc<AtomicBool>,
    pronto: &mpsc::Sender<Result<(), String>>,
    on_nivel: impl Fn(f32),
) -> AppResult<u64> {
    // O backend ALSA do cpal enxerga o PipeWire como "pipewire"/"pulse"/"default". Para
    // escolher a fonte virtual do EasyEffects, a plugin de PulseAudio respeita PULSE_SOURCE.
    let fonte = pulse_source.trim();
    if !fonte.is_empty() {
        std::env::set_var("PULSE_SOURCE", fonte);
    }

    let host = cpal::default_host();
    let device = escolher_dispositivo(&host, dispositivo)?;
    let suportado = device
        .default_input_config()
        .map_err(|e| AppError::Audio(format!("não foi possível ler a configuração do microfone: {e}")))?;
    let formato = suportado.sample_format();
    let config: cpal::StreamConfig = suportado.into();
    let canais = config.channels as usize;
    let taxa = config.sample_rate.0;

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let stream = match formato {
        SampleFormat::F32 => construir_stream::<f32>(&device, &config, tx, canais)?,
        SampleFormat::I16 => construir_stream::<i16>(&device, &config, tx, canais)?,
        SampleFormat::U16 => construir_stream::<u16>(&device, &config, tx, canais)?,
        SampleFormat::I32 => construir_stream::<i32>(&device, &config, tx, canais)?,
        outro => {
            return Err(AppError::Audio(format!("formato de amostra não suportado: {outro:?}")))
        }
    };

    let spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: WavFormat::Int,
    };
    let mut escritor = WavWriter::create(destino, spec)
        .map_err(|e| AppError::Audio(format!("não foi possível criar {}: {e}", destino.display())))?;
    let mut resampler = Resampler16k::novo(taxa)?;

    stream
        .play()
        .map_err(|e| AppError::Audio(format!("não foi possível iniciar a captura: {e}")))?;
    let _ = pronto.send(Ok(()));

    let mut total: u64 = 0;
    let mut ultimo_flush: u64 = 0;
    let mut pico = 0.0f32;
    let mut ultimo_nivel = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(bloco) => {
                for &s in &bloco {
                    pico = pico.max(s.abs());
                }
                let convertido = resampler.processar(&bloco)?;
                total += escrever(&mut escritor, &convertido)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if ultimo_nivel.elapsed() >= Duration::from_millis(100) {
            on_nivel(pico.min(1.0));
            pico = 0.0;
            ultimo_nivel = Instant::now();
        }
        // Atualiza o cabeçalho do WAV de tempos em tempos: se o app cair, o arquivo continua legível.
        if total - ultimo_flush >= (TARGET_RATE as u64) * 30 {
            escritor
                .flush()
                .map_err(|e| AppError::Audio(format!("falha ao gravar em disco: {e}")))?;
            ultimo_flush = total;
        }
        if stop.load(Ordering::SeqCst) {
            break;
        }
    }

    // Fecha o stream (derruba o remetente) e consome o que ainda estava na fila.
    drop(stream);
    while let Ok(bloco) = rx.try_recv() {
        let convertido = resampler.processar(&bloco)?;
        total += escrever(&mut escritor, &convertido)?;
    }
    let resto = resampler.finalizar()?;
    total += escrever(&mut escritor, &resto)?;
    escritor
        .finalize()
        .map_err(|e| AppError::Audio(format!("falha ao finalizar o WAV: {e}")))?;
    Ok(total)
}

fn construir_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: mpsc::Sender<Vec<f32>>,
    canais: usize,
) -> AppResult<cpal::Stream>
where
    T: SizedSample,
    f32: cpal::FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |dados: &[T], _| {
                // Mistura todos os canais em mono.
                let mono: Vec<f32> = dados
                    .chunks(canais)
                    .map(|quadro| {
                        let soma: f32 = quadro.iter().map(|&a| f32::from_sample(a)).sum();
                        soma / canais as f32
                    })
                    .collect();
                let _ = tx.send(mono);
            },
            |erro| eprintln!("[captura] erro no stream de áudio: {erro}"),
            None,
        )
        .map_err(|e| AppError::Audio(format!("não foi possível abrir o microfone: {e}")))
}

fn escrever(escritor: &mut Escritor, amostras: &[f32]) -> AppResult<u64> {
    for &a in amostras {
        let valor = (a.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        escritor
            .write_sample(valor)
            .map_err(|e| AppError::Audio(format!("falha ao gravar em disco: {e}")))?;
    }
    Ok(amostras.len() as u64)
}
