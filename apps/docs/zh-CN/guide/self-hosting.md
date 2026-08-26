# Docker自托管

## 环境要求

- Docker与Compose
- 完整开发栈建议4核CPU和8GB内存
- 随机的 `ARCHETYPE_MASTER_KEY`、管理员令牌和服务令牌

```bash
cp infra/docker/.env.example infra/docker/.env
docker-compose --env-file infra/docker/.env -f infra/docker/compose.yml up --build
```

开发栈在 `http://localhost:8787` 暴露Gateway。生产环境应启用Caddy配置，使用真实域名和HTTPS证书。

Compose项目包含Gateway、Function Host、Worker、平台PostgreSQL、示例PostgreSQL、示例MySQL和Caddy。只有Gateway/Caddy是公开应用边界；数据库和内部服务端口应留在Docker私有网络。

```bash
archetype project create --name "Shop"
archetype origin add --project PROJECT_ID --origin https://shop.example
ARCHETYPE_DATABASE_URL='postgres://...' archetype db add --project PROJECT_ID --dialect postgres
archetype deploy --project PROJECT_ID --entry src/functions/index.ts
archetype well-known generate --project PROJECT_ID --gateway https://runtime.example.com
```

不要把数据库URL写入前端环境变量或发现文件。`db add`只从 `ARCHETYPE_DATABASE_URL` 读取，并发送到经过认证的控制面进行信封加密。

## Secret管理

在仓库外生成 `ARCHETYPE_MASTER_KEY`、`ARCHETYPE_ADMIN_TOKEN` 和 `ARCHETYPE_SERVICE_TOKEN`。Gateway为每个数据库凭证生成随机数据密钥，用AES-256-GCM加密凭证，再用安装主密钥包裹数据密钥。主密钥必须单独备份，丢失后已保存凭证无法恢复。

再次运行 `db add` 可以轮换数据库密码。服务和管理员Token应在维护窗口轮换。主密钥轮换需要解密并重新包裹所有数据密钥，应作为显式迁移执行。

## 使用Caddy部署生产HTTPS

在 `infra/docker/Caddyfile` 中配置公开Runtime域名，把DNS指向主机，并只暴露80/443端口。Caddy终止TLS并代理Gateway。发现文件必须使用最终的 `https://` Gateway URL，Gateway项目中也必须注册网站的精确Origin。

## 日志与故障排查

先运行 `archetype doctor`，再检查 `docker-compose -f infra/docker/compose.yml ps` 和各服务日志。`archetype logs --project PROJECT_ID` 返回项目审计记录，但不会返回Secret值。常见问题包括Origin未注册、远程Gateway不是HTTPS、OIDC配置过期、部署摘要不一致或数据库不可用。

Gateway和内部服务都提供健康检查。重启Function Host或Worker是可恢复的：执行中的函数会失败，可用幂等键重试；已提交的Outbox记录和带租约任务会从PostgreSQL恢复。
