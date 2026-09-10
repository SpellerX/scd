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
//! - `1` = CPF de funcionário passou a ser opcional;
//! - `2` = as tabelas sincronizáveis (`companies`, `employees`,
//!   `employee_leave_periods`) ganharam **id UUID (TEXT)**, além de
//!   `updated_at` e `deleted_at` (soft delete) — base da sincronização com a
//!   nuvem (Supabase);
//! - `3` = `users` ganhou `updated_at` e `deleted_at` (soft delete) — usuários
//!   e acessos passam a ser sincronizados pela nuvem (mantendo o login local);
//! - `4` = `employee_leave_periods` ganhou `alarme_em` e `alarme_observacao` —
//!   o **alarme manual** ("agendar as férias"): quando a empresa já marcou as
//!   férias, o período entra em *Férias a vencer* e avisa na data escolhida,
//!   sem esperar o alerta automático (que só começa 12 meses antes do
//!   vencimento).
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
use uuid::Uuid;

/// Versão de esquema entendida por ESTE binário.
///
/// Toda mudança de esquema deve:
/// 1. criar uma migração em `migracao_para()` para a versão seguinte;
/// 2. atualizar esta constante;
/// 3. manter `init_schema()` criando o esquema FINAL (bancos novos já
///    nascem na última versão, sem passar pelas migrações).
pub const VERSAO_ATUAL: i64 = 4;

/// Abre (ou cria) o banco no caminho informado e leva o esquema à versão atual.
pub fn open(path: &Path) -> Result<Connection, String> {
    let mut conn = Connection::open(path)
        .map_err(|err| format!("Falha ao abrir o arquivo do banco: {err}"))?;

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
    migrar(&mut conn).map_err(|err| format!("Falha ao migrar o banco de dados: {err}"))?;

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
///
/// As tabelas sincronizáveis (`companies`, `employees`,
/// `employee_leave_periods`) usam **id TEXT (UUID)** — chave única GLOBAL,
/// necessária para sincronizar o mesmo registro entre várias máquinas — e
/// guardam `updated_at`/`deleted_at` (soft delete) para que remoções possam
/// ser propagadas pela nuvem sem apagar dados de quem está offline.
fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT    NOT NULL UNIQUE,
            display_name  TEXT    NOT NULL,
            password_hash TEXT    NOT NULL,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
            updated_at    TEXT    NOT NULL DEFAULT (datetime('now')),
            deleted_at    TEXT
        );

        CREATE TABLE IF NOT EXISTS companies (
            id             TEXT    NOT NULL PRIMARY KEY,
            cnpj           TEXT    NOT NULL,
            razao_social   TEXT    NOT NULL,
            nome_fantasia  TEXT,
            situacao       TEXT,
            municipio      TEXT,
            uf             TEXT,
            telefone       TEXT,
            email          TEXT,
            created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
            updated_at     TEXT    NOT NULL DEFAULT (datetime('now')),
            deleted_at     TEXT
        );

        -- Funcionários vinculados a uma empresa (base para o controle de férias).
        -- CPF é OPCIONAL (pode vir de planilha sem CPF); quando informado,
        -- é padronizado com os 11 dígitos e deve ser único (entre os ativos).
        CREATE TABLE IF NOT EXISTS employees (
            id             TEXT    NOT NULL PRIMARY KEY,
            nome           TEXT    NOT NULL,
            cpf            TEXT,
            data_admissao  TEXT,
            empresa_id     TEXT    NOT NULL REFERENCES companies(id),
            created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
            updated_at     TEXT    NOT NULL DEFAULT (datetime('now')),
            deleted_at     TEXT
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
        -- Alarme manual (opcional): data em que o app deve avisar, escolhida
        -- na tela Férias a vencer quando a empresa JÁ agendou as férias —
        -- evita esperar o alerta automático (que só começa 12 meses antes do
        -- vencimento). `alarme_observacao` é o texto livre do agendamento
        -- (separado de `observacao`, usada na regularização).
        CREATE TABLE IF NOT EXISTS employee_leave_periods (
            id                TEXT    NOT NULL PRIMARY KEY,
            employee_id       TEXT    NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
            inicio            TEXT    NOT NULL,
            vencimento        TEXT    NOT NULL,
            regularizada      INTEGER NOT NULL DEFAULT 0,
            regularizada_em   TEXT,
            observacao        TEXT,
            alarme_em         TEXT,
            alarme_observacao TEXT,
            created_at        TEXT    NOT NULL DEFAULT (datetime('now')),
            updated_at        TEXT    NOT NULL DEFAULT (datetime('now')),
            deleted_at        TEXT
        );

        -- Configurações gerais (chave → valor).
        CREATE TABLE IF NOT EXISTS app_settings (
            chave TEXT PRIMARY KEY,
            valor TEXT NOT NULL
        );",
    )
    .map_err(|err| format!("Falha ao criar o esquema: {err}"))?;

    // Índices de unicidade entre registros ATIVOS. Ficam num passo separado
    // (e condicional) porque referenciam `deleted_at`, coluna que só existe
    // no formato final — em bancos legados ela é criada pela migração v2.
    if tabela_tem_coluna(conn, "companies", "deleted_at")? {
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS companies_cnpj_ativo
                 ON companies(cnpj) WHERE deleted_at IS NULL;
             CREATE UNIQUE INDEX IF NOT EXISTS employees_cpf_ativo
                 ON employees(cpf) WHERE deleted_at IS NULL;
             CREATE UNIQUE INDEX IF NOT EXISTS periods_employee_inicio_ativo
                 ON employee_leave_periods(employee_id, inicio) WHERE deleted_at IS NULL;",
        )
        .map_err(|err| format!("Falha ao criar os índices de unicidade: {err}"))?;
    }
    Ok(())
}

