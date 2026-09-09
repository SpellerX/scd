//! Regras de negócio de funcionários (SCD).
//!
//! Cada funcionário é **vinculado a uma empresa** (`empresa_id`) — é essa
//! relação que permite controlar as férias por empresa nas próximas telas.
//! O CPF é **opcional**; quando informado, é padronizado para 11 dígitos,
//! validado (dígitos verificadores) e deve ser único.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::cnpj;

/// Funcionário devolvido ao frontend (já com o nome da empresa vinculada).
#[derive(Debug, Clone, Serialize)]
pub struct Funcionario {
    /// Id UUID (TEXT) — chave única global (sincronização com a nuvem).
    pub id: String,
    pub nome: String,
    pub cpf: Option<String>,
    pub data_admissao: Option<String>,
    /// Id UUID da empresa à qual o funcionário está vinculado.
    pub empresa_id: String,
    pub empresa_nome: String,
    pub created_at: String,
}

/// Resultado de uma importação de funcionários por planilha.
#[derive(Debug, Clone, Serialize)]
pub struct RelatorioImportacaoFuncionarios {
    /// Linhas de dados lidas (com nome preenchido).
    pub total: usize,
    /// Funcionários cadastrados.
    pub criados: usize,
    /// Linhas que falharam (com o motivo).
    pub erros: Vec<ErroImportacaoLinha>,
}

/// Uma linha que falhou na importação.
#[derive(Debug, Clone, Serialize)]
pub struct ErroImportacaoLinha {
    /// Número da linha na planilha (cabeçalho = 1).
    pub linha: usize,
    pub motivo: String,
}

/// Lista os funcionários (ordenados pelo nome), com o nome da empresa.
/// Exclui os apagados (soft delete) e os de empresas apagadas.
pub fn listar(conn: &Connection) -> Result<Vec<Funcionario>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT f.id, f.nome, f.cpf, f.data_admissao, f.empresa_id, c.razao_social,
                    f.created_at
               FROM employees f
               JOIN companies c ON c.id = f.empresa_id
              WHERE f.deleted_at IS NULL
                AND c.deleted_at IS NULL
              ORDER BY f.nome COLLATE NOCASE",
        )
        .map_err(|err| format!("Falha ao preparar a consulta de funcionários: {err}"))?;

    let funcionarios = stmt
        .query_map([], funcionario_da_linha)
        .map_err(|err| format!("Falha ao consultar funcionários: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler funcionários: {err}"))?;

    Ok(funcionarios)
}

/// Valida e cadastra um funcionário vinculado à empresa informada.
/// `cpf` pode ser vazio (opcional) — quando informado é validado.
pub fn inserir(
    conn: &Connection,
    nome: &str,
    cpf: &str,
    empresa_id: &str,
    data_admissao: Option<String>,
) -> Result<Funcionario, String> {
    let nome = nome.trim();
    if nome.is_empty() {
        return Err("Informe o nome do funcionário.".to_string());
    }

    let cpf = validar_cpf(cpf)?; // None = CPF não informado
    conferir_empresa(conn, empresa_id)?;

    // Data de admissão é OBRIGATÓRIA (base dos períodos de férias).
    let data_admissao = data_admissao
        .map(|data| data.trim().to_string())
        .filter(|data| !data.is_empty());
    let Some(data_admissao) = data_admissao else {
        return Err("A data de admissão é obrigatória.".to_string());
    };
    if !data_iso_valida(&data_admissao) {
        return Err(format!("Data de admissão inválida: {data_admissao}."));
    }

    // Id UUID gerado aqui (chave única global para a sincronização).
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, nome, cpf, data_admissao, empresa_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, nome, cpf, data_admissao, empresa_id],
    )
    .map_err(|err| {
        if cpf_duplicado(&err) {
            "Já existe um funcionário com este CPF.".to_string()
        } else {
            format!("Falha ao cadastrar o funcionário: {err}")
        }
    })?;

    buscar_por_id(conn, &id).and_then(|funcionario| {
        // Gera automaticamente os períodos de férias a partir da admissão
        // (cadastro manual e importação passam por aqui).
        if let Some(admissao) = &funcionario.data_admissao {
            crate::ferias::gerar_periodos_do_funcionario(conn, &funcionario.id, admissao)?;
        }
        Ok(funcionario)
    })
}

