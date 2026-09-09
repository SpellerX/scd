//! Camada de dados: abre o banco SQLite e garante o esquema na versão atual.
//!
//! O arquivo do banco fica no diretório de dados do aplicativo
//! (resolvido no `lib.rs`). Para "resetar" o banco em desenvolvimento,
//! apague o arquivo `scd.db` — o esquema e o usuário de demonstração
//! são recriados automaticamente na próxima inicialização.
//!
//! # Versões de esquema
//!
//! Diferente de um sistema de migrações "de verdade", a versão do esquema
//! é guardada no próprio banco via `PRAGMA user_version`:
//!
//! - `0` = banco antigo/sem versão (criado antes do versionamento);
//! - `1` = formato atual (`employees.cpf` é opcional).
//!
//! A cada inicialização o app confere a versão gravada e aplica, em ordem,
//! apenas as migrações em falta. Isso substitui o antigo "adivinhar pelo
//! formato da coluna" e evita dois problemas vistos em produção:
//!
//! 1. um banco **mais novo que o app** (ex.: restaurado de um backup de uma
//!    versão futura) era aberto silenciosamente — agora a inicialização
//!    falha com mensagem clara, em vez de corromper/ignorar o esquema;
//! 2. mudanças de esquema feitas à mão fora do app (backup + edição do
//!    arquivo) deixavam o banco sem registro de versão — o app agora
//!    reconcilia e grava a versão, para que a próxima migração saiba
//!    exatamente de onde partir.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

/// Versão de esquema entendida por ESTE binário.
///
/// Toda mudança de esquema deve:
/// 1. criar uma migração em `migracao_para()` para a versão seguinte;
/// 2. atualizar esta constante;
/// 3. manter `init_schema()` criando o esquema FINAL (bancos novos já
///    nascem na última versão, sem passar pelas migrações).
pub const VERSAO_ATUAL: i64 = 1;

/// Abre (ou cria) o banco no caminho informado e leva o esquema à versão atual.
pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|err| format!("Falha ao abrir o arquivo do banco: {err}"))?;

    // Chaves estrangeiras ON: DELETE de empresa com funcionários é bloqueado
    // pelo banco (além da checagem amigável feita na camada de regras).
    // busy_timeout: se outro processo estiver escrevendo no mesmo arquivo
    // (ex.: manutenção externa com sqlite3), o app espera em vez de falhar
    // com "database is locked".
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|err| format!("Falha ao ativar chaves estrangeiras: {err}"))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|err| format!("Falha ao definir o tempo de espera do banco: {err}"))?;

    init_schema(&conn).map_err(|err| format!("Falha ao garantir o esquema inicial: {err}"))?;
    migrar(&conn).map_err(|err| format!("Falha ao migrar o banco de dados: {err}"))?;

    Ok(conn)
}

/// Versão de esquema gravada no banco (`PRAGMA user_version`; 0 se nunca
/// foi gravada — bancos criados antes do versionamento).
fn versao_do_banco(conn: &Connection) -> rusqlite::Result<i64> {
    conn.pragma_query_value(None, "user_version", |linha| linha.get(0))
}

/// Grava a versão de esquema no banco.
fn gravar_versao(conn: &Connection, versao: i64) -> rusqlite::Result<()> {
    conn.pragma_update(None, "user_version", versao)
}

/// Esquema inicial do banco — SEMPRE o esquema final (última versão).
///
/// Statements **idempotentes** (`CREATE TABLE IF NOT EXISTS`), para que a
/// inicialização continue funcionando com bancos já existentes. Bancos
/// antigos têm suas diferenças resolvidas por `migrar()`, não aqui.
fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT    NOT NULL UNIQUE,
            display_name  TEXT    NOT NULL,
            password_hash TEXT    NOT NULL,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS companies (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            cnpj           TEXT    NOT NULL UNIQUE,
            razao_social   TEXT    NOT NULL,
            nome_fantasia  TEXT,
            situacao       TEXT,
            municipio      TEXT,
            uf             TEXT,
            telefone       TEXT,
            email          TEXT,
            created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
        );

        -- Funcionários vinculados a uma empresa (base para o controle de férias).
        -- CPF é OPCIONAL (pode vir de planilha sem CPF); quando informado,
        -- é padronizado com os 11 dígitos e deve ser único.
        CREATE TABLE IF NOT EXISTS employees (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            nome           TEXT    NOT NULL,
            cpf            TEXT    UNIQUE,
            data_admissao  TEXT,
            empresa_id     INTEGER NOT NULL REFERENCES companies(id),
            created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
        );

        -- Sessões persistentes (Manter-me conectado): token criado no
        -- login e conferido ao reabrir o app. O usuário continua logado.
        CREATE TABLE IF NOT EXISTS sessions (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id    INTEGER NOT NULL REFERENCES users(id),
            token      TEXT    NOT NULL UNIQUE,
            created_at TEXT    NOT NULL DEFAULT (datetime('now'))
        );

        -- Acessos por usuário: lista de ids de seção (itens do menu) que o
        -- usuário pode ver. O usuário admin tem acesso total e não usa
        -- esta tabela. Remover o usuário apaga os acessos (CASCADE).
        CREATE TABLE IF NOT EXISTS user_permissions (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            section_id TEXT    NOT NULL,
            UNIQUE (user_id, section_id)
        );

        -- Períodos de férias: gerados automaticamente a partir da data de
        -- admissão do funcionário (um período por ano). Convenção:
        --   inicio     = início do período aquisitivo (ex.: data de admissão);
        --   vencimento = fim do prazo concessivo (inicio + 24 meses) —
        --                após essa data o período fica VENCIDO.
        -- Regularizada = o funcionário já gozou/quitou aquele período.
        CREATE TABLE IF NOT EXISTS employee_leave_periods (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            employee_id   INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
            inicio        TEXT    NOT NULL,
            vencimento    TEXT    NOT NULL,
            regularizada  INTEGER NOT NULL DEFAULT 0,
            regularizada_em TEXT,
            observacao    TEXT,
            UNIQUE (employee_id, inicio)
        );

        -- Configurações gerais (chave → valor).
        CREATE TABLE IF NOT EXISTS app_settings (
            chave TEXT PRIMARY KEY,
            valor TEXT NOT NULL
        );",
    )?;
    Ok(())
}

