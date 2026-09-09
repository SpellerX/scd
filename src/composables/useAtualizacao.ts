// Verificação automática de atualizações (plugin updater do Tauri v2).
//
// Ao abrir o app, consulta o "endpoint" (latest.json publicado junto com a
// release no GitHub) e compara com a versão instalada:
//   - sem novidade  → nada acontece;
//   - com novidade  → pergunta ao usuário e, se aceitar, baixa e instala a
//     nova versão (o app reinicia sozinho no fim).
// Falhas (offline, dev sem release, etc.) são silenciosas: nunca travam o app.
import { ref } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { confirm } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";

const verificando = ref(false);
/** Erro da última verificação (só para diagnóstico interno). */
const ultimoErro = ref<string | null>(null);

export function useAtualizacao() {
  /** Confere e, se houver versão nova, pergunta/baixa/instala. */
  async function verificarEAtualizar(): Promise<void> {
    if (verificando.value) return;
    verificando.value = true;
    ultimoErro.value = null;
    try {
      const update: Update | null = await check();
      if (!update) return; // já está na versão mais recente

      const aceitou = await confirm(
        `Já existe a versão ${update.version} do SCD.\n\nBaixar e instalar agora?`,
        {
          title: "Atualização disponível",
          kind: "info",
          okLabel: "Baixar e atualizar",
          cancelLabel: "Agora não",
        },
      );
      if (!aceitou) return;

      // Download com acompanhamento (a API reporta cada pedaço baixado).
      await update.download((evento) => {
        if (evento.event === "Progress") {
          console.log(`[updater] baixando… (+${evento.data.chunkLength} bytes)`);
        }
      });

      await update.install();
      // Reinicia o app já na versão nova.
      await relaunch();
    } catch (err) {
      ultimoErro.value = typeof err === "string" ? err : "Não foi possível verificar atualizações.";
      console.error("[updater]", ultimoErro.value);
    } finally {
      verificando.value = false;
    }
  }

  return { verificando, ultimoErro, verificarEAtualizar };
}
