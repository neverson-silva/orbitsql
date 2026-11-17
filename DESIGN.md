# Internet Banking — Experience & Design System Specification

> **Document type:** Product Experience + UX + UI + Design System Foundation  
> **Market:** Brazil  
> **Product:** White-label Corporate Internet Banking  
> **Primary audience:** Real estate developers, incorporators, SPEs, allocators, holding companies, treasury teams, finance operators and corporate approvers  
> **Language:** Portuguese (Brazil)  
> **Experience standard:** Premium consumer-grade banking with enterprise financial depth  
> **Design maturity target:** 2026+

---

# 1. Product Design Mission

This product is a **real Internet Banking experience for companies**.

It is not:

- an administrative dashboard;
- a banking backoffice;
- an ERP;
- a generic B2B SaaS;
- a financial CRUD application;
- a collection of KPI cards;
- a white-label admin template.

The central design challenge is to combine:

**consumer-grade simplicity + corporate banking depth + financial trust + governance + real-estate context.**

The product may handle sophisticated corporate operations such as:

- multiple companies;
- multiple SPEs;
- multiple bank accounts;
- multiple operators;
- transaction approval chains;
- payment limits;
- PIX;
- transfers;
- boletos;
- CNAB 240 payment batches;
- CNAB 400 collection remittances;
- trusted devices;
- MFA;
- account statements;
- receipts;
- cash position;
- real-estate financial context.

The complexity of those capabilities must **not become visual complexity**.

The experience must continuously communicate:

> "I am managing money."

Never:

> "I am operating an administrative system."

---

# 2. Emotional Signature

The emotional signature is:

**Financial Confidence.**

The interface should feel:

- calm;
- precise;
- trustworthy;
- sophisticated;
- modern;
- responsive;
- intelligent;
- deliberate;
- secure without feeling hostile;
- powerful without feeling complicated.

A financial director should trust the interface to move millions of reais.

An operational user should understand the interface without requiring extensive training.

A company owner should be able to quickly understand:

1. how much money is available;
2. what changed;
3. what requires attention;
4. what is scheduled;
5. what requires approval;
6. what they can do next.

---

# 3. Core Design Principles

## 3.1 Money First

Money is the primary information architecture.

Important monetary information must not be visually subordinate to navigation, modules, illustrations or decorative surfaces.

Balance, transaction amount, beneficiary, due date and approval state should dominate their respective contexts.

---

## 3.2 Context Before Controls

Before presenting actions, establish context.

For every critical operation the user must clearly understand:

- company;
- SPE, when relevant;
- account;
- available balance;
- beneficiary/payer;
- amount;
- execution date;
- approval requirements.

---

## 3.3 Progressive Disclosure

Corporate banking is complex.

The interface should not expose all complexity simultaneously.

Show:

**essential information → relevant context → advanced details on demand.**

This principle is especially important for:

- CNAB;
- approval chains;
- operator permissions;
- account limits;
- transaction metadata;
- audit trails;
- reconciliation;
- batch errors.

---

## 3.4 Financial Calm

Do not communicate importance through visual noise.

Avoid:

- excessive saturated colors;
- excessive cards;
- unnecessary borders;
- giant warning banners;
- large decorative illustrations;
- multiple competing primary actions;
- excessive badges;
- aggressive shadows.

Important information becomes prominent through:

- typography;
- composition;
- spacing;
- contrast;
- semantic color;
- controlled motion.

---

## 3.5 Consumer Simplicity, Enterprise Depth

The default experience should be simple.

Depth should appear when the user's task requires it.

A PIX payment should feel almost as direct as consumer banking.

A CNAB 240 batch may expose hundreds of records, validation errors and approval requirements.

Both belong to the same product.

The Design System must support this range without becoming visually inconsistent.

---

## 3.6 Explain Financial Consequences

Critical actions must clearly explain their consequences.

Before confirmation, communicate:

- amount;
- destination;
- source;
- execution date;
- fees when applicable;
- whether execution is immediate or scheduled;
- whether another operator must approve;
- whether funds will move immediately.

When an error occurs, explicitly communicate whether money moved.

Never leave this ambiguous.

---

# 4. Anti-Patterns

The following patterns are prohibited unless the task objectively requires them.

## Generic Dashboard Syndrome

Avoid:

- four KPI cards across the top;
- generic analytics widgets;
- meaningless line charts;
- colored statistics cards;
- "dashboard template" composition.

The Home is a **financial cockpit**, not a BI dashboard.

---

## Card-Within-Card Syndrome

Do not place every piece of content inside a card.

Avoid:

Card  
→ Card  
→ Panel  
→ bordered element  
→ badge.

Use whitespace, typography and surface hierarchy before introducing containers.

---

## Administrative Backoffice Language

Avoid interfaces dominated by:

- records;
- entities;
- CRUD;
- configuration;
- management tables;
- technical IDs.

Translate technical banking operations into understandable financial tasks.

---

## Color as Decoration

Color exists to communicate:

