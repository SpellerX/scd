//! Regras do controle de férias (SCD).
//!
//! Modelo simplificado do direito trabalhista:
//! - ao cadastrar o funcionário com **data de admissão**, o sistema gera
//!   automaticamente os períodos de férias (um por ano de trabalho);
//! - cada período tem um **vencimento** = início + 24 meses
//!   (12 meses aquisitivos + 12 meses para gozar);
//! - passou do vencimento sem regularizar ⇒ **férias vencidas**;
//! - dentro do prazo concessivo ⇒ **férias a vencer** (com alerta
//!   configurável N dias antes do vencimento).

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// Período de férias vencido (ainda não regularizado).
#[derive(Debug, Clone, Serialize)]
pub struct FeriasVencida {
    /// Id UUID (TEXT) do período — chave única global.
    pub id: String,
    pub funcionario_id: String,
    pub funcionario_nome: String,
    pub empresa_nome: String,
    pub inicio: String,
    pub vencimento: String,
    pub dias_vencidas: i64,
    pub observacao: Option<String>,
}

/// Período de férias "a vencer" (dentro do prazo concessivo).
#[derive(Debug, Clone, Serialize)]
pub struct FeriasAVencer {
    /// Id UUID (TEXT) do período — chave única global.
    pub id: String,
    pub funcionario_id: String,
    pub funcionario_nome: String,
    pub empresa_nome: String,
    pub inicio: String,
    pub vencimento: String,
    pub dias_restantes: i64,
    pub dentro_alerta: bool,
}

/// Notificação interna (sino do cabeçalho).
#[derive(Debug, Clone, Serialize)]
pub struct Notificacao {
    /// Id estável usado pelo app para marcar como "vista" (v-<id>/a-<id>).
    pub id: String,
    /// "vencida" ou "alerta".
    pub tipo: String,
    pub titulo: String,
    pub mensagem: String,
}

/// Pendência em exibição na janela própria de notificação (estado do app).
#[derive(Debug, Clone, Serialize)]
pub struct PendenciaNotificacao {
    pub id: String,
    pub tipo: String,
    pub titulo: String,
    pub mensagem: String,
    /// Id da seção para onde o botão de ação leva o usuário.
    pub secao: String,
}

/// Chave de configuração do alerta (dias antes do vencimento).
const CHAVE_ALERTA_DIAS: &str = "ferias.alerta_dias";

// ═══════════════════ Geração automática (no cadastro do funcionário) ═══════════════════

/// Gera os períodos de férias em falta a partir da data de admissão:
/// um período para cada ano cujo início já chegou (até hoje).
/// Chamado automaticamente pelo cadastro/importação de funcionários.
pub fn gerar_periodos_do_funcionario(
    conn: &Connection,
    employee_id: &str,
    admissao_iso: &str,
) -> Result<(), String> {
    let hoje: String = conn
        .query_row("SELECT date('now', 'localtime')", [], |linha| linha.get(0))
        .map_err(|err| format!("Falha ao ler a data atual: {err}"))?;

    let mut inicio = admissao_iso.trim().to_string();
    if !data_valida(&inicio) || inicio > hoje {
        return Ok(()); // admissão futura/inválida: nada a gerar por enquanto
    }

    while inicio <= hoje {
        let ja_existe: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM employee_leave_periods
                                WHERE employee_id = ?1 AND inicio = ?2
                                  AND deleted_at IS NULL)",
                params![employee_id, inicio],
                |linha| linha.get(0),
            )
            .map_err(|err| format!("Falha ao conferir períodos de férias: {err}"))?;

        if !ja_existe {
            let vencimento = adicionar_meses(&inicio, 24);
            // Id UUID gerado aqui (chave única global para a sincronização).
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO employee_leave_periods (id, employee_id, inicio, vencimento)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, employee_id, inicio, vencimento],
            )
            .map_err(|err| format!("Falha ao gerar o período de férias: {err}"))?;
        }

        inicio = adicionar_meses(&inicio, 12);
    }

    Ok(())
}

/// Gera os períodos em falta de TODOS os funcionários com data de admissão.
/// Rodado na inicialização: cobre funcionários cadastrados antes da criação
/// deste módulo (idempotente — períodos existentes não são duplicados).
pub fn gerar_periodos_existentes(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, data_admissao FROM employees
              WHERE data_admissao IS NOT NULL AND deleted_at IS NULL",
        )
        .map_err(|err| format!("Falha ao preparar a geração de períodos: {err}"))?;

    let funcionarios = stmt
        .query_map([], |linha| {
            Ok((linha.get::<_, String>(0)?, linha.get::<_, String>(1)?))
        })
        .map_err(|err| format!("Falha ao consultar funcionários: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler funcionários: {err}"))?;

    for (id, admissao) in funcionarios {
        gerar_periodos_do_funcionario(conn, &id, &admissao)?;
    }

    Ok(())
}

