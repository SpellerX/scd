/**
 * Utilitários de CPF (frontend).
 * A validação definitiva (dígitos verificadores) acontece no backend;
 * aqui só ajudamos a digitar e exibir no padrão 000.000.000-00.
 */
import { apenasDigitos } from "./digitos";

/** Formata até 11 dígitos como CPF: 000.000.000-00. */
export function formatarCpf(digitos: string): string {
  const d = apenasDigitos(digitos).slice(0, 11);
  return d
    .replace(/^(\d{3})(\d)/, "$1.$2")
    .replace(/^(\d{3})\.(\d{3})(\d)/, "$1.$2.$3")
    .replace(/\.(\d{3})(\d)/, ".$1-$2");
}

/** Um CPF é "bem formado" quando tem exatamente 11 dígitos. */
export function cpfValido(digitos: string): boolean {
  return apenasDigitos(digitos).length === 11;
}