- identity;
- hierarchy;
- action;
- financial semantics;
- state;
- risk;
- focus.

Never use color merely to make a screen "less boring".

---

# 5. White-Label Design Philosophy

This is a white-label banking platform.

A tenant may have a primary identity that is:

- blue;
- green;
- purple;
- red;
- orange;
- cyan;
- magenta;
- brown;
- black;
- low saturation;
- extremely saturated;
- very light;
- very dark.

Therefore the interface must never depend on a specific hue to work.

Brand identity is an **input into the Design System**, not the Design System itself.

---

# 6. Color Architecture

Color must be organized by semantic responsibility.

The conceptual families are:

```text
Structural
Brand
Action
Financial
Feedback
Risk
Navigation
Data Visualization
Focus
```

These families must not be casually interchangeable.

---

# 7. Perceptual Color Model

Color generation and evaluation should be perceptual.

Prefer perceptual reasoning such as **OKLCH** rather than manipulating RGB/HSL values directly.

Think in terms of:

- perceived lightness;
- chroma;
- hue;
- contrast;
- perceptual distance;
- semantic role;
- occupied visual area.

Accessibility is a hard constraint.

Brand fidelity is optimized inside that constraint.

The governing principle is:

> Semantic intent → accessibility → minimum perceptual correction → brand preservation.

---

# 8. Brand Preservation

Brand colors are anchors.

They should not be unnecessarily transformed.

If an original brand color already satisfies the requirements of a role, preserve it.

When correction is necessary, prefer:

1. minimal lightness adjustment;
2. minimal chroma adjustment;
3. hue modification only as a last resort.

The objective is not maximum contrast.

The objective is the **closest valid color**.

---

# 9. Large-Area Color Theory

Perceived color dominance depends not only on color values but also on occupied area.

A saturated color used in a 40px button behaves very differently from the same color covering a full-height sidebar.

Therefore:

> chromatic expression must consider semantic role AND visual exposure.

Large structural areas should generally receive lower chromatic expression than compact action elements.

Conceptual hierarchy:

```text
Page / Canvas
    very low brand expression

Card / Elevated Surface
    very low brand expression

Top Shell
    low brand expression

Navigation Shell
    low → medium brand expression

Selected Navigation Item
    medium brand expression

Brand Content
    medium → high expression

Focus
    medium → high expression

Primary Action
    high expression
```

The navigation shell must never visually overpower the primary financial content merely because the tenant uses a saturated brand color.

---

# 10. Structural Surfaces

Structural surfaces form the environment.

Conceptual roles:

```text
surface.page
surface.section
surface.card
surface.subtle
surface.elevated
surface.overlay
surface.topbar
surface.navigation
```

Structural surfaces should be predominantly neutral while carrying a subtle chromatic signature derived from the tenant brand.

Avoid pure sterile gray unless appropriate.

Avoid turning structural surfaces directly into saturated brand colors.

---

# 11. Surface Hierarchy

Hierarchy must remain perceptible without excessive borders.

Conceptually:

```text
Page
 ↓
Section
 ↓
Card
 ↓
Elevated
 ↓
Overlay
```

Differences may be communicated using:

- perceptual lightness;
- extremely subtle chroma;
- spatial separation;
- elevation;
- borders where necessary.

Do not make every layer equally prominent.

---

# 12. Content Hierarchy

Text roles:

```text
content.primary
content.secondary
content.tertiary
content.disabled
content.inverse
content.brand
```

Perceptual prominence must follow:

```text
primary
>
secondary
>
tertiary
>
disabled
```

Disabled content must remain readable while clearly losing emphasis.

---

# 13. Financial Color Semantics

Financial movement and system feedback are separate concepts.

Never assume:

```text
money coming in = success
money leaving = danger
```

An outgoing PIX may be perfectly successful.

An incoming payment may be pending, disputed or blocked.

Financial roles should include concepts such as:

```text
finance.balance
finance.inflow
finance.outflow
finance.projected
finance.pending
finance.blocked
finance.overdue
finance.settled
```

These roles must remain perceptually distinguishable.

---

# 14. Feedback Semantics

Feedback communicates application or transaction state.

Conceptual families:

```text
feedback.success
feedback.warning
feedback.danger
feedback.info
```

Each feedback family should support:

```text
solid
on-solid
content
surface
surface-subtle
border
```

Feedback colors should not be reused as arbitrary chart colors.

---

# 15. Risk Semantics

Risk is contextual.

Conceptual levels:

```text
risk.low
risk.medium
risk.high
```

Each may provide:

```text
surface
content
border
icon
```

Risk must never be communicated using color alone.

Use:

- text;
- iconography;
- labeling;
- contextual explanation.

---

# 16. Dark Mode

Dark mode is a first-class banking experience.

It must not be produced through:

- RGB inversion;
- simple lightness reversal;
- black backgrounds + neon branding;
- independent token manipulation.

Dark mode must preserve the semantic relationships established by the light theme.

The process is conceptually:

