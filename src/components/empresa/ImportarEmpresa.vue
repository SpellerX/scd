<script setup lang="ts">
// Importação em lote de empresas por planilha (.xls ou .xlsx).
// Abre o seletor de arquivos, envia o caminho ao backend e mostra o
// andamento em tempo real (evento "importacao-progresso") + relatório final.
import { onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { mensagemDeErro } from "../../utils/erros";
import { useEmpresas, type RelatorioImportacao } from "../../composables/useEmpresas";

const { importarPlanilha, listar } = useEmpresas();

const importando = ref(false);
const erro = ref<string | null>(null);
const relatorio = ref<RelatorioImportacao | null>(null);

/** Progresso recebido do backend (evento "importacao-progresso"). */
const progresso = ref<{ linha: number; processadas: number; total: number } | null>(null);

let desinscrever: UnlistenFn | null = null;

onMounted(async () => {
  desinscrever = await listen<{ linha: number; processadas: number; total: number }>(
    "importacao-progresso",
    (evento) => {
      progresso.value = evento.payload;
    },
  );
});

onUnmounted(() => {
  desinscrever?.();
});

function percentual(): number {
  const total = progresso.value?.total ?? 0;
  const feitas = progresso.value?.processadas ?? 0;
  if (total === 0) return 0;
  return Math.min(100, Math.round((feitas / total) * 100));
}

async function escolherArquivo() {
  erro.value = null;
  relatorio.value = null;
  progresso.value = null;

  // Seletor nativo de arquivos (plugin dialog do Tauri).
  const caminho = await open({
    multiple: false,
    filters: [{ name: "Planilhas Excel", extensions: ["xls", "xlsx"] }],
  });
  if (!caminho) return; // usuário cancelou

  importando.value = true;
  try {
    relatorio.value = await importarPlanilha(caminho);
    await listar();
  } catch (err) {
    erro.value = mensagemDeErro(err);
  } finally {
    importando.value = false;
    progresso.value = null;
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
      Cadastra várias empresas de uma vez (.xls ou .xlsx).
    </p>

    <!-- Regras de formatação da planilha -->
    <ul class="mt-3 list-inside list-disc space-y-1 text-xs text-neutral-500 dark:text-neutral-400">
      <li>A 1ª linha deve ser o cabeçalho com uma coluna contendo "cnpj".</li>
      <li>Opcionais: razão social (coluna com "raz") e fantasia (coluna com "fantas").</li>
      <li>Linhas sem razão social são completadas pelas fontes públicas (Minha Receita/BrasilAPI).</li>
      <li>Dica: deixe a coluna de CNPJ formatada como texto na planilha.</li>
    </ul>

    <button
      type="button"
      :disabled="importando"
      @click="escolherArquivo"
      class="mt-4 rounded-lg border border-indigo-300 bg-indigo-50 px-4 py-2 text-sm font-medium text-indigo-700 transition hover:bg-indigo-100 disabled:cursor-not-allowed disabled:opacity-60 dark:border-indigo-800 dark:bg-indigo-950 dark:text-indigo-300 dark:hover:bg-indigo-900"
    >
      {{ importando ? "Importando..." : "Selecionar planilha..." }}
    </button>

    <!-- Andamento em tempo real -->
    <div v-if="importando && progresso" class="mt-4 space-y-1.5" role="status">
      <p class="text-sm text-neutral-600 dark:text-neutral-300">
        Processando linha {{ progresso.linha }} de {{ progresso.total }}...
      </p>
      <div class="h-2 w-full overflow-hidden rounded-full bg-neutral-200 dark:bg-neutral-700">
        <div
          class="h-full rounded-full bg-indigo-600 transition-all"
          :style="{ width: percentual() + '%' }"
        />
      </div>
      <p class="text-xs text-neutral-400 dark:text-neutral-500">
        Linhas sem razão social consultam a Receita e podem levar alguns
        segundos cada.
      </p>
    </div>

    <p
      v-else-if="erro"
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
          Criadas: {{ relatorio.criadas }}
        </span>
        <span class="rounded-full bg-amber-100 px-3 py-1 font-medium text-amber-800 dark:bg-amber-900 dark:text-amber-200">
          Duplicadas: {{ relatorio.duplicadas }}
        </span>
      </div>

      <!-- Linhas com erro -->
      <div v-if="relatorio.erros.length > 0">
        <h3 class="text-sm font-semibold text-red-700 dark:text-red-300">
          Linhas com problema ({{ relatorio.erros.length }})
        </h3>
        <ul class="mt-2 max-h-40 space-y-1 overflow-y-auto rounded-lg bg-red-50 p-3 text-xs text-red-700 dark:bg-red-950 dark:text-red-300">
          <li v-for="(item, i) in relatorio.erros" :key="i">
            <strong>Linha {{ item.linha }}:</strong> {{ item.motivo }}
          </li>
        </ul>
      </div>
    </div>
  </section>
</template>
