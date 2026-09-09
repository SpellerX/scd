/**
 * Utilitários de CNPJ (frontend).
 * A padronização definitiva acontece no backend (normalizar_cnpj);
 * aqui só ajudamos a digitar e exibir no padrão 00.000.000/0000-00.
 */
import { apenasDigitos } from "./digitos";

export { apenasDigitos }; // mantido para quem já importava daqui

/** Formata até 14 dígitos como CNPJ: 00.000.000/0000-00. */
export function formatarCnpj(digitos: string): string {
  const d = apenasDigitos(digitos).slice(0, 14);
  return d
    .replace(/^(\d{2})(\d)/, "$1.$2")
    .replace(/^(\d{2})\.(\d{3})(\d)/, "$1.$2.$3")
    .replace(/\.(\d{3})(\d)/, ".$1/$2")
    .replace(/(\d{4})(\d)/, "$1-$2");
}

/** Um CNPJ é válido quando tem exatamente 14 dígitos. */
export function cnpjValido(digitos: string): boolean {
  return apenasDigitos(digitos).length === 14;
}
