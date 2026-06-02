# Copilot AI Credits — billing & API research

Research for Headroom's Copilot data source after GitHub's **June 1 2026** move of
**all** Copilot plans to usage-based **AI Credits** billing (`1 credit = $0.01`, priced
on token consumption). Goal: read an **individual Pro / Pro+** user's credit consumption.

> Status legend: ✅ official docs · ⚠️ official but ambiguous/underspecified · 🟡 third-party / unverified · 🔬 needs a real account to confirm.
> Constraint we're designing to: **detect the billing regime at runtime; never hardcode quota numbers in app logic.**

---

## 1. Included-credit allocations per plan

| Plan | Seat price | Standard monthly credits | June 1 – Sep 1 2026 promo |
|------|-----------|--------------------------|---------------------------|
| Free | $0 | No credits — 50 agent/chat req + 2,000 completions/mo (not credit-metered) | — |
| Pro | $10/mo | ~1,000 base (+500 "flex" → 1,500 total per the individuals doc) ⚠️ | 🔬 unclear if individuals get a promo |
| Pro+ | $39/mo | ~3,900 base (+3,100 "flex" → 7,000 total per the individuals doc) ⚠️ | 🔬 unclear |
| Business | $19/user/mo | ~1,900 credits ($19) | ~3,000 credits ($30/seat) Jun–Aug 🟡 |
| Enterprise | $39/user/mo | ~3,900 credits ($39) | promo mentioned, amount unconfirmed 🔬 |

- Code completions + Next Edit suggestions remain **included/unlimited** on all paid plans and **do not** consume credits. ✅
- The "base + flex" split for individuals and the exact promo amounts are **not cleanly stated in one official page** — treat the numbers above as reference only and **confirm against a real account** (§5). This is *why* app logic must detect the regime, not hardcode caps.

