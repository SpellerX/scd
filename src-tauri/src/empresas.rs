//! Regras de negócio do cadastro de empresas (SCD).
//!
//! Esta camada conhece SQLite e os módulos `cnpj`/`planilha` — não conhece
//! o Tauri. Os comandos em `commands/empresas.rs` só traduzem as chamadas.

use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::cnpj::{self, DadosEmpresa};

/// Empresa cadastrada, devolvida ao frontend.
#[derive(Debug, Clone, Serialize)]
pub struct Empresa {
    pub id: i64,
    pub cnpj: String,
    pub razao_social: String,
    pub nome_fantasia: Option<String>,
    pub situacao: Option<String>,
    pub municipio: Option<String>,
    pub uf: Option<String>,
    pub telefone: Option<String>,
    pub email: Option<String>,
    pub created_at: String,
}

/// Uma linha que falhou na importação.
#[derive(Debug, Clone, Serialize)]
pub struct ErroImportacao {
    /// Número da linha na planilha (começando em 1, cabeçalho incluso).
    pub linha: usize,
    pub motivo: String,
}

/// Resumo do resultado de uma importação por planilha.
#[derive(Debug, Clone, Serialize)]
pub struct RelatorioImportacao {
    pub total: usize,
    pub criadas: usize,
    pub duplicadas: usize,
    pub erros: Vec<ErroImportacao>,
}

/// Progresso da importação, enviado ao frontend a cada linha processada.
#[derive(Debug, Clone, Serialize)]
pub struct ProgressoImportacao {
    /// Número da linha sendo processada (cabeçalho = linha 1).
    pub linha: usize,
    /// Quantas linhas de dados já foram percorridas (incluindo a atual).
    pub processadas: usize,
    /// Total de linhas de dados da planilha.
    pub total: usize,
}

/// Lista as empresas cadastradas (ordenadas pelo nome).
pub fn listar(conn: &Connection) -> Result<Vec<Empresa>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, cnpj, razao_social, nome_fantasia, situacao, municipio,
                    uf, telefone, email, created_at
               FROM companies
              ORDER BY razao_social COLLATE NOCASE",
        )
        .map_err(|err| format!("Falha ao preparar a consulta: {err}"))?;

    let empresas = stmt
        .query_map([], |linha| empresa_da_linha(linha))
        .map_err(|err| format!("Falha ao consultar empresas: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler empresas: {err}"))?;

    Ok(empresas)
}

/// Confere se um CNPJ (normalizado) já está cadastrado.
pub fn existe(conn: &Connection, cnpj: &str) -> Result<bool, String> {
    conn.query_row("SELECT 1 FROM companies WHERE cnpj = ?1", [cnpj], |_| Ok(()))
        .optional()
        .map(|opt| opt.is_some())
        .map_err(|err| format!("Falha ao consultar o CNPJ: {err}"))
}

/// Remove uma empresa cadastrada pelo id.
/// Recusa se a empresa tiver funcionários vinculados ou se o id não existir.
pub fn remover(conn: &Connection, id: i64) -> Result<(), String> {
    if crate::funcionarios::empresa_tem_funcionarios(conn, id)? {
        return Err(
            "Esta empresa possui funcionários vinculados. Exclua ou mova os funcionários antes."
                .to_string(),
        );
    }

    let removidas = conn
        .execute("DELETE FROM companies WHERE id = ?1", [id])
        .map_err(|err| format!("Falha ao excluir a empresa: {err}"))?;

    if removidas == 0 {
        return Err("Empresa não encontrada ou já removida.".to_string());
    }
    Ok(())
}

/// Resumo de uma exclusão em lote de empresas.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResultadoRemocao {
    /// Empresas efetivamente removidas.
    pub removidas: usize,
    /// Empresas mantidas por terem funcionários vinculados (ou inexistentes).
    pub bloqueadas: usize,
}

/// Remove várias empresas de uma vez (exclusão em lote).
/// Empresas com funcionários vinculados são **puladas** (não quebram o lote);
/// tudo roda numa única transação.
pub fn remover_varias(conn: &mut Connection, ids: &[i64]) -> Result<ResultadoRemocao, String> {
    if ids.is_empty() {
        return Ok(ResultadoRemocao { removidas: 0, bloqueadas: 0 });
    }

    let transacao = conn
        .transaction()
        .map_err(|err| format!("Falha ao iniciar a exclusão em lote: {err}"))?;

    let mut removidas = 0usize;
    let mut bloqueadas = 0usize;

    for &id in ids {
        if crate::funcionarios::empresa_tem_funcionarios(&transacao, id)? {
            bloqueadas += 1;
            continue;
        }

        let excluida = transacao
            .execute("DELETE FROM companies WHERE id = ?1", [id])
            .map_err(|err| format!("Falha ao excluir a empresa {id}: {err}"))?;

        if excluida == 0 {
            bloqueadas += 1; // id inexistente — não quebra o lote
        } else {
            removidas += 1;
        }
    }

    transacao
        .commit()
        .map_err(|err| format!("Falha ao finalizar a exclusão em lote: {err}"))?;

    Ok(ResultadoRemocao { removidas, bloqueadas })
}

