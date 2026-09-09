/** Formata "yyyy-mm-dd" (ISO, vinda do banco) para exibição "dd/mm/aaaa". */
export function formatarDataBr(iso: string): string {
  if (!iso) return "—";
  const partes = iso.split("-");
  if (partes.length !== 3) return iso;
  return `${partes[2]}/${partes[1]}/${partes[0]}`;
}
