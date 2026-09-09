//! Regras de autenticação do SCD.
//!
//! Aqui ficam as operações sobre usuários e senhas (hasheadas com bcrypt).
//! A sessão ativa do app é **em memória** (ver `AppState` no `lib.rs`); já
//! as **sessões persistentes** ("Manter-me conectado") usam a tabela
//! `sessions` com um token aleatório conferido ao reabrir o aplicativo.
//!
//! **Padronização de dados**: nomes de usuário são sempre normalizados para
//! minúsculas (e sem espaços nas pontas), tanto ao criar o usuário quanto ao
//! autenticar. Assim `Admin`, `ADMIN` ou `" admin "` entram como `admin`.
//!
//! Esta camada é chamada pelos comandos em `commands/auth.rs` e não conhece
//! o Tauri — só SQLite e bcrypt —, o que a mantém fácil de testar e alterar.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

/// Usuário devolvido ao frontend (nunca contém a senha/hash).
#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
}

/// Resposta do login: usuário + token de sessão (somente quando o usuário
/// marcou "Manter-me conectado" — o token fica salvo no app).
#[derive(Debug, Clone, Serialize)]
pub struct RespostaLogin {
    pub user: User,
    pub token: Option<String>,
}

/// Registro completo vindo do banco (com o hash — uso interno apenas).
struct UserRecord {
    user: User,
    password_hash: String,
}

/// Valida usuário + senha. Retorna `Ok(Some(user))` se as credenciais
/// conferem, `Ok(None)` se usuário não existe ou a senha está errada,
/// e `Err` apenas para falhas internas (banco etc.).
///
/// O nome de usuário é normalizado ANTES da busca, então o login ignora
/// maiúsculas/minúsculas e espaços.
pub fn authenticate(
    conn: &Connection,
    username: &str,
    password: &str,
) -> Result<Option<User>, String> {
    let username = normalize_username(username);
    let record = find_by_username(conn, &username)
        .map_err(|err| format!("Falha ao consultar o usuário: {err}"))?;

    let Some(record) = record else {
        // Usuário inexistente: mesma resposta da senha errada,
        // para não revelar quais usuários existem.
        return Ok(None);
    };

    if verify_password(password, &record.password_hash) {
        Ok(Some(record.user))
    } else {
        Ok(None)
    }
}

/// Compara a senha digitada com o hash salvo.
/// Falhas internas viram `false` (mesma resposta de "senha errada").
fn verify_password(password: &str, password_hash: &str) -> bool {
    bcrypt::verify(password, password_hash).unwrap_or(false)
}

/// Padronização de nomes de usuário: **minúsculas**, sem espaços nas pontas
/// e com espaços internos colapsados num único espaço (ex.: "Maria  Silva"
/// vira "maria silva"). Assim "nome e sobrenome" funcionam como login, sem
/// quebras por espaços duplicados.
/// É a ÚNICA regra de normalização de usuários — todo dado que entra no
/// banco (ou é comparado com ele) passa por aqui.
fn normalize_username(username: &str) -> String {
    username
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Cria um usuário com a senha já hasheada.
/// O nome de usuário é normalizado (minúsculas) antes de ser gravado.
pub fn create_user(
    conn: &Connection,
    username: &str,
    display_name: &str,
    password: &str,
) -> Result<(), String> {
    let username = normalize_username(username);

    // Regra mínima de senha (evita contas com senha vazia/fraca demais).
    if password.trim().len() < 4 {
        return Err("A senha deve ter pelo menos 4 caracteres.".to_string());
    }

    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|err| format!("Falha ao gerar o hash da senha: {err}"))?;

    conn.execute(
        "INSERT INTO users (username, display_name, password_hash) VALUES (?1, ?2, ?3)",
        rusqlite::params![username, display_name, password_hash],
    )
    .map_err(|err| format!("Falha ao criar o usuário (nome já em uso?): {err}"))?;

    Ok(())
}

/// Nome do superusuário do sistema: acesso TOTAL (ignora permissões por
/// módulo) e protegido contra exclusão. Configurado na primeira execução.
pub const SUPER_USUARIO: &str = "fernando";

