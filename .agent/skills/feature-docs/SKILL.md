---
name: feature-docs
description: Create and maintain mandatory repository-local supplemental feature PRDs and detailed designs named feature-NN.md for every Quick Browser requirement delivered outside the numbered version specifications. Use for any temporary or ad hoc request, compatibility fix, UI refinement, behavior change, or cross-version feature before implementation or, for an urgent fix, before the task is completed.
---

# Quick Browser Feature Documentation

## Purpose

Use the feature track for user-requested work that supplements, fixes, or crosses the numbered `NN-Archetype-*` version specifications. Feature documents preserve requirement history without renumbering or rewriting V3-V7 release contracts.

## Mandatory requirement capture

- Every temporary or ad hoc product requirement outside an active numbered PRD MUST have a paired feature PRD and detailed design.
- Create or update the feature pair before changing implementation code when the requirement is known in advance.
- For an urgent diagnostic or hotfix where the root cause must be established first, create or backfill the feature pair in the same task before declaring completion.
- Never finish, commit, or publish an out-of-version behavior change with code only and no feature documentation.
- Group one coherent user request or tightly related correction set into one feature. Use the next feature ID for an unrelated requirement instead of appending indefinitely to an old feature.
- Small implementation details that do not change observable behavior may stay in the owning detailed design; user-visible behavior, compatibility, persistence, security policy, or workflow changes always require feature capture.

## Required paths

Every feature has exactly two same-named files:

- PRD: `docs/prd/feature-NN.md`
- Detailed design: `docs/detailed-design/feature-NN.md`

`NN` is two digits starting at `01`. Determine the next ID by scanning both directories, then increment the highest feature number without reuse or gaps.

## Pairing rules

1. Both files use the same `feature-NN` ID.
2. The PRD links to `../detailed-design/feature-NN.md`.
3. The detailed design links to `../prd/feature-NN.md`.
4. Add the pair to the Feature table in both directory README indexes.
5. Do not rename or consume the numbered version specification sequence.
6. If a later version absorbs the feature, preserve the feature files and add reciprocal traceability links.

## PRD content

Include:

- Feature ID, status, source, related implementation or version, and detailed-design link.
- Background and user problem.
- Individually traceable requirements such as `F01-01`.
- Observable acceptance criteria for every requirement.
- Security, compatibility, persistence, and localization constraints where relevant.
- Explicit non-goals and links to follow-up features.

Do not reduce a multi-part user request to a title and prose summary. Preserve each requested behavior as a separate row.

When requirements arrive incrementally, update the existing feature only while they remain part of the same user goal. Record newly discovered acceptance constraints and non-goals before implementation is considered complete.

## Detailed-design content

Include:

- Feature ID, status, PRD link, and owning modules.
- Existing behavior and root cause when the feature is corrective.
- UI/component placement, state transitions, persistence and data flow.
- Network and security policy, including limits and intentional rejection paths.
- Failure/fallback behavior.
- Tests and implementation evidence.
- Known boundaries that remain unsupported.

Document the implementation that actually exists. Mark planned, implemented, validated, and completed states honestly.

## Index format

Keep numbered version specifications and supplemental features in separate tables. Use `Feature ID`, `PRD`, `对应详设`, `状态`, and a concise scope description.

## Validation

Before finishing:

1. Confirm every `docs/prd/feature-NN.md` has one same-named detailed design and vice versa.
2. Confirm both README indexes list every feature pair.
3. Confirm all relative Markdown links resolve.
4. Confirm there is no `docs/详设/` directory or stale link.
5. Search requirements and implementation references for stale feature IDs.
6. Run `git diff --check`.
7. Compare the implementation diff and conversation requirements against feature rows; no user-visible out-of-version change may remain undocumented.
