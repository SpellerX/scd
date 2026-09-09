//! Leitura de planilhas (.xls e .xlsx) para importação em lote.
//!
//! Usa a crate `calamine` (lê .xls, .xlsx, .ods etc. sem depender do Excel).
//! Este módulo só ENTREGA as células como texto — a interpretação das
//! colunas fica na camada de regras (`empresas.rs`).

use calamine::{open_workbook_auto, Data, Reader};

/// Lê a PRIMEIRA aba da planilha e devolve as células como texto,
/// linha a linha. A primeira linha é o cabeçalho.
pub fn ler_primeira_aba(caminho: &str) -> Result<Vec<Vec<String>>, String> {
    let mut pasta = open_workbook_auto(caminho)
        .map_err(|err| format!("Não foi possível abrir a planilha: {err}"))?;

    let range = pasta
        .worksheet_range_at(0)
        .ok_or_else(|| "A planilha não tem nenhuma aba.".to_string())?
        .map_err(|err| format!("Erro ao ler a planilha: {err}"))?;

    let linhas = range
        .rows()
        .map(|linha| linha.iter().map(celula_para_texto).collect())
        .collect();

    Ok(linhas)
}

/// Converte uma célula para texto limpo.
/// Células de data/fórmula sem valor não interessam à importação.
fn celula_para_texto(celula: &Data) -> String {
    match celula {
        Data::String(s) => s.trim().to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            // Números inteiros não podem virar "12345.0".
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{f:.0}")
            } else {
                f.to_string()
            }
        }
        Data::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}
