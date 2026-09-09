/**
 * Converte o erro recebido de um comando Tauri (invoke) numa mensagem amigável.
 * Comandos Rust que retornam Err chegam ao JS como string — qualquer outra
 * coisa (rede, serialização) cai no padrão informado.
 */
export function mensagemDeErro(
  erro: unknown,
  padrao = "Algo deu errado. Tente novamente.",
): string {
  return typeof erro === "string" && erro.trim() !== "" ? erro : padrao;
}
