FROM node:26.6.0-bookworm-slim AS build

ENV COREPACK_HOME=/corepack
ENV PATH=/corepack:$PATH
RUN corepack enable && corepack prepare pnpm@10.33.2 --activate
WORKDIR /app

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.json tsconfig.base.json vitest.config.ts ./
COPY services/gateway/package.json ./services/gateway/package.json
COPY services/function-host/package.json ./services/function-host/package.json
COPY services/worker/package.json ./services/worker/package.json
COPY packages/protocol/package.json ./packages/protocol/package.json
COPY packages/function-sdk/package.json ./packages/function-sdk/package.json
COPY packages/db-adapters/package.json ./packages/db-adapters/package.json
RUN pnpm install --frozen-lockfile \
  --filter @archetype/gateway... \
  --filter @archetype/function-host... \
  --filter @archetype/worker...
COPY services ./services
COPY packages ./packages
RUN pnpm exec tsc -b --force \
  services/gateway \
  services/function-host \
  services/worker

FROM node:26.6.0-bookworm-slim AS runtime
ENV NODE_ENV=production
WORKDIR /app
COPY --from=build /app /app
CMD ["sh", "-c", "node $START_FILE"]
