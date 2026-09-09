import { ref } from "vue";
import type { MenuItem } from "../config/menu";

/**
 * Navegação interna do SCD (qual seção está aberta).
 *
 * Mesmo padrão do useAuth: estado em nível de módulo, então qualquer
 * componente que chame useNavigation() enxerga a MESMA seção ativa.
 *
 * Enquanto o app não adota o vue-router, a navegação é por "seção ativa"
 * (id do MenuItem). `null` = tela inicial do dashboard.
 */
const activeSection = ref<MenuItem | null>(null);

export function useNavigation() {
  /** Abre a seção escolhida no menu. */
  function openSection(section: MenuItem) {
    activeSection.value = section;
  }

  /** Volta para a tela inicial do dashboard. */
  function goHome() {
    activeSection.value = null;
  }

  return { activeSection, openSection, goHome };
}