```text
Brand Identity
       ↓
Compiled Light Semantic System
       ↓
Semantic Relationships
       ↓
Dark Semantic Families
       ↓
Pair Validation
       ↓
Perceptual Correction
       ↓
Compiled Dark Theme
```

Large dark surfaces should remain calm.

Chromatic accents should remain controlled.

---

# 17. Navigation Color

Navigation is a **structural branded surface**, not a giant primary button.

Its base surface should be quieter than:

- primary actions;
- focus accents;
- selected navigation;
- important financial signals.

Selected navigation should become more expressive than its surrounding navigation surface.

Never create:

```text
bright brand wall
+
dark selected rectangle
```

as the default dark-mode navigation strategy.

---

# 18. Application Shell

The shell is one of the product's defining visual signatures.

Sidebar and top header must visually form **one continuous architectural frame**.

They must not appear as two unrelated rectangles.

Conceptually:

```text
┌─────────────────────────────────────────────────────────┐
│ Navigation Shell        Top Shell                       │
│                        ╭────────────────────────────────│
│                        │                                │
│ Navigation             │ Main Banking Canvas            │
│                        │                                │
│                        │                                │
└────────────────────────┴────────────────────────────────┘
```

The upper-left transition into the main canvas uses a deliberate continuous radius.

The content should feel inserted into the surrounding shell.

This is not decoration.

It establishes spatial hierarchy:

```text
Application Shell
    ↓
Financial Workspace
```

---

# 19. Sidebar

The sidebar represents financial navigation.

Not system modules.

Primary navigation may contain:

- Início
- Contas
- Movimentações
- PIX
- Pagamentos
- Transferências
- Cobranças
- Lotes e arquivos
- Aprovações
- Empreendimentos
- Comprovantes

Secondary areas may contain:

- Operadores e acessos
- Segurança
- Dispositivos
- Notificações

Avoid excessive nesting.

Navigation labels must describe user intentions, not internal architecture.

---

# 20. Top Shell

The top shell provides global context.

Potential elements:

- company/SPE context;
- active account;
- global search;
- pending approvals;
- notifications;
- operator identity;
- contextual utilities.

It must remain visually quiet.

It should not become a second navigation dashboard.

---

# 21. Context Switching

Multi-company and multi-account context is fundamental.

A user may simultaneously operate:

```text
Grupo Aurora

├── Aurora Desenvolvimento Ltda.
│   ├── Conta operacional
│   └── Conta administrativa
│
├── Residencial Vista Norte SPE Ltda.
│   ├── Conta de obra
│   └── Conta de recebíveis
│
└── Parque Central SPE Ltda.
    └── Conta operacional
```

The user must always know:

- which legal entity is active;
- which account is active;
- whether an action affects one account or consolidated context.

Context switching should feel like financial navigation, not tenant administration.

---

# 22. Main Content Canvas

The main canvas is the primary financial workspace.

It should be visually calmer than the shell.

Prefer:

- generous but disciplined whitespace;
- editorial financial typography;
- restrained surface separation;
- adaptive content width;
- contextual side rails when useful.

Do not stretch content indefinitely on large monitors.

---

# 23. Layout Grid

Use a responsive grid based on a small spatial primitive.

A conceptual 4px foundation may generate:

```text
4
8
12
16
20
24
32
40
48
64
```

Spacing must represent relationships.

Closer elements imply stronger semantic relationships.

Do not mechanically apply identical spacing everywhere.

---

# 24. Responsive Strategy

The application must support both:

**mobile-first interaction thinking**
and
**excellent desktop productivity.**

Corporate users may spend hours on desktop.

Mobile users may need to:

- approve;
- inspect balance;
- make PIX;
- inspect transactions;
- access receipts;
- respond to urgent financial events.

Desktop should support:

- dense transaction analysis;
- batch operations;
- CNAB;
- multi-account navigation;
- approvals;
- treasury workflows.

Mobile must not simply be a scaled-down desktop.

---

# 25. Large-Screen Composition

On large monitors, use composition rather than uncontrolled stretching.

Possible structure:

```text
Main Financial Narrative
+
Contextual Rail
```

The rail may contain:

- pending approvals;
- financial attention;
- contextual actions;
- upcoming events.

Avoid empty horizontal expansion.

---

# 26. Typography Philosophy

Typography carries the majority of visual hierarchy.

Prefer typography before introducing:

- cards;
- borders;
- colored containers;
- shadows.

The system should distinguish:

```text
Display Financial
Page Title
Section Title
Body
Supporting
Label
Microcopy
Numeric
```

---

# 27. Financial Typography

Monetary values deserve a dedicated typographic treatment.

Example:

```text
Saldo disponível

R$ 12.458.920,34
```

Large monetary values should:

- scan immediately;
- use tabular numerals where appropriate;
- maintain clear decimal hierarchy;
- avoid unnecessary visual effects.

Never use gambling-style rolling number animations.

---

# 28. Numeric Alignment

Use tabular numeric features for:

