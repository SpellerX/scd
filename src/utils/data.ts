/** Formata "yyyy-mm-dd" (ISO, vinda do banco) para exibição "dd/mm/aaaa". */
export function formatarDataBr(iso: string): string {
  if (!iso) return "—";
  const partes = iso.split("-");
  if (partes.length !== 3) return iso;
  return `${partes[2]}/${partes[1]}/${partes[0]}`;
}

/**
 * Data de hoje em ISO (yyyy-mm-dd) no fuso local — mesmo "hoje" usado pelo
 * backend (`date('now','localtime')`). Serve para `<input type="date">`
 * (valor inicial e limite mínimo).
 */
export function hojeIso(): string {
  const agora = new Date();
  const mes = String(agora.getMonth() + 1).padStart(2, "0");
  const dia = String(agora.getDate()).padStart(2, "0");
  return `${agora.getFullYear()}-${mes}-${dia}`;
}