/// Remove um funcionário pelo id (soft delete: marca `deleted_at` nele e nos
/// períodos de férias dele, para a remoção poder ser sincronizada depois).
pub fn remover(conn: &Connection, id: &str) -> Result<(), String> {
    let removidos = conn
        .execute(
            "UPDATE employees
                SET deleted_at = datetime('now'),
                    updated_at = datetime('now')
              WHERE id = ?1 AND deleted_at IS NULL",
            [id],
        )
        .map_err(|err| format!("Falha ao excluir o funcionário: {err}"))?;

    if removidos == 0 {
        return Err("Funcionário não encontrado ou já removido.".to_string());
    }

    // Acompanha os períodos de férias do funcionário (soft delete em cascata).
    conn.execute(
        "UPDATE employee_leave_periods
            SET deleted_at = datetime('now'),
                updated_at = datetime('now')
          WHERE employee_id = ?1 AND deleted_at IS NULL",
        [id],
    )
    .map_err(|err| format!("Falha ao excluir as férias do funcionário: {err}"))?;

    Ok(())
}

/// A empresa tem funcionários vinculados ATIVOS? (usado ao excluir empresas)
pub fn empresa_tem_funcionarios(conn: &Connection, empresa_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM employees
                        WHERE empresa_id = ?1 AND deleted_at IS NULL)",
        [empresa_id],
        |linha| linha.get(0),
    )
    .map_err(|err| format!("Falha ao consultar funcionários da empresa: {err}"))
}

// ═══════════════════ Importação por planilha (.xls/.xlsx) ═══════════════════

