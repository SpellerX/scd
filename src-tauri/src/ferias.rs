//! Regras do controle de férias (SCD).
//!
//! Modelo simplificado do direito trabalhista:
//! - ao cadastrar o funcionário com **data de admissão**, o sistema gera
//!   automaticamente os períodos de férias (um por ano de trabalho);
//! - cada período tem um **vencimento** = início + 24 meses
//!   (12 meses aquisitivos + 12 meses para gozar);
//! - passou do vencimento sem regularizar ⇒ **férias vencidas**;
//! - dentro do prazo concessivo ⇒ **férias a vencer** (com alerta
//!   configurável N dias antes do vencimento);
//! - **alarme manual** (opcional, por período): quando a empresa JÁ agendou as
//!   férias do funcionário, o período entra em *Férias a vencer* na hora e o
//!   aviso dispara na data escolhida — sem esperar o alerta automático, que
//!   só começa 12 meses antes do vencimento. O alerta automático continua
//!   valendo normalmente para os períodos **sem** alarme.

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
    /// Entrou na janela automática (N dias antes do vencimento)?
    pub dentro_alerta: bool,
    /// O período está no prazo automático de exibição (12 meses antes do
    /// vencimento)? `false` = está na lista por causa do alarme manual.
    pub na_janela_automatica: bool,
    /// Alarme manual definido (férias agendadas pela empresa)?
    pub agendado: bool,
    /// Data em que o aviso deve aparecer (yyyy-mm-dd).
    pub alarme_em: Option<String>,
    /// Texto livre do agendamento (ex.: "agendadas de 10/03 a 05/04").
    pub alarme_observacao: Option<String>,
    /// Dias entre hoje e a data do alarme (negativo = atrasado).
    pub dias_para_alarme: Option<i64>,
    /// A data do alarme já chegou (o aviso está pendente)?
    pub alarme_disparado: bool,
}

/// Período de férias de um funcionário — usado na tela para escolher a qual
/// período o alarme manual será vinculado.
#[derive(Debug, Clone, Serialize)]
pub struct PeriodoDoFuncionario {
    /// Id UUID (TEXT) do período.
    pub id: String,
    pub inicio: String,
    pub vencimento: String,
    /// Dias até o vencimento (sempre ≥ 0: só devolvemos períodos a vencer).
    pub dias_restantes: i64,
    pub alarme_em: Option<String>,
    pub alarme_observacao: Option<String>,
}

/// Notificação interna (sino do cabeçalho).
#[derive(Debug, Clone, Serialize)]
pub struct Notificacao {
    /// Id estável usado pelo app para marcar como "vista" (v-<id>/a-<id>/g-<id>).
    pub id: String,
    /// "vencida", "alarme" (agendamento manual) ou "alerta" (prazo automático).
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
///
/// Entram na lista os períodos da **janela automática** (12 meses antes do
/// vencimento) e também os que têm **alarme manual** (`alarme_em`), mesmo
/// antes dessa janela: é o caso das empresas que já agendaram as férias e não
/// querem esperar o aviso automático.
pub fn listar_a_vencer(conn: &Connection, alerta_dias: i64) -> Result<Vec<FeriasAVencer>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, f.id, f.nome, c.razao_social, p.inicio, p.vencimento,
                    COALESCE(CAST(julianday(p.vencimento) - julianday('now','localtime') AS INTEGER), 0),
                    p.alarme_em,
                    p.alarme_observacao,
                    CAST(julianday(p.alarme_em) - julianday(date('now','localtime')) AS INTEGER),
                    (p.alarme_em IS NOT NULL AND p.alarme_em <= date('now','localtime')),
                    (date(p.vencimento, '-12 months') <= date('now','localtime'))
               FROM employee_leave_periods p
               JOIN employees f ON f.id = p.employee_id
               JOIN companies c ON c.id = f.empresa_id
              WHERE p.regularizada = 0
                AND p.vencimento > date('now','localtime')
                AND (date(p.vencimento, '-12 months') <= date('now','localtime')
                     OR p.alarme_em IS NOT NULL)
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
                na_janela_automatica: linha.get(11)?,
                agendado: false, // preenchido abaixo
                alarme_em: linha.get(7)?,
                alarme_observacao: linha.get(8)?,
                dias_para_alarme: linha.get(9)?,
                alarme_disparado: linha.get(10)?,
            })
        })
        .map_err(|err| format!("Falha ao consultar férias a vencer: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler férias a vencer: {err}"))?;

    Ok(linhas
        .into_iter()
        .map(|mut item| {
            item.dentro_alerta = item.dias_restantes <= alerta_dias;
            item.agendado = item.alarme_em.is_some();
            item
        })
        .collect())
}

