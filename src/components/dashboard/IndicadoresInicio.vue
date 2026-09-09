<script setup lang="ts">
// Indicadores da tela inicial (dashboard): centraliza os principais números
// do sistema numa única visão, com cartões legíveis e acesso rápido.
//
// Dados vêm dos estados compartilhados de useEmpresas/useFuncionarios
// (carregados ao montar) — mesma fonte usada pelas telas de cadastro.
import { computed, onMounted } from "vue";
import { useEmpresas } from "../../composables/useEmpresas";
import { useFuncionarios } from "../../composables/useFuncionarios";
import { useFerias } from "../../composables/useFerias";
import { useNavigation } from "../../composables/useNavigation";
import { topMenu } from "../../config/menu";
import { formatarCnpj } from "../../utils/cnpj";
import Icone from "../ui/Icone.vue";

const { empresas, listar: listarEmpresas, erroLista: erroEmpresas } = useEmpresas();
const { funcionarios, listar: listarFuncionarios, erroLista: erroFuncionarios } = useFuncionarios();
const { vencidas, aVencer, carregar: carregarFerias, erro: erroFerias } = useFerias();
const { openSection } = useNavigation();

onMounted(() => {
  void listarEmpresas();
  void listarFuncionarios();
  void carregarFerias();
});

/** Qualquer erro de leitura vira um aviso visível no dashboard. */
const erroDashboard = computed(
  () => erroEmpresas.value || erroFuncionarios.value || erroFerias.value,
);

// ── Cálculo dos indicadores ────────────────────────────────────────────────

/** Converte o created_at do banco ("YYYY-MM-DD HH:MM:SS" UTC) numa Date. */
function dataDeCriacao(texto: string): Date {
  return new Date(texto.replace(" ", "T") + "Z");
}

function chaveMesLocal(data: Date): string {
  return `${data.getFullYear()}-${String(data.getMonth() + 1).padStart(2, "0")}`;
}

function formatarData(texto: string): string {
  try {
    return dataDeCriacao(texto).toLocaleDateString("pt-BR");
  } catch {
    return texto.slice(0, 10);
  }
}

/** Empresas cadastradas no mês atual. */
const empresasNoMes = computed(() => {
  const mesAtual = chaveMesLocal(new Date());
  return empresas.value.filter((e) => chaveMesLocal(dataDeCriacao(e.created_at)) === mesAtual).length;
});

/** Últimas empresas cadastradas (mais recentes primeiro). */
const ultimasEmpresas = computed(() =>
  [...empresas.value]
    .sort((a, b) => dataDeCriacao(b.created_at).getTime() - dataDeCriacao(a.created_at).getTime())
    .slice(0, 6),
);

// ── Configuração dos cartões de KPI ────────────────────────────────────────

interface Kpi {
  chave: string;
  rotulo: string;
  icone: "empresa" | "funcionario" | "ferias-vencidas" | "ferias-a-vencer";
  valor: number;
  nota: string;
  cor: string; // classes de gradiente do azulejo
}

const kpis = computed<Kpi[]>(() => [
  {
    chave: "empresas",
    rotulo: "Empresas",
    icone: "empresa",
    valor: empresas.value.length,
    nota: `+${empresasNoMes.value} no mês`,
    cor: "from-indigo-500 to-violet-600",
  },
  {
    chave: "funcionarios",
    rotulo: "Funcionários",
    icone: "funcionario",
    valor: funcionarios.value.length,
    nota: "vinculados às empresas",
    cor: "from-sky-500 to-indigo-600",
  },
  {
    chave: "ferias-vencidas",
    rotulo: "Férias vencidas",
    icone: "ferias-vencidas",
    valor: vencidas.value.length,
    nota: vencidas.value.length > 0 ? "aguardando regularização" : "em dia",
    cor: "from-amber-500 to-orange-600",
  },
  {
    chave: "ferias-a-vencer",
    rotulo: "Férias a vencer",
    icone: "ferias-a-vencer",
    valor: aVencer.value.length,
    nota: aVencer.value.length > 0 ? "dentro do prazo" : "nenhum período",
    cor: "from-emerald-500 to-teal-600",
  },
]);

