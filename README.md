# SCD — Sistema de Controle de Demandas

Aplicativo desktop com **Tauri v2** (Rust) + **Vue 3** (TypeScript/Vite) + **Tailwind CSS v4**.

> Foco do projeto: **código fácil de ler e alterar** — cada parte tem um lugar
> óbvio e os padrões de extensão estão comentados no próprio código.

## Funcionalidades (atual)

- **Tela de login** (página inicial): autentica no backend Rust, com opção
  **"Manter-me conectado"** — a sessão fica salva e o app abre já logado
  (token persistido na tabela `sessions`; "Sair" encerra a sessão)
- **Banco local SQLite**: usuários com senha hasheada (bcrypt)
- **Dados padronizados**: nomes de usuário em minúsculas e CNPJs sempre com os
  14 dígitos, normalizados num único ponto do backend
- **Dashboard com menu superior**: grupos **Cadastro** (Empresa,
  Funcionários, Férias vencidas, Férias a vencer) e **Sistema** (Usuários)
- **Dashboard de indicadores** (tela inicial): totais de empresas,
  funcionários, férias vencidas e a vencer + últimas empresas cadastradas.
  Cada cartão é **clicável e abre a tela correspondente** (quem não tem acesso
  à seção vê o número, mas o cartão não navega — mesma regra do menu)
- **Cadastro de Empresa**:
  - busca de dados públicos pelo CNPJ (**Minha Receita** — API aberta, sem
    limite prático de consultas; **BrasilAPI** entra como fallback);
  - importação em lote por planilha **.xls/.xlsx**;
  - listagem das empresas cadastradas, com **exclusão individual e em lote**
    (seleção por checkbox + confirmação nativa)
- **Cadastro de Funcionários**: colaboradores **vinculados a uma empresa**
  (nome, **data de admissão obrigatória** — gera as férias automaticamente —
  e **CPF opcional** validado quando informado), com **importação em lote por
  planilha .xls/.xlsx** (colunas: nome e admissão obrigatórias; cpf opcional;
  empresa por linha via coluna `cnpj` ou empresa padrão escolhida na tela) +
  listagem/exclusão — a base para o controle de férias. Empresas com
  funcionários vinculados **não podem ser excluídas**
- **Administração de usuários** (**Sistema → Usuários**): cadastrar/excluir
  logins (nome de exibição, usuário em minúsculas e senha com mínimo de 4
  caracteres); o superusuário criado no primeiro acesso é protegido contra
  exclusão
- **Acessos por usuário**: o administrador escolhe **quais módulos cada
  usuário pode ver** (menu é filtrado automaticamente no login); o
  superusuário sempre tem acesso total
- **Controle de férias automático**: ao cadastrar/importar funcionário com
  data de admissão, o sistema gera os períodos (12 meses aquisitivos + 12
  para gozar). **Férias vencidas** (prazo expirado) têm botão
  **Regularizar** (já gozou, com observação). **Férias a vencer** mostram o
  prazo e permitem configurar **alerta de N dias** antes do vencimento
- **Férias agendadas (alarme manual)**: na tela *Férias a vencer*, escolha o
  **funcionário**, o **período** e a **data do aviso** para as férias que a
  empresa **já marcou**. O período entra na lista na hora (selo "Agendado",
  mesmo antes dos 12 meses finais do prazo) e o aviso dispara na data
  escolhida — sem esperar o alerta automático. Pode editar ou remover o
  alarme a qualquer momento; o alerta automático continua valendo para os
  períodos **sem** alarme
- **Notificações**: sino interno no cabeçalho **+ janela de notificação
  própria** (segunda janela nativa sem borda, canto superior direito, estilo
  MSN) que permanece até o usuário agir: **Regularizar agora** (abre o app na
  página de férias vencidas) ou **Lembrar mais tarde** (pausa até o dia
  seguinte). Funciona em dev e no app instalado — não usa o plugin de
  notificação do Tauri