/// Períodos de um funcionário que ainda podem ser agendados (a vencer e não
/// regularizados), do mais próximo ao mais distante do vencimento.
///
/// É a lista que a tela "Férias a vencer" mostra depois de escolher o
/// funcionário, para vincular o alarme ao período certo.
pub fn listar_periodos_do_funcionario(
    conn: &Connection,
    funcionario_id: &str,
) -> Result<Vec<PeriodoDoFuncionario>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.inicio, p.vencimento,
                    COALESCE(CAST(julianday(p.vencimento) - julianday('now','localtime') AS INTEGER), 0),
                    p.alarme_em,
                    p.alarme_observacao
               FROM employee_leave_periods p
               JOIN employees f ON f.id = p.employee_id
              WHERE p.employee_id = ?1
                AND p.regularizada = 0
                AND p.vencimento > date('now','localtime')
                AND p.deleted_at IS NULL
                AND f.deleted_at IS NULL
              ORDER BY p.vencimento",
        )
        .map_err(|err| format!("Falha ao preparar os períodos do funcionário: {err}"))?;

    let periodos = stmt
        .query_map([funcionario_id], |linha| {
            Ok(PeriodoDoFuncionario {
                id: linha.get(0)?,
                inicio: linha.get(1)?,
                vencimento: linha.get(2)?,
                dias_restantes: linha.get(3)?,
                alarme_em: linha.get(4)?,
                alarme_observacao: linha.get(5)?,
            })
        })
        .map_err(|err| format!("Falha ao consultar os períodos do funcionário: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler os períodos do funcionário: {err}"))?;

    Ok(periodos)
}

// ═══════════════════ Alarme manual ("férias agendadas") ═════════════════════

/// Define (ou substitui) o alarme manual de um período: a partir de
/// `alarme_em` o período passa a notificar, mesmo que o alerta automático
/// ainda não tenha começado.
///
/// Regras:
/// - a data precisa ser ISO válida (aaaa-mm-dd) e **não pode estar no
///   passado** — um alarme retroativo notificaria na hora e confundiria;
/// - o período precisa existir, estar ativo e **não estar regularizado**;
/// - período já vencido é recusado: nesse caso ele já aparece em
///   *Férias vencidas* (com notificação própria).
pub fn definir_alarme(
    conn: &Connection,
    periodo_id: &str,
    alarme_em: &str,
    observacao: Option<String>,
) -> Result<(), String> {
    let data = alarme_em.trim();
    if !data_valida(data) {
        return Err(format!(
            "Data do aviso inválida: \"{data}\". Informe uma data no formato aaaa-mm-dd."
        ));
    }

    let hoje = hoje_iso(conn)?;
    if data < hoje.as_str() {
        return Err(format!(
            "A data do aviso ({}) não pode estar no passado.",
            data_br(data)
        ));
    }

    let observacao = observacao
        .map(|texto| texto.trim().to_string())
        .filter(|texto| !texto.is_empty());

    let periodo: Option<(String, bool)> = conn
        .query_row(
            "SELECT vencimento, regularizada
               FROM employee_leave_periods
              WHERE id = ?1 AND deleted_at IS NULL",
            [periodo_id],
            |linha| Ok((linha.get(0)?, linha.get::<_, i64>(1)? != 0)),
        )
        .optional()
        .map_err(|err| format!("Falha ao conferir o período de férias: {err}"))?;

    let Some((vencimento, regularizada)) = periodo else {
        return Err("Período de férias não encontrado.".to_string());
    };
    if regularizada {
        return Err("Este período já foi regularizado — não há o que agendar.".to_string());
    }
    if vencimento.as_str() <= hoje.as_str() {
        return Err(
            "Este período já venceu: ele aparece em \"Férias vencidas\" (regularize por lá)."
                .to_string(),
        );
    }

    let alterados = conn
        .execute(
            "UPDATE employee_leave_periods
                SET alarme_em = ?1,
                    alarme_observacao = ?2,
                    updated_at = datetime('now')
              WHERE id = ?3 AND deleted_at IS NULL",
            params![data, observacao, periodo_id],
        )
        .map_err(|err| format!("Falha ao salvar o alarme das férias: {err}"))?;

    if alterados == 0 {
        return Err("Período de férias não encontrado.".to_string());
    }
    Ok(())
}