- monetary values;
- balances;
- transaction lists;
- reconciliation;
- CNAB summaries;
- financial comparisons.

Monospace is not required merely because the content is numerical.

Financial information should still feel like banking, not a developer tool.

---

# 29. Shapes

Geometry should feel modern, sophisticated and controlled.

Use:

- medium radii for controls;
- larger radii for important containers;
- expressive radius for the shell/content intersection;
- pill geometry only where semantically appropriate.

Avoid excessive bubbly geometry.

The product must not feel playful or childish.

---

# 30. Radius Hierarchy

Conceptual relationship:

```text
small control
<
field/button
<
card/container
<
major shell transition
```

Nested radii should maintain visual harmony.

---

# 31. Elevation

Elevation is functional.

Use it to communicate:

- layering;
- overlays;
- menus;
- drawers;
- contextual panels.

Avoid generic floating-card shadows.

Primary depth should come from:

- surfaces;
- composition;
- controlled borders;
- spatial separation.

---

# 32. Borders

Borders are a last-mile structural tool.

Prefer:

1. spacing;
2. typography;
3. surface distinction;
4. then borders.

When borders are required, establish hierarchy:

```text
border.subtle
border.default
border.strong
border.focus
```

Their perceived strength must follow the same order.

---

# 33. Atomic Design Architecture

The UI follows Atomic Design principles.

The hierarchy is:

```text
Atoms
↓
Molecules
↓
Organisms
↓
Templates
↓
Pages
```

Atomic Design must create consistency without forcing every screen into the same visual composition.

---

# 34. Atoms

Examples:

- Button
- Icon
- Text
- CurrencyValue
- Badge
- StatusIndicator
- Avatar
- Divider
- Input
- Checkbox
- Radio
- Switch
- Tooltip
- Spinner
- Skeleton
- FocusRing

Atoms must not understand banking workflows.

---

# 35. Molecules

Examples:

- AccountSelector
- CompanySelector
- AmountInput
- TransactionRow
- ApprovalIndicator
- FinancialStatus
- DateField
- SearchField
- BeneficiaryIdentity
- AccountIdentity
- BalanceSummary
- FinancialMetric
- FilterControl

Molecules combine primitives around a small semantic purpose.

---

# 36. Organisms

Examples:

- ApplicationSidebar
- TopShell
- AccountSwitcher
- TransactionList
- StatementFilters
- PaymentComposer
- PixComposer
- ApprovalQueue
- ApprovalTimeline
- CNABUploader
- CNABValidationSummary
- OperatorPermissionEditor
- TrustedDeviceManager
- NotificationCenter

Organisms represent meaningful product capabilities.

---

# 37. Templates

Examples:

- AuthenticationTemplate
- FinancialHomeTemplate
- FinancialListTemplate
- TransactionComposerTemplate
- ReviewTemplate
- ConfirmationTemplate
- ApprovalTemplate
- DetailTemplate
- BatchOperationTemplate
- SecurityTemplate

Templates define spatial composition without containing tenant-specific data.

---

# 38. Pages

Pages combine templates with actual financial scenarios.

Examples:

- Início
- Extrato
- Fazer PIX
- Pagar boleto
- Revisar pagamento
- Comprovante
- Remessa CNAB 240
- Aprovar lote
- Emitir cobrança
- Operadores
- Dispositivos confiáveis

---

# 39. Buttons

Button hierarchy should be semantic.

Conceptual variants:

```text
Primary
Secondary
Quiet
Destructive
```

Do not create a different visual button type for every business action.

Examples of primary actions:

- Continuar
- Revisar pagamento
- Enviar PIX
- Enviar para aprovação
- Aprovar

Only one visually dominant primary action should normally exist per decision context.

---

# 40. Primary Action

Primary actions use strong brand expression.

They must remain visually dominant without becoming oversized.

Primary actions require:

- default;
- hover;
- pressed;
- focus;
- loading;
- disabled.

Disabled state must not rely solely on opacity.

---

# 41. Destructive Action

Destructive actions include:

- revogar dispositivo;
- cancelar agendamento;
- remover operador;
- rejeitar operação when consequence is destructive;
- cancel financial instructions when irreversible.

Danger styling must be reserved for genuine destructive consequences.

---

# 42. Inputs

Inputs must feel appropriate for high-trust financial interaction.

They should provide:

- persistent labels where needed;
- clear focus;
- inline validation;
- helper text;
- formatted financial values;
- accessible error messages.

Do not rely on placeholders as labels.

---

# 43. Amount Input

Amount fields are financially important components.

Example:

```text
Valor

R$ 125.000,00
```

The amount should receive stronger typography than ordinary text inputs.

Validation should communicate limits and available balance contextually.

---

# 44. Forms

Financial forms should not look like CRUD forms.

Use conversational sequencing and progressive disclosure.

For example, a PIX flow may follow:

```text
Para quem?
↓
Quanto?
↓
Quando?
↓
De qual conta?
↓
Revisão
↓
Confirmação / Aprovação
```