/// Aplica, em ordem, as migrações pendentes até `VERSAO_ATUAL`.
///
/// Cada versão aplicada é gravada no banco imediatamente após concluir, para
/// que uma interrupção no meio não reexecute migrações já concluídas.
fn migrar(conn: &mut Connection) -> Result<(), String> {
    let versao =
        versao_do_banco(conn).map_err(|err| format!("Falha ao ler a versão do banco: {err}"))?;

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
fn migracao_para(conn: &mut Connection, versao: i64) -> Result<(), String> {
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
            // Banco já no formato v1 (nullable) → nada a fazer; a versão
            // é gravada pelo chamador. Cobre bancos normalizados fora do app.
            Ok(())
        }
        // v1 → v2: tabelas sincronizáveis passam a usar id UUID (TEXT) e
        // ganham updated_at/deleted_at (soft delete). Dados antigos são
        // reescritos com UUIDs determinísticos (mesmo registro = mesmo UUID
        // em qualquer máquina), preservando os vínculos entre tabelas.
        2 => migracao_v2(conn),
        // v2 → v3: `users` ganha updated_at/deleted_at — usuários e acessos
        // passam a ser sincronizados pela nuvem (soft delete preserva o login
        // local/offline).
        3 => migracao_v3(conn),
        // v3 → v4: períodos de férias ganham o alarme manual (agendamento).
        4 => migracao_v4(conn),
        _ => Err(format!("Migração para a versão {versao} não implementada.")),
    }
}

/// v3 → v4: `employee_leave_periods` ganha `alarme_em` e
/// `alarme_observacao` — o alarme manual de "férias agendadas".
///
/// Só acrescenta colunas (nenhum dado existente é reescrito), então cada
/// coluna é adicionada individualmente e a migração é idempotente: um banco
/// já normalizado à mão apenas registra a versão.
fn migracao_v4(conn: &mut Connection) -> Result<(), String> {
    for coluna in ["alarme_em", "alarme_observacao"] {
        if tabela_tem_coluna(conn, "employee_leave_periods", coluna)? {
            continue; // já no formato v4 (banco novo ou normalizado à mão)
        }
        conn.execute_batch(&format!(
            "ALTER TABLE employee_leave_periods ADD COLUMN {coluna} TEXT;"
        ))
        .map_err(|err| format!("Falha ao acrescentar a coluna {coluna} (v4): {err}"))?;
    }
    Ok(())
}

/// v2 → v3: `users` ganha `updated_at`/`deleted_at` (soft delete) para os
/// usuários serem sincronizados pela nuvem.
///
/// SQLite não permite `ADD COLUMN ... DEFAULT datetime('now')` (default não
/// constante), então a tabela é recriada — mesmo padrão da migração v2 —
/// preservando ids, dados e os vínculos de `sessions`/`user_permissions`.
fn migracao_v3(conn: &mut Connection) -> Result<(), String> {
    if tabela_tem_coluna(conn, "users", "deleted_at")? {
        return Ok(()); // já no formato v3 (ex.: banco novo/normalizado)
    }

    // O rebuild troca tabelas que têm FK entre si; SQLite exige desligar a
    // checagem de chaves estrangeiras FORA de uma transação para isso.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|err| format!("Falha ao preparar a migração de usuários: {err}"))?;

    let resultado = (|| -> Result<(), String> {
        conn.execute_batch("BEGIN;")
            .map_err(|err| format!("Falha ao iniciar a migração de usuários: {err}"))?;
        conn.execute_batch(
            "CREATE TABLE users_nova (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                username      TEXT    NOT NULL UNIQUE,
                display_name  TEXT    NOT NULL,
                password_hash TEXT    NOT NULL,
                created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at    TEXT    NOT NULL DEFAULT (datetime('now')),
                deleted_at    TEXT
            );

            INSERT INTO users_nova (id, username, display_name, password_hash, created_at)
                SELECT id, username, display_name, password_hash, created_at FROM users;

            -- Recria também os filhos (referenciam users por FK) para trocar a
            -- tabela users; os dados são preservados (logins persistentes e
            -- permissões continuam valendo).
            CREATE TABLE sessions_nova (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id    INTEGER NOT NULL REFERENCES users(id),
                token      TEXT    NOT NULL UNIQUE,
                created_at TEXT    NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO sessions_nova (id, user_id, token, created_at)
                SELECT id, user_id, token, created_at FROM sessions;

            CREATE TABLE user_permissions_nova (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                section_id TEXT    NOT NULL,
                UNIQUE (user_id, section_id)
            );
            INSERT INTO user_permissions_nova (id, user_id, section_id)
                SELECT id, user_id, section_id FROM user_permissions;

            DROP TABLE user_permissions;
            DROP TABLE sessions;
            DROP TABLE users;

            ALTER TABLE users_nova RENAME TO users;
            ALTER TABLE sessions_nova RENAME TO sessions;
            ALTER TABLE user_permissions_nova RENAME TO user_permissions;

            COMMIT;",
        )
        .map_err(|err| format!("Falha ao migrar a tabela users (v3): {err}"))?;
        Ok(())
    })();

    if resultado.is_err() {
        let _ = conn.execute_batch("ROLLBACK;");
    }
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|err| format!("Falha ao religar as chaves estrangeiras: {err}"))?;
    resultado
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

    Ok(linhas
        .iter()
        .any(|(nome, notnull)| nome == "cpf" && *notnull != 0))
}