/// Aplica, em ordem, as migrações pendentes até `VERSAO_ATUAL`.
///
/// Cada versão aplicada é gravada no banco imediatamente após concluir, para
/// que uma interrupção no meio não reexecute migrações já concluídas.
fn migrar(conn: &Connection) -> Result<(), String> {
    let versao = versao_do_banco(conn).map_err(|err| format!("Falha ao ler a versão do banco: {err}"))?;

    if versao > VERSAO_ATUAL {
        return Err(format!(
            "O banco de dados está na versão {versao}, mais nova que a suportada por este \
             aplicativo ({VERSAO_ATUAL}). Ele foi criado/atualizado por uma versão mais recente \
             do SCD. Atualize o aplicativo ou restaure um backup compatível."
        ));
    }

    for alvo in (versao + 1)..=VERSAO_ATUAL {
        migracao_para(conn, alvo)?;
        gravar_versao(conn, alvo).map_err(|err| {
            format!("Falha ao gravar a versão {alvo} no banco (migração concluída, mas sem registro): {err}")
        })?;
    }
    Ok(())
}

/// Executa a migração que leva o banco da versão `versao - 1` para `versao`.
///
/// Migrações devem ser **idempotentes na prática**: se o banco já estiver no
/// formato alvo (ex.: arquivo normalizado à mão), a migração não altera dados
/// e apenas registra a versão.
fn migracao_para(conn: &Connection, versao: i64) -> Result<(), String> {
    match versao {
        // v0 → v1: CPF de funcionário deixou de ser obrigatório.
        // (versões antigas criaram `employees.cpf TEXT NOT NULL UNIQUE`).
        1 => {
            if coluna_cpf_e_obrigatoria(conn)? {
                conn.execute_batch(
                    "ALTER TABLE employees RENAME TO employees_antigos;

                     CREATE TABLE employees (
                         id             INTEGER PRIMARY KEY AUTOINCREMENT,
                         nome           TEXT    NOT NULL,
                         cpf            TEXT    UNIQUE,
                         data_admissao  TEXT,
                         empresa_id     INTEGER NOT NULL REFERENCES companies(id),
                         created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
                     );

                     INSERT INTO employees (id, nome, cpf, data_admissao, empresa_id, created_at)
                        SELECT id, nome, cpf, data_admissao, empresa_id, created_at
                          FROM employees_antigos;

                     DROP TABLE employees_antigos;",
                )
                .map_err(|err| format!("Falha ao migrar o formato antigo de employees: {err}"))?;
            }
            // Banco já no formato final (nullable) → nada a fazer; a versão
            // é gravada pelo chamador. Cobre bancos normalizados fora do app.
            Ok(())
        }
        _ => Err(format!("Migração para a versão {versao} não implementada.")),
    }
}

/// A coluna `employees.cpf` ainda é NOT NULL (formato antigo)?
fn coluna_cpf_e_obrigatoria(conn: &Connection) -> Result<bool, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(employees)")
        .map_err(|err| format!("Falha ao inspecionar a tabela employees: {err}"))?;

    let linhas = stmt
        .query_map([], |linha| {
            // Colunas de PRAGMA table_info: cid, name, type, notnull, dflt_value, pk
            let nome: String = linha.get(1)?;
            let notnull: i64 = linha.get(3)?;
            Ok((nome, notnull))
        })
        .map_err(|err| format!("Falha ao consultar a tabela employees: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler a tabela employees: {err}"))?;

    Ok(linhas.iter().any(|(nome, notnull)| nome == "cpf" && *notnull != 0))
}

