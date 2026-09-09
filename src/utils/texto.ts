/**
 * Utilitário de normalização de texto para buscas.
 * Torna a busca insensível a MAIÚSCULAS/minúsculas e a acentos:
 * "jose" encontra "José", "CAIXA" encontra "caixa", etc.
 */
export function normalizarTexto(valor: string): string {
  return valor
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
    .trim();
}
