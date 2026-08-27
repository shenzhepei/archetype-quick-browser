# CLI参考

| 命令 | 用途 |
| --- | --- |
| `archetype init` | 创建函数项目和发现文件骨架 |
| `archetype doctor` | 检查Node、Docker、Compose和Gateway |
| `archetype runtime up` | 启动自托管环境 |
| `archetype project create` | 创建项目 |
| `archetype origin add` | 注册HTTPS应用Origin |
| `archetype db add` | 加密绑定PostgreSQL/MySQL URL |
| `archetype dev` | 从源码运行Gateway、Function Host和Worker |
| `archetype deploy` | 打包、计算摘要并激活函数与Worker |
| `archetype logs` | 查看最近项目审计记录 |
| `archetype well-known generate` | 生成同源发现文件 |

管理自动化请求必须显式配置 `ARCHETYPE_ADMIN_TOKEN`，不存在默认Token；`project create` 支持 `--organization`。人工管理员应使用OIDC企业控制台。数据库绑定从 `ARCHETYPE_DATABASE_URL` 读取，避免把连接串写在命令参数中。
