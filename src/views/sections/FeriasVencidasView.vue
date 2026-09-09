<script setup lang="ts">
// Seção "Férias vencidas" (menu Cadastro): períodos cujo prazo já passou.
// Para regularizar (funcionário já gozou/quitou), abra a linha e registre
// a observação — a pendência some da lista e do sino de notificações.
import { onMounted, ref } from "vue";
import { mensagemDeErro } from "../../utils/erros";
import { formatarDataBr } from "../../utils/data";
import { useFerias } from "../../composables/useFerias";
import { useNotificacoes } from "../../composables/useNotificacoes";

const { vencidas, carregando, erro, carregar, regularizar } = useFerias();
const { carregar: carregarNotificacoes } = useNotificacoes();

const linhaEmEdicao = ref<number | null>(null);
const observacao = ref("");
const salvando = ref(false);
const erroAcao = ref<string | null>(null);
const sucesso = ref<string | null>(null);

onMounted(() => {
  void carregar();
});

function iniciarEdicao(id: number) {
  erroAcao.value = null;
  sucesso.value = null;
  observacao.value = "";
  linhaEmEdicao.value = id;
}

async function confirmarRegularizacao(id: number) {
  erroAcao.value = null;
  salvando.value = true;
  try {
    await regularizar(id, observacao.value.trim() || null);
    sucesso.value = "Férias regularizadas! A pendência foi encerrada.";
    linhaEmEdicao.value = null;
    await carregar();
    await carregarNotificacoes();
  } catch (err) {
    erroAcao.value = mensagemDeErro(err, "Não foi possível regularizar as férias.");
  } finally {
    salvando.value = false;
  }
}
</script>

<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold">Férias vencidas</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        Períodos cujo prazo para gozar já passou. Registre a regularização
        quando o funcionário já tiver gozado essas férias.
      </p>
    </div>

    <p
      v-if="erro"
      role="alert"
      class="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erro }}
    </p>

    <p
      v-if="sucesso"
      role="status"
      class="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300"
    >
      {{ sucesso }}
    </p>

    <p
      v-if="erroAcao"
      role="alert"
      class="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erroAcao }}
    </p>

    <p
      v-if="carregando"
      class="text-sm text-neutral-500 dark:text-neutral-400"
      role="status"
    >
      Carregando férias…
    </p>

    <!-- Lista vazia = tudo em dia -->
    <div
      v-else-if="vencidas.length === 0"
      class="rounded-2xl border border-dashed border-neutral-300 bg-white p-10 text-center dark:border-neutral-700 dark:bg-neutral-800"
    >
      <p class="text-3xl">🎉</p>
      <p class="mt-2 text-sm font-medium">Nenhuma férias vencida.</p>
      <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
        Os períodos são gerados automaticamente pela data de admissão.
      </p>
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
            <th class="px-4 py-2.5 font-medium">Prazo vencido em</th>
            <th class="px-4 py-2.5 font-medium">Dias</th>
            <th class="px-4 py-2.5 text-right font-medium">Ações</th>
          </tr>
        </thead>
        <tbody>
          <template v-for="item in vencidas" :key="item.id">
            <tr class="border-b border-neutral-100 last:border-0 dark:border-neutral-700/50">
              <td class="px-4 py-3 font-medium">{{ item.funcionario_nome }}</td>
              <td class="px-4 py-3 text-neutral-500 dark:text-neutral-400">{{ item.empresa_nome }}</td>
              <td class="px-4 py-3">
                {{ formatarDataBr(item.inicio) }} → {{ formatarDataBr(item.vencimento) }}
              </td>
              <td class="px-4 py-3">{{ formatarDataBr(item.vencimento) }}</td>
              <td class="px-4 py-3">
                <span class="rounded-full bg-red-100 px-2 py-0.5 text-xs font-semibold text-red-700 dark:bg-red-950 dark:text-red-300">
                  {{ item.dias_vencidas }} dia(s)
                </span>
              </td>
              <td class="px-4 py-3 text-right">
                <button
                  v-if="linhaEmEdicao !== item.id"
                  type="button"
                  @click="iniciarEdicao(item.id)"
                  class="rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-indigo-500"
                >
                  Regularizar
                </button>
              </td>
            </tr>

            <!-- Linha de regularização -->
            <tr v-if="linhaEmEdicao === item.id" class="bg-indigo-50/50 dark:bg-indigo-950/20">
              <td colspan="6" class="px-4 py-3">
                <form class="flex flex-wrap items-center gap-2" @submit.prevent="confirmarRegularizacao(item.id)">
                  <p class="w-full text-sm">
                    O funcionário já gozou as férias do período
                    {{ formatarDataBr(item.inicio) }} → {{ formatarDataBr(item.vencimento) }}?
                  </p>
                  <input
                    v-model="observacao"
                    type="text"
                    placeholder="Observação (opcional) — ex.: gozadas de 10/01 a 08/02"
                    class="min-w-0 flex-1 rounded-lg border border-neutral-300 bg-white px-3 py-1.5 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
                  />
                  <button
                    type="submit"
                    :disabled="salvando"
                    class="rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-emerald-500 disabled:opacity-60"
                  >
                    {{ salvando ? "Salvando..." : "Confirmar regularização" }}
                  </button>
                  <button
                    type="button"
                    @click="linhaEmEdicao = null"
                    class="rounded-lg px-3 py-1.5 text-xs font-medium text-neutral-500 transition hover:bg-neutral-100 dark:text-neutral-400 dark:hover:bg-neutral-700"
                  >
                    Cancelar
                  </button>
                </form>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>
  </div>
</template>
