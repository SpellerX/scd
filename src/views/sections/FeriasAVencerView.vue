<script setup lang="ts">
// Seção "Férias a vencer" (menu Cadastro): direitos adquiridos dentro do
// prazo concessivo, com o alerta configurável (N dias antes do vencimento).
// Quando um período entra na janela do alerta, ele aparece no sino do app.
import { onMounted, ref } from "vue";
import { mensagemDeErro } from "../../utils/erros";
import { formatarDataBr } from "../../utils/data";
import { useFerias } from "../../composables/useFerias";

const { aVencer, carregando, erro, carregar, obterAlertaDias, definirAlertaDias } = useFerias();

const alertaDias = ref<number>(15);
const configurado = ref(false);
const salvandoAlerta = ref(false);
const erroAlerta = ref<string | null>(null);

onMounted(async () => {
  try {
    alertaDias.value = await obterAlertaDias();
  } catch {
    alertaDias.value = 15;
  }
  await carregar();
});

async function salvarAlerta() {
  erroAlerta.value = null;
  salvandoAlerta.value = true;
  try {
    await definirAlertaDias(alertaDias.value);
    configurado.value = true;
    await carregar(); // recalcula dentro_alerta com o novo limite
  } catch (err) {
    erroAlerta.value = mensagemDeErro(err);
  } finally {
    salvandoAlerta.value = false;
  }
}

/** Texto/badge para os dias restantes. */
function seloDePrazo(dias: number, alerta: boolean) {
  if (dias <= 0) return { texto: "vencem hoje", alerta: true };
  if (dias === 1) return { texto: "vencem amanhã", alerta: true };
  if (alerta) return { texto: `vencem em ${dias} dias ⚠`, alerta: true };
  return { texto: `faltam ${dias} dias`, alerta: false };
}
</script>

<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold">Férias a vencer</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        Direitos adquiridos que ainda estão dentro do prazo para gozar.
        Configure o alerta para ser avisado antes do vencimento.
      </p>
    </div>

    <!-- Configuração do alerta -->
    <section
      class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
    >
      <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
        Alerta de vencimento
      </h2>
      <form class="mt-3 flex flex-wrap items-end gap-3" @submit.prevent="salvarAlerta">
        <div class="flex flex-col gap-1.5">
          <label for="alerta-dias" class="text-sm font-medium">Avisar com antecedência de</label>
          <input
            id="alerta-dias"
            v-model.number="alertaDias"
            type="number"
            min="1"
            max="365"
            class="w-28 rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
          />
        </div>
        <button
          type="submit"
          :disabled="salvandoAlerta"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow transition hover:bg-indigo-500 disabled:opacity-60"
        >
          {{ salvandoAlerta ? "Salvando..." : "Salvar alerta" }}
        </button>
        <p class="w-full text-xs text-neutral-400 dark:text-neutral-500">
          Quando faltar esse número de dias para o prazo expirar (ou na data
          do vencimento), o período aparece como notificação no sino do app.
        </p>
        <p
          v-if="configurado"
          role="status"
          class="w-full text-sm text-emerald-600 dark:text-emerald-400"
        >
          Alerta salvo!
        </p>
        <p
          v-if="erroAlerta"
          role="alert"
          class="w-full rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
        >
          {{ erroAlerta }}
        </p>
      </form>
    </section>

    <p
      v-if="erro"
      role="alert"
      class="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erro }}
    </p>

    <p v-if="carregando" class="text-sm text-neutral-500 dark:text-neutral-400" role="status">
      Carregando férias…
    </p>

    <div
      v-else-if="aVencer.length === 0"
      class="rounded-2xl border border-dashed border-neutral-300 bg-white p-10 text-center dark:border-neutral-700 dark:bg-neutral-800"
    >
      <p class="text-3xl">🗓️</p>
      <p class="mt-2 text-sm font-medium">Nenhum período a vencer no momento.</p>
    </div>

    <div
      v-else
      class="overflow-x-auto rounded-2xl border border-neutral-200 bg-white shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
    >
      <table class="w-full min-w-[760px] border-collapse text-left text-sm">
        <thead>
          <tr class="border-b border-neutral-200 text-xs uppercase tracking-wide text-neutral-400 dark:border-neutral-700">
            <th class="px-4 py-2.5 font-medium">Funcionário</th>
            <th class="px-4 py-2.5 font-medium">Empresa</th>
            <th class="px-4 py-2.5 font-medium">Período</th>
            <th class="px-4 py-2.5 font-medium">Prazo final</th>
            <th class="px-4 py-2.5 font-medium">Situação</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="item in aVencer"
            :key="item.id"
            class="border-b border-neutral-100 last:border-0 dark:border-neutral-700/50"
            :class="item.dentro_alerta ? 'bg-amber-50/60 dark:bg-amber-950/20' : ''"
          >
            <td class="px-4 py-3 font-medium">{{ item.funcionario_nome }}</td>
            <td class="px-4 py-3 text-neutral-500 dark:text-neutral-400">{{ item.empresa_nome }}</td>
            <td class="px-4 py-3">
              {{ formatarDataBr(item.inicio) }} → {{ formatarDataBr(item.vencimento) }}
            </td>
            <td class="px-4 py-3">{{ formatarDataBr(item.vencimento) }}</td>
            <td class="px-4 py-3">
              <span
                class="rounded-full px-2.5 py-1 text-xs font-semibold"
                :class="
                  seloDePrazo(item.dias_restantes, item.dentro_alerta).alerta
                    ? 'bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300'
                    : 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300'
                "
              >
                {{ seloDePrazo(item.dias_restantes, item.dentro_alerta).texto }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