#[cfg(test)]
mod testes {
    //! Testes da camada de banco: cada caso usa um arquivo temporário
    //! próprio, para não tocar no banco real do usuário.

    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);

    /// Caminho temporário único por teste (limpo ao final do teste).
    fn caminho_temporario(nome: &str) -> std::path::PathBuf {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "scd-teste-{nome}-{}-{n}.db",
            std::process::id()
        ))
    }

    fn remover(caminho: &std::path::Path) {
        let _ = std::fs::remove_file(caminho);
        // Arquivos auxiliares que o SQLite possa ter criado.
        let _ = std::fs::remove_file(caminho.with_extension("db-wal"));
        let _ = std::fs::remove_file(caminho.with_extension("db-shm"));
    }

    fn cpf_e_opcional(conn: &Connection) -> bool {
        coluna_cpf_e_obrigatoria(conn).expect("inspeção falhou") == false
    }

    #[test]
    fn banco_novo_nasce_na_versao_atual() {
        let caminho = caminho_temporario("novo");
        let conn = open(&caminho).expect("abrir banco novo falhou");

        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        assert!(cpf_e_opcional(&conn), "employees.cpf deve nascer opcional");

        remover(&caminho);
    }

    #[test]
    fn banco_legado_migra_preservando_dados() {
        let caminho = caminho_temporario("legado");

        // Monta um banco no FORMATO ANTIGO (employees.cpf NOT NULL), com
        // dados reais que precisam sobreviver à migração.
        let conn = Connection::open(&caminho).expect("criar banco legado falhou");
        conn.execute_batch(
            "CREATE TABLE companies (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                cnpj           TEXT    NOT NULL UNIQUE,
                razao_social   TEXT    NOT NULL,
                nome_fantasia  TEXT,
                situacao       TEXT,
                municipio      TEXT,
                uf             TEXT,
                telefone       TEXT,
                email          TEXT,
                created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE employees (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                nome           TEXT    NOT NULL,
                cpf            TEXT    NOT NULL UNIQUE,
                data_admissao  TEXT,
                empresa_id     INTEGER NOT NULL REFERENCES companies(id),
                created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO companies (cnpj, razao_social) VALUES ('12345678000199', 'Empresa Teste');
            INSERT INTO employees (nome, cpf, data_admissao, empresa_id)
                VALUES ('Fulano', '52998224725', '2023-01-02', 1);",
        )
        .expect("montar banco legado falhou");
        drop(conn);

        // `open` deve detectar o formato antigo e migrar.
        let conn = open(&caminho).expect("migração do banco legado falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        assert!(cpf_e_opcional(&conn), "cpf deve ter virado opcional");

        // Dados preservados intactos.
        let (nome, cpf): (String, Option<String>) = conn
            .query_row(
                "SELECT nome, cpf FROM employees WHERE id = 1",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .expect("funcionário migrado sumiu");
        assert_eq!(nome, "Fulano");
        assert_eq!(cpf.as_deref(), Some("52998224725"));

        // Reabrir de novo NÃO pode reexecutar a migração nem perder nada.
        drop(conn);
        let conn = open(&caminho).expect("segunda abertura falhou");
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM employees", [], |l| l.get(0))
            .unwrap();
        assert_eq!(total, 1, "abrir de novo não pode duplicar/perder dados");

        remover(&caminho);
    }

    #[test]
    fn banco_no_formato_atual_nao_altera_nada() {
        let caminho = caminho_temporario("atual");
        let conn = open(&caminho).expect("abrir banco falhou");
        conn.execute_batch(
            "INSERT INTO companies (cnpj, razao_social) VALUES ('12345678000199', 'Empresa Atual');
             INSERT INTO employees (nome, cpf, data_admissao, empresa_id)
                 VALUES ('Beltrano', NULL, '2024-05-01', 1);",
        )
        .expect("inserir dados falhou");
        drop(conn);

        // Reabrir (roda init_schema + migrar de novo) não pode mudar nada.
        let conn = open(&caminho).expect("segunda abertura falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        let total_empresas: i64 = conn
            .query_row("SELECT COUNT(*) FROM companies", [], |l| l.get(0))
            .unwrap();
        let cpf_nulo: i64 = conn
            .query_row("SELECT COUNT(*) FROM employees WHERE cpf IS NULL", [], |l| l.get(0))
            .unwrap();
        assert_eq!(total_empresas, 1);
        assert_eq!(cpf_nulo, 1, "funcionário com CPF nulo deve permanecer");

        remover(&caminho);
    }

    #[test]
    fn banco_com_versao_futura_falha_com_mensagem_clara() {
        let caminho = caminho_temporario("futuro");
        let conn = open(&caminho).expect("abrir banco falhou");
        gravar_versao(&conn, VERSAO_ATUAL + 5).expect("gravar versão futura falhou");
        drop(conn);

        let erro = open(&caminho).expect_err("banco mais novo que o app deveria falhar");
        assert!(
            erro.contains("mais nova que a suportada"),
            "mensagem de erro inesperada: {erro}"
        );

        remover(&caminho);
    }
}
