---
name: docs-conventions
description: Maintain Quick Browser PRD and detailed-design documents, including creating, splitting, renaming, numbering, pairing, indexing, or validating files under docs/prd and docs/detailed-design. Use whenever documentation structure or PRD/design correspondence changes.
---

# Documentation Conventions

This skill owns numbered version specifications. Every temporary or supplemental requirement outside an active numbered PRD must use the repository-local `feature-docs` skill and receive a paired `feature-NN.md` PRD and detailed design.

## Required layout

- Store PRDs in `docs/prd/`.
- Store detailed designs in the English-named directory `docs/detailed-design/`.
- Never create or restore `docs/详设/`.

## Numbering and pairing

1. Assign every topic a two-digit specification number such as `01` or `02`.
2. Use the same number for that topic's PRD and detailed design.
3. Start each filename with `NN-`, for example:
   - `docs/prd/02-Archetype-扩展系统-PRD.md`
   - `docs/detailed-design/02-Archetype-扩展系统详设.md`
4. Determine a new number by scanning both directories and incrementing the highest existing number.
5. Do not reuse or skip a number, and do not give paired documents different numbers.

These filename rules apply to numbered version specifications. `feature-NN.md` is the deliberate exception governed by `feature-docs`.

## Document metadata and links

- Include `规范号` in each document's metadata table.
- Link every PRD to its same-number detailed design.
- Link every detailed design to its same-number PRD.
- Update `docs/prd/README.md` and `docs/detailed-design/README.md` after any document change.
- Search the whole repository for stale paths and names after moving or renaming files.

## Validation

Before finishing, verify:

1. No `docs/详设/` directory or stale `详设/` link remains.
2. Every numbered PRD has one same-number detailed design and vice versa.
3. All relative Markdown links resolve to existing files.
4. Both README indexes list every numbered pair.
