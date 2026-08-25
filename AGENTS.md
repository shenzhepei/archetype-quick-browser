# Archetype Agent Guide

- PRD 放在 `docs/prd/`，详细设计放在 `docs/detailed-design/`。
- 版本规范使用同号 `NN-<name>.md` 并互相链接，同时更新两个目录的 README 索引。
- 版本外临时需求必须先创建同名 `docs/prd/feature-NN.md` 与 `docs/detailed-design/feature-NN.md`，逐项记录验收标准后再改代码。
- 不得提交只有用户可见代码变更、没有对应 PRD/详设的实现。
- 使用 Node.js 24+、pnpm、React、TypeScript、Vite 和 SCSS；运行时英文默认并完整支持简体中文。
