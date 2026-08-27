# 企业控制面

Gateway在 `/console/` 提供管理员控制台。它是独立于C端浏览器的企业工作区。管理员通过OIDC Authorization Code + PKCE登录，生成的8小时会话只存在于同源HttpOnly Cookie中。OIDC state、PKCE verifier和nonce作为一次性事务持久化到PostgreSQL。

## 角色

| 角色 | 权限 |
| --- | --- |
| 所有者 / 管理员 | 管理成员、创建和配置项目、部署、读取审计日志 |
| 开发者 | 读取和配置项目、部署、读取审计日志 |
| 运维 | 读取和配置项目、读取审计日志，但不能部署 |
| 审计员 | 只读项目和审计日志 |

每个项目必须属于一个组织。服务端会在每次请求时根据可信OIDC Subject和当前组织成员关系重新授权；界面显示的角色不是授权依据。

## 管理员OIDC

生产环境在Gateway配置：

```dotenv
ARCHETYPE_PUBLIC_URL=https://runtime.example.com
ARCHETYPE_CONTROL_OIDC_ISSUER=https://identity.example.com
ARCHETYPE_CONTROL_OIDC_CLIENT_ID=archetype-control
ARCHETYPE_CONTROL_OIDC_CLIENT_SECRET=replace-me
ARCHETYPE_CONTROL_BOOTSTRAP_SUBJECTS=first-owner-subject
```

回调地址是 `https://runtime.example.com/v1/control/auth/callback`。`ARCHETYPE_CONTROL_DEV_LOGIN=true` 会显式启用本地 `development-admin` 身份，生产环境不得使用。

## 人工访问与自动化

控制台使用人工OIDC会话。CLI使用单独配置的 `ARCHETYPE_ADMIN_TOKEN` 执行部署自动化，无法获得控制台Cookie。Gateway源码不提供默认自动化Token。数据库URL是只写输入：Gateway对其进行信封加密，控制台和CLI都不能回读。