/// Importa funcionários a partir das linhas da planilha (cabeçalho incluso).
///
/// Regras:
/// - coluna **nome**: cabeçalho contendo "nome" (obrigatória);
/// - coluna **admissão**: cabeçalho contendo "admiss" (OBRIGATÓRIA — base
///   para a geração automática dos períodos de férias);
/// - coluna **cpf**: cabeçalho contendo "cpf" (opcional; validado quando houver);
/// - coluna **cnpj** (empresa por linha): opcional — se existir, cada linha é
///   vinculada à empresa daquele CNPJ; linhas sem CNPJ (ou sem a coluna)
///   usam `empresa_padrao` escolhida na tela.
pub fn importar_linhas(
    conn: &Connection,
    linhas: &[Vec<String>],
    empresa_padrao: Option<String>,
) -> Result<RelatorioImportacaoFuncionarios, String> {
    let cabecalho = &linhas[0];

    let col_nome = achar_coluna(cabecalho, &["nome"]).ok_or_else(|| {
        "Planilha sem coluna de Nome (cabeçalho deve conter \"nome\").".to_string()
    })?;
    let col_cpf = achar_coluna(cabecalho, &["cpf"]);
    let col_admissao = achar_coluna(cabecalho, &["admiss"]).ok_or_else(|| {
        "Planilha sem coluna de Admissão (cabeçalho deve conter \"admissão\" ou \"admissao\")."
            .to_string()
    })?;
    let col_cnpj = achar_coluna(cabecalho, &["cnpj"]);

    if col_cnpj.is_none() && empresa_padrao.is_none() {
        return Err(
            "Informe a empresa de destino (seletor) ou inclua uma coluna de CNPJ na planilha."
                .to_string(),
        );
    }

    let mut relatorio = RelatorioImportacaoFuncionarios {
        total: 0,
        criados: 0,
        erros: Vec::new(),
    };

    // Linha 0 = cabeçalho; começamos da 1 (e linha real = índice + 1).
    for (indice, linha) in linhas.iter().enumerate().skip(1) {
        let numero_linha = indice + 1;

        let nome = linha.get(col_nome).cloned().unwrap_or_default();
        if nome.trim().is_empty() {
            continue; // linha vazia — ignorada em silêncio
        }
        relatorio.total += 1;

        // 1) Descobre a empresa da linha (CNPJ por linha > empresa padrão)
        let empresa_id =
            match resolver_empresa_da_linha(conn, linha, col_cnpj, empresa_padrao.as_deref()) {
                Ok(Some(id)) => id,
                Ok(None) => {
                    relatorio.erros.push(ErroImportacaoLinha {
                        linha: numero_linha,
                        motivo: "Linha sem empresa (informe o CNPJ ou a empresa padrão)."
                            .to_string(),
                    });
                    continue;
                }
                Err(motivo) => {
                    relatorio.erros.push(ErroImportacaoLinha {
                        linha: numero_linha,
                        motivo,
                    });
                    continue;
                }
            };

        // 2) CPF (opcional) — valida aqui para gerar erro de linha amigável
        //    (a inserção valida de novo internamente).
        let cpf_celula = col_cpf
            .and_then(|c| linha.get(c))
            .cloned()
            .unwrap_or_default();
        if let Err(motivo) = validar_cpf(&cpf_celula) {
            relatorio.erros.push(ErroImportacaoLinha {
                linha: numero_linha,
                motivo,
            });
            continue;
        }

        // 3) Data de admissão (OBRIGATÓRIA; vários formatos aceitos)
        let admissao_celula = linha.get(col_admissao).cloned().unwrap_or_default();
        let data_admissao = if admissao_celula.trim().is_empty() {
            relatorio.erros.push(ErroImportacaoLinha {
                linha: numero_linha,
                motivo: "Data de admissão não informada (obrigatória).".to_string(),
            });
            continue;
        } else {
            match data_de_celula(&admissao_celula) {
                Some(data) => data,
                None => {
                    relatorio.erros.push(ErroImportacaoLinha {
                        linha: numero_linha,
                        motivo: format!("Data de admissão inválida: \"{admissao_celula}\"."),
                    });
                    continue;
                }
            }
        };

        // 4) Cadastra
        match inserir(conn, &nome, &cpf_celula, &empresa_id, Some(data_admissao)) {
            Ok(_) => relatorio.criados += 1,
            Err(motivo) => relatorio.erros.push(ErroImportacaoLinha {
                linha: numero_linha,
                motivo,
            }),
        }
    }

    Ok(relatorio)
}

/// Resolve a empresa de uma linha: coluna de CNPJ (se houver e preenchida)
/// ou a empresa padrão escolhida na tela. Devolve o id UUID da empresa.
fn resolver_empresa_da_linha(
    conn: &Connection,
    linha: &[String],
    col_cnpj: Option<usize>,
    empresa_padrao: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(coluna) = col_cnpj {
        let cnpj_celula = linha.get(coluna).cloned().unwrap_or_default();
        if !cnpj_celula.trim().is_empty() {
            let cnpj = cnpj::normalizar_cnpj(&cnpj_celula)?;
            let empresa_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM companies
                      WHERE cnpj = ?1 AND deleted_at IS NULL",
                    [&cnpj],
                    |l| l.get(0),
                )
                .optional()
                .map_err(|err| format!("Falha ao consultar o CNPJ da empresa: {err}"))?;

            return match empresa_id {
                Some(id) => Ok(Some(id)),
                None => Err(format!(
                    "Empresa com CNPJ {cnpj} não está cadastrada. Cadastre-a antes."
                )),
            };
        }
    }
    Ok(empresa_padrao.map(str::to_string))
}

/// Acha o índice da coluna cujo cabeçalho contenha alguma das palavras.
fn achar_coluna(cabecalho: &[String], palavras: &[&str]) -> Option<usize> {
    cabecalho.iter().position(|celula| {
        let nome = celula.to_lowercase();
        palavras.iter().any(|palavra| nome.contains(palavra))
    })
}