// ═══════════════════ Consultas das telas ════════════════════════════════════

/// Férias VENCIDAS (prazo concessivo expirado e ainda não regularizadas).
/// Só considera registros ativos (funcionário, empresa e período vivos).
pub fn listar_vencidas(conn: &Connection) -> Result<Vec<FeriasVencida>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, f.id, f.nome, c.razao_social, p.inicio, p.vencimento,
                    COALESCE(CAST(julianday('now','localtime') - julianday(p.vencimento) AS INTEGER), 0),
                    p.observacao
               FROM employee_leave_periods p
               JOIN employees f ON f.id = p.employee_id
               JOIN companies c ON c.id = f.empresa_id
              WHERE p.regularizada = 0
                AND p.vencimento <= date('now','localtime')
                AND p.deleted_at IS NULL
                AND f.deleted_at IS NULL
                AND c.deleted_at IS NULL
              ORDER BY p.vencimento",
        )
        .map_err(|err| format!("Falha ao preparar a consulta de férias vencidas: {err}"))?;

    let linhas = stmt
        .query_map([], |linha| {
            Ok(FeriasVencida {
                id: linha.get(0)?,
                funcionario_id: linha.get(1)?,
                funcionario_nome: linha.get(2)?,
                empresa_nome: linha.get(3)?,
                inicio: linha.get(4)?,
                vencimento: linha.get(5)?,
                dias_vencidas: linha.get(6)?,
                observacao: linha.get(7)?,
            })
        })
        .map_err(|err| format!("Falha ao consultar férias vencidas: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler férias vencidas: {err}"))?;

    Ok(linhas)
}

/// Férias A VENCER (direito adquirido, dentro do prazo concessivo).
/// `alerta_dias` marca os períodos que já caíram na janela de alerta.
pub fn listar_a_vencer(conn: &Connection, alerta_dias: i64) -> Result<Vec<FeriasAVencer>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, f.id, f.nome, c.razao_social, p.inicio, p.vencimento,
                    COALESCE(CAST(julianday(p.vencimento) - julianday('now','localtime') AS INTEGER), 0)
               FROM employee_leave_periods p
               JOIN employees f ON f.id = p.employee_id
               JOIN companies c ON c.id = f.empresa_id
              WHERE p.regularizada = 0
                AND p.vencimento > date('now','localtime')
                AND date(p.vencimento, '-12 months') <= date('now','localtime')
                AND p.deleted_at IS NULL
                AND f.deleted_at IS NULL
                AND c.deleted_at IS NULL
              ORDER BY p.vencimento",
        )
        .map_err(|err| format!("Falha ao preparar a consulta de férias a vencer: {err}"))?;

    let linhas = stmt
        .query_map([], |linha| {
            Ok(FeriasAVencer {
                id: linha.get(0)?,
                funcionario_id: linha.get(1)?,
                funcionario_nome: linha.get(2)?,
                empresa_nome: linha.get(3)?,
                inicio: linha.get(4)?,
                vencimento: linha.get(5)?,
                dias_restantes: linha.get(6)?,
                dentro_alerta: false, // preenchido abaixo
            })
        })
        .map_err(|err| format!("Falha ao consultar férias a vencer: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler férias a vencer: {err}"))?;

    Ok(linhas
        .into_iter()
        .map(|mut item| {
            item.dentro_alerta = item.dias_restantes <= alerta_dias;
            item
        })
        .collect())
}

/// Registra que as férias do período já foram gozadas/quitadas
/// ("regularizar" um período vencido).
pub fn regularizar(
    conn: &Connection,
    periodo_id: &str,
    observacao: Option<String>,
) -> Result<(), String> {
    let alterados = conn
        .execute(
            "UPDATE employee_leave_periods
                SET regularizada = 1,
                    regularizada_em = datetime('now','localtime'),
                    observacao = ?1,
                    updated_at = datetime('now')
              WHERE id = ?2 AND deleted_at IS NULL",
            params![observacao, periodo_id],
        )
        .map_err(|err| format!("Falha ao regularizar as férias: {err}"))?;

    if alterados == 0 {
        return Err("Período de férias não encontrado.".to_string());
    }
    Ok(())
}

// ═══════════════════ Configuração do alerta ═════════════════════════════════

/// Dias de antecedência configurados para o alerta (padrão: 15).
pub fn obter_alerta_dias(conn: &Connection) -> Result<i64, String> {
    let valor: Option<String> = conn
        .query_row(
            "SELECT valor FROM app_settings WHERE chave = ?1",
            [CHAVE_ALERTA_DIAS],
            |linha| linha.get(0),
        )
        .optional()
        .map_err(|err| format!("Falha ao ler o alerta: {err}"))?;

    Ok(valor.and_then(|v| v.parse().ok()).unwrap_or(15))
}