/// Remove o alarme manual de um período (o alerta automático volta a valer).
pub fn remover_alarme(conn: &Connection, periodo_id: &str) -> Result<(), String> {
    let alterados = conn
        .execute(
            "UPDATE employee_leave_periods
                SET alarme_em = NULL,
                    alarme_observacao = NULL,
                    updated_at = datetime('now')
              WHERE id = ?1 AND deleted_at IS NULL",
            [periodo_id],
        )
        .map_err(|err| format!("Falha ao remover o alarme das férias: {err}"))?;

    if alterados == 0 {
        return Err("Período de férias não encontrado.".to_string());
    }
    Ok(())
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

/// Monta as notificações internas: férias vencidas + alarmes manuais de
/// férias agendadas + períodos dentro do alerta automático configurado.
///
/// O alarme manual tem **prioridade sobre o alerta automático do MESMO
/// período** (evita dois avisos iguais sobre o mesmo fato): quem agendou
/// recebe o aviso na data escolhida. O alerta automático continua valendo
/// normalmente para todos os períodos **sem** alarme.
///
/// Períodos que passaram do vencimento saem desta lista e entram em
/// *Férias vencidas* (notificação `vencida`), que já é uma pendência por si só.
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
        // Férias agendadas pela empresa: avisa na data escolhida, sem esperar
        // a janela automática. Enquanto a data não chega, o período aparece
        // apenas na tela (com o selo "Agendado").
        if item.agendado {
            if !item.alarme_disparado {
                continue;
            }
            let data_alarme = item.alarme_em.as_deref().unwrap_or_default();
            notificacoes.push(Notificacao {
                id: format!("g-{}", item.id),
                tipo: "alarme".to_string(),
                titulo: "Férias agendadas".to_string(),
                mensagem: format!(
                    "{} ({}) — aviso agendado para {} (prazo final em {})",
                    item.funcionario_nome,
                    item.empresa_nome,
                    data_br(data_alarme),
                    data_br(&item.vencimento)
                ),
            });
            continue;
        }

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

    // Vencidas primeiro, depois os agendamentos do dia, depois os alertas de prazo.
    notificacoes.sort_by_key(|n| match n.tipo.as_str() {
        "vencida" => 0,
        "alarme" => 1,
        _ => 2,
    });
    Ok(notificacoes)
}

// ═══════════════════ Helpers de data (sem dependência externa) ═══════════════════

/// Data de hoje (yyyy-mm-dd) segundo o relógio local — mesma noção de "hoje"
/// usada pelas consultas SQL (`date('now','localtime')`).
fn hoje_iso(conn: &Connection) -> Result<String, String> {
    conn.query_row("SELECT date('now','localtime')", [], |linha| linha.get(0))
        .map_err(|err| format!("Falha ao ler a data atual: {err}"))
}

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