/// A tabela informada já tem a coluna? (usado para detectar formato migrado.)
fn tabela_tem_coluna(conn: &Connection, tabela: &str, coluna: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({tabela})"))
        .map_err(|err| format!("Falha ao inspecionar a tabela {tabela}: {err}"))?;

    let nomes = stmt
        .query_map([], |linha| linha.get::<_, String>(1))
        .map_err(|err| format!("Falha ao consultar a tabela {tabela}: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler a tabela {tabela}: {err}"))?;

    Ok(nomes.iter().any(|nome| nome == coluna))
}

// ═══════════════════ Migração v2 (ids UUID + sincronização) ═══════════════════

/// UUID determinístico de uma empresa (derivado do CNPJ): o mesmo registro
/// recebe o mesmo UUID em qualquer máquina, sem colisão entre bancos.
fn uuid_da_empresa(cnpj: &str) -> String {
    uuid_de_nome(&format!("scd/empresa/{cnpj}"))
}

/// UUID v5 de um "nome" (string) — base dos UUIDs determinísticos.
fn uuid_de_nome(nome: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, nome.as_bytes()).to_string()
}

/// UUID determinístico de um funcionário legado: derivado do CPF quando
/// existe; sem CPF, do UUID da empresa + id antigo (não há chave natural).
fn uuid_do_funcionario_legado(funcionario: &FuncionarioLegado, empresa: &EmpresaLegada) -> String {
    match &funcionario.cpf {
        Some(cpf) => uuid_de_nome(&format!("scd/funcionario/{cpf}")),
        None => uuid_de_nome(&format!(
            "scd/funcionario/{}/{}",
            uuid_da_empresa(&empresa.cnpj),
            funcionario.id
        )),
    }
}

/// v1 → v2: reescreve `companies`, `employees` e `employee_leave_periods`
/// com **id UUID** e colunas `updated_at`/`deleted_at`, preservando os
/// dados e os vínculos (empresa → funcionários → períodos de férias).
fn migracao_v2(conn: &mut Connection) -> Result<(), String> {
    // Banco já no formato v2 (ex.: normalizado à mão) → apenas registra a versão.
    if tabela_tem_coluna(conn, "companies", "deleted_at")? {
        return Ok(());
    }

    // 1) Lê os dados antigos (ids inteiros) ANTES de reescrever as tabelas.
    let empresas = ler_empresas_legadas(conn)?;
    let funcionarios = ler_funcionarios_legados(conn)?;
    let periodos = ler_periodos_legados(conn)?;

    // 2) Reescrita dentro de uma transação (tudo ou nada).
    let tx = conn
        .transaction()
        .map_err(|err| format!("Falha ao iniciar a migração para UUID: {err}"))?;

    tx.execute_batch(
        "DROP TABLE IF EXISTS employee_leave_periods;
         DROP TABLE IF EXISTS employees;
         DROP TABLE IF EXISTS companies;",
    )
    .map_err(|err| format!("Falha ao remover as tabelas antigas: {err}"))?;

    // Recria o esquema final (mesmas tabelas do init_schema; as demais
    // tabelas — users, sessions etc. — já existem e não são tocadas).
    init_schema(&tx).map_err(|err| format!("Falha ao recriar o esquema: {err}"))?;

    // 3) Copia os dados na ordem de dependência (empresas → funcionários →
    //    períodos), para respeitar as chaves estrangeiras.
    for empresa in &empresas {
        let id = uuid_da_empresa(&empresa.cnpj);
        tx.execute(
            "INSERT INTO companies
                (id, cnpj, razao_social, nome_fantasia, situacao, municipio, uf,
                 telefone, email, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                id,
                empresa.cnpj,
                empresa.razao_social,
                empresa.nome_fantasia,
                empresa.situacao,
                empresa.municipio,
                empresa.uf,
                empresa.telefone,
                empresa.email,
                empresa.created_at,
                empresa.created_at, // sem histórico de edição: updated_at = created_at
            ],
        )
        .map_err(|err| format!("Falha ao migrar a empresa {}: {err}", empresa.razao_social))?;
    }

    for funcionario in &funcionarios {
        // Empresa da qual o funcionário depende (FK) — mesmo UUID derivado acima.
        let empresa_legada = empresas
            .iter()
            .find(|empresa| empresa.id == funcionario.empresa_id)
            .ok_or_else(|| {
                format!(
                    "Funcionário {} referencia a empresa inexistente {}.",
                    funcionario.nome, funcionario.empresa_id
                )
            })?;
        let id = uuid_do_funcionario_legado(funcionario, &empresa_legada);
        tx.execute(
            "INSERT INTO employees
                (id, nome, cpf, data_admissao, empresa_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                funcionario.nome,
                funcionario.cpf,
                funcionario.data_admissao,
                uuid_da_empresa(&empresa_legada.cnpj),
                funcionario.created_at,
                funcionario.created_at,
            ],
        )
        .map_err(|err| format!("Falha ao migrar o funcionário {}: {err}", funcionario.nome))?;
    }

    for periodo in &periodos {
        let funcionario_legado = funcionarios
            .iter()
            .find(|funcionario| funcionario.id == periodo.employee_id)
            .ok_or_else(|| {
                format!(
                    "Período de férias referencia o funcionário inexistente {}.",
                    periodo.employee_id
                )
            })?;
        // UUID do funcionário dono do período — mesma regra do loop acima.
        let empresa_legada = empresas
            .iter()
            .find(|empresa| empresa.id == funcionario_legado.empresa_id)
            .ok_or_else(|| "Empresa do funcionário do período sumiu.".to_string())?;
        let employee_uuid = uuid_do_funcionario_legado(funcionario_legado, &empresa_legada);
        let id = uuid_de_nome(&format!("scd/ferias/{employee_uuid}/{}", periodo.inicio));

        tx.execute(
            "INSERT INTO employee_leave_periods
                (id, employee_id, inicio, vencimento, regularizada,
                 regularizada_em, observacao)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                employee_uuid,
                periodo.inicio,
                periodo.vencimento,
                periodo.regularizada,
                periodo.regularizada_em,
                periodo.observacao,
            ],
        )
        .map_err(|err| format!("Falha ao migrar o período de férias {id}: {err}"))?;
    }

    tx.commit()
        .map_err(|err| format!("Falha ao finalizar a migração para UUID: {err}"))?;
    Ok(())
}