- **Sincronização opcional com a nuvem (Supabase)** (`Sistema → Sincronização`):
  uma **conta "dona"** por empresa mantém empresas, funcionários e férias em
  sincronia entre várias máquinas. O SQLite local continua sendo a fonte da
  UI (app funciona offline); o app envia as alterações locais e recebe as de
  outras máquinas a cada ~60 s (last-write-wins). Login local e permissões
  por módulo não mudam. Veja a seção *Sincronização com a nuvem* abaixo.
- **Atualização automática**: o app confere no GitHub Releases, 3 s depois de
  abrir, se há versão nova; pergunta, baixa, instala e reinicia sozinho. Veja
  a seção *Atualizações automáticas* abaixo (inclui o que a release precisa
  ter para o updater funcionar).

## Estrutura do projeto

```
scd/
├── index.html              # HTML base (aplicação é montada no #app)
├── vite.config.ts          # Vite: plugins vue() + tailwindcss(), ajustes p/ Tauri
├── src/                    # ── FRONTEND (Vue 3) ─────────────────────────
│   ├── main.ts             # Bootstrap: monta o App.vue e importa o CSS
│   ├── style.css           # Entrada do Tailwind (@import "tailwindcss")
│   ├── App.vue             # Shell/raiz: tema + tela conforme autenticação
│   ├── config/
│   │   └── menu.ts         # ⭐ Itens do menu superior (edite aqui p/ mudar o menu)
│   ├── composables/        # Estado/lógica reutilizável do Vue
│   │   ├── useAuth.ts      # Sessão (login/logout/current_user)
│   │   ├── useNavigation.ts# Seção ativa do dashboard
│   │   ├── useEmpresas.ts  # Lista/estado do cadastro de empresas
│   │   └── useFuncionarios.ts # Lista/estado do cadastro de funcionários
│   ├── utils/              # Funções puras (máscaras CPF/CNPJ, erros)
│   ├── components/         # Peças de UI reutilizáveis (sem regra de negócio)
│   │   ├── LoginForm.vue
│   │   ├── navigation/TopMenu.vue   # Menu superior com submenus
│   │   ├── empresa/                 # Componentes da tela de empresa
│   │   │   ├── BuscaEmpresa.vue     # Busca por CNPJ (fontes públicas) + salvar
│   │   │   ├── ImportarEmpresa.vue  # Importação .xls/.xlsx + progresso
│   │   │   └── ListaEmpresas.vue    # Tabela + exclusão individual/em lote
│   │   ├── funcionarios/            # Componentes da tela de funcionários
│   │   │   ├── CadastroFuncionario.vue # Formulário (CPF opcional) + vínculo
│   │   │   ├── ImportarFuncionarios.vue # Importação .xls/.xlsx + relatório
│   │   │   └── ListaFuncionarios.vue   # Listagem + exclusão
│   │   └── ferias/                  # Componentes da tela de férias
│   │       └── AgendarFerias.vue    # Alarme manual (férias agendadas)
│   └── views/              # Telas do app
│       ├── LoginView.vue   # Página inicial: login
│       ├── DashboardView.vue # Área autenticada: cabeçalho + menu + conteúdo
│       └── sections/
│           ├── EmpresaView.vue      # Seção "Empresa"
│           ├── FuncionariosView.vue # Seção "Funcionários"
│           ├── FeriasVencidasView.vue # Seção "Férias vencidas"
│           ├── FeriasAVencerView.vue  # Seção "Férias a vencer" (alerta + alarme)
│           ├── SincronizacaoView.vue  # Seção "Sincronização" (Sistema)
│           └── UsuariosView.vue     # Seção "Usuários" (Sistema)
└── src-tauri/              # ── BACKEND (Rust / Tauri) ───────────────────
    ├── tauri.conf.json     # Configuração do Tauri (janela, build, bundle)
    ├── capabilities/default.json  # Permissões (ex.: dialog de arquivos)
    └── src/
        ├── main.rs         # Entrada do binário (só chama run())
        ├── lib.rs          # Monta o app: banco, estado global e comandos
        ├── db.rs           # Camada de dados: abre o SQLite e o esquema
        ├── auth.rs         # Regras de autenticação
        ├── cnpj.rs         # Fontes públicas de CNPJ (Minha Receita/BrasilAPI) + normalização
        ├── empresas.rs     # Regras do cadastro de empresas (+ importação)
        ├── funcionarios.rs # Regras dos funcionários (vínculo com empresas)
        ├── ferias.rs       # Períodos, alerta automático e alarme manual
        ├── planilha.rs     # Leitura de .xls/.xlsx (crate calamine)
        └── commands/       # Comandos Tauri expostos ao frontend
            ├── mod.rs      # Registro dos módulos de comando
            ├── auth.rs     # login, logout, current_user
            ├── empresas.rs # listar/buscar/salvar/excluir/importar empresas
            ├── funcionarios.rs # listar/criar/excluir/importar funcionários
            ├── ferias.rs   # listar/regularizar férias + agendar alarme
            ├── usuarios.rs # listar/criar/excluir usuários
            └── sync.rs     # estado/conectar/desconectar/sincronizar (nuvem)
    └── supabase/schema.sql     # Esquema da nuvem (tabelas espelho + RLS)
```