/// Define quantos dias antes do vencimento o alerta deve disparar.
pub fn definir_alerta_dias(conn: &Connection, dias: i64) -> Result<(), String> {
    let dias = dias.clamp(1, 365);
    salvar_config(conn, CHAVE_ALERTA_DIAS, &dias.to_string())
}

/// Lê uma configuração genérica (chave → valor) da tabela app_settings.
pub fn obter_config(conn: &Connection, chave: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT valor FROM app_settings WHERE chave = ?1",
        [chave],
        |linha| linha.get(0),
    )
    .optional()
    .map_err(|err| format!("Falha ao ler a configuração: {err}"))
}

/// Grava uma configuração genérica (upsert na tabela app_settings).
pub fn salvar_config(conn: &Connection, chave: &str, valor: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (chave, valor) VALUES (?1, ?2)
         ON CONFLICT(chave) DO UPDATE SET valor = excluded.valor",
        rusqlite::params![chave, valor],
    )
    .map_err(|err| format!("Falha ao salvar a configuração: {err}"))?;
    Ok(())
}

// ═══════════════════ Notificações (sino do app) ═════════════════════════════

/// Monta as notificações internas: férias vencidas + períodos dentro do
/// alerta configurado ("prestes a vencer").
pub fn listar_notificacoes(conn: &Connection) -> Result<Vec<Notificacao>, String> {
    let alerta = obter_alerta_dias(conn)?;
    let mut notificacoes = Vec::new();

    for vencida in listar_vencidas(conn)? {
        notificacoes.push(Notificacao {
            id: format!("v-{}", vencida.id),
            tipo: "vencida".to_string(),
            titulo: "Férias vencidas".to_string(),
            mensagem: format!(
                "{} ({}) — vencidas há {} dia(s) (prazo {})",
                vencida.funcionario_nome,
                vencida.empresa_nome,
                vencida.dias_vencidas,
                data_br(&vencida.vencimento)
            ),
        });
    }

    for item in listar_a_vencer(conn, alerta)? {
        if item.dentro_alerta {
            let rotulo = if item.dias_restantes <= 0 {
                "vencem hoje".to_string()
            } else if item.dias_restantes == 1 {
                "vencem amanhã".to_string()
            } else {
                format!("vencem em {} dias", item.dias_restantes)
            };
            notificacoes.push(Notificacao {
                id: format!("a-{}", item.id),
                tipo: "alerta".to_string(),
                titulo: "Férias a vencer".to_string(),
                mensagem: format!(
                    "{} ({}) — {rotulo} ({})",
                    item.funcionario_nome,
                    item.empresa_nome,
                    data_br(&item.vencimento)
                ),
            });
        }
    }

    // Vencidas primeiro, depois alertas por prazo.
    notificacoes.sort_by_key(|n| n.tipo != "vencida");
    Ok(notificacoes)
}

// ═══════════════════ Helpers de data (sem dependência externa) ═══════════════════

/// Soma meses a uma data ISO (yyyy-mm-dd), ajustando o dia ao fim do mês
/// quando necessário (ex.: 31/01 + 1 mês = 28/02).
fn adicionar_meses(iso: &str, meses: i64) -> String {
    let (ano, mes, dia) = match parse_iso(iso) {
        Some(v) => v,
        None => return iso.to_string(),
    };

    let total = ano * 12 + (mes - 1) + meses;
    let ano_novo = total.div_euclid(12);
    let mes_novo = total.rem_euclid(12) + 1;
    let dia_maximo = dias_do_mes(ano_novo, mes_novo);

    // Dia com zero à esquerda (yyyy-mm-dd) — SQLite e comparações de texto
    // dependem disso para interpretar a data corretamente.
    format!("{ano_novo:04}-{mes_novo:02}-{:02}", dia.min(dia_maximo))
}

/// "yyyy-mm-dd" → (ano, mês, dia); `None` se inválido.
fn parse_iso(iso: &str) -> Option<(i64, i64, i64)> {
    let partes: Vec<&str> = iso.split('-').collect();
    if partes.len() != 3 {
        return None;
    }
    let ano = partes[0].parse::<i64>().ok()?;
    let mes = partes[1].parse::<i64>().ok()?;
    let dia = partes[2].parse::<i64>().ok()?;
    if !(1..=12).contains(&mes) || dia < 1 || dia > dias_do_mes(ano, mes) {
        return None;
    }
    Some((ano, mes, dia))
}

fn data_valida(iso: &str) -> bool {
    parse_iso(iso).is_some()
}

fn dias_do_mes(ano: i64, mes: i64) -> i64 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (ano % 4 == 0 && ano % 100 != 0) || ano % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// "yyyy-mm-dd" → "dd/mm/aaaa" (exibição).
fn data_br(iso: &str) -> String {
    match parse_iso(iso) {
        Some((ano, mes, dia)) => format!("{dia:02}/{mes:02}/{ano}"),
        None => iso.to_string(),
    }
}
