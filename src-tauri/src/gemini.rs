use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use crate::{
    config::Settings,
    error::{AppError, AppResult},
};

const PROMPT: &str = include_str!("../prompts/ata.md");

/// O Gemini Pro aguenta bem mais que isso, mas uma transcrição desse tamanho indica problema na gravação.
const LIMITE_CARACTERES: usize = 600_000;
const TENTATIVAS: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topico {
    pub titulo: String,
    pub explicacao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataImportante {
    pub data: String,
    pub descricao: String,
}

/// Estrutura da ata. Precisa ficar igual ao `schema()` abaixo e ao que o frontend renderiza.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ata {
    pub titulo: String,
    pub resumo: String,
    #[serde(default)]
    pub topicos_principais: Vec<Topico>,
    #[serde(default)]
    pub datas_importantes: Vec<DataImportante>,
    #[serde(default)]
    pub tarefas_e_avisos: Vec<String>,
}

fn schema() -> Value {
    json!({
        "type": "OBJECT",
        "properties": {
            "titulo": { "type": "STRING" },
            "resumo": { "type": "STRING" },
            "topicos_principais": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "titulo": { "type": "STRING" },
                        "explicacao": { "type": "STRING" }
                    },
                    "required": ["titulo", "explicacao"]
                }
            },
            "datas_importantes": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "data": { "type": "STRING" },
                        "descricao": { "type": "STRING" }
                    },
                    "required": ["data", "descricao"]
                }
            },
            "tarefas_e_avisos": { "type": "ARRAY", "items": { "type": "STRING" } }
        },
        "required": ["titulo", "resumo", "topicos_principais", "datas_importantes", "tarefas_e_avisos"]
    })
}

/// Envia a transcrição ao Gemini e devolve a ata validada, como JSON formatado (é o que vai para `ata_gemini`).
pub async fn gerar_ata(settings: &Settings, transcricao: &str, disciplina: &str) -> AppResult<String> {
    let chave = settings.gemini_key().ok_or_else(|| {
        AppError::Gemini(
            "chave da API não configurada. Defina em Configurações ou na variável GEMINI_API_KEY.".into(),
        )
    })?;
    if transcricao.chars().count() > LIMITE_CARACTERES {
        return Err(AppError::Gemini(
            "a transcrição é grande demais para uma única chamada. Divida a gravação em partes.".into(),
        ));
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        settings.gemini_model.trim()
    );
    let corpo = json!({
        "systemInstruction": { "parts": [{ "text": PROMPT }] },
        "contents": [{
            "role": "user",
            "parts": [{ "text": format!("Disciplina: {disciplina}\n\nTranscrição bruta da aula:\n\n{transcricao}") }]
        }],
        "generationConfig": {
            "temperature": 0.2,
            "responseMimeType": "application/json",
            "responseSchema": schema()
        }
    });

    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| AppError::Gemini(e.to_string()))?;

    let mut ultimo_erro = String::new();
    for tentativa in 0..TENTATIVAS {
        if tentativa > 0 {
            tokio::time::sleep(Duration::from_secs(3 * 2u64.pow(tentativa))).await;
        }
        let resposta = match cliente
            .post(&url)
            .header("x-goog-api-key", &chave)
            .json(&corpo)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                ultimo_erro = format!("falha de rede: {e}");
                continue;
            }
        };

        let status = resposta.status();
        let valor: Value = resposta
            .json()
            .await
            .map_err(|e| AppError::Gemini(format!("resposta ilegível: {e}")))?;

        if status.is_success() {
            return extrair_ata(&valor);
        }

        let detalhe = valor["error"]["message"].as_str().unwrap_or("sem detalhes").to_string();
        ultimo_erro = format!("HTTP {}: {detalhe}", status.as_u16());
        // Só vale tentar de novo em limite de uso e erros temporários do servidor.
        if !(status.as_u16() == 429 || status.is_server_error()) {
            break;
        }
    }
    Err(AppError::Gemini(ultimo_erro))
}

fn extrair_ata(resposta: &Value) -> AppResult<String> {
    let texto = resposta["candidates"][0]["content"]["parts"][0]["text"].as_str();
    let Some(texto) = texto else {
        let motivo = resposta["promptFeedback"]["blockReason"]
            .as_str()
            .or_else(|| resposta["candidates"][0]["finishReason"].as_str())
            .unwrap_or("resposta vazia");
        return Err(AppError::Gemini(format!("o modelo não devolveu conteúdo ({motivo}).")));
    };

    // Mesmo com responseMimeType, alguns modelos devolvem o JSON dentro de uma cerca ```.
    let limpo = texto
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let ata: Ata = serde_json::from_str(limpo)
        .map_err(|e| AppError::Gemini(format!("a resposta não segue o formato esperado: {e}")))?;
    if ata.resumo.trim().is_empty() {
        return Err(AppError::Gemini("a ata voltou sem resumo.".into()));
    }
    serde_json::to_string_pretty(&ata).map_err(|e| AppError::Gemini(e.to_string()))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn resposta(texto: &str) -> Value {
        json!({ "candidates": [{ "content": { "parts": [{ "text": texto }] } }] })
    }

    #[test]
    fn aceita_json_valido_e_normaliza() {
        let v = resposta(r#"{"titulo":"Pilhas","resumo":"Falamos de pilhas.","topicos_principais":[],"datas_importantes":[],"tarefas_e_avisos":[]}"#);
        let json = extrair_ata(&v).unwrap();
        assert!(json.contains("\"titulo\": \"Pilhas\""));
    }

    #[test]
    fn aceita_json_dentro_de_cerca_de_codigo() {
        let v = resposta("```json\n{\"titulo\":\"A\",\"resumo\":\"B\"}\n```");
        assert!(extrair_ata(&v).is_ok());
    }

    #[test]
    fn rejeita_formato_errado_e_resumo_vazio() {
        assert!(extrair_ata(&resposta("não é json")).is_err());
        assert!(extrair_ata(&resposta(r#"{"titulo":"A","resumo":"  "}"#)).is_err());
        assert!(extrair_ata(&json!({ "candidates": [] })).is_err());
    }
}
