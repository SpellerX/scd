// Testes automatizados das regras de exibição/validação das férias.
//
// Como rodar (nenhuma dependência extra — usa o executor de testes do Node):
//   npm test
//
// O import usa a extensão `.ts` porque o Node exige o caminho completo do
// arquivo ao executar TypeScript diretamente.
import { describe, test } from "node:test";
import assert from "node:assert/strict";

import {
  dataIsoValida,
  rotuloDoPeriodo,
  seloDoAlarme,
  seloDePrazo,
  validarAgendamento,
} from "./ferias.ts";

describe("seloDePrazo (alerta automático)", () => {
  test("vence hoje e amanhã sempre aparecem como alerta", () => {
    assert.deepEqual(seloDePrazo(0, false), { texto: "vencem hoje", alerta: true });
    assert.deepEqual(seloDePrazo(-3, false), { texto: "vencem hoje", alerta: true });
    assert.deepEqual(seloDePrazo(1, false), { texto: "vencem amanhã", alerta: true });
  });

  test("dentro da janela de N dias mostra o aviso com ⚠", () => {
    assert.deepEqual(seloDePrazo(10, true), { texto: "vencem em 10 dias ⚠", alerta: true });
  });

  test("fora da janela apenas informa quantos dias faltam", () => {
    assert.deepEqual(seloDePrazo(120, false), { texto: "faltam 120 dias", alerta: false });
  });
});

describe("seloDoAlarme (férias agendadas)", () => {
  test("data do aviso já chegou ⇒ pendente", () => {
    assert.equal(seloDoAlarme(true, 0), "aviso pendente");
    assert.equal(seloDoAlarme(true, -5), "aviso pendente");
  });

  test("conta os dias até a data do aviso", () => {
    assert.equal(seloDoAlarme(false, 1), "avisa amanhã");
    assert.equal(seloDoAlarme(false, 3), "avisa em 3 dia(s)");
  });

  test("sem data de alarme não inventa contagem", () => {
    assert.equal(seloDoAlarme(false, null), "sem aviso");
  });
});

describe("rotuloDoPeriodo (seletor de agendamento)", () => {
  test("mostra o prazo, os dias restantes e o alarme já definido", () => {
    const rotulo = rotuloDoPeriodo({
      inicio: "2024-05-01",
      vencimento: "2026-05-01",
      dias_restantes: 400,
      alarme_em: "2026-03-10",
    });
    assert.equal(
      rotulo,
      "01/05/2024 → 01/05/2026 — faltam 400 dia(s) · agendado para 10/03/2026",
    );
  });

  test("sem alarme, não menciona agendamento", () => {
    const rotulo = rotuloDoPeriodo({
      inicio: "2024-05-01",
      vencimento: "2026-05-01",
      dias_restantes: 0,
      alarme_em: null,
    });
    assert.equal(rotulo, "01/05/2024 → 01/05/2026 — vence hoje");
  });
});

describe("dataIsoValida", () => {
  test("aceita datas reais (inclusive 29/02 em ano bissexto)", () => {
    assert.equal(dataIsoValida("2026-03-10"), true);
    assert.equal(dataIsoValida("2024-02-29"), true);
  });

  test("recusa datas impossíveis e formatos diferentes", () => {
    assert.equal(dataIsoValida("2026-02-30"), false, "30 de fevereiro não existe");
    assert.equal(dataIsoValida("2025-02-29"), false, "2025 não é bissexto");
    assert.equal(dataIsoValida("2026-13-01"), false, "mês 13");
    assert.equal(dataIsoValida("2026-00-10"), false, "mês 0");
    assert.equal(dataIsoValida("10/03/2026"), false, "formato brasileiro");
    assert.equal(dataIsoValida(""), false);
    assert.equal(dataIsoValida("amanhã"), false);
  });
});

describe("validarAgendamento (formulário de alarme)", () => {
  const base = { funcionarioId: "f-1", periodoId: "p-1", alarmeEm: "2026-03-10", hoje: "2026-03-01" };

  test("aprova um agendamento completo e futuro", () => {
    assert.equal(validarAgendamento(base), null);
  });

  test("exige funcionário, período e data", () => {
    assert.match(validarAgendamento({ ...base, funcionarioId: null })!, /funcionário/);
    assert.match(validarAgendamento({ ...base, periodoId: null })!, /período/);
    assert.match(validarAgendamento({ ...base, alarmeEm: "" })!, /data do aviso/);
  });

  test("recusa data inválida", () => {
    assert.match(validarAgendamento({ ...base, alarmeEm: "2026-02-30" })!, /inválida/);
    assert.match(validarAgendamento({ ...base, alarmeEm: "10/03/2026" })!, /inválida/);
  });

  test("recusa data no passado, mas aceita hoje (aviso imediato)", () => {
    assert.match(
      validarAgendamento({ ...base, alarmeEm: "2026-02-28", hoje: "2026-03-01" })!,
      /passado/,
    );
    assert.equal(validarAgendamento({ ...base, alarmeEm: "2026-03-01", hoje: "2026-03-01" }), null);
  });
});
