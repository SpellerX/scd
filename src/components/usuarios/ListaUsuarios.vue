<script setup lang="ts">
// Listagem dos usuários cadastrados (estado compartilhado via useUsuarios).
// O usuário padrão "admin" tem exclusão protegida (backend) e recebe um
// selo "Administrador" aqui.
import { ref } from "vue";
import { confirm } from "@tauri-apps/plugin-dialog";
import { mensagemDeErro } from "../../utils/erros";
import { useUsuarios, type Usuario } from "../../composables/useUsuarios";

const { usuarios, carregandoLista, erroLista, remover, listar } = useUsuarios();

const removendoId = ref<number | null>(null);
const erroExcluir = ref<string | null>(null);

function dataDeCriacao(texto: string): string {
  try {
    return new Date(texto.replace(" ", "T") + "Z").toLocaleDateString("pt-BR");
  } catch {
    return texto.slice(0, 10);
  }
}

async function excluir(usuario: Usuario) {
  erroExcluir.value = null;

  const confirmou = await confirm(
    `Excluir o usuário "${usuario.username}" (${usuario.display_name})?\nEsta ação não pode ser desfeita.`,
    { title: "Excluir usuário", kind: "warning", okLabel: "Excluir", cancelLabel: "Cancelar" },
  );
  if (!confirmou) return;

  removendoId.value = usuario.id;
  try {
    await remover(usuario.id);
    await listar();
  } catch (err) {
    erroExcluir.value = mensagemDeErro(err, "Não foi possível excluir o usuário.");
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
        Usuários cadastrados
      </h2>
      <span
        class="rounded-full bg-neutral-100 px-2.5 py-0.5 text-xs font-medium text-neutral-600 dark:bg-neutral-700 dark:text-neutral-300"
      >
        {{ usuarios.length }}
      </span>
    </div>

    <p
      v-if="carregandoLista"
      class="mt-4 text-sm text-neutral-500 dark:text-neutral-400"
      role="status"
    >
      Carregando usuários…
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
      v-else-if="usuarios.length === 0"
      class="mt-4 rounded-lg border border-dashed border-neutral-300 p-6 text-center text-sm text-neutral-400 dark:border-neutral-600"
    >
      Nenhum usuário cadastrado ainda.
    </p>

    <div v-else class="mt-4 overflow-x-auto">
      <table class="w-full min-w-[560px] border-collapse text-left text-sm">
        <thead>
          <tr class="border-b border-neutral-200 text-xs uppercase tracking-wide text-neutral-400 dark:border-neutral-700">
            <th class="py-2 pr-4 font-medium">Usuário</th>
            <th class="py-2 pr-4 font-medium">Nome</th>
            <th class="py-2 pr-4 font-medium">Criado em</th>
            <th class="py-2 text-right font-medium">Ações</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="usuario in usuarios"
            :key="usuario.id"
            class="border-b border-neutral-100 last:border-0 dark:border-neutral-700/50"
          >
            <td class="py-2.5 pr-4 font-mono text-xs font-medium">
              {{ usuario.username }}
              <span
                v-if="usuario.username === 'admin'"
                class="ml-2 rounded-full bg-indigo-100 px-2 py-0.5 text-[10px] font-semibold text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300"
              >
                Administrador
              </span>
            </td>
            <td class="py-2.5 pr-4">{{ usuario.display_name }}</td>
            <td class="py-2.5 pr-4 text-neutral-500 dark:text-neutral-400">
              {{ dataDeCriacao(usuario.created_at) }}
            </td>
            <td class="py-2.5 text-right">
              <button
                type="button"
                :disabled="usuario.username === 'admin' || removendoId === usuario.id"
                @click="excluir(usuario)"
                class="rounded-lg px-2.5 py-1 text-xs font-medium text-red-600 transition hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-40 dark:text-red-400 dark:hover:bg-red-950"
                :title="
                  usuario.username === 'admin'
                    ? 'O usuário administrador padrão não pode ser excluído'
                    : `Excluir ${usuario.username}`
                "
              >
                {{ removendoId === usuario.id ? "Excluindo..." : "Excluir" }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