/// Grava uma empresa nova (CNPJ já deve ter sido conferido como inexistente).
pub fn inserir(conn: &Connection, dados: &DadosEmpresa) -> Result<Empresa, String> {
    conn.execute(
        "INSERT INTO companies
            (cnpj, razao_social, nome_fantasia, situacao, municipio, uf, telefone, email)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            dados.cnpj,
            dados.razao_social,
            dados.nome_fantasia,
            dados.situacao,
            dados.municipio,
            dados.uf,
            dados.telefone,
            dados.email
        ],
    )
    .map_err(|err| format!("Falha ao salvar a empresa: {err}"))?;

    buscar_por_id(conn, conn.last_insert_rowid())
}

/// Busca uma empresa pelo id (usado logo após o INSERT).
fn buscar_por_id(conn: &Connection, id: i64) -> Result<Empresa, String> {
    conn.query_row(
        "SELECT id, cnpj, razao_social, nome_fantasia, situacao, municipio,
                uf, telefone, email, created_at
           FROM companies WHERE id = ?1",
        [id],
        empresa_da_linha,
    )
    .map_err(|err| format!("Falha ao carregar a empresa salva: {err}"))
}

/// Converte uma linha do SQL no struct `Empresa`.
fn empresa_da_linha(linha: &rusqlite::Row) -> rusqlite::Result<Empresa> {
    Ok(Empresa {
        id: linha.get(0)?,
        cnpj: linha.get(1)?,
        razao_social: linha.get(2)?,
        nome_fantasia: linha.get(3)?,
        situacao: linha.get(4)?,
        municipio: linha.get(5)?,
        uf: linha.get(6)?,
        telefone: linha.get(7)?,
        email: linha.get(8)?,
        created_at: linha.get(9)?,
    })
}

/// Padronização final dos dados de uma empresa antes de gravar
/// (vale para a BrasilAPI e para a planilha): CNPJ com 14 dígitos,
/// textos sem espaços nas pontas e campos vazios viram `None`.
pub fn normalizar_dados(dados: DadosEmpresa) -> Result<DadosEmpresa, String> {
    let cnpj = cnpj::normalizar_cnpj(&dados.cnpj)?;

    let razao_social = dados.razao_social.trim();
    if razao_social.is_empty() {
        return Err("Razão social não informada.".to_string());
    }

    let opcional = |texto: Option<String>| {
        texto.and_then(|v| {
            let v = v.trim().to_string();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        })
    };

    Ok(DadosEmpresa {
        cnpj,
        razao_social: razao_social.to_string(),
        nome_fantasia: opcional(dados.nome_fantasia),
        situacao: opcional(dados.situacao),
        municipio: opcional(dados.municipio),
        uf: opcional(dados.uf),
        telefone: opcional(dados.telefone),
        email: opcional(dados.email),
    })
}

/// Intervalo mínimo entre consultas à BrasilAPI (limite ~3 por segundo).
const ESPERA_ENTRE_CONSULTAS: Duration = Duration::from_millis(400);

