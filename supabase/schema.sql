-- ═══════════════════════════════════════════════════════════════════════════
-- SCD — Esquema da nuvem (Supabase / Postgres)
--
-- Espelho das tabelas locais sincronizáveis do app (companies, employees,
-- employee_leave_periods). Como aplicar:
--   1. Crie o projeto no Supabase;
--   2. Abra o SQL Editor e cole TODO este arquivo, executando em ordem;
--   3. O app usa uma "conta dona" (e-mail/senha criada em Authentication →
--      Users → Add user). Cada linha guarda `owner_id = auth.uid()` dessa
--      conta; o RLS garante que ninguém além do dono enxerga os dados.
--
-- Regras espelhadas do app:
--   - ids UUID (chave única global entre máquinas);
--   - soft delete (`deleted_at`) em vez de DELETE físico;
--   - unicidade de CNPJ/CPF/período apenas entre registros ATIVOS;
--   - `updated_at` mantido pelo banco (trigger) para a sincronização delta;
--   - exclusão de empresa com funcionários ativos é bloqueada por trigger.
-- ═══════════════════════════════════════════════════════════════════════════

-- ── Trigger global: atualiza updated_at em qualquer UPDATE ────────────────
create or replace function public.scd_set_updated_at()
returns trigger
language plpgsql
as $$
begin
    new.updated_at = now();
    return new;
end;
$$;

-- ── Tabela: companies ──────────────────────────────────────────────────────
create table if not exists public.companies (
    id            uuid        primary key default gen_random_uuid(),
    cnpj          text        not null,
    razao_social  text        not null,
    nome_fantasia text,
    situacao      text,
    municipio     text,
    uf            text,
    telefone      text,
    email         text,
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now(),
    deleted_at    timestamptz,
    owner_id      uuid        not null default auth.uid()
);

drop trigger if exists companies_updated_at on public.companies;
create trigger companies_updated_at
    before update on public.companies
    for each row execute function public.scd_set_updated_at();

-- CNPJ único entre empresas ATIVAS do mesmo dono.
create unique index if not exists companies_cnpj_ativo
    on public.companies (owner_id, cnpj)
    where deleted_at is null;

-- ── Tabela: employees ──────────────────────────────────────────────────────
create table if not exists public.employees (
    id            uuid        primary key default gen_random_uuid(),
    nome          text        not null,
    cpf           text,
    data_admissao date,
    empresa_id    uuid        not null references public.companies(id),
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now(),
    deleted_at    timestamptz,
    owner_id      uuid        not null default auth.uid()
);

drop trigger if exists employees_updated_at on public.employees;
create trigger employees_updated_at
    before update on public.employees
    for each row execute function public.scd_set_updated_at();

-- CPF único entre funcionários ATIVOS do mesmo dono.
create unique index if not exists employees_cpf_ativo
    on public.employees (owner_id, cpf)
    where deleted_at is null and cpf is not null;

-- ── Tabela: employee_leave_periods ─────────────────────────────────────────
create table if not exists public.employee_leave_periods (
    id                uuid        primary key default gen_random_uuid(),
    employee_id       uuid        not null references public.employees(id) on delete cascade,
    inicio            date        not null,
    vencimento        date        not null,
    regularizada      boolean     not null default false,
    regularizada_em   timestamptz,
    observacao        text,
    -- Alarme manual: data do aviso escolhida quando a empresa já agendou as
    -- férias (o período aparece em "Férias a vencer" antes da janela
    -- automática de 12 meses) + texto livre do agendamento.
    alarme_em         date,
    alarme_observacao text,
    created_at        timestamptz not null default now(),
    updated_at        timestamptz not null default now(),
    deleted_at        timestamptz,
    owner_id          uuid        not null default auth.uid()
);

-- Bancos já criados antes do alarme manual: acrescenta as colunas (idempotente).
alter table public.employee_leave_periods
    add column if not exists alarme_em date;
alter table public.employee_leave_periods
    add column if not exists alarme_observacao text;

drop trigger if exists employee_leave_periods_updated_at on public.employee_leave_periods;
create trigger employee_leave_periods_updated_at
    before update on public.employee_leave_periods
    for each row execute function public.scd_set_updated_at();

