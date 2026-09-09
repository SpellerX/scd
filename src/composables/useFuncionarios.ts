import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Cadastro de funcionários: estado compartilhado + chamadas aos comandos Rust
 * (src-tauri/src/commands/funcionarios.rs). Mesmo padrão do useEmpresas:
 * refs em nível de módulo = todos os componentes enxergam a MESMA lista.
 */

/** Funcionário (espelho do struct Rust `Funcionario`). CPF é opcional. */
export interface Funcionario {
  /** Id UUID — chave única global (sincronização). */
  id: string;
  nome: string;
  cpf: string | null;
  data_admissao: string | null;
  /** Id UUID da empresa à qual o funcionário está vinculado. */
  empresa_id: string;
  empresa_nome: string;
  created_at: string;
}

/** Resultado da importação por planilha (espelho do struct Rust). */
export interface RelatorioImportacaoFuncionarios {
  total: number;
  criados: number;
  erros: { linha: number; motivo: string }[];
}

const funcionarios = ref<Funcionario[]>([]);
const carregandoLista = ref(false);
const erroLista = ref<string | null>(null);

export function useFuncionarios() {
  /** Atualiza a lista de funcionários cadastrados. */
  async function listar() {
    carregandoLista.value = true;
    erroLista.value = null;
    try {
      funcionarios.value = await invoke<Funcionario[]>("listar_funcionarios");
    } catch (err) {
      erroLista.value =
        typeof err === "string" ? err : "Não foi possível carregar os funcionários.";
    } finally {
      carregandoLista.value = false;
    }
  }

  /** Cadastra um funcionário e o vincula à empresa escolhida. CPF opcional. */
  function criar(dados: {
    nome: string;
    cpf: string;
    empresaId: string;
    dataAdmissao: string | null;
  }): Promise<Funcionario> {
    return invoke<Funcionario>("criar_funcionario", dados);
  }

  /**
   * Importa funcionários de um arquivo .xls/.xlsx.
   * `empresaId` é a empresa padrão (usada quando a planilha não tem CNPJ).
   */
  function importarPlanilha(
    caminho: string,
    empresaId: string | null,
  ): Promise<RelatorioImportacaoFuncionarios> {
    return invoke<RelatorioImportacaoFuncionarios>("importar_funcionarios_planilha", {
      caminho,
      empresaId,
    });
  }

  /** Exclui um funcionário (pelo id UUID). */
  function remover(id: string): Promise<void> {
    return invoke<void>("remover_funcionario", { id });
  }

  return {
    funcionarios,
    carregandoLista,
    erroLista,
    listar,
    criar,
    importarPlanilha,
    remover,
  };
}