#[cfg(test)]
mod testes {
    //! Testes das regras de férias (alarme manual e alerta automático).
    //! Cada caso usa um arquivo temporário próprio, como nos testes de `db`.
    //!
    //! Os períodos são criados com datas RELATIVAS a hoje (via `date('now')`),
    //! para os testes continuarem válidos em qualquer data.

    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);

    const EMPRESA_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const FUNC_ID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

    fn caminho_temporario() -> PathBuf {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("scd-ferias-teste-{}-{n}.db", std::process::id()))
    }

    /// Banco com uma empresa e um funcionário, pronto para receber períodos.
    /// Devolve também o caminho do arquivo (apagado no fim do teste).
    fn banco_de_teste() -> (Connection, PathBuf) {
        let caminho = caminho_temporario();
        let conn = crate::db::open(&caminho).expect("abrir banco de teste falhou");
        conn.execute_batch(&format!(
            "INSERT INTO companies (id, cnpj, razao_social)
                 VALUES ('{EMPRESA_ID}', '12345678000199', 'Empresa Teste');
             INSERT INTO employees (id, nome, cpf, data_admissao, empresa_id)
                 VALUES ('{FUNC_ID}', 'Fulano', NULL, date('now','localtime','-2 years'),
                         '{EMPRESA_ID}');"
        ))
        .expect("montar empresa/funcionário falhou");
        (conn, caminho)
    }

    /// Cria um período de férias cujo vencimento fica a `meses_ate_vencer`
    /// meses de hoje (ex.: `18` = daqui a 18 meses, `-1` = vencido há 1 mês).
    /// O início é sempre 24 meses antes do vencimento (regra do sistema), o
    /// que também mantém os períodos distintos no índice (funcionário, início).
    fn inserir_periodo(conn: &Connection, id: &str, meses_ate_vencer: i64) {
        conn.execute(
            "INSERT INTO employee_leave_periods (id, employee_id, inicio, vencimento)
             VALUES (?1, ?2, date('now','localtime', ?3, '-24 months'),
                     date('now','localtime', ?3))",
            params![id, FUNC_ID, format!("{meses_ate_vencer:+} months")],
        )
        .expect("inserir período falhou");
    }

    /// Data de hoje + `dias` (yyyy-mm-dd), no fuso local.
    fn data_relativa(conn: &Connection, dias: i64) -> String {
        conn.query_row(
            "SELECT date('now','localtime', ?1)",
            [format!("{dias:+} days")],
            |linha| linha.get(0),
        )
        .expect("calcular data relativa falhou")
    }

    fn remover(caminho: &std::path::Path) {
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn alarme_traz_periodo_fora_da_janela_automatica_e_avisa_na_data() {
        let (conn, caminho) = banco_de_teste();
        const PERIODO: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

        // Vencimento em +18 meses ⇒ a janela automática (12 meses antes) só
        // começaria daqui a 6 meses: hoje o período não aparece em lugar algum.
        inserir_periodo(&conn, PERIODO, 18);
        assert!(
            listar_a_vencer(&conn, 15).unwrap().is_empty(),
            "sem alarme, o período ainda não deve aparecer"
        );
        assert!(listar_notificacoes(&conn).unwrap().is_empty());

        // Alarme para daqui a 5 dias: o período entra na lista JÁ, marcado como
        // agendado e ainda fora da janela automática, mas sem notificar.
        let aviso = data_relativa(&conn, 5);
        definir_alarme(&conn, PERIODO, &aviso, Some("gozo de 10/03 a 05/04".into())).unwrap();

        let lista = listar_a_vencer(&conn, 15).unwrap();
        assert_eq!(lista.len(), 1, "o período agendado deve aparecer na lista");
        let item = &lista[0];
        assert!(item.agendado, "deve estar marcado como agendado");
        assert!(!item.na_janela_automatica, "ainda fora da janela automática");
        assert_eq!(item.alarme_em.as_deref(), Some(aviso.as_str()));
        assert_eq!(
            item.alarme_observacao.as_deref(),
            Some("gozo de 10/03 a 05/04")
        );
        assert_eq!(item.dias_para_alarme, Some(5));
        assert!(!item.alarme_disparado, "o aviso ainda não chegou");
        assert!(
            listar_notificacoes(&conn).unwrap().is_empty(),
            "antes da data do alarme não há notificação"
        );

        // Chegando o dia escolhido, o aviso vira notificação de agendamento.
        let hoje = data_relativa(&conn, 0);
        definir_alarme(&conn, PERIODO, &hoje, Some("gozo de 10/03 a 05/04".into())).unwrap();
        let avisos = listar_notificacoes(&conn).unwrap();
        assert_eq!(avisos.len(), 1);
        assert_eq!(avisos[0].tipo, "alarme");
        assert_eq!(avisos[0].id, format!("g-{PERIODO}"));
        assert!(avisos[0].mensagem.contains("Fulano"));
        assert!(avisos[0].mensagem.contains("aviso agendado"));

        // Remover o alarme devolve o período à regra automática (some da lista).
        remover_alarme(&conn, PERIODO).unwrap();
        assert!(listar_a_vencer(&conn, 15).unwrap().is_empty());
        assert!(listar_notificacoes(&conn).unwrap().is_empty());

        remover(&caminho);
    }

    #[test]
    fn alerta_automatico_continua_valendo_para_os_periodos_sem_alarme() {
        let (conn, caminho) = banco_de_teste();
        const COM_ALARME: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        const SEM_ALARME: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";

        // Sem alarme, dentro da janela automática e do alerta de 15 dias.
        inserir_periodo(&conn, SEM_ALARME, 0);
        conn.execute(
            "UPDATE employee_leave_periods
                SET vencimento = date('now','localtime','+10 days') WHERE id = ?1",
            [SEM_ALARME],
        )
        .unwrap();
        // Com alarme, bem antes da janela automática.
        inserir_periodo(&conn, COM_ALARME, 18);
        definir_alarme(&conn, COM_ALARME, &data_relativa(&conn, 0), None).unwrap();

        let avisos = listar_notificacoes(&conn).unwrap();
        assert_eq!(avisos.len(), 2, "os dois períodos devem avisar: {avisos:?}");
        assert_eq!(avisos[0].tipo, "alarme", "agendamento vem antes do prazo");
        assert_eq!(avisos[0].id, format!("g-{COM_ALARME}"));
        assert_eq!(avisos[1].tipo, "alerta", "alerta automático segue valendo");
        assert_eq!(avisos[1].id, format!("a-{SEM_ALARME}"));

        // Agendar o período que estava no alerta automático: um único aviso
        // (do agendamento), sem repetir o mesmo fato duas vezes.
        definir_alarme(&conn, SEM_ALARME, &data_relativa(&conn, 0), None).unwrap();
        let avisos = listar_notificacoes(&conn).unwrap();
        assert_eq!(avisos.len(), 2);
        assert!(avisos.iter().all(|n| n.tipo == "alarme"), "sem duplicidade");
        assert!(
            !avisos.iter().any(|n| n.id == format!("a-{SEM_ALARME}")),
            "o alerta automático do período agendado não deve aparecer duas vezes"
        );

        remover(&caminho);
    }

    #[test]
    fn definir_alarme_recusa_data_invalida_passado_periodo_vencido_ou_regularizado() {
        let (conn, caminho) = banco_de_teste();
        const A_VENCER: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        const VENCIDO: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
        const REGULARIZADO: &str = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";

        inserir_periodo(&conn, A_VENCER, 18);
        inserir_periodo(&conn, VENCIDO, -1);
        inserir_periodo(&conn, REGULARIZADO, 19); // início distinto (índice único)
        conn.execute(
            "UPDATE employee_leave_periods SET regularizada = 1 WHERE id = ?1",
            [REGULARIZADO],
        )
        .unwrap();

        let erro = definir_alarme(&conn, A_VENCER, "31/12/2026", None).unwrap_err();
        assert!(erro.contains("inválida"), "mensagem inesperada: {erro}");

        let erro = definir_alarme(&conn, A_VENCER, &data_relativa(&conn, -1), None).unwrap_err();
        assert!(erro.contains("passado"), "mensagem inesperada: {erro}");

        let erro = definir_alarme(&conn, VENCIDO, &data_relativa(&conn, 1), None).unwrap_err();
        assert!(erro.contains("já venceu"), "mensagem inesperada: {erro}");

        let erro =
            definir_alarme(&conn, REGULARIZADO, &data_relativa(&conn, 1), None).unwrap_err();
        assert!(erro.contains("regularizado"), "mensagem inesperada: {erro}");

        let erro = definir_alarme(&conn, "id-inexistente", &data_relativa(&conn, 1), None)
            .unwrap_err();
        assert!(erro.contains("não encontrado"), "mensagem inesperada: {erro}");

        // Hoje é aceito (alarme que dispara imediatamente).
        let hoje = data_relativa(&conn, 0);
        assert!(definir_alarme(&conn, A_VENCER, &hoje, None).is_ok());

        remover(&caminho);
    }

    #[test]
    fn alarme_pode_ser_substituido_e_atualiza_o_updated_at() {
        let (conn, caminho) = banco_de_teste();
        const PERIODO: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        inserir_periodo(&conn, PERIODO, 18);

        // Marca o updated_at com uma data conhecida e antiga: assim o teste
        // prova que salvar/remover o alarme mexem no carimbo de alteração
        // (é isso que faz a sincronização enviar a mudança para a nuvem) —
        // sem depender do relógio (datetime('now') tem resolução de segundo).
        let envelhecer = || {
            conn.execute(
                "UPDATE employee_leave_periods
                    SET updated_at = '2000-01-01 00:00:00' WHERE id = ?1",
                [PERIODO],
            )
            .unwrap();
        };
        let ler_alarme = || -> (Option<String>, Option<String>, String) {
            conn.query_row(
                "SELECT alarme_em, alarme_observacao, updated_at
                   FROM employee_leave_periods WHERE id = ?1",
                [PERIODO],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .unwrap()
        };

        // 1) Definir o alarme: grava data, limpa a observação em branco
        //    (nada de string vazia no banco) e carimba o updated_at.
        envelhecer();
        let aviso = data_relativa(&conn, 3);
        definir_alarme(&conn, PERIODO, &aviso, Some("   ".to_string())).unwrap();

        let (alarme, observacao, atualizado_em) = ler_alarme();
        assert_eq!(alarme.as_deref(), Some(aviso.as_str()));
        assert_eq!(observacao, None, "observação em branco deve virar NULL");
        assert_ne!(
            atualizado_em, "2000-01-01 00:00:00",
            "o alarme precisa carimbar updated_at para ser sincronizado"
        );

        // 2) Definir de novo SUBSTITUI data e observação do mesmo período.
        envelhecer();
        let novo_aviso = data_relativa(&conn, 30);
        definir_alarme(&conn, PERIODO, &novo_aviso, Some("gozo de 10/04 a 05/05".into())).unwrap();

        let (alarme, observacao, atualizado_em) = ler_alarme();
        assert_eq!(alarme.as_deref(), Some(novo_aviso.as_str()));
        assert_eq!(observacao.as_deref(), Some("gozo de 10/04 a 05/05"));
        assert_ne!(atualizado_em, "2000-01-01 00:00:00");

        // 3) Remover limpa os dois campos e também carimba o updated_at.
        envelhecer();
        remover_alarme(&conn, PERIODO).unwrap();

        let (alarme, observacao, atualizado_em) = ler_alarme();
        assert_eq!(alarme, None, "remover o alarme limpa a data");
        assert_eq!(observacao, None, "remover o alarme limpa a observação");
        assert_ne!(
            atualizado_em, "2000-01-01 00:00:00",
            "remover o alarme também precisa ser sincronizado"
        );

        // Remover duas vezes continua funcionando (idempotente na prática).
        remover_alarme(&conn, PERIODO).unwrap();

        remover(&caminho);
    }

    #[test]
    fn periodos_do_funcionario_trazem_apenas_os_agendaveis() {
        let (conn, caminho) = banco_de_teste();
        const A_VENCER: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        const VENCIDO: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";

        inserir_periodo(&conn, A_VENCER, 18);
        inserir_periodo(&conn, VENCIDO, -1);

        let periodos = listar_periodos_do_funcionario(&conn, FUNC_ID).unwrap();
        assert_eq!(periodos.len(), 1, "apenas o período a vencer é agendável");
        assert_eq!(periodos[0].id, A_VENCER);
        assert!(periodos[0].dias_restantes > 0);
        assert_eq!(periodos[0].alarme_em, None);

        // Depois de regularizado, o período sai das opções de agendamento.
        conn.execute(
            "UPDATE employee_leave_periods SET regularizada = 1 WHERE id = ?1",
            [A_VENCER],
        )
        .unwrap();
        assert!(listar_periodos_do_funcionario(&conn, FUNC_ID)
            .unwrap()
            .is_empty());

        // Funcionário sem períodos devolve lista vazia (não é erro).
        assert!(listar_periodos_do_funcionario(&conn, "id-inexistente")
            .unwrap()
            .is_empty());

        remover(&caminho);
    }
}