/// Garante o superusuário do sistema e remove a conta demo antiga ("admin").
///
/// Na primeira execução (banco vazio) cria `fernando / admin150202`. Também
/// roda em bancos antigos: cria o superusuário se ainda não existir e apaga o
/// usuário demo "admin" (que deixou de ser o superusuário), junto com as
/// sessões dele.
pub fn ensure_usuario_inicial(conn: &Connection) -> Result<(), String> {
    // 1) Superusuário sempre presente (ativo).
    let existe: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username = ?1 AND deleted_at IS NULL)",
            [SUPER_USUARIO],
            |linha| linha.get(0),
        )
        .map_err(|err| format!("Falha ao conferir o superusuário: {err}"))?;
    if !existe {
        create_user(conn, SUPER_USUARIO, "Fernando", "admin150202")?;
    }

    // 2) Remove o demo antigo "admin" (ele não é mais o superusuário).
    let admin_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM users WHERE username = 'admin' AND deleted_at IS NULL",
            [],
            |linha| linha.get(0),
        )
        .optional()
        .map_err(|err| format!("Falha ao consultar o usuário demo: {err}"))?;

    if let Some(id) = admin_id {
        conn.execute("DELETE FROM sessions WHERE user_id = ?1", [id])
            .map_err(|err| format!("Falha ao encerrar as sessões do demo: {err}"))?;
        conn.execute("DELETE FROM users WHERE id = ?1", [id])
            .map_err(|err| format!("Falha ao remover o usuário demo: {err}"))?;
    }
    Ok(())
}

/// Busca um usuário pelo nome (com o hash, para conferência de senha).
fn find_by_username(conn: &Connection, username: &str) -> rusqlite::Result<Option<UserRecord>> {
    conn.query_row(
        "SELECT id, username, display_name, password_hash
           FROM users
          WHERE username = ?1
            AND deleted_at IS NULL",
        [username],
        |row| {
            Ok(UserRecord {
                user: User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    display_name: row.get(2)?,
                },
                password_hash: row.get(3)?,
            })
        },
    )
    .optional()
}

// ═══════════════════ Sessões persistentes ("Manter-me conectado") ═══════════════════

/// Cria uma sessão persistente para o usuário e devolve o token.
/// O token é aleatório (32 bytes hex) e fica gravado na tabela `sessions`.
pub fn criar_sessao(conn: &Connection, user_id: i64) -> Result<String, String> {
    let token = gerar_token()?;

    conn.execute(
        "INSERT INTO sessions (user_id, token) VALUES (?1, ?2)",
        rusqlite::params![user_id, token],
    )
    .map_err(|err| format!("Falha ao criar a sessão: {err}"))?;

    Ok(token)
}

/// Conferir se um token corresponde a uma sessão válida; devolve o usuário.
pub fn usuario_por_token(conn: &Connection, token: &str) -> Result<Option<User>, String> {
    conn.query_row(
        "SELECT u.id, u.username, u.display_name
           FROM sessions s
           JOIN users u ON u.id = s.user_id
          WHERE s.token = ?1
            AND u.deleted_at IS NULL",
        [token],
        |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|err| format!("Falha ao conferir a sessão: {err}"))
}

/// Encerra uma sessão persistente (logout / "Sair").
pub fn encerrar_sessao(conn: &Connection, token: &str) -> Result<(), String> {
    conn.execute("DELETE FROM sessions WHERE token = ?1", [token])
        .map_err(|err| format!("Falha ao encerrar a sessão: {err}"))?;
    Ok(())
}

/// Gera um token aleatório de 32 bytes em hexadecimal.
fn gerar_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| format!("Falha ao gerar o token: {err}"))?;

    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

// ═══════════════════ Administração de usuários ══════════════════════════════

/// Usuário como visto na tela de administração (inclui data de criação).
#[derive(Debug, Clone, Serialize)]
pub struct Usuario {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub created_at: String,
}

/// Lista todos os usuários cadastrados (para a tela Sistema → Usuários).
pub fn listar_usuarios(conn: &Connection) -> Result<Vec<Usuario>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, username, display_name, created_at
               FROM users
              WHERE deleted_at IS NULL
              ORDER BY username COLLATE NOCASE",
        )
        .map_err(|err| format!("Falha ao preparar a consulta de usuários: {err}"))?;

    let usuarios = stmt
        .query_map([], |linha| {
            Ok(Usuario {
                id: linha.get(0)?,
                username: linha.get(1)?,
                display_name: linha.get(2)?,
                created_at: linha.get(3)?,
            })
        })
        .map_err(|err| format!("Falha ao consultar usuários: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Falha ao ler usuários: {err}"))?;

    Ok(usuarios)
}