instead of exposing twenty fields simultaneously.

---

# 45. Critical Transaction Review

Every critical money movement should have a dedicated review state.

Show clearly:

```text
De
Conta operacional
Agência 0001 · Conta 123456-7

Para
Construtora Horizonte Ltda.

Valor
R$ 280.000,00

Quando
Hoje

Aprovação
Necessita de mais 1 aprovador
```

The user must be able to visually audit the operation before confirming.

---

# 46. Transaction Success

Never reduce transaction completion to a generic green message.

The completion state should include:

- amount;
- beneficiary;
- date/time;
- status;
- transaction identifier;
- receipt;
- next actions.

Example:

```text
PIX enviado

R$ 18.450,00

para Construtora Horizonte Ltda.

12 set 2026 · 14:32

[Ver comprovante]
[Fazer outro PIX]
```

---

# 47. Transaction Failure

Financial errors must remove uncertainty.

Example:

```text
Não foi possível concluir o PIX.

Nenhum valor foi debitado da conta.

Você pode tentar novamente ou consultar os detalhes da tentativa.
```

Never show only:

```text
Algo deu errado.
```

---

# 48. Home Experience

Home is a financial cockpit.

It should answer:

```text
Quanto tenho?
O que mudou?
O que vai acontecer?
O que exige minha atenção?
O que posso fazer agora?
```

The Home is not a dashboard.

---

# 49. Home Hierarchy

Recommended conceptual hierarchy:

```text
Context
↓
Available Balance
↓
Cash Movement / Projection
↓
Financial Attention
↓
Primary Actions
↓
Recent Activity
↓
Real-Estate Context
```

Not every section needs a card.

---

# 50. Balance

Balance is one of the strongest visual anchors.

Prefer editorial presentation.

Example:

```text
Saldo disponível

R$ 12.458.920,34

Saldo consolidado em 4 contas
```

Avoid giant colored balance cards unless the composition specifically requires one.

---

# 51. Balance Privacy

Users must be able to hide sensitive financial values.

Privacy mode should affect relevant values consistently.

Reveal/hide motion must be subtle and immediate.

---

# 52. Cash Movement

Cash visualization should explain movement rather than resemble generic analytics.

Possible concepts:

- current balance;
- expected inflows;
- expected payments;
- blocked balance;
- projected balance.

Charts should support a financial narrative.

---

# 53. Quick Actions

Common actions may include:

- Fazer PIX
- Pagar
- Transferir
- Criar cobrança
- Enviar remessa

Quick actions should be compact.

Avoid giant feature cards.

---

# 54. Financial Attention

A compact attention organism may surface:

- pagamentos aguardando aprovação;
- remessas com divergências;
- boletos vencidos;
- suspicious access;
- scheduled transactions requiring funds.

Attention should prioritize relevance, not visual alarm.

---

# 55. Transaction Lists

Transaction lists should be financial lists by default, not spreadsheet tables.

A transaction row may contain:

```text
Counterparty
Description / type
Date
Context
Status
Amount
```

Hierarchy should make the counterparty and amount easy to scan.

---

# 56. Tables

Tables are appropriate when comparison across columns is the task.

Examples:

- CNAB validation;
- large operator lists;
- reconciliation;
- batch records;
- permission matrices when necessary.

Do not default to tables for every list.

---

# 57. Transaction Detail

Transaction detail should reveal:

- amount;
- status;
- beneficiary/payer;
- source;
- destination;
- timestamps;
- identifiers;
- project/SPE context;
- authorization history;
- receipt.

Secondary technical metadata should be progressively disclosed.

---

# 58. PIX

PIX must feel native to Brazilian banking.

Support:

- chave PIX;
- dados bancários;
- QR Code;
- Pix Copia e Cola;
- agendamento;
- favoritos/recentes;
- limits;
- confirmation;
- receipt.

Do not model PIX as a generic international transfer.

---

# 59. Payments

Payments should support Brazilian conventions.

For boleto payment, after entering the linha digitável, immediately expose:

- beneficiary;
- issuer;
- due date;
- original amount;
- discount;
- interest;
- fine;
- final amount.

The user should verify these before proceeding.

---

# 60. CNAB 240

CNAB 240 is an enterprise feature but must not feel archaic.

The experience should transform a technical file operation into a clear financial workflow:

```text
Enviar arquivo
↓
Validar
↓
Revisar lote
↓
Resolver divergências
↓
Enviar para aprovação
↓
Processar
↓
Resultado
↓
Arquivo de retorno
```

---

# 61. CNAB Validation

Never communicate only:

```text
Arquivo inválido.
```

Expose:

- record;
- line;
- field;
- reason;
- severity;
- recommended correction.

Provide an overall summary:

```text
248 pagamentos

R$ 4.820.300,00

236 válidos
8 com alertas
4 inválidos
```

---

# 62. CNAB 400 / Cobrança

Collection flows should support:

