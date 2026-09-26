# Evidence Requirements

## Purpose

`nexus_evidence_requirements` defines a small, runtime-neutral contract-evidence
algebra.

It answers:

> Given evidence relevant to a predicate, what assessment state does that
> evidence provide, and how do multiple assessments compose?

It does **not** execute actions, select strategies, access `runtime_core`, or
decide which evidence should be retrieved.

## Assessment states

The six states are:

- `SC` — satisfied / confirmed
- `SP` — satisfied / partial
- `RC` — rejected / contradicted
- `RP` — rejected / partial
- `NP` — not proven
- `NN` — no evidence / not evaluated

`NN` represents absence of evidence. It is not the same thing as a successful
or failed observation.

## Evidence absence

`Predicate::assess` accepts `Option<&Evidence>` because the evaluator must
distinguish absence from an actual observation.

`evaluate(..., None)` always produces `NN` and does not invoke the predicate.

This prevents predicates from treating missing evidence as an observed result.

## ALL algebra

`ALL` combines requirements that must all hold.

`RC` is absorbing:

```text
RC ALL X = RC
X ALL RC = RC

RP is the next rejection state when no RC is present.

Among non-rejection states:

SC > SP > NP > NN

The lower state therefore determines the result of ALL.

The complete six-state table is tested in tests/algebra_tables.rs.

ANY algebra

ANY is not independently authored.

It is derived mechanically from ALL using duality:

ANY(a, b) = dual(ALL(dual(a), dual(b)))

The implementation and tests enforce this relationship.

Scope and identity

ActionIdentity, TargetId, and Scope are lightweight runtime-neutral
identifiers.

ActionIdentity stores an already-computed canonical action key. This crate
does not depend on AgentAction and does not compute runtime action identity.

Boundary

This crate is deliberately independent of runtime_core.

It is intended to become a reusable cognitive capability that can later be
adapted to existing runtime evidence.

It does not replace:

strategy validation,
action authorization,
completion contracts,
execution,
legacy runtime Evidence,
belief updates,
or human approval.

Those are separate responsibilities.
