<script setup lang="ts">
// Seção "Sincronização" (menu Sistema): conecta a conta da nuvem (Supabase)
// que sincroniza os dados entre máquinas. A conta é por DISPOSITIVO/empresa —
// o login local e as permissões por usuário continuam como estão.
import { onMounted, ref } from "vue";
import { confirm } from "@tauri-apps/plugin-dialog";
import { mensagemDeErro } from "../../utils/erros";
import { useSync, type ConfiguracaoConexao } from "../../composables/useSync";

const {
  estado,
  carregandoEstado,
  sincronizando,
  erro,
  ultimoResumo,
  carregarEstado,
  conectar,
  desconectar,
  sincronizarAgora,
  verificarEExecutar,
} = useSync();

// Formulário de conexão
const projetoUrl = ref("");
const anonKey = ref("");
const email = ref("");
const senha = ref("");
const conectando = ref(false);

const podeConectar = () =>
  projetoUrl.value.trim() !== "" &&
  anonKey.value.trim() !== "" &&
  email.value.trim() !== "" &&
  senha.value !== "";

async function conectarConta() {
  erro.value = null;
  if (!podeConectar()) {
    erro.value = "Preencha todos os campos para conectar a conta da nuvem.";
    return;
  }
  conectando.value = true;
  try {
    const config: ConfiguracaoConexao = {
      projetoUrl: projetoUrl.value.trim(),
      anonKey: anonKey.value.trim(),
      email: email.value.trim(),
      senha: senha.value,
    };
    await conectar(config);
    projetoUrl.value = "";
    anonKey.value = "";
    email.value = "";
    senha.value = "";
  } catch (err) {
    erro.value = mensagemDeErro(err, "Não foi possível conectar à conta da nuvem.");
  } finally {
    conectando.value = false;
  }
}

async function sincronizar() {
  try {
    await sincronizarAgora();
  } catch {
    // erro já fica visível no banner (vindo do composable)
  }
}

async function desconectarConta() {
  erro.value = null;
  const confirmou = await confirm(
    "Desconectar a conta da nuvem deste computador? Os dados locais permanecem, mas param de ser sincronizados.",
    { title: "Desconectar sincronização", kind: "warning", okLabel: "Desconectar", cancelLabel: "Cancelar" },
  );
  if (!confirmou) return;
  try {
    await desconectar();
  } catch (err) {
    erro.value = mensagemDeErro(err, "Não foi possível desconectar.");
  }
}

onMounted(() => {
  // Ao abrir a tela, tenta conectar/sincronizar automaticamente (o backend
  // usa a configuração do .env ou a embutida no build, se houver) e depois
  // reflete o estado real.
  void carregarEstado();
  void verificarEExecutar().then(() => {
    void carregarEstado();
  });
});
</script>

<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold">Sincronização</h1>
      <p class="mt-1 max-w-2xl text-neutral-500 dark:text-neutral-400">
        Conecte a conta da empresa no <strong>Supabase</strong> para manter
        empresas, funcionários e férias em sincronia entre os computadores.
        O app continua funcionando offline: as alterações locais são enviadas
        quando houver conexão, e o que mudar em outra máquina chega sozinho.
      </p>
    </div>

    <!-- ════════ Aguardando o estado ════════ -->
    <p v-if="carregandoEstado" class="text-sm text-neutral-500 dark:text-neutral-400" role="status">
      Carregando…
    </p>

    <!-- ════════ Formulário de conexão ════════ -->
    <section
      v-else-if="!estado?.conectado"
      class="max-w-2xl rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
    >
      <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
        Conectar conta da nuvem
      </h2>
      <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
        Crie a conta (e-mail/senha) em <strong>Authentication → Users</strong> do
        projeto Supabase e informe os dados abaixo. Eles ficam salvos apenas
        neste computador.
      </p>

      <div class="mt-4 grid gap-3">
        <div class="flex flex-col gap-1.5">
          <label for="sync-url" class="text-sm font-medium">URL do projeto</label>
          <input
            id="sync-url"
            v-model="projetoUrl"
            type="text"
            placeholder="https://xxxx.supabase.co"
            autocomplete="off"
            spellcheck="false"
            class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <label for="sync-anon" class="text-sm font-medium">Chave anônima (anon key)</label>
          <input
            id="sync-anon"
            v-model="anonKey"
            type="text"
            placeholder="eyJhbGciOiJIUzI1NiIs..."
            autocomplete="off"
            spellcheck="false"
            class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <label for="sync-email" class="text-sm font-medium">E-mail da conta</label>
          <input
            id="sync-email"
            v-model="email"
            type="email"
            placeholder="contato@empresa.com.br"
            autocomplete="email"
            spellcheck="false"
            class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <label for="sync-senha" class="text-sm font-medium">Senha da conta</label>
          <input
            id="sync-senha"
            v-model="senha"
            type="password"
            autocomplete="current-password"
            class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
          />
        </div>
      </div>

      <button
        type="button"
        :disabled="conectando"
        @click="conectarConta"
        class="mt-4 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {{ conectando ? "Conectando..." : "Conectar e sincronizar" }}
      </button>

      <p v-if="erro" role="alert" class="mt-3 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300">
        {{ erro }}
      </p>
    </section>

    <!-- ════════ Painel da conta conectada ════════ -->
    <section
      v-else
      class="max-w-2xl rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
    >
      <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
        Conta conectada
      </h2>

      <dl class="mt-3 space-y-2 text-sm">
        <div class="flex items-center gap-2">
          <dt class="w-36 text-neutral-500 dark:text-neutral-400">E-mail:</dt>
          <dd class="font-medium">{{ estado?.email }}</dd>
        </div>
        <div class="flex items-center gap-2">
          <dt class="w-36 text-neutral-500 dark:text-neutral-400">Última sincronização:</dt>
          <dd>{{ estado?.ultima_sync ?? "—" }}</dd>
        </div>
      </dl>

      <div class="mt-4 flex flex-wrap gap-2">
        <button
          type="button"
          :disabled="sincronizando"
          @click="sincronizar"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {{ sincronizando ? "Sincronizando..." : "Sincronizar agora" }}
        </button>
        <button
          type="button"
          :disabled="sincronizando"
          @click="desconectarConta"
          class="rounded-lg border border-red-300 bg-red-50 px-4 py-2 text-sm font-medium text-red-700 transition hover:bg-red-100 disabled:opacity-60 dark:border-red-900 dark:bg-red-950 dark:text-red-300 dark:hover:bg-red-900"
        >
          Desconectar
        </button>
      </div>

      <!-- Resumo do último ciclo -->
      <div v-if="ultimoResumo" role="status" class="mt-4 rounded-xl border border-emerald-200 bg-emerald-50 p-4 text-sm text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200">
        <p>
          <strong>{{ ultimoResumo.enviados }}</strong> registro(s) enviado(s) ·
          <strong>{{ ultimoResumo.recebidos }}</strong> recebido(s).
        </p>
        <ul v-if="ultimoResumo.avisos.length > 0" class="mt-2 list-inside list-disc space-y-1 text-xs">
          <li v-for="(aviso, i) in ultimoResumo.avisos" :key="i">{{ aviso }}</li>
        </ul>
      </div>

      <p v-if="erro" role="alert" class="mt-3 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300">
        {{ erro }}
      </p>
    </section>
  </div>
</template>
