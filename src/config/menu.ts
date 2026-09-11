/**
 * Configuração central do menu superior (navegação do SCD).
 *
 * Para adicionar uma nova entrada no menu, edite apenas este arquivo:
 *  - um "grupo" vira um item no menu superior (hoje: só "Cadastro");
 *  - os "items" do grupo viram o submenu (dropdown) desse item.
 *
 * Convenção dos rótulos: como o submenu já aparece dentro do grupo
 * (ex.: "Cadastro"), os itens NÃO repetem o nome do grupo
 * (use "Empresa", e não "Cadastro de empresa").
 *
 * Padronização: ids sempre minúsculas com hífen (ex.: cadastro-empresa),
 * pois são usados na navegação interna.
 */

/** Nomes de ícones disponíveis (ver src/components/ui/Icone.vue). */
export type IconeMenu =
  | "cadastro"
  | "empresa"
  | "funcionario"
  | "ferias-vencidas"
  | "ferias-a-vencer"
  | "usuario"
  | "sincronizacao";

export interface MenuItem {
  /** Identificador estável da seção (usado na navegação). */
  id: string;
  /** Texto exibido no menu. */
  label: string;
  /** Descrição curta — aparece no placeholder até a tela real ser criada. */
  description: string;
  /** Ícone exibido nos cards da tela inicial (opcional). */
  icon?: IconeMenu;
}

export interface MenuGroup {
  /** Texto do item no menu superior. */
  label: string;
  /** Subitens exibidos no dropdown. */
  items: MenuItem[];
  /** Ícone do card na tela inicial (opcional). */
  icon?: IconeMenu;
}

export const topMenu: MenuGroup[] = [
  {
    label: "Cadastro",
    icon: "cadastro",
    items: [
      {
        id: "cadastro-empresa",
        label: "Empresa",
        description: "Cadastro das empresas atendidas pelo sistema.",
        icon: "empresa",
      },
      {
        id: "cadastro-funcionarios",
        label: "Funcionários",
        description: "Cadastro de colaboradores vinculados a uma empresa.",
        icon: "funcionario",
      },
      {
        id: "cadastro-ferias-vencidas",
        label: "Férias vencidas",
        description: "Registro das férias já vencidas dos colaboradores.",
        icon: "ferias-vencidas",
      },
      {
        id: "ferias-a-vencer",
        label: "Férias a vencer",
        description: "Acompanhamento das férias que vencem em breve.",
        icon: "ferias-a-vencer",
      },
    ],
  },
  {
    label: "Sistema",
    icon: "usuario",
    items: [
      {
        id: "sistema-usuarios",
        label: "Usuários",
        description: "Cadastro e administração dos usuários do SCD.",
        icon: "usuario",
      },
      {
        id: "sistema-sincronizacao",
        label: "Sincronização",
        description: "Conta da nuvem que sincroniza os dados entre máquinas.",
        icon: "sincronizacao",
      },
    ],
  },
];

/**
 * Todas as seções, na ordem em que aparecem no menu — usado por quem precisa
 * levar o usuário a uma seção (ex.: cards da tela inicial) sem depender da
 * posição dela dentro do grupo.
 */
export const todasAsSecoes: MenuItem[] = topMenu.flatMap((grupo) => grupo.items);

/** Seção pelo id (`undefined` quando o id não existe — ex.: menu renomeado). */
export function secaoPorId(id: string): MenuItem | undefined {
  return todasAsSecoes.find((item) => item.id === id);
}