## Como alterar (padrões do projeto)

**Adicionar um item ao menu superior**
1. Edite `src/config/menu.ts` (grupo + subitens);
2. Se a seção ganhar tela própria, registre-a em `telasDasSecoes` no
   `src/views/DashboardView.vue` (senão ela abre o placeholder).

**Criar uma nova tela de seção**
1. Crie `src/views/sections/MinhaSecaoView.vue`;
2. Registre em `telasDasSecoes` (DashboardView) com o `id` do item do menu.

**Criar um novo comando Rust (frontend ↔ backend)**
1. Crie `src-tauri/src/commands/<area>.rs` com `#[tauri::command]`;
2. Declare `pub mod <area>;` em `commands/mod.rs` e registre a função no
   `generate_handler!` de `lib.rs`;
3. Chame do Vue com `invoke(...)` (veja `useEmpresas.ts` como exemplo).

**Nova tabela no banco**
1. Adicione o `CREATE TABLE IF NOT EXISTS` em `src-tauri/src/db.rs`;
2. Crie as funções de acesso num módulo de regras (como `empresas.rs`).

## Planilha de importação de empresas

- Formatos: **.xls** e **.xlsx** (lidos sem depender do Excel instalado).
- A **1ª linha deve ser o cabeçalho**; o sistema procura:
  - coluna de **CNPJ**: obrigatória — cabeçalho contendo `cnpj`;
  - **razão social**: opcional — cabeçalho contendo `raz` (ex.: "Razão social");
  - **nome fantasia**: opcional — cabeçalho contendo `fantas`.
- Linhas com razão social são salvas direto; sem razão social, o sistema
  consulta as fontes públicas (Minha Receita primeiro; BrasilAPI como
  fallback, com retry automático no limite de consultas) — importações
  grandes podem demorar.
- CNPJ já cadastrado é pulado e contado como "duplicada"; linhas com problema
  aparecem no relatório final com o número da linha e o motivo.
- Dica: formate a coluna de CNPJ como **texto** na planilha, para preservar
  zeros à esquerda.

## Banco de dados

