# Project Agent Rules

This file defines repository-specific inputs for agents. `$workflow-orchestrator`
owns workflow routing, phase behavior, document lifecycle, subagent rules, and
conflict handling.

## Persistent Memory & Document Schemas

Use repository-local documents when durable engineering memory is useful. Documents
in `docs/` are technical specifications and delivery records:

- **Living Architecture (`docs/architecture/`)**: Continuously updated reflection of system design.
- **Specifications (`docs/specs/spec-<0000>-<name>-YYYY-MM-DD.md`)**: Architectural design, goals, requirements, tradeoffs, and validation strategy.
- **Task Checklists (`docs/tasks/task-<0000>.md`)**: Ordered implementation tasks, assigned file targets, step-by-step notes, and validation steps.
- **Result Summaries (`docs/results/result-<0000>.md`)**: Factual completion record, validation outcomes, Residual Risks & Tech Debt, and Future Milestones. Synthesized and archived directly by the orchestrator upon passing review.
- **Decisions (`docs/decisions/decision-<0000>.md`)**: Architecture Decision Records (ADRs) tracking irreversible design choices.

Document Footprint Rules by Workflow Level:
- **Medium/Large & High Risk**: Require `docs/specs/`, `docs/tasks/`, and `docs/results/`. Both `docs/specs/` and `docs/tasks/` are produced together during Phase 1 (Planning); `docs/results/` is archived directly by the orchestrator after passing review.
- **Small**: Master specs (`docs/specs/`) are omitted to avoid doc bloat. `docs/tasks/` checklist is created only if multi-step tracking is needed; `docs/results/` optional.
- **Tiny**: Zero persistent documents in `docs/`. Direct implementation and validation.

## Workflow-Orchestrator Use

Invoke `$workflow-orchestrator` when a request is broad, architecture-affecting,
compatibility-sensitive, asks for subagents, includes an approved chat plan handoff,
or needs durable specs, tasks, results, or decisions.

Tiny localized changes may be handled directly when no durable document is
useful and no approved handoff requires the orchestrator.

When routing to `$workflow-orchestrator`, pass explicit document paths,
repository validation commands, allowed files or modules, intended change
boundaries, TDD expectations or exceptions, and stop conditions.

## Interactive Chat Plan Mode Bridge

The `Workflow-Orchestrator Execution Handoff` is an interactive chat routing
marker. It appears exclusively in conversational `<proposed_plan>` responses
within the chat interface to signal the orchestrator to initiate execution upon
user approval. It is an ephemeral session-control instruction for the chat runner,
distinct from the technical content stored in repository `docs/`.

### Workflow Level Consistency & Transition Gate
- The workflow level approved in the chat plan is the active operating mode.
- The orchestrator must not silently change the workflow level. If implementation inspection reveals the actual blast radius is smaller or larger than planned, explain the evidence and ask the user whether to adjust the level or maintain the approved pipeline.
- For Medium/Large and High Risk, the orchestrator executes the subagent pipeline:
  `plan-agent -> execution-agent -> review-agent`
  followed by direct result archival by the orchestrator.
- `plan-agent` takes the approved plan as input and produces both `docs/specs/spec-<0000>-<name>-YYYY-MM-DD.md` (architecture specification) and `docs/tasks/task-<0000>.md` (executable task checklist).
- Upon `review-agent` approval, the orchestrator synthesizes the delivery record and writes `docs/results/result-<0000>.md` directly (including Completed Work, Validation, Residual Risks, and Future Milestones).

Recommended chat handoff marker format (used inside chat responses only):

```text
Workflow-Orchestrator Execution Handoff:
- Use $workflow-orchestrator.
- Workflow level: Medium/Large.
- Mandatory sequence: plan-agent -> execution-agent -> review-agent (orchestrator directly archives docs/results/).
- Mandatory documents: docs/specs, docs/tasks, and docs/results.
- Phase 1 start: Invoke plan-agent with this plan as input to create docs/specs/ and docs/tasks/.
- Follow the plan's module boundaries and allowed-file constraints.
- Validation: run repository validation commands from AGENTS.md.
- Stop for major ambiguity, scope expansion, permission escalation, or validation failure requiring user choice.
```

For Medium/Large and High Risk workflows, the orchestrator automatically dispatches
subagents via `invoke_subagent` for each phase. The orchestrator acts as a quality gatekeeper:
it audits subagent drafts before publication, uses `send_message` to request revisions if drafts
have gaps, and publishes validated documents. If `review-agent` reports P1/Blocker defects or
test failures, the orchestrator loops back to `execution-agent` to fix the defects before re-reviewing.
Implementation is delegated to `execution-agent`; the orchestrator does not edit production code locally.
If subagents cannot be dispatched for any reason, the orchestrator stops and reports immediately (Fail-Fast).

## Document Conflicts & Obsoletion

Before implementation-bound plans for broad or high-risk work, check relevant
documents for material conflicts. Resolve or document conflicts in the current
spec, task, or result summary.

- **Living Docs vs Point-in-Time Ledgers**: `docs/architecture/` reflects current truth. Historical specs, tasks, and results (`docs/{specs,tasks,results}`) are point-in-time ledgers: preserve historical body text.
- **Full Supersession (Obsoletes)**: When an entire previous feature/spec is replaced, the new spec declares `## Superseded Documents`, and the old spec receives a top warning banner: `> [!WARNING]\n> **Status: Superseded** by [spec-XXXX](...)`.
- **Partial Amendment (Updates / Amends)**: When only specific sections, fields, or rules of an earlier spec become obsolete:
  - Keep old spec active with notice: `> [!NOTE]\n> **Status: Active (Partially Amended)**` and inline `> [!WARNING]\n> **Section Superseded**` callouts.
  - In the new spec, include `## Amended Documents & Scope` detailing affected vs unaffected sections, rationale, and migration strategy.
- **Decisions (`docs/decisions/`)**: Follow ADR lifecycle (`Proposed -> Accepted -> Superseded`). Update old ADR header to `Status: Superseded by [decision-YYYY](...)`.

## Validation

Default execution-session validation for this repository:

```bash
cargo test
```

For Tiny documentation-only changes, source validation is not required unless
the documentation affects executable behavior, commands, generated files, or
project configuration. If validation is unavailable or environmental, record
the command, reason, and residual risk.

## Coding Conventions

- Prefer explicit code.
- Avoid magic abstractions.
- Keep diffs small.
- Preserve existing architecture.
- Load only the minimum context required.