/// Empresa no formato legado (id inteiro), lida antes da reescrita v2.
struct EmpresaLegada {
    id: i64,
    cnpj: String,
    razao_social: String,
    nome_fantasia: Option<String>,
    situacao: Option<String>,
    municipio: Option<String>,
    uf: Option<String>,
    telefone: Option<String>,
    email: Option<String>,
    created_at: String,
}

fn ler_empresas_legadas(conn: &Connection) -> Result<Vec<EmpresaLegada>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, cnpj, razao_social, nome_fantasia, situacao, municipio, uf,
                    telefone, email, created_at
               FROM companies
              ORDER BY id",
        )
        .map_err(|err| format!("Falha ao ler empresas antigas: {err}"))?;

    let empresas = stmt
        .query_map([], |linha| {
            Ok(EmpresaLegada {
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
        })
        .map_err(|err| format!("Falha ao consultar empresas antigas: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler empresas antigas: {err}"))?;

    Ok(empresas)
}

/// Funcionário no formato legado (id inteiro + empresa inteira).
struct FuncionarioLegado {
    id: i64,
    nome: String,
    cpf: Option<String>,
    data_admissao: Option<String>,
    empresa_id: i64,
    created_at: String,
}

fn ler_funcionarios_legados(conn: &Connection) -> Result<Vec<FuncionarioLegado>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, nome, cpf, data_admissao, empresa_id, created_at
               FROM employees
              ORDER BY id",
        )
        .map_err(|err| format!("Falha ao ler funcionários antigos: {err}"))?;

    let funcionarios = stmt
        .query_map([], |linha| {
            Ok(FuncionarioLegado {
                id: linha.get(0)?,
                nome: linha.get(1)?,
                cpf: linha.get(2)?,
                data_admissao: linha.get(3)?,
                empresa_id: linha.get(4)?,
                created_at: linha.get(5)?,
            })
        })
        .map_err(|err| format!("Falha ao consultar funcionários antigos: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler funcionários antigos: {err}"))?;

    Ok(funcionarios)
}

/// Período de férias no formato legado (funcionário inteiro).
/// Obs.: a tabela legada NÃO tinha `created_at` — os novos períodos migrados
/// nascem com os padrões atuais (datetime('now')).
struct PeriodoLegado {
    employee_id: i64,
    inicio: String,
    vencimento: String,
    regularizada: i64,
    regularizada_em: Option<String>,
    observacao: Option<String>,
}

