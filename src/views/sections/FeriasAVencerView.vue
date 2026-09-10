<script setup lang="ts">
// Seção "Férias a vencer" (menu Cadastro): direitos adquiridos dentro do
// prazo concessivo, com o alerta automático configurável (N dias antes do
// vencimento) e o ALARME MANUAL por período.
//
// Alarme manual: para as empresas que já agendam as férias do funcionário.
// O período entra na lista na hora (selo "Agendado") e o aviso dispara na data
// escolhida — sem esperar o alerta automático, que só começa 12 meses antes do
// vencimento. O alerta automático continua valendo para os demais períodos.
import { onMounted, ref } from "vue";
import { confirm } from "@tauri-apps/plugin-dialog";
import { mensagemDeErro } from "../../utils/erros";
import { formatarDataBr } from "../../utils/data";
import { seloDePrazo, seloDoAlarme } from "../../utils/ferias";
import { useFerias, type FeriasAVencer } from "../../composables/useFerias";
import { useNotificacoes } from "../../composables/useNotificacoes";
import AgendarFerias from "../../components/ferias/AgendarFerias.vue";

const {
  aVencer,
  carregando,
  erro,
  carregar,
  obterAlertaDias,
  definirAlertaDias,
  removerAlarme,
} = useFerias();
const { carregar: carregarNotificacoes } = useNotificacoes();

const alertaDias = ref<number>(15);
const configurado = ref(false);
const salvandoAlerta = ref(false);
const erroAlerta = ref<string | null>(null);

/** Pedido de agendamento para o formulário ("Agendar"/"Editar" numa linha). */
const pedido = ref<{ funcionarioId: string; periodoId: string } | null>(null);
const removendoId = ref<string | null>(null);
const erroAcao = ref<string | null>(null);

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

/** Recarrega lista + sino depois de mexer num alarme. */
async function atualizarTudo() {
  await carregar();
  await carregarNotificacoes();
}

/** Manda a linha para o formulário de agendamento. */
function agendar(item: FeriasAVencer) {
  erroAcao.value = null;
  pedido.value = { funcionarioId: item.funcionario_id, periodoId: item.id };
}

/** Remove o alarme direto na linha (com confirmação nativa). */
async function desagendar(item: FeriasAVencer) {
  erroAcao.value = null;
  const confirmou = await confirm(
    `Remover o alarme de ${item.funcionario_nome} (aviso em ${formatarDataBr(item.alarme_em ?? "")})?`,
    {
      title: "Remover alarme",
      kind: "warning",
      okLabel: "Remover",
      cancelLabel: "Cancelar",
    },
  );
  if (!confirmou) return;

  removendoId.value = item.id;
  try {
    await removerAlarme(item.id);
    await atualizarTudo();
  } catch (err) {
    erroAcao.value = mensagemDeErro(err, "Não foi possível remover o alarme.");
  } finally {
    removendoId.value = null;
  }
}

/** Texto/badge para os dias restantes (alerta automático). */
// `seloDePrazo` e `seloDoAlarme` vêm de utils/ferias.ts (funções puras,
// cobertas por testes automatizados: `npm test`).
</script>

<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold">Férias a vencer</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        Direitos adquiridos que ainda estão dentro do prazo para gozar.
        Configure o alerta automático ou agende um aviso para as férias que a
        empresa já marcou.
      </p>
    </div>

    <!-- Agendamento manual (alarme por funcionário/período) -->
    <AgendarFerias :pedido="pedido" @atualizado="atualizarTudo" />

    <!-- Configuração do alerta automático -->
    <section
      class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
    >
      <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
        Alerta de vencimento (automático)
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
          Vale para os períodos <strong>sem</strong> alarme agendado.
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

    <p
      v-if="erroAcao"
      role="alert"
      class="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erroAcao }}
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
      <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
        Se a empresa já marcou as férias de alguém, use "Agendar férias" acima.
      </p>
    </div>

    <div
      v-else
      class="overflow-x-auto rounded-2xl border border-neutral-200 bg-white shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
    >
      <table class="w-full min-w-[980px] border-collapse text-left text-sm">
        <thead>
          <tr class="border-b border-neutral-200 text-xs uppercase tracking-wide text-neutral-400 dark:border-neutral-700">
            <th class="px-4 py-2.5 font-medium">Funcionário</th>
            <th class="px-4 py-2.5 font-medium">Empresa</th>
            <th class="px-4 py-2.5 font-medium">Período</th>
            <th class="px-4 py-2.5 font-medium">Prazo final</th>
            <th class="px-4 py-2.5 font-medium">Situação</th>
            <th class="px-4 py-2.5 font-medium">Alarme</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="item in aVencer"
            :key="item.id"
            class="border-b border-neutral-100 last:border-0 dark:border-neutral-700/50"
            :class="
              item.alarme_disparado || item.dentro_alerta
                ? 'bg-amber-50/60 dark:bg-amber-950/20'
                : item.agendado
                  ? 'bg-indigo-50/40 dark:bg-indigo-950/20'
                  : ''
            "
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
              <span
                v-if="item.agendado && !item.na_janela_automatica"
                class="mt-1 block text-[11px] text-neutral-400 dark:text-neutral-500"
              >
                fora do prazo automático
              </span>
            </td>
            <td class="px-4 py-3">
              <div v-if="item.agendado" class="flex flex-col gap-1">
                <span
                  class="w-fit rounded-full px-2.5 py-1 text-xs font-semibold"
                  :class="
                    item.alarme_disparado
                      ? 'bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300'
                      : 'bg-indigo-100 text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300'
                  "
                >
                  Agendado para {{ formatarDataBr(item.alarme_em ?? "") }} ·
                  {{ seloDoAlarme(item.alarme_disparado, item.dias_para_alarme) }}
                </span>
                <span
                  v-if="item.alarme_observacao"
                  class="text-[11px] text-neutral-500 dark:text-neutral-400"
                >
                  {{ item.alarme_observacao }}
                </span>
                <span class="flex gap-2">
                  <button
                    type="button"
                    @click="agendar(item)"
                    class="rounded-lg px-2 py-1 text-[11px] font-semibold text-indigo-600 transition hover:bg-indigo-50 dark:text-indigo-300 dark:hover:bg-indigo-950/40"
                  >
                    Editar
                  </button>
                  <button
                    type="button"
                    :disabled="removendoId === item.id"
                    @click="desagendar(item)"
                    class="rounded-lg px-2 py-1 text-[11px] font-semibold text-neutral-500 transition hover:bg-neutral-100 disabled:opacity-60 dark:text-neutral-400 dark:hover:bg-neutral-700"
                  >
                    {{ removendoId === item.id ? "Removendo…" : "Remover" }}
                  </button>
                </span>
              </div>
              <button
                v-else
                type="button"
                @click="agendar(item)"
                class="rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-indigo-500"
              >
                Agendar
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
