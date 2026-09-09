<script setup lang="ts">
// Importação em lote de funcionários por planilha (.xls ou .xlsx).
// Empresa por linha (coluna "cnpj") quando houver; senão usa a empresa
// escolhida aqui (empresa padrão). Mostra relatório com criados/erros.
import { computed, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { mensagemDeErro } from "../../utils/erros";
import { useEmpresas } from "../../composables/useEmpresas";
import {
  useFuncionarios,
  type RelatorioImportacaoFuncionarios,
} from "../../composables/useFuncionarios";
import SelectBusca from "../ui/SelectBusca.vue";

const { empresas } = useEmpresas();
const { importarPlanilha, listar } = useFuncionarios();

const empresaPadraoId = ref<number | null>(null);
const importando = ref(false);
const erro = ref<string | null>(null);
const relatorio = ref<RelatorioImportacaoFuncionarios | null>(null);

const opcoesEmpresas = computed(() =>
  empresas.value.map((empresa) => ({ id: empresa.id, label: empresa.razao_social })),
);

async function escolherArquivo() {
  erro.value = null;
  relatorio.value = null;

  const caminho = await open({
    multiple: false,
    filters: [{ name: "Planilhas Excel", extensions: ["xls", "xlsx"] }],
  });
  if (!caminho) return; // usuário cancelou

  importando.value = true;
  try {
    relatorio.value = await importarPlanilha(caminho, empresaPadraoId.value);
    await listar();
  } catch (err) {
    erro.value = mensagemDeErro(err);
  } finally {
    importando.value = false;
  }
}
</script>

<template>
  <section
    class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
  >
    <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
      Importar planilha
    </h2>
    <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
      Adiciona vários funcionários de uma vez (.xls ou .xlsx) — ideal para
      relatórios exportados de outros sistemas.
    </p>

    <!-- Regras de formatação da planilha -->
    <ul class="mt-3 list-inside list-disc space-y-1 text-xs text-neutral-500 dark:text-neutral-400">
      <li>
        Colunas obrigatórias: <strong>nome</strong> e <strong>admissão</strong>
        (data — usada para gerar as férias automaticamente).
      </li>
      <li>Opcional: <strong>cpf</strong>.</li>
      <li>
        Empresa: se a planilha tiver coluna <strong>cnpj</strong>, cada linha
        é vinculada à empresa dela; senão, todas usam a empresa escolhida
        abaixo (obrigatória nesse caso).
      </li>
      <li>CPF repetido é pulado com aviso; linha sem empresa vira erro no relatório.</li>
    </ul>

    <div class="mt-4 flex flex-col gap-1.5">
      <label for="imp-empresa" class="text-sm font-medium">
        Empresa padrão <span v-if="!empresaPadraoId" class="text-neutral-400">(se a planilha não tiver CNPJ)</span>
      </label>
      <SelectBusca
        v-model="empresaPadraoId"
        :options="opcoesEmpresas"
        input-id="imp-empresa"
        placeholder="Busque a empresa (ou deixe para o CNPJ da planilha)..."
      />
    </div>

    <button
      type="button"
      :disabled="importando"
      @click="escolherArquivo"
      class="mt-4 rounded-lg border border-indigo-300 bg-indigo-50 px-4 py-2 text-sm font-medium text-indigo-700 transition hover:bg-indigo-100 disabled:cursor-not-allowed disabled:opacity-60 dark:border-indigo-800 dark:bg-indigo-950 dark:text-indigo-300 dark:hover:bg-indigo-900"
    >
      {{ importando ? "Importando..." : "Selecionar planilha..." }}
    </button>

    <p
      v-if="erro"
      role="alert"
      class="mt-3 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erro }}
    </p>

    <!-- Resumo da importação -->
    <div v-if="relatorio" class="mt-4 space-y-3">
      <div class="flex flex-wrap gap-2 text-sm">
        <span class="rounded-full bg-neutral-100 px-3 py-1 font-medium dark:bg-neutral-700">
          Total: {{ relatorio.total }}
        </span>
        <span class="rounded-full bg-emerald-100 px-3 py-1 font-medium text-emerald-800 dark:bg-emerald-900 dark:text-emerald-200">
          Criados: {{ relatorio.criados }}
        </span>
        <span v-if="relatorio.erros.length > 0" class="rounded-full bg-red-100 px-3 py-1 font-medium text-red-700 dark:bg-red-900/60 dark:text-red-300">
          Erros: {{ relatorio.erros.length }}
        </span>
      </div>

      <div v-if="relatorio.erros.length > 0">
        <h3 class="text-sm font-semibold text-red-700 dark:text-red-300">Linhas com problema</h3>
        <ul class="mt-2 max-h-40 space-y-1 overflow-y-auto rounded-lg bg-red-50 p-3 text-xs text-red-700 dark:bg-red-950 dark:text-red-300">
          <li v-for="(item, i) in relatorio.erros" :key="i">
            <strong>Linha {{ item.linha }}:</strong> {{ item.motivo }}
          </li>
        </ul>
      </div>
    </div>
  </section>
</template>