- Arquivo: `scd.db`, criado no diretório de dados do app
  (Windows: `%APPDATA%\com.scd.app\`).
- **Resetar em desenvolvimento**: feche o app e apague o arquivo — o esquema
  e o usuário demo são recriados no próximo início.
- Primeiro acesso: o app cria o superusuário `fernando` (senha definida em
  `auth::ensure_usuario_inicial` — não exibida em nenhuma tela). Troque essa
  senha antes de distribuir (não há tela de troca ainda; altere o valor em
  `auth.rs` + apague o banco local, ou implemente a troca de senha).
- Desde o esquema v2, as tabelas sincronizáveis (`companies`, `employees`,
  `employee_leave_periods`) usam **id UUID**, `updated_at` e `deleted_at`
  (soft delete) — a migração acontece automaticamente ao abrir o app.
- Esquema **v4**: `employee_leave_periods` ganhou `alarme_em` e
  `alarme_observacao` (alarme manual das férias agendadas). Bancos antigos são
  atualizados sozinhos ao abrir o app — nenhum dado é perdido.

## Sincronização com a nuvem (Supabase)

Sincronizar entre máquinas é **opcional** e feito por **dispositivo**:

1. Crie um projeto no [Supabase](https://supabase.com) e, no **SQL Editor**,
   execute o conteúdo de `supabase/schema.sql` (tabelas espelho + RLS por dono).
   Se o projeto já existia antes do alarme manual, **execute o arquivo de
   novo**: ele acrescenta as colunas `alarme_em`/`alarme_observacao`
   (comandos idempotentes, nada é apagado).
2. Em **Authentication → Users**, crie a conta (e-mail/senha) da empresa.
3. No app: **Sistema → Sincronização** → informe a **Project URL** e a
   **anon key** (Settings → API) + a conta criada → **Conectar e sincronizar**.

O app sincroniza sozinho ao abrir e a cada ~60 s (ou pelo botão
"Sincronizar agora"). Conflitos usam **last-write-wins** por `updated_at`;
exclusões são suaves (`deleted_at`), então nada é perdido por quem está
offline. Credenciais/tokens ficam apenas no dispositivo (tabela `app_settings`).

**Desenvolvimento/teste**: em vez de digitar na tela, copie `.env.example`
para `.env` (na raiz do projeto) e preencha `SCD_SUPABASE_URL`,
`SCD_SUPABASE_ANON_KEY`, `SCD_SUPABASE_EMAIL` e `SCD_SUPABASE_SENHA` — se o
app ainda não tiver conta conectada, a primeira sincronização usa esses dados
automaticamente. Em produção, cada empresa informa a própria conta pela tela
(a senha nunca vai em build/instalador).

**Instalador já configurado (usuário não digita nada)**: cadastre no GitHub
(**Settings → Secrets and variables → Actions**) os secrets
`SCD_SUPABASE_URL`, `SCD_SUPABASE_ANON_KEY`, `SCD_SUPABASE_EMAIL` e
`SCD_SUPABASE_SENHA`. O workflow de release os injeta no build como
`SCD_EMBUTIDO_*`, que o código lê na compilação (`option_env!`) — no primeiro
uso em qualquer máquina o app conecta e sincroniza sozinho (sem `.env`, sem
tela). Use apenas em distribuição privada: quem tiver o instalador consegue
extrair esses dados.

## Atualizações automáticas (auto-update)

O app confere atualizações **3 s depois de abrir** (`App.vue` →
`src/composables/useAtualizacao.ts`): se houver versão nova, pergunta ao
usuário, baixa com progresso, instala em modo passivo e reinicia sozinho.
Falhas (offline, sem release) são **silenciosas de propósito** — o app nunca
trava por causa disso (o erro fica em `console.error`).

**De onde o app lê**: `plugins.updater.endpoints` no `tauri.conf.json` →
`https://github.com/SpellerX/scd/releases/latest/download/latest.json`.
Cada pacote é conferido com a chave pública (`plugins.updater.pubkey`), que
precisa ser o **mesmo par** da chave privada usada na assinatura
(`~/.tauri/scd-updater.key` + `.pub`).

**Checklist de uma release que o updater consegue instalar**:

1. `"createUpdaterArtifacts": true` em `bundle` no `tauri.conf.json`. **Sem
   isso o build não gera o `.sig` (assinatura) nem o pacote do updater, o
   `tauri-action` não publica o `latest.json` e o app NUNCA se atualiza** — o
   endpoint devolve 404 e, como o erro é silencioso, nada aparece na tela;
2. secrets `TAURI_SIGNING_PRIVATE_KEY` e
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` no repositório (sem eles o build com
   assinatura falha);
3. **versão nova nos 3 arquivos**: `package.json`, `src-tauri/Cargo.toml` e
   `src-tauri/tauri.conf.json` (o updater só instala se a versão da release
   for maior que a instalada);
4. tag + push (`git tag v0.2.2 && git push origin v0.2.2`) — o workflow
   `release.yml` compila, assina e publica.

**Como conferir se a release ficou publicável**: na página da release
(aba *Assets*) devem existir o `latest.json` **e** o
`..._x64-setup.exe.sig`. Sem os dois, o updater está quebrado para aquela
versão. Teste rápido do endpoint (no navegador ou num terminal com rede):

```powershell
(Invoke-WebRequest -Method Head `
  https://github.com/SpellerX/scd/releases/latest/download/latest.json).StatusCode
# 200 = updater publicável | 404 = falta o latest.json na release
```

O `release.yml` fixa `updaterJsonPreferNsis: true` para o `latest.json`
apontar para o **mesmo tipo de instalador** que o app recomenda (o
`setup.exe` do NSIS) — com o padrão do `tauri-action` e o bundle `all`, ele
apontaria para o `.msi`, e misturar NSIS com MSI na atualização causa
problema de registro/desinstalação.

Instalações antigas passam a se atualizar sozinhas a partir da primeira
release publicada **já com** o `latest.json` (não é preciso reinstalar à mão).

## Comandos úteis

```bash
npm install           # instala as dependências
npm run tauri dev     # app completo: janela nativa + backend + HMR
npm run build         # type-check (vue-tsc) + build do frontend
npm test              # testes do frontend (regras de exibição/validação)
npm run tauri build   # binário/instalador de produção
```

## Testes automatizados

Dois conjuntos, ambos sem serviço externo (nenhum precisa de rede):

**Backend (Rust)** — `cd src-tauri && cargo test`:

- `db.rs` — abertura/migração do esquema (v0 → versão atual), incluindo a
  migração **v4** (colunas do alarme) sem perder dados;
- `ferias.rs` — regras das férias: alarme manual (data no passado/período
  vencido/regularizado são recusados), período agendado entrando na lista
  mesmo fora da janela automática, aviso só na data escolhida, alerta
  automático intacto para quem **não** tem alarme, `updated_at` carimbado ao
  salvar/remover o alarme (para a sincronização enviar a mudança);
- `sync.rs` — payload/ida-e-volta do alarme entre máquinas e a regra
  last-write-wins (registro remoto mais antigo não sobrescreve o local).

O teste marcado como `ignored` (`e2e_sincroniza_com_nuvem_real`) é a prova de
ponta a ponta contra um Supabase de verdade: exige `.env` com credenciais e
internet — rode com
`cargo test --lib e2e_sincroniza_com_nuvem_real -- --ignored --nocapture`.

**Frontend** — `npm test` (executor de testes do próprio Node, sem
dependências novas; usa `--test-isolation=none` para rodar tudo num processo):

- cobre as funções puras de `src/utils/ferias.ts` — selo de prazo do alerta
  automático, selo do alarme ("aviso pendente"/"avisa em N dias"), rótulo do
  período no seletor e a validação do formulário de agendamento (que são
  exatamente as regras usadas pelas telas);
- cobre os cartões da tela inicial (`src/components/dashboard/kpis.ts`) —
  valores, notas e, principalmente, **para onde cada cartão leva**: o teste
  falha se um destino apontar para uma seção que não existe no menu
  (`src/config/menu.ts`), o que pegaria um id renomeado por engano.

Os arquivos `*.test.ts` ficam fora do `vue-tsc` (veja `exclude` no
`tsconfig.json`): o executor do Node não usa os tipos do projeto e
`@types/node` não é dependência. Tipos e build continuam verificados por
`npm run build`.

## Pré-requisitos

- Node.js 18+ e Rust (stable): https://tauri.app/start/prerequisites/
- Windows: WebView2 Runtime (padrão no Win10/11)
