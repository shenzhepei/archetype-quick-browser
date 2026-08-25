# Quick Browser Agent Guide

## 文档规范

- 产品需求文档统一放在 `docs/prd/`。
- 详细设计文档统一放在英文目录 `docs/detailed-design/`；禁止新建或恢复 `docs/详设/`。
- PRD 与详细设计使用相同的两位规范号建立一一对应关系，例如 `01`、`02`。
- 文件名必须以规范号和连字符开头：`NN-<name>.md`，例如 `02-Archetype-扩展系统-PRD.md` 和 `02-Archetype-扩展系统详设.md`。
- 新增文档时，从两个目录现有规范号的最大值递增；不得复用、跳号或为同一主题分配不同编号。
- 每份 PRD 必须链接到同号详细设计，每份详细设计必须链接到同号 PRD。
- 新增、重命名或删除文档后，必须同步更新两个目录的 `README.md` 索引和仓库内全部相对链接。
- 文档正文可以使用中文；目录名及规范号格式必须遵循以上约束。

## 文档技能

创建、拆分、重命名或检查 PRD/详细设计时，使用 `.agent/skills/docs-conventions/SKILL.md`。

## Feature 补充文档

- 版本规范之外的补充需求、兼容性修复和跨版本功能使用 `.agent/skills/feature-docs/SKILL.md`。
- 所有不属于当前版本 PRD 的临时或追加需求都必须建立 Feature PRD/详设；通常先写文档再改代码，紧急修复最迟在同一任务完成前补齐。
- 不得提交或发布只有代码、没有 Feature PRD/详设的用户可见行为变更。
- Feature PRD 放在 `docs/prd/feature-NN.md`，同名详设放在 `docs/detailed-design/feature-NN.md`。
- `feature-NN.md` 是上述 `NN-<name>.md` 版本规范命名规则的唯一例外；Feature 序列独立连续编号。
- 每对 Feature 文档必须互链，并同步更新两个目录的 `README.md` Feature 索引。
