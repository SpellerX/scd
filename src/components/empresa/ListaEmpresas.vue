<script setup lang="ts">
// Listagem das empresas cadastradas (estado compartilhado via useEmpresas).
// Fluxos de exclusão:
//  - individual: botão "Excluir" na linha;
//  - em lote: botão "Selecionar" ativa checkboxes → "Excluir selecionadas (N)".
import { computed, ref } from "vue";
import { confirm } from "@tauri-apps/plugin-dialog";
import { mensagemDeErro } from "../../utils/erros";
import { formatarCnpj } from "../../utils/cnpj";
import { useEmpresas, type Empresa } from "../../composables/useEmpresas";

const { empresas, carregandoLista, erroLista, remover, removerVarias, listar } = useEmpresas();

/** Modo de seleção em lote (checkboxes visíveis). */
const modoSelecao = ref(false);
/** Ids das empresas marcadas. */
const selecionadas = ref<number[]>([]);

/** id da empresa com exclusão individual em andamento. */
const removendoId = ref<number | null>(null);
/** Exclusão em lote em andamento. */
const removendoEmLote = ref(false);
/** Erro de exclusão (individual ou em lote). */
const erroExcluir = ref<string | null>(null);

const selecionadasTotal = computed(() => selecionadas.value.length);
const todasSelecionadas = computed(
  () => empresas.value.length > 0 && selecionadas.value.length === empresas.value.length,
);

function alternarModoSelecao() {
  modoSelecao.value = !modoSelecao.value;
  if (!modoSelecao.value) selecionadas.value = [];
}

function alternarSelecao(id: number) {
  const indice = selecionadas.value.indexOf(id);
  if (indice === -1) {
    selecionadas.value.push(id);
  } else {
    selecionadas.value.splice(indice, 1);
  }
}

function alternarSelecaoTodas() {
  selecionadas.value = todasSelecionadas.value
    ? []
    : empresas.value.map((empresa) => empresa.id);
}

async function excluirSelecionadas() {
  if (selecionadasTotal.value === 0) return;

  erroExcluir.value = null;
  const confirmou = await confirm(
    `Excluir ${selecionadasTotal.value} empresa(s) selecionada(s)?\nEsta ação não pode ser desfeita.`,
    { title: "Excluir em lote", kind: "warning", okLabel: "Excluir", cancelLabel: "Cancelar" },
  );
  if (!confirmou) return;

  removendoEmLote.value = true;
  try {
    const idsSelecionados = [...selecionadas.value];
    const resultado = await removerVarias(idsSelecionados);

    selecionadas.value = [];
    modoSelecao.value = false;
    await listar();

    // Empresas com funcionários vinculados são mantidas (e informadas).
    if (resultado.bloqueadas > 0) {
      erroExcluir.value = `${resultado.bloqueadas} empresa(s) não foi(ram) removida(s) por terem funcionários vinculados.`;
    } else if (resultado.removidas === 0) {
      erroExcluir.value = "Nenhuma empresa foi removida.";
    }
  } catch (err) {
    erroExcluir.value = mensagemDeErro(err, "Não foi possível excluir as empresas.");
  } finally {
    removendoEmLote.value = false;
  }
}

async function excluirUnica(empresa: Empresa) {
  erroExcluir.value = null;

  const confirmou = await confirm(
    `Excluir "${empresa.razao_social}" (${formatarCnpj(empresa.cnpj)})?\nEsta ação não pode ser desfeita.`,
    { title: "Excluir empresa", kind: "warning", okLabel: "Excluir", cancelLabel: "Cancelar" },
  );
  if (!confirmou) return;

  removendoId.value = empresa.id;
  try {
    await remover(empresa.id);
    await listar();
  } catch (err) {
    erroExcluir.value = mensagemDeErro(err, "Não foi possível excluir a empresa.");
  } finally {
    removendoId.value = null;
  }
}
</script>

