# Dockerfile for Next.js Admin Panel
# Supports both development and production builds

# ============================================================================
# Base stage: Install dependencies
# ============================================================================
FROM node:20-alpine AS base

WORKDIR /app

# Install bun (if using bun instead of npm/yarn)
RUN npm install -g bun

COPY package.json bun.lock ./
RUN bun install --frozen-lockfile

# ============================================================================
# Development stage
# ============================================================================
FROM base AS development

ENV NODE_ENV=development

COPY . .

EXPOSE 3000

CMD ["bun", "run", "dev", "--port", "3000"]

# ============================================================================
# Builder stage: Build production app
# ============================================================================
FROM base AS builder

ENV NODE_ENV=production

COPY . .

RUN bun run build

# ============================================================================
# Production stage: Serve built app
# ============================================================================
FROM node:20-alpine AS production

WORKDIR /app

ENV NODE_ENV=production

# Copy built app from builder
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

EXPOSE 3000

CMD ["node", "server.js"]