// ── Acesso rápido à seção de empresas ─────────────────────────────────────

const secaoEmpresa = computed(() =>
  topMenu.flatMap((g) => g.items).find((item) => item.id === "cadastro-empresa"),
);

function abrirEmpresas() {
  if (secaoEmpresa.value) openSection(secaoEmpresa.value);
}

const hoje = new Date().toLocaleDateString("pt-BR", {
  weekday: "long",
  day: "numeric",
  month: "long",
});
</script>

<template>
  <div class="space-y-6">
    <!-- Cabeçalho da visão geral -->
    <div class="text-center">
      <h1 class="text-2xl font-bold tracking-tight">Visão geral</h1>
      <p class="mt-1 text-neutral-500 capitalize dark:text-neutral-400">{{ hoje }}</p>
    </div>

    <!-- Erros de carregamento (diagnóstico) -->
    <p
      v-if="erroDashboard"
      role="alert"
      class="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ erroDashboard }}
    </p>

    <!-- Cartões de indicadores (KPIs) -->
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <div
        v-for="kpi in kpis"
        :key="kpi.chave"
        class="flex items-center gap-4 rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
      >
        <span
          class="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br text-white shadow-lg"
          :class="kpi.cor"
        >
          <Icone :nome="kpi.icone" tamanho="lg" />
        </span>
        <span class="min-w-0">
          <span class="block text-2xl font-bold leading-tight tabular-nums">
            {{ kpi.valor }}
          </span>
          <span class="block truncate text-sm text-neutral-500 dark:text-neutral-400">
            {{ kpi.rotulo }}
          </span>
          <span
            v-if="kpi.nota"
            class="mt-0.5 inline-block rounded-full px-2 py-px text-[10px] font-medium"
            :class="
              kpi.chave === 'ferias-vencidas' && kpi.valor > 0
                ? 'bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300'
                : 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/60 dark:text-emerald-300'
            "
          >
            {{ kpi.nota }}
          </span>
        </span>
      </div>
    </div>

    <!-- Últimas empresas cadastradas -->
    <div
      class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
    >
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
          Últimas empresas cadastradas
        </h2>
        <button
          type="button"
          @click="abrirEmpresas"
          class="rounded-lg px-2.5 py-1 text-xs font-medium text-indigo-600 transition hover:bg-indigo-50 dark:text-indigo-300 dark:hover:bg-indigo-950"
        >
          Ver todas →
        </button>
      </div>

      <ul v-if="ultimasEmpresas.length > 0" class="mt-3 divide-y divide-neutral-100 dark:divide-neutral-700/60">
        <li v-for="empresa in ultimasEmpresas" :key="empresa.id">
          <button
            type="button"
            @click="abrirEmpresas"
            class="flex w-full items-center gap-3 py-2.5 text-left transition hover:bg-neutral-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-indigo-500 dark:hover:bg-neutral-700/40"
          >
            <Icone nome="empresa" tamanho="sm" class="text-neutral-400 dark:text-neutral-500" />
            <span class="min-w-0 flex-1">
              <span class="block truncate text-sm font-medium">{{ empresa.razao_social }}</span>
              <span class="block text-xs text-neutral-400 dark:text-neutral-500">
                {{ formatarCnpj(empresa.cnpj) }}
              </span>
            </span>
            <span class="shrink-0 text-xs text-neutral-400 dark:text-neutral-500">
              {{ formatarData(empresa.created_at) }}
            </span>
          </button>
        </li>
      </ul>

      <p
        v-else
        class="mt-3 rounded-lg border border-dashed border-neutral-300 p-6 text-center text-sm text-neutral-400 dark:border-neutral-600"
      >
        Nenhuma empresa cadastrada ainda — comece em Cadastro → Empresa.
      </p>
    </div>
  </div>
</template>