<template>
  <section
    class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
  >
    <div class="flex flex-wrap items-center justify-between gap-2">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
        Empresas cadastradas
      </h2>

      <div class="flex items-center gap-2">
        <span
          class="rounded-full bg-neutral-100 px-2.5 py-0.5 text-xs font-medium text-neutral-600 dark:bg-neutral-700 dark:text-neutral-300"
        >
          {{ empresas.length }}
        </span>

        <!-- Entrar/sair do modo de seleção em lote -->
        <button
          v-if="!modoSelecao"
          type="button"
          @click="alternarModoSelecao"
          class="rounded-lg border border-neutral-300 px-3 py-1 text-xs font-medium text-neutral-700 transition hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-neutral-600 dark:text-neutral-200 dark:hover:bg-neutral-800"
          :disabled="empresas.length === 0"
        >
          Selecionar
        </button>
        <button
          v-else
          type="button"
          @click="alternarModoSelecao"
          class="rounded-lg border border-neutral-300 px-3 py-1 text-xs font-medium text-neutral-700 transition hover:bg-neutral-100 dark:border-neutral-600 dark:text-neutral-200 dark:hover:bg-neutral-800"
        >
          Cancelar seleção
        </button>
      </div>
    </div>

    <!-- Barra de ações da seleção em lote -->
    <div
      v-if="modoSelecao"
      class="mt-4 flex flex-wrap items-center gap-3 rounded-lg bg-indigo-50 px-3 py-2 dark:bg-indigo-950/60"
    >
      <span class="text-sm font-medium text-indigo-800 dark:text-indigo-200">
        {{ selecionadasTotal }} selecionada(s)
      </span>
      <div class="flex-1" />
      <button
        type="button"
        :disabled="selecionadasTotal === 0 || removendoEmLote"
        @click="excluirSelecionadas"
        class="rounded-lg bg-red-600 px-3 py-1.5 text-xs font-semibold text-white shadow transition hover:bg-red-500 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {{ removendoEmLote ? "Excluindo..." : `Excluir selecionadas (${selecionadasTotal})` }}
      </button>
    </div>

    <p
      v-if="carregandoLista"
      class="mt-4 text-sm text-neutral-500 dark:text-neutral-400"
      role="status"
    >
      Carregando empresas…
    </p>

    <p
      v-else-if="erroLista"
      role="alert"
      class="mt-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erroLista }}
    </p>

    <p
      v-if="erroExcluir"
      role="alert"
      class="mt-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erroExcluir }}
    </p>

    <p
      v-else-if="empresas.length === 0"
      class="mt-4 rounded-lg border border-dashed border-neutral-300 p-6 text-center text-sm text-neutral-400 dark:border-neutral-600"
    >
      Nenhuma empresa cadastrada ainda. Busque por CNPJ ou importe uma
      planilha acima.
    </p>

    <!-- Tabela (rolagem horizontal em telas estreitas) -->
    <div v-else class="mt-4 overflow-x-auto">
      <table class="w-full min-w-[760px] border-collapse text-left text-sm">
        <thead>
          <tr class="border-b border-neutral-200 text-xs uppercase tracking-wide text-neutral-400 dark:border-neutral-700">
            <th v-if="modoSelecao" class="w-10 py-2 pr-2 font-medium">
              <input
                type="checkbox"
                aria-label="Selecionar todas as empresas"
                :checked="todasSelecionadas"
                @change="alternarSelecaoTodas"
                class="h-4 w-4 accent-indigo-600"
              />
            </th>
            <th class="py-2 pr-4 font-medium">CNPJ</th>
            <th class="py-2 pr-4 font-medium">Razão social</th>
            <th class="py-2 pr-4 font-medium">Fantasia</th>
            <th class="py-2 pr-4 font-medium">UF</th>
            <th class="py-2 pr-4 font-medium">Situação</th>
            <th v-if="!modoSelecao" class="py-2 text-right font-medium">Ações</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="empresa in empresas"
            :key="empresa.id"
            class="border-b border-neutral-100 last:border-0 dark:border-neutral-700/50"
            :class="modoSelecao && selecionadas.includes(empresa.id) ? 'bg-indigo-50/60 dark:bg-indigo-950/30' : ''"
          >
            <td v-if="modoSelecao" class="py-2.5 pr-2">
              <input
                type="checkbox"
                :id="`sel-empresa-${empresa.id}`"
                :checked="selecionadas.includes(empresa.id)"
                @change="alternarSelecao(empresa.id)"
                class="h-4 w-4 accent-indigo-600"
              />
            </td>
            <td class="py-2.5 pr-4 font-mono text-xs">
              <label v-if="modoSelecao" :for="`sel-empresa-${empresa.id}`" class="cursor-pointer">
                {{ formatarCnpj(empresa.cnpj) }}
              </label>
              <template v-else>{{ formatarCnpj(empresa.cnpj) }}</template>
            </td>
            <td class="py-2.5 pr-4 font-medium">{{ empresa.razao_social }}</td>
            <td class="py-2.5 pr-4 text-neutral-500 dark:text-neutral-400">
              {{ empresa.nome_fantasia || "—" }}
            </td>
            <td class="py-2.5 pr-4">{{ empresa.uf || "—" }}</td>
            <td class="py-2.5 pr-4">
              <span
                class="rounded-full px-2 py-0.5 text-xs font-medium"
                :class="
                  empresa.situacao?.toUpperCase() === 'ATIVA'
                    ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900 dark:text-emerald-200'
                    : 'bg-neutral-100 text-neutral-600 dark:bg-neutral-700 dark:text-neutral-300'
                "
              >
                {{ empresa.situacao || "—" }}
              </span>
            </td>
            <td v-if="!modoSelecao" class="py-2.5 text-right">
              <button
                type="button"
                :disabled="removendoId === empresa.id"
                @click="excluirUnica(empresa)"
                class="rounded-lg px-2.5 py-1 text-xs font-medium text-red-600 transition hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-50 dark:text-red-400 dark:hover:bg-red-950"
                :title="`Excluir ${empresa.razao_social}`"
              >
                {{ removendoId === empresa.id ? "Excluindo..." : "Excluir" }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
