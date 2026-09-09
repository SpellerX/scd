/**
 * Função genérica de dígitos, compartilhada por CNPJ, CPF etc.
 * A padronização definitiva acontece no backend; aqui apenas limpamos a digitação.
 */

/** Remove tudo que não for dígito. */
export function apenasDigitos(valor: string): string {
  return valor.replace(/\D/g, "");
}
