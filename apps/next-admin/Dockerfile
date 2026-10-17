# Build with the repository root as context:
# docker build -f apps/next-admin/Dockerfile --target production .
FROM node:24-alpine AS base

WORKDIR /workspace
RUN apk add --no-cache git \
    && git config --global url."https://github.com/".insteadOf ssh://git@github.com/

# Local file dependencies must retain their repository-relative locations.
COPY packages/richtext/ packages/richtext/
COPY packages/rustok-ui-grid/ packages/rustok-ui-grid/
RUN npm ci --prefix packages/richtext \
    && npm run build --prefix packages/richtext
COPY apps/next-admin/ apps/next-admin/
WORKDIR /workspace/apps/next-admin
ENV HUSKY=0
RUN npm ci

FROM base AS development
ENV NODE_ENV=development
EXPOSE 3000
CMD ["npm", "run", "dev", "--", "--hostname", "0.0.0.0", "--port", "3000"]

FROM base AS builder
ENV NODE_ENV=production
RUN npm run build

FROM node:24-alpine AS production
WORKDIR /app
ENV NODE_ENV=production HOSTNAME=0.0.0.0 PORT=3000
COPY --from=builder /workspace/apps/next-admin/.next/standalone ./
COPY --from=builder /workspace/apps/next-admin/.next/static ./apps/next-admin/.next/static
COPY --from=builder /workspace/apps/next-admin/public ./apps/next-admin/public
COPY --from=builder /workspace/packages/richtext/dist ./packages/richtext/dist
EXPOSE 3000
CMD ["node", "apps/next-admin/server.js"]
