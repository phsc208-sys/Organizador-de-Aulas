use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{AppError, AppResult};

/// Migrations em ordem. A versão aplicada fica em `PRAGMA user_version`.
const MIGRACOES: [&str; 2] = [
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_status.sql"),
];

pub fn abrir(caminho: &Path) -> AppResult<Connection> {
    let conn = Connection::open(caminho)?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
    )?;
    migrar(&conn)?;
    Ok(conn)
}

fn migrar(conn: &Connection) -> AppResult<()> {
    let atual: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRACOES.iter().enumerate() {
        let versao = (i + 1) as i64;
        if versao <= atual {
            continue;
        }
        let script = format!("BEGIN;\n{sql}\nPRAGMA user_version = {versao};\nCOMMIT;");
        if let Err(e) = conn.execute_batch(&script) {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(e.into());
        }
    }
    Ok(())
}

/// Se o app foi fechado no meio do pipeline, marca esses registros como erro para poderem ser reprocessados.
pub fn recuperar_interrompidos(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE registros_aula
            SET status = 'erro', erro = 'Processamento interrompido (o app foi fechado).'
          WHERE status IN ('transcrevendo', 'resumindo')",
        [],
    )?;
    Ok(())
}

// ───────────── Disciplinas ─────────────

#[derive(Debug, Serialize)]
pub struct Disciplina {
    pub id: i64,
    pub nome: String,
    pub professor: Option<String>,
    pub semestre: Option<String>,
    pub carga_horaria: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DisciplinaInput {
    pub nome: String,
    pub professor: Option<String>,
    pub semestre: Option<String>,
    pub carga_horaria: Option<i64>,
}

fn limpar(valor: &Option<String>) -> Option<String> {
    valor
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn validar(dados: &DisciplinaInput) -> AppResult<String> {
    let nome = dados.nome.trim().to_string();
    if nome.is_empty() {
        return Err(AppError::Msg("Informe o nome da disciplina.".into()));
    }
    if matches!(dados.carga_horaria, Some(h) if h < 0) {
        return Err(AppError::Msg("A carga horária não pode ser negativa.".into()));
    }
    Ok(nome)
}

fn ler_disciplina(r: &rusqlite::Row) -> rusqlite::Result<Disciplina> {
    Ok(Disciplina {
        id: r.get(0)?,
        nome: r.get(1)?,
        professor: r.get(2)?,
        semestre: r.get(3)?,
        carga_horaria: r.get(4)?,
    })
}

pub fn listar_disciplinas(conn: &Connection) -> AppResult<Vec<Disciplina>> {
    let mut stmt = conn.prepare(
        "SELECT id, nome, professor, semestre, carga_horaria
           FROM disciplinas ORDER BY nome COLLATE NOCASE",
    )?;
    let linhas = stmt.query_map([], ler_disciplina)?;
    Ok(linhas.collect::<Result<Vec<_>, _>>()?)
}

pub fn obter_disciplina(conn: &Connection, id: i64) -> AppResult<Disciplina> {
    conn.query_row(
        "SELECT id, nome, professor, semestre, carga_horaria FROM disciplinas WHERE id = ?1",
        params![id],
        ler_disciplina,
    )
    .optional()?
    .ok_or_else(|| AppError::Msg("Disciplina não encontrada.".into()))
}

pub fn criar_disciplina(conn: &Connection, dados: &DisciplinaInput) -> AppResult<Disciplina> {
    let nome = validar(dados)?;
    conn.execute(
        "INSERT INTO disciplinas (nome, professor, semestre, carga_horaria) VALUES (?1, ?2, ?3, ?4)",
        params![nome, limpar(&dados.professor), limpar(&dados.semestre), dados.carga_horaria],
    )?;
    obter_disciplina(conn, conn.last_insert_rowid())
}

pub fn atualizar_disciplina(
    conn: &Connection,
    id: i64,
    dados: &DisciplinaInput,
) -> AppResult<Disciplina> {
    let nome = validar(dados)?;
    let alteradas = conn.execute(
        "UPDATE disciplinas SET nome = ?1, professor = ?2, semestre = ?3, carga_horaria = ?4 WHERE id = ?5",
        params![nome, limpar(&dados.professor), limpar(&dados.semestre), dados.carga_horaria, id],
    )?;
    if alteradas == 0 {
        return Err(AppError::Msg("Disciplina não encontrada.".into()));
    }
    obter_disciplina(conn, id)
}

pub fn caminhos_audio_da_disciplina(conn: &Connection, id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT caminho_audio FROM registros_aula WHERE disciplina_id = ?1")?;
    let linhas = stmt.query_map(params![id], |r| r.get::<_, String>(0))?;
    Ok(linhas.collect::<Result<Vec<_>, _>>()?)
}

/// Apaga a disciplina; os registros vão junto por ON DELETE CASCADE.
pub fn excluir_disciplina(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM disciplinas WHERE id = ?1", params![id])?;
    Ok(())
}

// ───────────── Registros de aula ─────────────

#[derive(Debug, Clone, Serialize)]
pub struct RegistroAula {
    pub id: i64,
    pub disciplina_id: i64,
    pub data_hora: Option<String>,
    pub caminho_audio: String,
    pub transcricao_bruta: Option<String>,
    pub ata_gemini: Option<String>,
    pub status: String,
    pub erro: Option<String>,
    pub duracao_seg: Option<i64>,
}

/// Versão leve para a lista (sem o texto completo da transcrição e da ata).
#[derive(Debug, Clone, Serialize)]
pub struct RegistroLista {
    pub id: i64,
    pub disciplina_id: i64,
    pub data_hora: Option<String>,
    pub status: String,
    pub erro: Option<String>,
    pub duracao_seg: Option<i64>,
    pub titulo: Option<String>,
    pub tem_transcricao: bool,
    pub tem_ata: bool,
}

pub fn inserir_registro(
    conn: &Connection,
    disciplina_id: i64,
    caminho_audio: &str,
    duracao_seg: i64,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO registros_aula (disciplina_id, caminho_audio, status, duracao_seg)
         VALUES (?1, ?2, 'gravado', ?3)",
        params![disciplina_id, caminho_audio, duracao_seg],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn listar_registros(conn: &Connection, disciplina_id: i64) -> AppResult<Vec<RegistroLista>> {
    let mut stmt = conn.prepare(
        "SELECT id, disciplina_id, data_hora, status, erro, duracao_seg,
                CASE WHEN ata_gemini IS NOT NULL AND json_valid(ata_gemini)
                     THEN json_extract(ata_gemini, '$.titulo') END,
                (transcricao_bruta IS NOT NULL AND length(trim(transcricao_bruta)) > 0),
                (ata_gemini IS NOT NULL AND length(ata_gemini) > 0)
           FROM registros_aula
          WHERE disciplina_id = ?1
          ORDER BY data_hora DESC, id DESC",
    )?;
    let linhas = stmt.query_map(params![disciplina_id], |r| {
        Ok(RegistroLista {
            id: r.get(0)?,
            disciplina_id: r.get(1)?,
            data_hora: r.get(2)?,
            status: r.get(3)?,
            erro: r.get(4)?,
            duracao_seg: r.get(5)?,
            titulo: r.get(6)?,
            tem_transcricao: r.get(7)?,
            tem_ata: r.get(8)?,
        })
    })?;
    Ok(linhas.collect::<Result<Vec<_>, _>>()?)
}

pub fn obter_registro(conn: &Connection, id: i64) -> AppResult<RegistroAula> {
    conn.query_row(
        "SELECT id, disciplina_id, data_hora, caminho_audio, transcricao_bruta, ata_gemini,
                status, erro, duracao_seg
           FROM registros_aula WHERE id = ?1",
        params![id],
        |r| {
            Ok(RegistroAula {
                id: r.get(0)?,
                disciplina_id: r.get(1)?,
                data_hora: r.get(2)?,
                caminho_audio: r.get(3)?,
                transcricao_bruta: r.get(4)?,
                ata_gemini: r.get(5)?,
                status: r.get(6)?,
                erro: r.get(7)?,
                duracao_seg: r.get(8)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::Msg("Aula não encontrada.".into()))
}

pub fn definir_status(conn: &Connection, id: i64, status: &str, erro: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE registros_aula SET status = ?1, erro = ?2 WHERE id = ?3",
        params![status, erro, id],
    )?;
    Ok(())
}

pub fn salvar_transcricao(conn: &Connection, id: i64, texto: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE registros_aula SET transcricao_bruta = ?1, status = 'transcrito', erro = NULL WHERE id = ?2",
        params![texto, id],
    )?;
    Ok(())
}

pub fn salvar_ata(conn: &Connection, id: i64, ata_json: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE registros_aula SET ata_gemini = ?1, status = 'concluido', erro = NULL WHERE id = ?2",
        params![ata_json, id],
    )?;
    Ok(())
}

pub fn excluir_registro(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM registros_aula WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn memoria() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrar(&conn).unwrap();
        conn
    }

    fn disciplina(nome: &str) -> DisciplinaInput {
        DisciplinaInput {
            nome: nome.into(),
            professor: Some("  ".into()),
            semestre: Some("2026/2".into()),
            carga_horaria: Some(60),
        }
    }

    #[test]
    fn migracoes_aplicam_e_sao_idempotentes() {
        let conn = memoria();
        migrar(&conn).unwrap();
        let versao: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(versao, 2);
    }

    #[test]
    fn disciplina_vazia_e_rejeitada_e_campos_em_branco_viram_nulo() {
        let conn = memoria();
        assert!(criar_disciplina(&conn, &disciplina("   ")).is_err());
        let d = criar_disciplina(&conn, &disciplina(" Arquitetura ")).unwrap();
        assert_eq!(d.nome, "Arquitetura");
        assert_eq!(d.professor, None);
    }

    #[test]
    fn registro_nasce_gravado_e_segue_o_pipeline() {
        let conn = memoria();
        let d = criar_disciplina(&conn, &disciplina("Redes")).unwrap();
        let id = inserir_registro(&conn, d.id, "/tmp/a.wav", 120).unwrap();
        assert_eq!(obter_registro(&conn, id).unwrap().status, "gravado");

        salvar_transcricao(&conn, id, "texto bruto").unwrap();
        assert_eq!(obter_registro(&conn, id).unwrap().status, "transcrito");

        salvar_ata(&conn, id, r#"{"titulo":"Camada de enlace"}"#).unwrap();
        let lista = listar_registros(&conn, d.id).unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].status, "concluido");
        assert_eq!(lista[0].titulo.as_deref(), Some("Camada de enlace"));
        assert!(lista[0].tem_transcricao && lista[0].tem_ata);
    }

    #[test]
    fn excluir_disciplina_leva_os_registros_junto() {
        let conn = memoria();
        let d = criar_disciplina(&conn, &disciplina("Sistemas Operacionais")).unwrap();
        inserir_registro(&conn, d.id, "/tmp/b.wav", 60).unwrap();
        excluir_disciplina(&conn, d.id).unwrap();
        let restantes: i64 = conn
            .query_row("SELECT COUNT(*) FROM registros_aula", [], |r| r.get(0))
            .unwrap();
        assert_eq!(restantes, 0);
    }

    #[test]
    fn interrompidos_viram_erro() {
        let conn = memoria();
        let d = criar_disciplina(&conn, &disciplina("BD")).unwrap();
        let id = inserir_registro(&conn, d.id, "/tmp/c.wav", 60).unwrap();
        definir_status(&conn, id, "transcrevendo", None).unwrap();
        recuperar_interrompidos(&conn).unwrap();
        assert_eq!(obter_registro(&conn, id).unwrap().status, "erro");
    }
}
