<script setup lang="ts">
// Agendamento manual das férias ("alarme") — usado na tela "Férias a vencer".
//
// Por que existe: o alerta automático só começa 12 meses antes do vencimento
// do período. Quando a empresa JÁ marcou as férias do funcionário, não faz
// sentido esperar: aqui se escolhe o funcionário, o período e a data em que o
// app deve avisar. O período passa a aparecer na lista na hora (selo
// "Agendado") e o aviso dispara na data escolhida.
//
// O alerta automático continua valendo normalmente para os períodos SEM alarme.
import { computed, onMounted, ref, watch } from "vue";
import { mensagemDeErro } from "../../utils/erros";
import { hojeIso } from "../../utils/data";
import { rotuloDoPeriodo, validarAgendamento } from "../../utils/ferias";
import { useFerias, type PeriodoDoFuncionario } from "../../composables/useFerias";
import { useFuncionarios } from "../../composables/useFuncionarios";
import SelectBusca from "../ui/SelectBusca.vue";

/** Pedido vindo da lista ("Agendar"/"Editar" numa linha da tabela). */
const props = defineProps<{
  pedido?: { funcionarioId: string; periodoId: string } | null;
}>();

/** Avisa a tela para recarregar a lista e o sino de notificações. */
const emit = defineEmits<{ atualizado: [] }>();

const { definirAlarme, removerAlarme, listarPeriodosFuncionario } = useFerias();
const { funcionarios, listar: listarFuncionarios } = useFuncionarios();

const funcionarioId = ref<string | null>(null);
const periodoId = ref<string | null>(null);
const alarmeEm = ref("");
const observacao = ref("");

const periodos = ref<PeriodoDoFuncionario[]>([]);
const carregandoPeriodos = ref(false);
const salvando = ref(false);
const erro = ref<string | null>(null);
const sucesso = ref<string | null>(null);

/** Período pedido pela lista, selecionado assim que os períodos carregarem. */
const periodoPendente = ref<string | null>(null);

/** Hoje (local): valor inicial do aviso e limite mínimo do campo de data. */
const hoje = hojeIso();

/** Opções do SelectBusca (formato { id, label }) — nome + empresa. */
const opcoesFuncionarios = computed(() =>
  funcionarios.value.map((f) => ({ id: f.id, label: `${f.nome} — ${f.empresa_nome}` })),
);

const periodoSelecionado = computed(
  () => periodos.value.find((p) => p.id === periodoId.value) ?? null,
);

/** O período escolhido já tem alarme? (o botão vira "Atualizar alarme") */
const jaAgendado = computed(() => Boolean(periodoSelecionado.value?.alarme_em));

const podeSalvar = computed(
  () => Boolean(funcionarioId.value) && Boolean(periodoId.value) && Boolean(alarmeEm.value),
);

onMounted(async () => {
  if (funcionarios.value.length === 0) await listarFuncionarios();
  if (props.pedido) await aplicarPedido(props.pedido);
});

// Trocar o funcionário recarrega os períodos agendáveis dele e, se houver um
// pedido da lista esperando, já seleciona o período pedido.
watch(funcionarioId, async (id) => {
  await carregarPeriodos(id);
  selecionarPendente();
});

// Trocar o período traz o alarme já existente para os campos (edição).
watch(periodoId, () => {
  const periodo = periodoSelecionado.value;
  alarmeEm.value = periodo?.alarme_em ?? hoje;
  observacao.value = periodo?.alarme_observacao ?? "";
});

// Pedido vindo da lista (clique em "Agendar"/"Editar" numa linha).
watch(
  () => props.pedido,
  (pedido) => {
    if (pedido) void aplicarPedido(pedido);
  },
);

/** Carrega os períodos agendáveis (a vencer e não regularizados). */
async function carregarPeriodos(id: string | null) {
  periodoId.value = null;
  periodos.value = [];
  erro.value = null;
  if (!id) return;

  carregandoPeriodos.value = true;
  try {
    periodos.value = await listarPeriodosFuncionario(id);
  } catch (err) {
    erro.value = mensagemDeErro(err, "Não foi possível carregar os períodos do funcionário.");
  } finally {
    carregandoPeriodos.value = false;
  }
}

/** Abre o formulário já no funcionário/período pedidos pela lista. */
async function aplicarPedido(pedido: { funcionarioId: string; periodoId: string }) {
  sucesso.value = null;
  erro.value = null;
  periodoPendente.value = pedido.periodoId;

  if (funcionarioId.value === pedido.funcionarioId) {
    // Mesma pessoa: o `watch` não dispara, então carrega e seleciona aqui.
    await carregarPeriodos(pedido.funcionarioId);
    selecionarPendente();
    return;
  }
  // Pessoa diferente: o `watch` de funcionarioId carrega e seleciona.
  funcionarioId.value = pedido.funcionarioId;
}

/** Seleciona o período pedido (se ele ainda existir na lista carregada). */
function selecionarPendente() {
  const pendente = periodoPendente.value;
  periodoPendente.value = null;
  if (!pendente) return;
  if (periodos.value.some((p) => p.id === pendente)) periodoId.value = pendente;
}

/** Texto de cada opção do seletor de períodos (regra em utils/ferias.ts). */
function rotuloPeriodo(periodo: PeriodoDoFuncionario): string {
  return rotuloDoPeriodo(periodo);
}

