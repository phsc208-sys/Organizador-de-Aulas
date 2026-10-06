use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::error::{AppError, AppResult};

/// Diretórios do app. `data` guarda banco, áudios e modelos; `config` guarda o config.json.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub data: PathBuf,
    pub config: PathBuf,
}

impl Dirs {
    pub fn db_path(&self) -> PathBuf {
        self.data.join("aulas.sqlite")
    }
    pub fn audio_dir(&self) -> PathBuf {
        self.data.join("audio")
    }
    /// Pasta oculta onde a gravação em andamento é escrita.
    pub fn tmp_dir(&self) -> PathBuf {
        self.data.join(".gravacoes_tmp")
    }
    pub fn models_dir(&self) -> PathBuf {
        self.data.join("models")
    }
    pub fn settings_path(&self) -> PathBuf {
        self.config.join("config.json")
    }

    pub fn garantir(&self) -> AppResult<()> {
        for d in [
            &self.data,
            &self.config,
            &self.audio_dir(),
            &self.tmp_dir(),
            &self.models_dir(),
        ] {
            fs::create_dir_all(d)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub whisper_model: String,
    pub whisper_language: String,
    pub whisper_threads: u32,
    /// Trecho do nome do dispositivo de entrada do cpal (vazio = microfone padrão).
    pub input_device: String,
    /// Fonte do PipeWire/PulseAudio a usar (ex.: "easyeffects_source"). Vazio = não força.
    pub pulse_source: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gemini_api_key: String::new(),
            gemini_model: "gemini-2.5-pro".into(),
            whisper_model: "ggml-small.bin".into(),
            whisper_language: "pt".into(),
            whisper_threads: 4,
            input_device: String::new(),
            pulse_source: String::new(),
        }
    }
}

impl Settings {
    pub fn load(dirs: &Dirs) -> AppResult<Self> {
        let path = dirs.settings_path();
        if !path.exists() {
            let s = Settings::default();
            s.save(dirs)?;
            return Ok(s);
        }
        let texto = fs::read_to_string(&path)?;
        serde_json::from_str(&texto)
            .map_err(|e| AppError::Config(format!("config.json inválido ({}): {e}", path.display())))
    }

    pub fn save(&self, dirs: &Dirs) -> AppResult<()> {
        let path = dirs.settings_path();
        let texto = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::Config(e.to_string()))?;
        fs::write(&path, texto)?;
        // O arquivo contém a chave da API: só o dono pode ler.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// Chave do config.json; se vazia, usa a variável de ambiente GEMINI_API_KEY.
    pub fn gemini_key(&self) -> Option<String> {
        let k = self.gemini_api_key.trim();
        if !k.is_empty() {
            return Some(k.to_string());
        }
        std::env::var("GEMINI_API_KEY")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }
}