- boleto creation;
- payer;
- due date;
- amount;
- registration;
- remittance generation;
- CNAB 400;
- return processing;
- settlement;
- reconciliation.

The experience must remain banking-first.

Do not transform the product into accounts-receivable ERP software.

---

# 63. Approval Experience

Approval is a first-class banking capability.

The user should immediately understand:

- what is being approved;
- amount;
- beneficiary;
- initiator;
- company/account;
- due date;
- risk/context;
- approvals completed;
- approvals remaining.

---

# 64. Approval Queue

Avoid generic administrative tables as the primary presentation.

Prioritize:

- urgency;
- amount;
- due date;
- transaction type;
- approval progress.

---

# 65. Approval Timeline

Audit history should use a human-readable timeline.

Example:

```text
14:21
Criado por Mariana Costa

14:25
Aprovado por Carlos Mendes

Aguardando
1 aprovação restante
```

Technical audit metadata may be expanded on demand.

---

# 66. Operators

Operator management is part of corporate banking governance.

Possible roles:

- Administrador
- Financeiro
- Tesouraria
- Aprovador
- Consulta
- Auditoria

Do not expose implementation-specific RBAC terminology unnecessarily.

---

# 67. Operator Permissions

Avoid presenting a giant permission matrix immediately.

Organize permissions by banking domain:

```text
Contas
PIX
Pagamentos
Transferências
Cobranças
Lotes e arquivos
Aprovações
Comprovantes
Operadores
```

Use progressive disclosure for granular permissions.

---

# 68. Limits

Limits should communicate:

- configured limit;
- used amount;
- remaining amount;
- period;
- pending changes;
- approval state.

Examples:

```text
PIX diário
R$ 500.000

Utilizado hoje
R$ 120.000

Disponível
R$ 380.000
```

---

# 69. MFA

MFA should feel reassuring rather than punitive.

Explain:

- why verification is required;
- what is being authorized;
- available method;
- expiration/resend;
- alternative verification when allowed.

---

# 70. Trusted Devices

Device management should show:

- recognizable device name;
- browser/application;
- operating system;
- approximate location if available;
- trusted since;
- last activity;
- status.

Actions:

- confiar neste dispositivo;
- renomear;
- revogar.

---

# 71. Global Search

Global search should search financial concepts.

Examples:

- transaction;
- beneficiary;
- payer;
- amount;
- boleto;
- receipt;
- company;
- SPE;
- account;
- operator.

It may also surface actions:

- Fazer PIX
- Pagar boleto
- Criar cobrança
- Enviar remessa

Search must remain understandable to non-technical users.

---

# 72. Real-Estate Context

Real-estate context enriches banking; it does not replace it.

Possible entities:

```text
Empreendimento
SPE
Conta de obra
Conta de recebíveis
Conta operacional
Unidade
Contrato
```

An enterprise view may aggregate:

- linked accounts;
- consolidated balance;
- movements;
- expected payments;
- collections;
- approvals.

Avoid becoming a construction management ERP.

---

# 73. Empty States

Empty states should be compact and contextual.

Example:

```text
Nenhum pagamento aguardando aprovação.

Quando uma nova solicitação chegar, ela aparecerá aqui.
```

Avoid oversized illustrations that consume the entire workspace.

---

# 74. Loading

Loading must preserve spatial stability.

Prefer:

- skeletons;
- localized loading;
- progressive rendering.

Avoid blocking the entire application for small data refreshes.

---

# 75. Optimistic UI

Optimistic interaction may be used only when financially safe.

Never visually declare:

```text
Pagamento concluído
```

before execution has been confirmed.

---

# 76. Microinteraction Philosophy

Microinteractions communicate state, causality and confidence.

They are not decoration.

Every important interactive component should define, when applicable:

- default;
- hover;
- focus;
- pressed;
- selected;
- loading;
- success;
- warning;
- error;
- disabled.

---

# 77. Motion Language

Motion should communicate:

```text
cause
→
transition
→
new state
```

Examples:

- account switcher expands from its trigger;
- transaction detail enters from its originating row;
- successful copy morphs icon into confirmation;
- balance privacy transition masks/reveals values;
- approval status updates without abrupt page replacement.

---

# 78. Motion Timing

Recommended conceptual ranges:

```text
Hover                 100–150ms
Micro state           120–180ms
Popover/menu          150–220ms
Drawer/panel          200–280ms
Page continuity       250–350ms
```

Avoid:

- excessive bounce;
- spring effects everywhere;
- slow ornamental animation;
- animation that delays financial work.

---

# 79. Motion Easing

Prefer:

- ease-out for entry;
- ease-in for exit;
- smooth standard easing for state transitions.

Motion should feel precise and controlled.

---

# 80. Reduced Motion

Respect reduced-motion preferences.

All critical state transitions must remain understandable without animation.

---

# 81. Microcopy

All product copy is Portuguese (Brazil).

Language should be:

- direct;
- calm;
- precise;
- financial;
- human.

Avoid unnecessary technical terminology.

Example:

Bad:

