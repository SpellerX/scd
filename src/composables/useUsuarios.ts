import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Administração de usuários (Sistema → Usuários): estado compartilhado +
 * chamadas aos comandos Rust (src-tauri/src/commands/usuarios.rs).
 */

/** Usuário cadastrado (espelho do struct Rust `auth::Usuario`). */
export interface Usuario {
  id: number;
  username: string;
  display_name: string;
  created_at: string;
}

const usuarios = ref<Usuario[]>([]);
const carregandoLista = ref(false);
const erroLista = ref<string | null>(null);

export function useUsuarios() {
  /** Atualiza a lista de usuários cadastrados. */
  async function listar() {
    carregandoLista.value = true;
    erroLista.value = null;
    try {
      usuarios.value = await invoke<Usuario[]>("listar_usuarios");
    } catch (err) {
      erroLista.value = typeof err === "string" ? err : "Não foi possível carregar os usuários.";
    } finally {
      carregandoLista.value = false;
    }
  }

  /** Cadastra um novo usuário (login). */
  function criar(dados: { username: string; nomeExibicao: string; senha: string }): Promise<void> {
    return invoke<void>("criar_usuario", dados);
  }

  /** Exclui um usuário (pelo id). */
  function remover(id: number): Promise<void> {
    return invoke<void>("remover_usuario", { id });
  }

  /** Seções (itens de menu) liberadas para o usuário. */
  function listarPermissoes(userId: number): Promise<string[]> {
    return invoke<string[]>("listar_permissoes_do_usuario", { userId });
  }

  /** Substitui os acessos do usuário pela lista informada. */
  function salvarPermissoes(userId: number, secoes: string[]): Promise<void> {
    return invoke<void>("salvar_permissoes_do_usuario", { userId, secoes });
  }

  return {
    usuarios,
    carregandoLista,
    erroLista,
    listar,
    criar,
    remover,
    listarPermissoes,
    salvarPermissoes,
  };
}