// ═══════════════════ Helpers (validação e datas) ════════════════════════════

/// Confere se a empresa existe e está ativa (FK bloquearia, mas o erro
/// amigável é melhor).
fn conferir_empresa(conn: &Connection, empresa_id: &str) -> Result<(), String> {
    let existe: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM companies
                            WHERE id = ?1 AND deleted_at IS NULL)",
            [empresa_id],
            |linha| linha.get(0),
        )
        .map_err(|err| format!("Falha ao conferir a empresa: {err}"))?;
    if !existe {
        return Err(
            "A empresa escolhida não existe. Atualize a lista e tente de novo.".to_string(),
        );
    }
    Ok(())
}

/// Confere se a data está no formato ISO (yyyy-mm-dd) e é uma data real.
fn data_iso_valida(iso: &str) -> bool {
    let partes: Vec<&str> = iso.split('-').collect();
    if partes.len() != 3 {
        return false;
    }
    let (Ok(ano), Ok(mes), Ok(dia)) = (
        partes[0].parse::<i64>(),
        partes[1].parse::<i64>(),
        partes[2].parse::<i64>(),
    ) else {
        return false;
    };
    (1..=12).contains(&mes) && (1..=31).contains(&dia) && ano >= 1900 && ano <= 2200
}

/// Normaliza e valida o CPF. Vazio → `None` (CPF é opcional).
fn validar_cpf(raw: &str) -> Result<Option<String>, String> {
    let digitos: Vec<u8> = raw
        .chars()
        .filter_map(|c| c.to_digit(10).map(|d| d as u8))
        .collect();

    if digitos.is_empty() {
        return Ok(None);
    }
    if digitos.len() != 11 {
        return Err("CPF inválido — informe os 11 dígitos (ou deixe em branco).".to_string());
    }

    // Dígitos verificadores (mesma regra da Receita Federal).
    let digito = |pesos: &[u8]| -> u8 {
        let soma: u32 = pesos
            .iter()
            .zip(digitos.iter())
            .map(|(peso, digito)| (*peso as u32) * (*digito as u32))
            .sum();
        let resto = soma % 11;
        if resto < 2 {
            0
        } else {
            11 - resto as u8
        }
    };

    let pesos_dv1 = [10, 9, 8, 7, 6, 5, 4, 3, 2];
    let pesos_dv2 = [11, 10, 9, 8, 7, 6, 5, 4, 3, 2];

    if digito(&pesos_dv1) != digitos[9] || digito(&pesos_dv2) != digitos[10] {
        return Err("CPF inválido — os dígitos verificadores não conferem.".to_string());
    }

    Ok(Some(digitos.iter().map(|d| char::from(b'0' + d)).collect()))
}

