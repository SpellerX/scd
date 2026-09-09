import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Cadastro de empresas: estado compartilhado + chamadas aos comandos Rust
 * (src-tauri/src/commands/empresas.rs). Mesmo padrão do useAuth:
 * refs em nível de módulo = todos os componentes enxergam a MESMA lista.
 */

/** Empresa cadastrada no banco (espelho do struct Rust `Empresa`). */
export interface Empresa {
  /** Id UUID — chave única global (necessária para a sincronização). */
  id: string;
  cnpj: string;
  razao_social: string;
  nome_fantasia: string | null;
  situacao: string | null;
  municipio: string | null;
  uf: string | null;
  telefone: string | null;
  email: string | null;
  created_at: string;
}

/** Dados vindos da BrasilAPI (ainda não salvos — espelho de `DadosEmpresa`). */
export type DadosEmpresa = Omit<Empresa, "id" | "created_at">;

/** Resultado da importação por planilha (espelho de `RelatorioImportacao`). */
export interface RelatorioImportacao {
  total: number;
  criadas: number;
  duplicadas: number;
  erros: { linha: number; motivo: string }[];
}

/** Resultado da exclusão em lote (espelho de `ResultadoRemocao`). */
export interface ResultadoRemocao {
  removidas: number;
  bloqueadas: number;
}

const empresas = ref<Empresa[]>([]);
const carregandoLista = ref(false);
const erroLista = ref<string | null>(null);

export function useEmpresas() {
  /** Atualiza a lista de empresas cadastradas. */
  async function listar() {
    carregandoLista.value = true;
    erroLista.value = null;
    try {
      empresas.value = await invoke<Empresa[]>("listar_empresas");
    } catch (err) {
      erroLista.value =
        typeof err === "string" ? err : "Não foi possível carregar as empresas.";
    } finally {
      carregandoLista.value = false;
    }
  }

  /** Consulta os dados de um CNPJ na BrasilAPI (sem salvar). */
  function buscarPorCnpj(cnpj: string): Promise<DadosEmpresa> {
    return invoke<DadosEmpresa>("buscar_empresa_por_cnpj", { cnpj });
  }

  /**
   * Grava os dados já conferidos na tela (recusa CNPJ duplicado).
   * Não consulta a BrasilAPI de novo — a busca foi feita uma única vez.
   */
  function salvar(dados: DadosEmpresa): Promise<Empresa> {
    return invoke<Empresa>("salvar_empresa", { dados });
  }

  /** Importa empresas de um arquivo .xls/.xlsx (caminho absoluto). */
  function importarPlanilha(caminho: string): Promise<RelatorioImportacao> {
    return invoke<RelatorioImportacao>("importar_empresas_planilha", { caminho });
  }

  /** Exclui uma empresa cadastrada (pelo id UUID). */
  function remover(id: string): Promise<void> {
    return invoke<void>("remover_empresa", { id });
  }

  /** Exclui várias empresas de uma vez (ids UUID das linhas selecionadas). */
  function removerVarias(ids: string[]): Promise<ResultadoRemocao> {
    return invoke<ResultadoRemocao>("remover_empresas", { ids });
  }

  return {
    empresas,
    carregandoLista,
    erroLista,
    listar,
    buscarPorCnpj,
    salvar,
    remover,
    removerVarias,
    importarPlanilha,
  };
}