/// Importa empresas a partir das linhas lidas da planilha (cabeçalho incluso).
///
/// Regras da importação:
/// - coluna de CNPJ: cabeçalho contendo "cnpj" (obrigatória);
/// - colunas opcionais: razão social (cabeçalho com "raz") e fantasia ("fantas");
/// - se a linha já tem razão social, salva direto;
/// - sem razão social, consulta as fontes públicas de CNPJ (`cnpj.rs`).
///
/// A cada linha, `on_progress` é chamado para o frontend mostrar o andamento
/// (importações com consulta externa podem demorar — feedback é essencial).
pub async fn importar_linhas<F>(
    db: &Mutex<Connection>,
    http: &reqwest::Client,
    linhas: &[Vec<String>],
    mut on_progress: F,
) -> Result<RelatorioImportacao, String>
where
    F: FnMut(ProgressoImportacao) + Send,
{
    let cabecalho = &linhas[0];

    // Total de linhas de dados (a 1ª linha é o cabeçalho).
    let total_linhas = linhas.len().saturating_sub(1);

    // Localiza as colunas pelo nome (case-insensitive).
    let col_cnpj = achar_coluna(cabecalho, &["cnpj"])
        .ok_or_else(|| "Planilha sem coluna de CNPJ (cabeçalho deve conter \"cnpj\").".to_string())?;
    let col_razao = achar_coluna(cabecalho, &["raz"]);
    let col_fantasia = achar_coluna(cabecalho, &["fantas"]);

    let mut relatorio = RelatorioImportacao {
        total: 0,
        criadas: 0,
        duplicadas: 0,
        erros: Vec::new(),
    };
    // Marca se alguma consulta à API já foi feita (para respeitar o intervalo).
    let mut ja_consultou = false;

    // Linha 0 = cabeçalho; começamos da 1 (e linha real = índice + 1).
    for (indice, linha) in linhas.iter().enumerate().skip(1) {
        let numero_linha = indice + 1;

        // Informa o andamento (linha atual ainda não processada).
        on_progress(ProgressoImportacao {
            linha: numero_linha,
            processadas: indice,
            total: total_linhas,
        });

        let cnpj_celula = linha.get(col_cnpj).cloned().unwrap_or_default();
        let cnpj = match normalizar_cnpj_planilha(&cnpj_celula) {
            Ok(cnpj) => cnpj,
            Err(motivo) => {
                // Linha totalmente vazia é ignorada em silêncio.
                if cnpj_celula.trim().is_empty() {
                    continue;
                }
                relatorio.erros.push(ErroImportacao { linha: numero_linha, motivo });
                continue;
            }
        };

        relatorio.total += 1;

        // Já cadastrada? Conta como duplicada e segue para a próxima linha.
        let ja_existe = {
            let conn = db.lock().unwrap();
            existe(&conn, &cnpj)?
        };
        if ja_existe {
            relatorio.duplicadas += 1;
            continue;
        }

        // Monta os dados: se a planilha trouxe a razão social, não consulta a API.
        let razao = col_razao
            .and_then(|c| linha.get(c))
            .cloned()
            .unwrap_or_default();
        let fantasia = col_fantasia
            .and_then(|c| linha.get(c))
            .cloned()
            .unwrap_or_default();

        let dados = if razao.trim().is_empty() {
            // Consulta a BrasilAPI (com pausa para respeitar o limite de uso).
            if ja_consultou {
                tokio::time::sleep(ESPERA_ENTRE_CONSULTAS).await;
            }
            ja_consultou = true;
            match cnpj::buscar_por_cnpj(http, &cnpj).await {
                Ok(dados) => dados,
                Err(motivo) => {
                    relatorio
                        .erros
                        .push(ErroImportacao { linha: numero_linha, motivo });
                    continue;
                }
            }
        } else {
            DadosEmpresa {
                cnpj: cnpj.clone(),
                razao_social: razao.trim().to_string(),
                nome_fantasia: opcional(&fantasia),
                situacao: None,
                municipio: None,
                uf: None,
                telefone: None,
                email: None,
            }
        };

        // Padronização final (idem para dados da API e da planilha).
        let dados = match normalizar_dados(dados) {
            Ok(dados) => dados,
            Err(motivo) => {
                relatorio
                    .erros
                    .push(ErroImportacao { linha: numero_linha, motivo });
                continue;
            }
        };

        let conn = db.lock().unwrap();
        match inserir(&conn, &dados) {
            Ok(_) => relatorio.criadas += 1,
            Err(motivo) => relatorio
                .erros
                .push(ErroImportacao { linha: numero_linha, motivo }),
        }
    }

    Ok(relatorio)
}

/// Acha o índice da coluna cujo cabeçalho contenha alguma das palavras.
fn achar_coluna(cabecalho: &[String], palavras: &[&str]) -> Option<usize> {
    cabecalho.iter().position(|celula| {
        let nome = celula.to_lowercase();
        palavras.iter().any(|palavra| nome.contains(palavra))
    })
}

/// Normaliza um CNPJ vindo da planilha, restaurando zeros à esquerda perdidos
/// quando a célula era numérica (12 ou 13 dígitos ⇒ completa para 14) e
/// conferindo os dígitos verificadores **sem consultar a API**.
fn normalizar_cnpj_planilha(celula: &str) -> Result<String, String> {
    let digitos: String = celula.chars().filter(|c| c.is_ascii_digit()).collect();
    if digitos.is_empty() {
        return Err("Linha sem CNPJ.".to_string());
    }

    let normalizado = match digitos.len() {
        14 => digitos,
        // Zeros à esquerda podem ter sido "comidos" pela célula numérica.
        12 | 13 => format!("{:0>14}", digitos),
        _ => return Err(format!("CNPJ com {} dígito(s) — esperado 14.", digitos.len())),
    };

    if !crate::cnpj::validar_cnpj(&normalizado) {
        return Err(format!("CNPJ {celula} inválido (dígitos verificadores não conferem)."));
    }
    Ok(normalizado)
}

/// `Some(texto)` se não vazio; senão `None` (padronização dos dados).
fn opcional(texto: &str) -> Option<String> {
    let texto = texto.trim();
    if texto.is_empty() {
        None
    } else {
        Some(texto.to_string())
    }
}