/// Converte uma célula de data (texto ou número serial do Excel) para
/// `yyyy-mm-dd`. Retorna `None` quando o valor não parece data.
fn data_de_celula(celula: &str) -> Option<String> {
    let celula = celula.trim();

    // Número serial do Excel (dias desde 30/12/1899) — datas numéricas.
    if let Ok(dias) = celula.parse::<f64>() {
        if (15000.0..=60000.0).contains(&dias) {
            return excel_serial_para_data(dias);
        }
    }

    // dd/mm/aaaa, dd-mm-aaaa, aaaa-mm-dd
    let partes: Vec<&str> = celula
        .split(['/', '-', '.'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if partes.len() == 3 {
        let numero = |p: &str| p.parse::<u32>().ok();
        match (numero(partes[0]), numero(partes[1]), numero(partes[2])) {
            (Some(dia), Some(mes), Some(ano))
                if ano > 1000 && (1..=31).contains(&dia) && (1..=12).contains(&mes) =>
            {
                return Some(format!("{ano:04}-{mes:02}-{dia:02}"));
            }
            // ISO (aaaa-mm-dd)
            (Some(ano), Some(mes), Some(dia))
                if ano > 1000 && (1..=12).contains(&mes) && (1..=31).contains(&dia) =>
            {
                return Some(format!("{ano:04}-{mes:02}-{dia:02}"));
            }
            _ => {}
        }
    }
    None
}

/// Converte o número serial do Excel para `yyyy-mm-dd`.
/// (Data base do Excel: 30/12/1899; séries ≥ 61 já contemplam o "bug de 1900".)
fn excel_serial_para_data(serial: f64) -> Option<String> {
    let dias = serial.floor() as i64;
    let base = dias_desde_civil(1899, 12, 30);
    let (ano, mes, dia) = civil_desde_dias(base + dias)?;
    Some(format!("{ano:04}-{mes:02}-{dia:02}"))
}

/// Dias desde 1970-01-01 de uma data civil (algoritmo de Howard Hinnant).
fn dias_desde_civil(ano: i64, mes: i64, dia: i64) -> i64 {
    let ano = if mes <= 2 { ano - 1 } else { ano };
    let era = if ano >= 0 { ano } else { ano - 399 } / 400;
    let ano_da_era = ano - era * 400;
    let mes_ajustado = mes + (if mes > 2 { -3 } else { 9 });
    let dia_do_ano = (153 * mes_ajustado + 2) / 5 + dia - 1;
    let dia_da_era = ano_da_era * 365 + ano_da_era / 4 - ano_da_era / 100 + dia_do_ano;
    era * 146097 + dia_da_era - 719468
}

/// Converte dias desde 1970-01-01 em data civil `(ano, mes, dia)`.
fn civil_desde_dias(dias: i64) -> Option<(i64, i64, i64)> {
    let z = dias + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let dia_da_era = z - era * 146097;
    let ano_da_era =
        (dia_da_era - dia_da_era / 1460 + dia_da_era / 36524 - dia_da_era / 146096) / 365;
    let mut ano = ano_da_era + era * 400;
    let dia_do_ano = dia_da_era - (365 * ano_da_era + ano_da_era / 4 - ano_da_era / 100);
    let mes_ajustado = (5 * dia_do_ano + 2) / 153;
    let dia = dia_do_ano - (153 * mes_ajustado + 2) / 5 + 1;
    let mes = mes_ajustado + if mes_ajustado < 10 { 3 } else { -9 };
    if mes <= 2 {
        ano += 1;
    }
    if !(1..=12).contains(&mes) || dia < 1 || dia > 31 {
        return None;
    }
    Some((ano, mes, dia))
}

/// Mapeia uma linha do SQL no struct `Funcionario`.
fn funcionario_da_linha(linha: &rusqlite::Row) -> rusqlite::Result<Funcionario> {
    Ok(Funcionario {
        id: linha.get(0)?,
        nome: linha.get(1)?,
        cpf: linha.get(2)?,
        data_admissao: linha.get(3)?,
        empresa_id: linha.get(4)?,
        empresa_nome: linha.get(5)?,
        created_at: linha.get(6)?,
    })
}

/// Busca um funcionário pelo id UUID (recém-inserido ou não).
fn buscar_por_id(conn: &Connection, id: &str) -> Result<Funcionario, String> {
    conn.query_row(
        "SELECT f.id, f.nome, f.cpf, f.data_admissao, f.empresa_id, c.razao_social,
                f.created_at
           FROM employees f
           JOIN companies c ON c.id = f.empresa_id
          WHERE f.id = ?1 AND f.deleted_at IS NULL",
        [id],
        funcionario_da_linha,
    )
    .optional()
    .map_err(|err| format!("Falha ao carregar o funcionário salvo: {err}"))?
    .ok_or_else(|| "Funcionário salvo não encontrado.".to_string())
}

/// Identifica o erro de "CPF duplicado" vindo do SQLite (constraint UNIQUE).
fn cpf_duplicado(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _)
            if e.code == rusqlite::ErrorCode::ConstraintViolation
    )
}