Sources: [Plans for Copilot](https://docs.github.com/en/copilot/get-started/plans) ✅ · [Usage-based billing for individuals](https://docs.github.com/en/copilot/concepts/billing/usage-based-billing-for-individuals) ✅ · [Copilot moving to usage-based billing (blog)](https://github.blog/news-insights/company-news/github-copilot-is-moving-to-usage-based-billing/) ✅ · [Billing & plans changelog 2026-06-01](https://github.blog/changelog/2026-06-01-updates-to-github-copilot-billing-and-plans/) ✅ · promo seat amounts corroborated only by 🟡 third-party summaries — unverified.

---

## 2. API surfaces that expose AI Credit / token consumption

### 2a. Org usage — `GET /organizations/{org}/settings/billing/usage` ✅
- **Enhanced Billing Platform only** (org must be migrated). Returns line items per **product / SKU / model / unitType / pricePerUnit / quantity / gross/discount/netAmount**.
- **Auth: classic PAT only.** `manage_billing:copilot` or `read:org` scope. **Fine-grained PATs are NOT supported.** ✅
- Query params: `year`, `month`, `day` (no `hour`). Optional `cost_center`, `repository`, etc.
- Caller must be an **org admin**.

### 2b. Enterprise usage — `GET /enterprises/{enterprise}/settings/billing/usage` (+ `/usage/summary`) ✅
- Same Enhanced Billing schema; classic PAT, enterprise admin. Example from docs:
  ```bash
  gh api -H "X-GitHub-Api-Version: 2022-11-28" \
    /enterprises/ENTERPRISE/settings/billing/usage/summary
  ```

### 2c. **Individual / user usage — `GET /users/{username}/settings/billing/usage`** ✅ (this is the key one)
- **A documented user-level endpoint exists** (plus `/usage/summary` and a legacy `/premium_request/usage`). Same Enhanced Billing line-item schema:
  ```jsonc
  { "usageItems": [ {
      "date": "2026-06-02", "product": "copilot", "sku": "...",
      "quantity": 123, "unitType": "...", "pricePerUnit": 0.01,
      "grossAmount": ..., "discountAmount": ..., "netAmount": ...,
      "repositoryName": "..." } ] }
  ```
- **Auth: ⚠️ documented family-wide as classic PAT, no fine-grained.** Per-endpoint token table for the *user* path didn't render cleanly — **assume classic PAT, scope likely `manage_billing:copilot`, querying your OWN username only** → 🔬 confirm.
- **Returns consumption, not allowance.** No `remaining`/`entitlement` field — you get net credits/$ used; "remaining" must be derived (allowance − net). This differs from today's `copilot_internal/user`, which gives a live remaining snapshot.

### 2d. Real-time quota snapshot — `GET https://api.github.com/copilot_internal/user` (undocumented) ⚠️🟡
- What Headroom uses today. Returns `quota_snapshots.{premium_interactions,chat,completions}` with `entitlement / quota_remaining / unlimited / overage_permitted / percent_remaining` and `copilot_plan`, `quota_reset_date`. Near-real-time.
- **Undocumented / unstable** — it's what the github.com billing overview calls under the hood. **High risk it changed shape in the June 1 AI-Credits migration** (the quota_id may no longer be `premium_interactions`). Bug we're chasing (blank Business card) is likely this. 🔬
- Note: live payload uses `quota_reset_date` — our code currently reads `quota_reset_date_utc` (bug, separate fix).

### 2e. GitHub CLI
- `gh api /users/{me}/settings/billing/usage` works for the documented endpoints (inherits `gh auth` token + scopes). ✅ No dedicated `gh` billing command for Copilot credits. `gh` is a fallback/debug path, not something to shell out to from a Tauri app.

### Rate limits & freshness ⚠️🔬
- Neither rate limits nor freshness are stated on the billing-usage docs. Billing/usage data is widely understood to lag (≈ up to ~daily), **not** real-time — **confirm latency against a real account.** `copilot_internal/user` is the only near-real-time source.

Sources: [REST: Billing usage](https://docs.github.com/en/rest/billing/usage) ✅ · [Automate usage reporting](https://docs.github.com/en/enterprise-cloud@latest/billing/tutorials/automate-usage-reporting) ✅ · [Usage-based billing for orgs & enterprises](https://docs.github.com/en/copilot/concepts/billing/usage-based-billing-for-organizations-and-enterprises) ✅ · [Budgets for usage-based billing](https://docs.github.com/en/copilot/concepts/billing/budgets-for-usage-based-billing) ✅ · `copilot_internal/user` shape from [zed #44499](https://github.com/zed-industries/zed/discussions/44499) 🟡 & [vscode-copilot-insights](https://github.com/kasuken/vscode-copilot-insights) 🟡.

---

## 3. Per-model token rates (new credits regime) & legacy multipliers

### New AI-Credits rates — per **1M tokens**, USD (× by token count, ÷ $0.01 → credits) ✅
| Model | Input | Cached in | Output | (Cache write) |
|-------|------:|----------:|-------:|--------------:|
| GPT-4.1 | $2.00 | $0.50 | $8.00 | — |
| GPT-5 mini | $0.25 | $0.025 | $2.00 | — |
| GPT-5.2 | $1.75 | $0.175 | $14.00 | — |
| GPT-5.4 | $2.50 | $0.25 | $15.00 | — |
| GPT-5.5 | $5.00 | $0.50 | $30.00 | — |
| Claude Haiku 4.5 | $1.00 | $0.10 | $5.00 | $1.25 |
| Claude Sonnet 4.x | $3.00 | $0.30 | $15.00 | $3.75 |
| Claude Opus 4.x | $5.00 | $0.50 | $25.00 | $6.25 |
| Gemini 2.5 Pro | $1.25 | $0.125 | $10.00 | — |
| Gemini 3.5 Flash | $1.50 | $0.15 | $9.00 | — |

### Legacy "model multipliers" (grandfathered **annual** Pro/Pro+ on request-based billing) ✅
Applies only to Pro/Pro+ annual subscribers who stayed on legacy after June 1 2026. 1 premium request × multiplier deducted from allowance (Pro 300/mo, Pro+ 1,500/mo; extra $0.04/req). Auto model selection = 10% discount.

| Model | × | Model | × |
|-------|--:|-------|--:|
| GPT-4o / 4o mini | 0.33 | Claude Haiku 4.5 | 0.33 |
| GPT-4.1 | 1 | Claude Sonnet 4.5 | 6 |
| GPT-5 mini / Raptor mini | 0.33 | Claude Sonnet 4.6 | 9 |
| GPT-5.1 / 5.1-Codex / 5.1-Codex-Max | 3 | Claude Opus 4.5 | 15 |
| GPT-5.1-Codex-Mini | 0.33 | Claude Opus 4.6 / 4.7 / 4.8 | 27 |
| GPT-5.2 / 5.2-Codex | 3 | Gemini 2.5 Pro | 1 |
| GPT-5.3-Codex / 5.4 / 5.4 mini | 6 | Gemini 3 Flash | 0.33 |
| GPT-5.5 | 57 | Gemini 3 Pro / 3.1 Pro | 6 |
| Copilot Code Review | 13 | Gemini 3.5 Flash | 14 |

Sources: [Models and pricing](https://docs.github.com/en/copilot/reference/copilot-billing/models-and-pricing) ✅ · [Model multipliers for annual plans (legacy)](https://docs.github.com/en/copilot/reference/copilot-billing/model-multipliers-for-annual-plans) ✅ · [Requests in Copilot (legacy)](https://docs.github.com/en/copilot/concepts/billing/copilot-requests) ✅.

> Headroom should **not** compute credits from token rates itself — read the net amount the API already converts. The rate tables are for display/explanation only.

---

## 4. Recommended data-access strategy (individual Pro/Pro+)

**Dual-source, regime-detecting.**

1. **Live gauge (primary, keep):** `copilot_internal/user` for the near-real-time remaining snapshot, as today — but **detect the regime** from the payload instead of assuming `premium_interactions`:
   - Match the headline quota against ids we've **actually observed** (`premium_interactions` legacy counter, Free `chat`/`completions`), each tagged with its regime. **Do not guess** the migrated AI-Credits id and **do not** promote an arbitrary bounded entry — an unconfirmed/unrecognized shape surfaces as **Unknown** (with its raw ids) plus a Copy-diagnostics action, never a number under a guessed label. Add the real credits id once a live payload confirms it.
   - If a recognized entry is `unlimited`, render **"Unlimited"**, never blank.
   - Reset date: defensive fallback chain (`quota_reset_date_utc` → `quota_reset_date` → next month), not a blind field swap — the #44499 sample carried both fields.
2. **Authoritative consumption (optional, opt-in):** `GET /users/{me}/settings/billing/usage` filtered to `product === "copilot"`, summing `netAmount` → credits used this cycle. Needs a **classic PAT** with billing scope — **not** the current OAuth device-flow `read:user` token and **not** fine-grained. So gate this behind an "Advanced → paste a classic billing PAT" path; don't force it on the common user.
3. **Polling:** live snapshot at the normal cadence; billing-usage endpoint **infrequently** (on open + ~hourly) since it's ~daily-fresh and rate-sensitive.

**Shared-pool modeling (Business/Enterprise seats):** an individual on an org seat draws from a shared org pool and has **no enforced per-user cap unless an admin set a user budget**. So:
- `hasUserBudget === false` → there is no meaningful "% remaining"; show **consumption** ("X credits / $Y used this cycle") + an "unlimited within org budget" note, not a progress bar.
- `hasUserBudget === true` → `remaining = budget − used`, show the bar.
- Detect via `overage_permitted` / a budget field in `copilot_internal/user`, or fall back to consumption-only when unknown.

### Normalized usage model (discriminated union on billing mode)

```ts
export type CopilotBillingMode = 'credits' | 'legacy_requests';
export type CopilotPoolScope = 'individual' | 'organization' | 'enterprise';

interface CopilotUsageBase {
  plan: string;            // raw copilot_plan from the API — never hardcoded
  planLabel: string;       // display ("Pro+", "Business", …)
  poolScope: CopilotPoolScope;
  hasUserBudget: boolean;  // true ⇒ an enforced per-user cap exists ⇒ "remaining" is meaningful
  resetsAt: string;        // ISO 8601, start of next cycle
  fetchedAt: string;       // ISO 8601
}

/** New usage-based regime (default after 2026-06-01). 1 credit = $0.01. */
export interface CreditsUsage extends CopilotUsageBase {
  mode: 'credits';
  includedCredits: number | null;   // monthly allowance; null = unknown / shared pool w/o cap
  usedCredits: number;               // net consumed this cycle
  remainingCredits: number | null;   // includedCredits − usedCredits, or null when no per-user cap
  overage: { permitted: boolean; budgetCredits: number | null; usedCredits: number };
  byModel?: Array<{ model: string; netCredits: number; netAmountUsd: number }>; // when usage API present
}

/** Grandfathered annual Pro/Pro+ still on request-based billing. */
export interface LegacyRequestUsage extends CopilotUsageBase {
  mode: 'legacy_requests';
  includedRequests: number | null;  // e.g. 300 / 1500
  usedRequests: number;
  remainingRequests: number | null;
  overage: { permitted: boolean; pricePerRequestUsd: number; usedRequests: number };
}

export type CopilotUsage = CreditsUsage | LegacyRequestUsage;
```

**Regime detection (no hardcoded caps):** classify as `legacy_requests` when the source still exposes a premium-request quota / the `premium_request` SKU; otherwise `credits`. Derive `includedCredits`/`includedRequests` from the API's own `entitlement`/budget fields, leaving `null` when the source doesn't provide them (shared pool) and rendering consumption-only.

---

## CONFIRMED — real migrated **Business** payload (2026-06-02)

A real Business seat (`copilot_plan: "business"`, `access_type_sku: "copilot_for_business_seat_quota"`) returned:

- **`token_based_billing: true`** at top level (and per snapshot) — the regime discriminator. Headroom now keys on this, not a guessed id.
- `quota_snapshots` = `chat`, `completions`, `premium_interactions` — **no `ai_credits`/`credits` id exists**. `premium_interactions` is the metered one (**`has_quota: true`**; chat/completions `has_quota: false`).
- On this seat `premium_interactions` is **`unlimited: true, entitlement: 0, remaining: 0, overage_permitted: true`** — i.e. **`copilot_internal/user` carries NO per-seat credit balance** for an org seat with no user budget. New fields seen: `has_quota`, `token_based_billing`, `quota_reset_at`, `timestamp_utc`.
- Both `quota_reset_date` ("2026-07-01") **and** `quota_reset_date_utc` present.

**Implication:** for a pooled Business/Enterprise seat, the live credit balance is **not** in this endpoint — Headroom shows "org-managed (pooled) — no individual quota" (no bar/count), and actual consumption must come from the billing usage API (§2).

## CONFIRMED — real migrated **Free individual** payload (2026-06-02)

A real Free seat (`access_type_sku: "free_limited_copilot"`, `copilot_plan: "individual"`, empty `organization_list`) returned **`token_based_billing: true`** but with **bounded request quotas**:

- `chat`: `unlimited:false, entitlement:200, remaining:181 (90.7%), has_quota:false`
- `completions`: `unlimited:false, entitlement:2000, has_quota:false`
- `premium_interactions`: `unlimited:false, entitlement:0, has_quota:false` (no credits on Free)

**Two corrections this forced:**
1. **`token_based_billing: true` marks _migration_, not "credits"/"pooled".** It's `true` on Free, whose meaningful quotas are plain request caps.
2. **chat/completions are NOT always unlimited** — on Free they're real caps and must be surfaced.

So the classifier now: surfaces the first **capped** quota in order `premium_interactions → chat → completions` (Free → "Chat 19/200"); treats `premium_interactions`-under-token-based as AI Credits; reserves **pooled** for a `has_quota && unlimited` holder with nothing bounded (Business); else **unknown**. This handles all three real payloads.

## 5. Open questions — still need a real **paid Pro/Pro+** account

1. **Does a migrated _paid_ Pro/Pro+ expose its credit balance, and how?** The user's individual account is **Free** (above), not paid. Hypothesis from the Free shape: a paid seat's `premium_interactions` is bounded with `entitlement` = included credits (Pro 1000 / Pro+ 3900) → Headroom renders the AI-Credits row automatically (coded + synthetically tested). **Risk:** if a paid seat instead reports `premium_interactions` as `unlimited:true / entitlement:0` (like Business) with credits only at account level, it would fall to pooled and hide the balance. **Capture a paid Pro+ raw payload to settle this.**
2. **User billing endpoint auth:** does `GET /users/{me}/settings/billing/usage` work with your **own** classic PAT, and which exact scope (`manage_billing:copilot`? a new `Plan` scope?)? Confirm fine-grained really fails and you can only query your own username.
3. **Freshness:** how stale is the user usage endpoint — minutes, or up to a day? (Decides whether it can drive the live gauge or only a daily "consumed" figure.)
4. **Allocations:** confirm Pro = 1,000 (+500 flex?) and Pro+ = 3,900 (+3,100 flex?) — is "flex" permanent or the Jun–Aug promo? Do individuals get any promo at all?
5. **`gh api /users/{me}/settings/billing/usage`** — does it return Copilot line items with your `gh` token's scopes?
6. **Legacy vs credits surfacing:** if you're on a grandfathered annual plan, do you appear via `/premium_request/usage` (requests) or `/usage` (credits)? Lets us validate the regime-detection branch.
7. **Reset boundary:** does the cycle reset on the 1st UTC, or on the seat's billing anniversary? Affects `resetsAt`.

---

_Compiled 2026-06-02. Official sources (docs.github.com, github.blog) marked ✅; third-party summaries 🟡 are unverified and used only where official pages were silent (promo seat amounts)._