```text
Falha no processamento do payload.
```

Good:

```text
Não foi possível processar este pagamento.
Nenhum valor foi debitado.
```

---

# 82. Confidence Microcopy

Use contextual microcopy to reduce uncertainty.

Examples:

```text
Você poderá revisar os dados antes de enviar.

Esta operação precisará de 2 aprovações.

Nenhum valor foi movimentado.

O pagamento está agendado para 15/09/2026.
```

---

# 83. Brazilian Localization

Currency:

```text
R$ 1.234.567,89
```

Dates:

```text
12/09/2026
```

Times:

```text
14:32
```

Identifiers must use Brazilian formatting when relevant:

- CPF;
- CNPJ;
- agência;
- conta;
- chave PIX;
- linha digitável.

---

# 84. Iconography

Icons should support recognition.

Use a consistent icon family and stroke language.

Avoid adding icons to every text label.

Important states must not depend solely on iconography.

---

# 85. Status Components

Status requires at least:

- semantic label;
- appropriate visual treatment;
- optionally icon.

Examples:

- Agendado
- Em processamento
- Aguardando aprovação
- Concluído
- Rejeitado
- Vencido
- Cancelado

Do not use ambiguous dots without labels for important financial states.

---

# 86. Badges

Badges should represent concise metadata.

They are not miniature containers for arbitrary text.

Avoid badge overload.

---

# 87. Drawers

Drawers are useful when context continuity matters.

Good examples:

- transaction detail from statement;
- receipt preview;
- contextual approval information.

Complex financial flows should normally use dedicated pages rather than cramped drawers.

---

# 88. Modals

Use modals for short, focused tasks.

Examples:

- confirmation;
- rename trusted device;
- simple filters;
- lightweight selection.

Do not put complex PIX, payment, CNAB or permission flows inside small modals.

---

# 89. Accessibility

WCAG AA is the minimum target.

All experiences must support:

- sufficient contrast;
- keyboard navigation;
- visible focus;
- semantic labeling;
- screen readers;
- appropriate touch targets;
- reduced motion;
- color-independent status communication.

Accessibility failures are design failures.

---

# 90. Focus

Focus must be unmistakable in both light and dark mode.

Focus styling must not be confused with:

- selected;
- hover;
- error.

Keyboard users must always know where they are.

---

# 91. Keyboard Interaction

Standard expectations should work consistently:

```text
Tab
Shift + Tab
Enter
Space
Escape
Arrow keys
```

Global search and high-frequency workflows may support shortcuts, but shortcuts must never be required to operate the product.

---

# 92. Touch Targets

Interactive targets should remain comfortably usable on mobile.

Do not preserve desktop density by shrinking controls below reasonable touch dimensions.

---

# 93. Density

Different banking tasks require different densities.

Conceptual modes:

```text
Comfortable
Standard
Dense
```

But density should be selected by context rather than making every page configurable.

Examples:

Home → comfortable

PIX → comfortable

Statement → standard

Approval queue → standard

CNAB validation → dense

---

# 94. Data Visualization

Charts have their own semantic palette.

Do not copy:

- feedback colors;
- risk colors;
- primary button colors

directly into chart series without semantic justification.

Chart colors must maintain perceptual separation.

---

# 95. Chart Hierarchy

For charts:

```text
grid
<
axis
<
reference
<
data
<
highlighted data
```

Grid lines must remain quiet.

The chart must not become more visually dominant than the financial conclusion it supports.

---

# 96. Chart Accessibility

Chart series should be distinguishable through more than hue when necessary.

Consider:

- luminance;
- chroma;
- stroke;
- shape;
- labels;
- direct annotation.

Critical meaning should not depend solely on color discrimination.

---

# 97. Financial Data Visualization

Prefer direct financial explanations.

Instead of a generic chart titled:

```text
Fluxo financeiro
```

prefer contextual communication such as:

```text
Saldo projetado para 30 dias

R$ 9,8 mi

R$ 2,4 mi em pagamentos previstos
R$ 1,7 mi em recebimentos esperados
```

Visualization supports this narrative.

It does not replace it.

---

# 98. Component State Contract

Every interactive component must explicitly support the states relevant to its behavior.

Do not create visual states ad hoc inside pages.

State semantics belong to the Design System.

---

# 99. Selected Is Not Hover

Selected, hover and focus are distinct concepts.

Never implement:

```text
selected = persistent hover
```

Selected represents state.

Hover represents temporary pointer intent.

Focus represents keyboard/input location.

---

# 100. Disabled Is Not Opacity

Disabled state must be intentionally designed.

Avoid simply applying:

```css
opacity: 0.5;
```

to entire components.

Maintain readability and semantic hierarchy.

---

# 101. Content Priority Test

Every page should pass this test:

**What should the user notice first?**

If the answer is:

- sidebar;
- giant brand area;
- decorative card;
- illustration;

instead of the relevant financial information, hierarchy is wrong.

---

# 102. Financial Trust Test

For every critical operation ask:

