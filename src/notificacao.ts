// Lógica da JANELA DE NOTIFICAÇÃO (noti.html, empacotada pelo Vite).
// Importa a API do Tauri normalmente (sem depender de window.__TAURI__),
// busca a pendência no backend e fecha somente por ação do usuário.
import { invoke } from "@tauri-apps/api/core";

interface Pendencia {
  id: string;
  tipo: "vencida" | "alarme" | "alerta" | string;
  titulo: string;
  mensagem: string;
  secao: string;
}

function elemento(id: string): HTMLElement {
  return document.getElementById(id)!;
}

function fechar() {
  void invoke("fechar_janela_notificacao").catch(() => window.close());
}

/** Estilo do cabeçalho conforme o tipo: vencida | alarme (agendado) | alerta. */
function estiloDoTipo(tipo: string): string {
  if (tipo === "vencida") return "vencida";
  if (tipo === "alarme") return "alarme";
  return "alerta";
}

function renderizar(pendencia: Pendencia) {
  const estilo = estiloDoTipo(pendencia.tipo);

  const topo = elemento("topo");
  topo.className = "topo " + estilo;
  elemento("titulo").textContent = pendencia.titulo || "SCD";

  // Marcador colorido conforme o tipo + mensagem.
  const marcador = elemento("marcador");
  marcador.className = "marcador " + estilo;
  marcador.textContent =
    pendencia.tipo === "vencida"
      ? "Férias vencidas"
      : pendencia.tipo === "alarme"
        ? "Férias agendadas"
        : "Aviso de prazo";

  const mensagem = elemento("mensagem");
  mensagem.textContent = pendencia.mensagem || "";
  mensagem.classList.remove("carregando");

  const btnAcao = elemento("btnAcao") as HTMLButtonElement;
  btnAcao.textContent = pendencia.tipo === "vencida" ? "Regularizar agora" : "Ver no sistema";

  btnAcao.addEventListener("click", () => {
    void invoke("acao_notificacao", {
      id: pendencia.id,
      acao: pendencia.tipo === "vencida" ? "regularizar" : "ver",
      secao: pendencia.secao,
    }).catch(() => fechar());
  });

  elemento("btnLembrar").addEventListener("click", () => {
    void invoke("acao_notificacao", {
      id: pendencia.id,
      acao: "lembrar",
      secao: pendencia.secao,
    }).catch(() => fechar());
  });
}

async function buscar() {
  try {
    const pendencia = await invoke<Pendencia | null>("obter_pendencia_notificacao");
    if (!pendencia) {
      fechar(); // nada para mostrar
      return;
    }
    renderizar(pendencia);
    // Avisa o Rust que a página está pronta (vigia de segurança não fecha).
    await invoke("notificacao_janela_pronta").catch(() => undefined);
  } catch {
    const mensagem = elemento("mensagem");
    mensagem.classList.remove("carregando");
    mensagem.textContent =
      "Não foi possível carregar o conteúdo. Clique em ✕ para fechar.";
  }
}

elemento("btnFechar").addEventListener("click", fechar);
document.addEventListener("keydown", (evento) => {
  if (evento.key === "Escape") fechar();
});

void buscar();
