//! Registro dos módulos de comando Tauri.
//!
//! Cada área de comandos do backend vira um `pub mod` aqui.
//! Exemplo: para criar comandos de "demandas", crie `commands/demandas.rs`
//! e adicione `pub mod demandas;` neste arquivo.

pub mod auth;
pub mod empresas;
pub mod ferias;
pub mod funcionarios;
pub mod sync;
pub mod usuarios;