> Could a finance director confidently authorize a high-value transaction from this screen?

If not, improve:

- information clarity;
- consequence visibility;
- context;
- review;
- confirmation.

---

# 103. Non-Technical User Test

Ask:

> Could a business operator understand this without knowing banking implementation details?

If not, simplify language and interaction.

---

# 104. White-Label Resilience Test

Temporarily imagine the tenant brand changing from:

```text
blue → orange
orange → green
green → purple
purple → monochromatic
```

If hierarchy collapses, the component depends too heavily on hue.

Fix the semantic architecture.

---

# 105. Backoffice Detection Test

Remove the tenant logo.

Ask:

> Does this still clearly look and behave like Internet Banking?

If it could equally be:

- CRM;
- project management;
- ERP;
- analytics;
- admin portal;

the experience is too generic.

---

# 106. Card Reduction Test

For every card ask:

> Does this content require containment?

If the answer is no:

remove the card.

Use composition.

---

# 107. Border Reduction Test

For every border ask:

> Is this boundary necessary for comprehension?

If spacing or surface hierarchy can communicate the same relationship:

remove the border.

---

# 108. Color Reduction Test

For every colored element ask:

> What semantic information does this color communicate?

If the answer is merely:

> It looks better.

remove or neutralize it.

---

# 109. Motion Quality Test

For every animation ask:

> What relationship or state transition does this explain?

If it explains nothing:

remove it.

---

# 110. Security Experience

Security should communicate control.

Not fear.

Important security surfaces include:

- MFA;
- trusted devices;
- active sessions;
- unusual login;
- limit changes;
- operator access.

Security messaging should be calm, specific and actionable.

---

# 111. Auditability

Corporate banking requires traceability.

Important operations should preserve:

- initiator;
- timestamp;
- approvers;
- status transitions;
- account context;
- operation identifier.

But audit information should not dominate primary workflows.

Use progressive disclosure.

---

# 112. Privacy and Sensitive Information

Sensitive financial data must be treated deliberately.

Support appropriate masking for:

- balances;
- account numbers;
- documents;
- sensitive identifiers.

Avoid accidentally exposing sensitive data in secondary UI surfaces.

---

# 113. Navigation Information Architecture

Navigation should be organized around banking intentions.

Preferred mental model:

```text
VISÃO FINANCEIRA

Início
Contas
Movimentações


MOVIMENTAR

PIX
Pagamentos
Transferências


RECEBER

Cobranças


OPERAÇÕES EM LOTE

Lotes e arquivos


GOVERNANÇA

Aprovações
Operadores e acessos


CONTEXTO

Empreendimentos


SUPORTE FINANCEIRO

Comprovantes
Notificações
Segurança
```

Exact grouping may evolve through usability testing.

Avoid exposing internal service architecture.

---

# 114. Application Experience Signature

The product should become recognizable through its experience even when white-labeled.

Its signature is the combination of:

1. unified navigation/top shell;
2. rounded architectural transition into the financial canvas;
3. editorial financial hierarchy;
4. restrained card usage;
5. strong monetary typography;
6. contextual financial actions;
7. intelligent progressive disclosure;
8. high-quality motion;
9. calm large-area color;
10. enterprise depth without administrative aesthetics.

---

# 115. Design Definition of Done

A screen is not complete merely because all requirements are visible.

It is complete when:

- primary financial intent is obvious;
- hierarchy is correct;
- semantic colors are correct;
- light mode works;
- dark mode works;
- mobile works;
- desktop works;
- keyboard works;
- focus works;
- loading works;
- empty state works;
- error state works;
- success state works;
- relevant microinteractions exist;
- no unnecessary cards remain;
- no unnecessary borders remain;
- no generic dashboard patterns remain.

---

# 116. Final Design Directive

This product moves real money.

Every design decision must respect that.

The interface must never prioritize visual novelty over financial clarity.

But financial seriousness must not become visual conservatism.

The product should feel unmistakably modern.

The final experience must combine:

```text
Consumer Banking Simplicity
        +
Corporate Banking Power
        +
Real-Estate Financial Context
        +
White-Label Adaptability
        +
Perceptual Color Discipline
        +
Atomic Design Consistency
        +
Excellent Motion
        +
Financial Trust
```

The objective is not to create the most decorative banking interface.

The objective is to create an Internet Banking experience that feels:

**calmer than traditional banking,  
more sophisticated than generic fintech,  
simpler than enterprise banking,  
and substantially more intentional than a backoffice.**

---

# 117. Absolute Rule

When there is a conflict between:

```text
generic visual convention
```

and:

```text
financial comprehension
```

choose financial comprehension.

When there is a conflict between:

```text
brand saturation
```

and:

```text
visual hierarchy
```

choose visual hierarchy while preserving brand identity.

When there is a conflict between:

```text
more information
```

and:

```text
clearer decision making
```

use progressive disclosure.

And when a design starts looking like an administrative dashboard:

**stop and redesign the composition instead of decorating the dashboard.**