fn ler_periodos_legados(conn: &Connection) -> Result<Vec<PeriodoLegado>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT employee_id, inicio, vencimento, regularizada,
                    regularizada_em, observacao
               FROM employee_leave_periods
              ORDER BY id",
        )
        .map_err(|err| format!("Falha ao ler períodos de férias antigos: {err}"))?;

    let periodos = stmt
        .query_map([], |linha| {
            Ok(PeriodoLegado {
                employee_id: linha.get(0)?,
                inicio: linha.get(1)?,
                vencimento: linha.get(2)?,
                regularizada: linha.get(3)?,
                regularizada_em: linha.get(4)?,
                observacao: linha.get(5)?,
            })
        })
        .map_err(|err| format!("Falha ao consultar períodos de férias antigos: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler períodos de férias antigos: {err}"))?;

    Ok(periodos)
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
        std::env::temp_dir().join(format!("scd-teste-{nome}-{}-{n}.db", std::process::id()))
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

    /// Esquema legado (versões 0/1): tabelas sincronizáveis com id inteiro.
    fn criar_esquema_legado(conn: &Connection, cpf_not_null: bool) {
        let cpf_coluna = if cpf_not_null {
            "TEXT NOT NULL"
        } else {
            "TEXT"
        };
        conn.execute_batch(&format!(
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
                cpf            {cpf_coluna} UNIQUE,
                data_admissao  TEXT,
                empresa_id     INTEGER NOT NULL REFERENCES companies(id),
                created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE employee_leave_periods (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                employee_id   INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
                inicio        TEXT    NOT NULL,
                vencimento    TEXT    NOT NULL,
                regularizada  INTEGER NOT NULL DEFAULT 0,
                regularizada_em TEXT,
                observacao    TEXT,
                UNIQUE (employee_id, inicio)
            );",
        ))
        .expect("montar banco legado falhou");
    }

    #[test]
    fn banco_novo_nasce_na_versao_atual() {
        let caminho = caminho_temporario("novo");
        let conn = open(&caminho).expect("abrir banco novo falhou");

        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        assert!(cpf_e_opcional(&conn), "employees.cpf deve nascer opcional");
        assert!(
            tabela_tem_coluna(&conn, "companies", "deleted_at").unwrap(),
            "empresas devem nascer com soft delete (v2)"
        );
        assert!(
            tabela_tem_coluna(&conn, "users", "deleted_at").unwrap(),
            "usuários devem nascer com soft delete (v3)"
        );
        assert!(
            tabela_tem_coluna(&conn, "employee_leave_periods", "alarme_em").unwrap(),
            "períodos devem nascer com o alarme manual (v4)"
        );

        // Empresa nasce com id UUID (TEXT) informado — como faz o domínio.
        let id = uuid_de_nome("teste/banco-novo");
        conn.execute(
            "INSERT INTO companies (id, cnpj, razao_social) VALUES (?1, '12345678000199', 'Nova')",
            [&id],
        )
        .expect("inserir empresa nova falhou");
        let (id_lido, updated_at): (String, String) = conn
            .query_row("SELECT id, updated_at FROM companies LIMIT 1", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!(id_lido, id, "id UUID deve ser preservado");
        assert!(!updated_at.is_empty(), "updated_at deve nascer preenchido");

        remover(&caminho);
    }

    #[test]
    fn banco_legado_v1_migra_para_uuid_preservando_dados_e_vinculos() {
        let caminho = caminho_temporario("legado-v1");
        let conn = Connection::open(&caminho).expect("criar banco legado falhou");
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        criar_esquema_legado(&conn, false); // v1: cpf já opcional
        conn.execute_batch(
            "INSERT INTO companies (cnpj, razao_social) VALUES ('12345678000199', 'Empresa Teste');
             INSERT INTO employees (nome, cpf, data_admissao, empresa_id)
                 VALUES ('Fulano', '52998224725', '2023-01-02', 1);
             INSERT INTO employee_leave_periods (employee_id, inicio, vencimento, regularizada)
                 VALUES (1, '2023-01-02', '2025-01-02', 0);",
        )
        .expect("inserir dados legados falhou");
        drop(conn);

        // `open` deve migrar tudo até a versão atual.
        let conn = open(&caminho).expect("migração do banco legado falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);

        // Dados preservados, agora com id UUID.
        let (id_empresa, razao): (String, String) = conn
            .query_row(
                "SELECT id, razao_social FROM companies WHERE cnpj = '12345678000199'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert_eq!(razao, "Empresa Teste");
        assert_eq!(
            id_empresa,
            uuid_da_empresa("12345678000199"),
            "uuid deve ser determinístico"
        );

        // Funcionário preservado, vinculado ao UUID da empresa.
        let (id_funcionario, empresa_id, cpf): (String, String, Option<String>) = conn
            .query_row(
                "SELECT id, empresa_id, cpf FROM employees WHERE nome = 'Fulano'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            empresa_id, id_empresa,
            "vínculo empresa→funcionário preservado"
        );
        assert_eq!(cpf.as_deref(), Some("52998224725"));

        // Período de férias preservado e vinculado ao UUID do funcionário.
        let (id_periodo, employee_id, inicio): (String, String, String) = conn
            .query_row(
                "SELECT id, employee_id, inicio FROM employee_leave_periods LIMIT 1",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            employee_id, id_funcionario,
            "vínculo funcionário→período preservado"
        );
        assert_eq!(inicio, "2023-01-02");
        assert!(!id_periodo.is_empty(), "período deve ter id UUID");

        // Reabrir NÃO pode reexecutar a migração nem perder nada.
        drop(conn);
        let conn = open(&caminho).expect("segunda abertura falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        let totais: (i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM companies),
                        (SELECT COUNT(*) FROM employees),
                        (SELECT COUNT(*) FROM employee_leave_periods)",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .unwrap();
        assert_eq!(totais, (1, 1, 1), "reabrir não pode duplicar/perder dados");

        remover(&caminho);
    }

    #[test]
    fn mesmos_dados_legados_geram_mesmos_uuids_em_maquinas_diferentes() {
        // Dois bancos legados com o MESMO conteúdo (ex.: duas cópias do mesmo
        // arquivo antigo) devem migrar para os MESMOS ids UUID — é isso que
        // permite duas máquinas sincronizarem sem duplicar registros.
        let mut ids: Vec<(String, String)> = Vec::new();
        for nome in ["maquina-a", "maquina-b"] {
            let caminho = caminho_temporario(nome);
            {
                let conn = Connection::open(&caminho).expect("criar banco legado falhou");
                conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
                criar_esquema_legado(&conn, false);
                conn.execute_batch(
                    "INSERT INTO companies (cnpj, razao_social) VALUES ('12345678000199', 'X');
                     INSERT INTO employees (nome, cpf, data_admissao, empresa_id)
                         VALUES ('Fulano', '52998224725', '2023-01-02', 1);",
                )
                .unwrap();
            }
            let conn = open(&caminho).expect("migrar falhou");
            let (empresa, funcionario): (String, String) = conn
                .query_row(
                    "SELECT (SELECT id FROM companies), (SELECT id FROM employees)",
                    [],
                    |l| Ok((l.get(0)?, l.get(1)?)),
                )
                .unwrap();
            ids.push((empresa, funcionario));
            drop(conn);
            remover(&caminho);
        }
        assert_eq!(
            ids[0], ids[1],
            "uuids devem ser determinísticos entre máquinas"
        );
    }

    #[test]
    fn banco_legado_v2_com_usuarios_migra_v3_preservando_vinculos() {
        let caminho = caminho_temporario("legado-users");
        let mut conn = Connection::open(&caminho).expect("criar banco legado falhou");
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        // Formato v2: users SEM updated_at/deleted_at + filhos (FK).
        conn.execute_batch(
            "CREATE TABLE users (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                username      TEXT    NOT NULL UNIQUE,
                display_name  TEXT    NOT NULL,
                password_hash TEXT    NOT NULL,
                created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE sessions (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id    INTEGER NOT NULL REFERENCES users(id),
                token      TEXT    NOT NULL UNIQUE,
                created_at TEXT    NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE user_permissions (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                section_id TEXT    NOT NULL,
                UNIQUE (user_id, section_id)
            );
            INSERT INTO users (username, display_name, password_hash)
                VALUES ('fernando', 'Fernando', 'hash');
            INSERT INTO sessions (user_id, token) VALUES (1, 'token-teste');
            INSERT INTO user_permissions (user_id, section_id) VALUES (1, 'cadastro-empresa');",
        )
        .expect("montar banco v2 falhou");
        gravar_versao(&conn, 2).expect("gravar versão 2 falhou");
        drop(conn);

        // `open` migra 2 → 3 e não pode quebrar nada.
        let conn = open(&caminho).expect("migração v3 falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        assert!(tabela_tem_coluna(&conn, "users", "deleted_at").unwrap());

        let (nome, hash): (String, String) = conn
            .query_row(
                "SELECT display_name, password_hash FROM users WHERE username = 'fernando'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .expect("usuário sumiu na migração");
        assert_eq!(nome, "Fernando");
        assert_eq!(hash, "hash");

        // Vínculos preservados (sessão e permissão continuam apontando p/ ele).
        let sessao: String = conn
            .query_row(
                "SELECT s.token FROM sessions s JOIN users u ON u.id = s.user_id
                  WHERE s.token = 'token-teste' AND u.username = 'fernando'",
                [],
                |l| l.get(0),
            )
            .expect("sessão quebrada na migração");
        assert_eq!(sessao, "token-teste");
        let secao: String = conn
            .query_row(
                "SELECT p.section_id FROM user_permissions p JOIN users u ON u.id = p.user_id
                  WHERE u.username = 'fernando'",
                [],
                |l| l.get(0),
            )
            .expect("permissão quebrada na migração");
        assert_eq!(secao, "cadastro-empresa");

        remover(&caminho);
    }

    #[test]
    fn banco_v3_ganha_as_colunas_do_alarme_sem_perder_periodos() {
        let caminho = caminho_temporario("v3-alarme");
        let conn = open(&caminho).expect("abrir banco falhou");

        // Simula um banco v3: recria `employee_leave_periods` no formato
        // anterior (sem as colunas do alarme) com um período já cadastrado.
        conn.execute_batch(
            "INSERT INTO companies (id, cnpj, razao_social)
                 VALUES ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', '12345678000199', 'Empresa');
             INSERT INTO employees (id, nome, cpf, data_admissao, empresa_id)
                 VALUES ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', 'Fulano', NULL, '2024-05-01',
                         'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa');

             DROP TABLE employee_leave_periods;
             CREATE TABLE employee_leave_periods (
                 id               TEXT    NOT NULL PRIMARY KEY,
                 employee_id      TEXT    NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
                 inicio           TEXT    NOT NULL,
                 vencimento       TEXT    NOT NULL,
                 regularizada     INTEGER NOT NULL DEFAULT 0,
                 regularizada_em  TEXT,
                 observacao       TEXT,
                 created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at       TEXT    NOT NULL DEFAULT (datetime('now')),
                 deleted_at       TEXT
             );
             INSERT INTO employee_leave_periods (id, employee_id, inicio, vencimento, observacao)
                 VALUES ('cccccccc-cccc-4ccc-8ccc-cccccccccccc',
                         'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', '2024-05-01', '2026-05-01',
                         'período antigo');
             PRAGMA user_version = 3;",
        )
        .expect("montar banco v3 falhou");
        drop(conn);

        // `open` migra 3 → 4 acrescentando as colunas do alarme.
        let conn = open(&caminho).expect("migração v4 falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        assert!(tabela_tem_coluna(&conn, "employee_leave_periods", "alarme_em").unwrap());
        assert!(tabela_tem_coluna(&conn, "employee_leave_periods", "alarme_observacao").unwrap());

        let (total, observacao, alarme): (i64, String, Option<String>) = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM employee_leave_periods), observacao, alarme_em
                   FROM employee_leave_periods LIMIT 1",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .expect("período antigo sumiu na migração");
        assert_eq!(total, 1, "a migração não pode perder períodos");
        assert_eq!(observacao, "período antigo", "observação deve ser preservada");
        assert_eq!(alarme, None, "alarme nasce vazio");

        // Reabrir não pode reexecutar a migração nem falhar.
        drop(conn);
        let conn = open(&caminho).expect("segunda abertura falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);

        remover(&caminho);
    }

    #[test]
    fn banco_legado_v0_cpf_obrigatorio_migra() {
        let caminho = caminho_temporario("legado-v0");
        let conn = Connection::open(&caminho).expect("criar banco legado falhou");
        criar_esquema_legado(&conn, true); // v0: employees.cpf NOT NULL
        conn.execute_batch(
            "INSERT INTO companies (cnpj, razao_social) VALUES ('12345678000199', 'Empresa Teste');
             INSERT INTO employees (nome, cpf, data_admissao, empresa_id)
                 VALUES ('Fulano', '52998224725', '2023-01-02', 1);",
        )
        .expect("montar banco v0 falhou");
        drop(conn);

        let conn = open(&caminho).expect("migração v0 falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        assert!(cpf_e_opcional(&conn), "cpf deve ter virado opcional");

        let (nome, cpf): (String, Option<String>) = conn
            .query_row(
                "SELECT nome, cpf FROM employees WHERE nome = 'Fulano'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .expect("funcionário migrado sumiu");
        assert_eq!(nome, "Fulano");
        assert_eq!(cpf.as_deref(), Some("52998224725"));

        remover(&caminho);
    }

    /// Prova de fogo com o banco REAL do usuário — **opt-in**, nunca roda por
    /// padrão (o caminho é da máquina, não do repositório).
    ///
    /// Copia o arquivo `scd.db` para um temporário e verifica, sem tocar no
    /// original, que a migração até a versão atual não perde nenhum registro e
    /// que um período real consegue receber o alarme manual (mesmo caminho que
    /// a tela "Férias a vencer" percorre).
    ///
    /// Como rodar (PowerShell):
    ///   $env:SCD_BANCO_REAL = "$env:APPDATA\com.scd.app\scd.db"
    ///   cargo test --lib banco_real -- --ignored --nocapture
    #[test]
    #[ignore = "usa o banco real do usuário (defina SCD_BANCO_REAL)"]
    fn banco_real_do_usuario_migra_e_aceita_alarme() {
        use rusqlite::OptionalExtension;

        let origem = std::env::var("SCD_BANCO_REAL")
            .expect("defina SCD_BANCO_REAL com o caminho do scd.db");
        let caminho = caminho_temporario("copia-real");
        std::fs::copy(&origem, &caminho).expect("copiar o banco real falhou");

        // Contagens no banco ORIGINAL (versão antiga), lendo a cópia crua.
        let contar = |conn: &Connection| -> (i64, i64, i64, i64) {
            conn.query_row(
                "SELECT (SELECT COUNT(*) FROM companies),
                        (SELECT COUNT(*) FROM employees),
                        (SELECT COUNT(*) FROM employee_leave_periods),
                        (SELECT COUNT(*) FROM employee_leave_periods WHERE regularizada = 0)",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
            )
            .expect("contar registros falhou")
        };
        let antes = contar(&Connection::open(&caminho).expect("abrir a cópia falhou"));
        let versao_antes = versao_do_banco(&Connection::open(&caminho).unwrap()).unwrap();

        // Migração de verdade (mesma função que o app roda ao abrir).
        let conn = open(&caminho).expect("migração do banco real falhou");
        assert_eq!(versao_do_banco(&conn).unwrap(), VERSAO_ATUAL);
        assert!(tabela_tem_coluna(&conn, "employee_leave_periods", "alarme_em").unwrap());
        assert!(tabela_tem_coluna(&conn, "employee_leave_periods", "alarme_observacao").unwrap());

        let depois = contar(&conn);
        assert_eq!(
            (depois.0, depois.1, depois.2, depois.3),
            (antes.0, antes.1, antes.2, antes.3),
            "a migração não pode perder nem duplicar registros"
        );
        println!(
            "banco real: v{versao_antes} → v{} | {} empresas | {} funcionários | {} períodos ({} em aberto)",
            VERSAO_ATUAL, depois.0, depois.1, depois.2, depois.3
        );

        // Diagnóstico: retrato dos funcionários reais (explica o que a tela mostra).
        {
            let mut stmt = conn
                .prepare(
                    "SELECT f.nome, COALESCE(f.data_admissao, '—'), (f.deleted_at IS NOT NULL),
                            (SELECT COUNT(*) FROM employee_leave_periods p
                              WHERE p.employee_id = f.id AND p.deleted_at IS NULL),
                            (SELECT COUNT(*) FROM employee_leave_periods p
                              WHERE p.employee_id = f.id AND p.deleted_at IS NULL
                                AND p.regularizada = 0
                                AND p.vencimento > date('now','localtime'))
                       FROM employees f
                      ORDER BY f.nome",
                )
                .unwrap();
            let linhas = stmt
                .query_map([], |l| {
                    Ok((
                        l.get::<_, String>(0)?,
                        l.get::<_, String>(1)?,
                        l.get::<_, i64>(2)?,
                        l.get::<_, i64>(3)?,
                        l.get::<_, i64>(4)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();

            println!("funcionários no banco real ({}):", linhas.len());
            for (nome, admissao, apagado, periodos, a_vencer) in &linhas {
                println!(
                    "  {nome} | admissão: {admissao} | {} | {periodos} período(s) ativo(s) | {a_vencer} a vencer",
                    if *apagado != 0 { "APAGADO" } else { "ativo" }
                );
            }
        }

        // Um período real já a vencer, de funcionário ATIVO (caminho da tela).
        let mut alvo: Option<(String, String, String)> = conn
            .query_row(
                "SELECT p.id, f.nome, p.vencimento
                   FROM employee_leave_periods p
                   JOIN employees f ON f.id = p.employee_id
                  WHERE p.regularizada = 0
                    AND p.vencimento > date('now','localtime')
                    AND p.deleted_at IS NULL
                    AND f.deleted_at IS NULL
                  ORDER BY p.vencimento LIMIT 1",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .optional()
            .expect("consultar períodos falhou");

        // Nada a vencer no banco real? Cadastra um funcionário de teste NA CÓPIA
        // (com 2 anos de casa, como o cadastro faz) para exercitar o fluxo
        // inteiro: geração de períodos → agendamento → lista → notificação.
        if alvo.is_none() {
            let mut empresa: Option<String> = conn
                .query_row(
                    "SELECT id FROM companies WHERE deleted_at IS NULL ORDER BY razao_social LIMIT 1",
                    [],
                    |l| l.get(0),
                )
                .optional()
                .expect("consultar empresas falhou");

            // Banco sem empresa ativa (tudo apagado)? Cria uma NA CÓPIA, para o
            // fluxo poder ser exercitado de ponta a ponta mesmo assim.
            if empresa.is_none() {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO companies (id, cnpj, razao_social)
                     VALUES (?1, '00000000000000', 'Empresa Teste (cópia)')",
                    [&id],
                )
                .expect("criar empresa de teste na cópia falhou");
                println!("nenhuma empresa ativa: criada empresa de teste NA CÓPIA.");
                empresa = Some(id);
            }

            if let Some(empresa_id) = empresa {
                let admissao: String = conn
                    .query_row("SELECT date('now','localtime','-2 years')", [], |l| l.get(0))
                    .unwrap();
                println!("nenhum período a vencer: criando funcionário de teste NA CÓPIA (admissão {admissao})");
                let funcionario = crate::funcionarios::inserir(
                    &conn,
                    "Teste Automático (cópia)",
                    "",
                    &empresa_id,
                    Some(admissao),
                )
                .expect("cadastrar funcionário de teste na cópia falhou");

                alvo = conn
                    .query_row(
                        "SELECT p.id, f.nome, p.vencimento
                           FROM employee_leave_periods p
                           JOIN employees f ON f.id = p.employee_id
                          WHERE p.employee_id = ?1
                            AND p.regularizada = 0
                            AND p.vencimento > date('now','localtime')
                            AND p.deleted_at IS NULL
                          ORDER BY p.vencimento LIMIT 1",
                        [&funcionario.id],
                        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
                    )
                    .optional()
                    .expect("consultar períodos do funcionário de teste falhou");
            } else {
                println!("banco real sem empresa ativa: não há como exercitar o cadastro.");
            }
        }

        let Some((periodo_id, funcionario, vencimento)) = alvo else {
            println!("fluxo de agendamento NÃO exercitado (sem funcionário/empresa utilizável).");
            remover(&caminho);
            return;
        };

        let hoje: String = conn
            .query_row("SELECT date('now','localtime')", [], |l| l.get(0))
            .unwrap();
        crate::ferias::definir_alarme(&conn, &periodo_id, &hoje, Some("teste".into()))
            .expect("agendar alarme em dados reais falhou");

        let lista = crate::ferias::listar_a_vencer(&conn, 15).expect("listar falhou");
        let item = lista
            .iter()
            .find(|i| i.id == periodo_id)
            .expect("período agendado deveria aparecer em Férias a vencer");
        assert!(item.agendado);
        assert!(item.alarme_disparado, "aviso de hoje já deve estar pendente");

        let avisos = crate::ferias::listar_notificacoes(&conn).expect("notificações falharam");
        assert!(
            avisos
                .iter()
                .any(|n| n.id == format!("g-{periodo_id}") && n.tipo == "alarme"),
            "o aviso agendado deveria estar nas notificações: {avisos:?}"
        );
        println!(
            "alarme OK em dados reais: {funcionario} (vencimento {vencimento}) → {} notificação(ões) de agendamento",
            avisos.iter().filter(|n| n.tipo == "alarme").count()
        );

        // A cópia é descartada: o banco real permanece intacto.
        remover(&caminho);
    }

    #[test]
    fn banco_no_formato_atual_nao_altera_nada() {
        let caminho = caminho_temporario("atual");
        let conn = open(&caminho).expect("abrir banco falhou");
        conn.execute_batch(
            "INSERT INTO companies (id, cnpj, razao_social)
                 VALUES ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', '12345678000199', 'Empresa Atual');
             INSERT INTO employees (id, nome, cpf, data_admissao, empresa_id)
                 VALUES ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', 'Beltrano', NULL, '2024-05-01',
                         'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa');",
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
            .query_row(
                "SELECT COUNT(*) FROM employees WHERE cpf IS NULL",
                [],
                |l| l.get(0),
            )
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