/// Remove um usuário pelo id (soft delete — a remoção também é sincronizada
/// pela nuvem; o login local/offline dos demais continua funcionando).
/// O superusuário (`fernando`) não pode ser excluído (protege o acesso ao app).
pub fn remover_usuario(conn: &Connection, id: i64) -> Result<(), String> {
    let username: Option<String> = conn
        .query_row(
            "SELECT username FROM users WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |linha| linha.get(0),
        )
        .optional()
        .map_err(|err| format!("Falha ao consultar o usuário: {err}"))?;

    let Some(username) = username else {
        return Err("Usuário não encontrado ou já removido.".to_string());
    };

    if username == SUPER_USUARIO {
        return Err(format!(
            "O usuário superadministrador ({SUPER_USUARIO}) não pode ser excluído."
        ));
    }

    // Encerra as sessões persistentes do usuário e marca o soft delete.
    conn.execute("DELETE FROM sessions WHERE user_id = ?1", [id])
        .map_err(|err| format!("Falha ao encerrar as sessões do usuário: {err}"))?;

    conn.execute(
        "UPDATE users
            SET deleted_at = datetime('now'),
                updated_at = datetime('now')
          WHERE id = ?1",
        [id],
    )
    .map_err(|err| format!("Falha ao excluir o usuário: {err}"))?;

    Ok(())
}

// ═══════════════════ Acessos por usuário ═══════════════════════════════════

/// O superusuário (`fernando`) é o administrador: tem acesso a TUDO (ignora
/// permissões por módulo).
pub fn e_administrador(conn: &Connection, user_id: i64) -> Result<bool, String> {
    let username: Option<String> = conn
        .query_row(
            "SELECT username FROM users WHERE id = ?1",
            [user_id],
            |linha| linha.get(0),
        )
        .optional()
        .map_err(|err| format!("Falha ao consultar o usuário: {err}"))?;

    Ok(username.as_deref() == Some(SUPER_USUARIO))
}

/// Seções (itens de menu) liberadas para o usuário.
pub fn listar_secoes_do_usuario(conn: &Connection, user_id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT section_id FROM user_permissions WHERE user_id = ?1 ORDER BY section_id")
        .map_err(|err| format!("Falha ao preparar a consulta de acessos: {err}"))?;

    let secoes = stmt
        .query_map([user_id], |linha| linha.get(0))
        .map_err(|err| format!("Falha ao consultar os acessos: {err}"))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|err| format!("Falha ao ler os acessos: {err}"))?;

    Ok(secoes)
}

/// Substitui os acessos de um usuário pela lista informada.
/// O superusuário tem acesso total e não pode ser editado por aqui.
pub fn salvar_secoes_do_usuario(
    conn: &mut Connection,
    user_id: i64,
    secoes: &[String],
) -> Result<(), String> {
    if e_administrador(conn, user_id)? {
        return Err(
            "O superusuário tem acesso total ao sistema e não precisa de permissões.".to_string(),
        );
    }

    let transacao = conn
        .transaction()
        .map_err(|err| format!("Falha ao iniciar a edição de acessos: {err}"))?;

    transacao
        .execute("DELETE FROM user_permissions WHERE user_id = ?1", [user_id])
        .map_err(|err| format!("Falha ao limpar os acessos atuais: {err}"))?;

    for secao in secoes {
        transacao
            .execute(
                "INSERT INTO user_permissions (user_id, section_id) VALUES (?1, ?2)",
                rusqlite::params![user_id, secao],
            )
            .map_err(|err| format!("Falha ao salvar o acesso \"{secao}\": {err}"))?;
    }

    // Marca o usuário como alterado — o push da sincronização usa isso para
    // enviar a nova lista de acessos para a nuvem.
    transacao
        .execute(
            "UPDATE users SET updated_at = datetime('now') WHERE id = ?1",
            [user_id],
        )
        .map_err(|err| format!("Falha ao registrar a alteração de acessos: {err}"))?;

    transacao
        .commit()
        .map_err(|err| format!("Falha ao finalizar a edição de acessos: {err}"))?;

    Ok(())
}