async function salvar() {
  // Regra de validação compartilhada com os testes (utils/ferias.ts).
  const problema = validarAgendamento({
    funcionarioId: funcionarioId.value,
    periodoId: periodoId.value,
    alarmeEm: alarmeEm.value,
    hoje,
  });
  if (problema) {
    erro.value = problema;
    return;
  }
  erro.value = null;
  sucesso.value = null;
  salvando.value = true;
  const idSalvo = periodoId.value!;
  try {
    await definirAlarme(idSalvo, alarmeEm.value, observacao.value.trim() || null);
    sucesso.value = "Alarme salvo! O período aparece na lista e o aviso dispara na data escolhida.";

    // Recarrega mantendo a seleção, para o formulário refletir o que foi salvo.
    await carregarPeriodos(funcionarioId.value);
    periodoId.value = idSalvo;
    emit("atualizado");
  } catch (err) {
    erro.value = mensagemDeErro(err, "Não foi possível salvar o alarme.");
  } finally {
    salvando.value = false;
  }
}

async function remover() {
  const periodo = periodoSelecionado.value;
  if (!periodo) return;

  erro.value = null;
  sucesso.value = null;
  salvando.value = true;
  try {
    await removerAlarme(periodo.id);
    sucesso.value = "Alarme removido. O período volta a seguir o alerta automático.";
    await carregarPeriodos(funcionarioId.value);
    emit("atualizado");
  } catch (err) {
    erro.value = mensagemDeErro(err, "Não foi possível remover o alarme.");
  } finally {
    salvando.value = false;
  }
}
</script>

<template>
  <section
    class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
  >
    <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
      Agendar férias (alarme)
    </h2>
    <p class="mt-1 text-xs text-neutral-400 dark:text-neutral-500">
      Para quando a empresa já marcou as férias do funcionário: escolha a pessoa
      e o período, defina a data do aviso e o sistema avisa nesse dia — sem
      esperar o alerta automático, que só começa 12 meses antes do vencimento.
    </p>

    <form class="mt-4 grid gap-4 md:grid-cols-2" @submit.prevent="salvar">
      <div class="flex flex-col gap-1.5">
        <label for="agendar-funcionario" class="text-sm font-medium">Funcionário</label>
        <SelectBusca
          v-model="funcionarioId"
          input-id="agendar-funcionario"
          :options="opcoesFuncionarios"
          placeholder="Digite o nome do funcionário…"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <label for="agendar-periodo" class="text-sm font-medium">Período de férias</label>
        <select
          id="agendar-periodo"
          v-model="periodoId"
          :disabled="!funcionarioId || carregandoPeriodos || periodos.length === 0"
          class="rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 disabled:opacity-60 dark:border-neutral-600 dark:bg-neutral-900"
        >
          <option value="">
            {{
              !funcionarioId
                ? "Escolha primeiro o funcionário"
                : carregandoPeriodos
                  ? "Carregando períodos…"
                  : periodos.length === 0
                    ? "Nenhum período a vencer para este funcionário"
                    : "Selecione o período…"
            }}
          </option>
          <option v-for="periodo in periodos" :key="periodo.id" :value="periodo.id">
            {{ rotuloPeriodo(periodo) }}
          </option>
        </select>
      </div>

      <div class="flex flex-col gap-1.5">
        <label for="agendar-data" class="text-sm font-medium">Avisar em</label>
        <input
          id="agendar-data"
          v-model="alarmeEm"
          type="date"
          :min="hoje"
          class="rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <label for="agendar-observacao" class="text-sm font-medium">
          Observação <span class="font-normal text-neutral-400">(opcional)</span>
        </label>
        <input
          id="agendar-observacao"
          v-model="observacao"
          type="text"
          maxlength="200"
          placeholder="Ex.: férias agendadas de 10/03 a 05/04"
          class="rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
        />
      </div>

      <div class="flex flex-wrap items-center gap-2 md:col-span-2">
        <button
          type="submit"
          :disabled="salvando || !podeSalvar"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow transition hover:bg-indigo-500 disabled:opacity-60"
        >
          {{ salvando ? "Salvando..." : jaAgendado ? "Atualizar alarme" : "Salvar alarme" }}
        </button>
        <button
          v-if="jaAgendado"
          type="button"
          :disabled="salvando"
          @click="remover"
          class="rounded-lg px-4 py-2 text-sm font-medium text-neutral-500 transition hover:bg-neutral-100 disabled:opacity-60 dark:text-neutral-400 dark:hover:bg-neutral-700"
        >
          Remover alarme
        </button>
        <p class="w-full text-xs text-neutral-400 dark:text-neutral-500">
          O aviso aparece no sino e na janela de notificação do sistema. Enquanto
          a data não chega, o período já fica listado com o selo "Agendado".
        </p>
      </div>

      <p
        v-if="sucesso"
        role="status"
        class="w-full text-sm text-emerald-600 md:col-span-2 dark:text-emerald-400"
      >
        {{ sucesso }}
      </p>
      <p
        v-if="erro"
        role="alert"
        class="w-full rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 md:col-span-2 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
      >
        {{ erro }}
      </p>
    </form>
  </section>
</template>