-- Um período por funcionário/ano, entre os ativos.
create unique index if not exists periods_employee_inicio_ativo
    on public.employee_leave_periods (employee_id, inicio)
    where deleted_at is null;

-- ── Bloqueio espelhado do app: empresa com funcionários ativos não pode
--    ser excluída (nem em soft delete) ─────────────────────────────────────
create or replace function public.scd_bloquear_exclusao_empresa()
returns trigger
language plpgsql
as $$
begin
    if (new.deleted_at is not null) then
        if exists (
            select 1 from public.employees e
            where e.empresa_id = old.id
              and e.deleted_at is null
              and e.owner_id = old.owner_id
        ) then
            raise exception 'Esta empresa possui funcionários vinculados. Exclua ou mova os funcionários antes.';
        end if;
    end if;
    return new;
end;
$$;

drop trigger if exists companies_nao_excluir_com_funcionarios on public.companies;
create trigger companies_nao_excluir_com_funcionarios
    before update of deleted_at on public.companies
    for each row execute function public.scd_bloquear_exclusao_empresa();

-- ── Row Level Security (cada dono só vê os próprios dados) ─────────────────
alter table public.companies              enable row level security;
alter table public.employees              enable row level security;
alter table public.employee_leave_periods enable row level security;

drop policy if exists companies_select on public.companies;
create policy companies_select on public.companies
    for select using (owner_id = auth.uid());
drop policy if exists companies_insert on public.companies;
create policy companies_insert on public.companies
    for insert with check (owner_id = auth.uid());
drop policy if exists companies_update on public.companies;
create policy companies_update on public.companies
    for update using (owner_id = auth.uid()) with check (owner_id = auth.uid());
drop policy if exists companies_delete on public.companies;
create policy companies_delete on public.companies
    for delete using (owner_id = auth.uid());

drop policy if exists employees_select on public.employees;
create policy employees_select on public.employees
    for select using (owner_id = auth.uid());
drop policy if exists employees_insert on public.employees;
create policy employees_insert on public.employees
    for insert with check (owner_id = auth.uid());
drop policy if exists employees_update on public.employees;
create policy employees_update on public.employees
    for update using (owner_id = auth.uid()) with check (owner_id = auth.uid());
drop policy if exists employees_delete on public.employees;
create policy employees_delete on public.employees
    for delete using (owner_id = auth.uid());

drop policy if exists periods_select on public.employee_leave_periods;
create policy periods_select on public.employee_leave_periods
    for select using (owner_id = auth.uid());
drop policy if exists periods_insert on public.employee_leave_periods;
create policy periods_insert on public.employee_leave_periods
    for insert with check (owner_id = auth.uid());
drop policy if exists periods_update on public.employee_leave_periods;
create policy periods_update on public.employee_leave_periods
    for update using (owner_id = auth.uid()) with check (owner_id = auth.uid());
drop policy if exists periods_delete on public.employee_leave_periods;
create policy periods_delete on public.employee_leave_periods
    for delete using (owner_id = auth.uid());

-- ═══════════════════════════════════════════════════════════════════════════
-- USUÁRIOS DO SCD (login local espelhado na nuvem para controle central).
-- Chave = `username` (único por dono entre ativos). Os acessos por módulo
-- viajam embutidos na coluna `secoes` (lista de ids de seção/menu).
-- ⚠️ Não confundir com `auth.users` (contas de autenticação do Supabase).
-- ═══════════════════════════════════════════════════════════════════════════
create table if not exists public.users (
    username      text        primary key,
    display_name  text        not null,
    password_hash text        not null,
    secoes        text[]      not null default '{}',
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now(),
    deleted_at    timestamptz,
    owner_id      uuid        not null default auth.uid()
);

drop trigger if exists users_updated_at on public.users;
create trigger users_updated_at
    before update on public.users
    for each row execute function public.scd_set_updated_at();

alter table public.users enable row level security;

drop policy if exists users_select on public.users;
create policy users_select on public.users
    for select using (owner_id = auth.uid());
drop policy if exists users_insert on public.users;
create policy users_insert on public.users
    for insert with check (owner_id = auth.uid());
drop policy if exists users_update on public.users;
create policy users_update on public.users
    for update using (owner_id = auth.uid()) with check (owner_id = auth.uid());
drop policy if exists users_delete on public.users;
create policy users_delete on public.users
    for delete using (owner_id = auth.uid());
