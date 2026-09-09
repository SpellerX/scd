<script setup lang="ts">
// Listagem dos funcionários cadastrados (estado compartilhado via useFuncionarios),
// mostrando a empresa a que cada um está vinculado. Exclusão individual com
// confirmação nativa.
import { ref } from "vue";
import { confirm } from "@tauri-apps/plugin-dialog";
import { formatarCpf } from "../../utils/cpf";
import { mensagemDeErro } from "../../utils/erros";
import { useFuncionarios, type Funcionario } from "../../composables/useFuncionarios";

const { funcionarios, carregandoLista, erroLista, remover, listar } = useFuncionarios();

const removendoId = ref<string | null>(null);
const erroExcluir = ref<string | null>(null);

async function excluir(funcionario: Funcionario) {
  erroExcluir.value = null;

  const confirmou = await confirm(
    `Excluir "${funcionario.nome}"${funcionario.cpf ? ` (${formatarCpf(funcionario.cpf)})` : ""}?\nEsta ação não pode ser desfeita.`,
    { title: "Excluir funcionário", kind: "warning", okLabel: "Excluir", cancelLabel: "Cancelar" },
  );
  if (!confirmou) return;

  removendoId.value = funcionario.id;
  try {
    await remover(funcionario.id);
    await listar();
  } catch (err) {
    erroExcluir.value = mensagemDeErro(err, "Não foi possível excluir o funcionário.");
  } finally {
    removendoId.value = null;
  }
}
</script>

<template>
  <section
    class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
  >
    <div class="flex items-center justify-between gap-2">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
        Funcionários cadastrados
      </h2>
      <span
        class="rounded-full bg-neutral-100 px-2.5 py-0.5 text-xs font-medium text-neutral-600 dark:bg-neutral-700 dark:text-neutral-300"
      >
        {{ funcionarios.length }}
      </span>
    </div>

    <p
      v-if="carregandoLista"
      class="mt-4 text-sm text-neutral-500 dark:text-neutral-400"
      role="status"
    >
      Carregando funcionários…
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
      v-else-if="funcionarios.length === 0"
      class="mt-4 rounded-lg border border-dashed border-neutral-300 p-6 text-center text-sm text-neutral-400 dark:border-neutral-600"
    >
      Nenhum funcionário cadastrado ainda.
    </p>

    <!-- Tabela (rolagem horizontal em telas estreitas) -->
    <div v-else class="mt-4 overflow-x-auto">
      <table class="w-full min-w-[640px] border-collapse text-left text-sm">
        <thead>
          <tr class="border-b border-neutral-200 text-xs uppercase tracking-wide text-neutral-400 dark:border-neutral-700">
            <th class="py-2 pr-4 font-medium">Nome</th>
            <th class="py-2 pr-4 font-medium">CPF</th>
            <th class="py-2 pr-4 font-medium">Empresa</th>
            <th class="py-2 pr-4 font-medium">Admissão</th>
            <th class="py-2 text-right font-medium">Ações</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="funcionario in funcionarios"
            :key="funcionario.id"
            class="border-b border-neutral-100 last:border-0 dark:border-neutral-700/50"
          >
            <td class="py-2.5 pr-4 font-medium">{{ funcionario.nome }}</td>
            <td class="py-2.5 pr-4 font-mono text-xs">
              {{ funcionario.cpf ? formatarCpf(funcionario.cpf) : "—" }}
            </td>
            <td class="py-2.5 pr-4 text-neutral-600 dark:text-neutral-300">
              {{ funcionario.empresa_nome }}
            </td>
            <td class="py-2.5 pr-4 text-neutral-500 dark:text-neutral-400">
              {{ funcionario.data_admissao || "—" }}
            </td>
            <td class="py-2.5 text-right">
              <button
                type="button"
                :disabled="removendoId === funcionario.id"
                @click="excluir(funcionario)"
                class="rounded-lg px-2.5 py-1 text-xs font-medium text-red-600 transition hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-50 dark:text-red-400 dark:hover:bg-red-950"
                :title="`Excluir ${funcionario.nome}`"
              >
                {{ removendoId === funcionario.id ? "Excluindo..." : "Excluir" }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
