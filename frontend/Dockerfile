# syntax=docker/dockerfile:1

FROM node:22-slim AS builder
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .

# Astro inlines PUBLIC_* env vars into the static bundle at build time,
# not read at container runtime — so the backend URL must be known here.
ARG PUBLIC_API_BASE=http://localhost:3001
ENV PUBLIC_API_BASE=$PUBLIC_API_BASE
RUN npm run build

# Static output (no SSR/server endpoints in this app) — plain nginx serves
# it directly, no Node runtime needed in the final image.
FROM nginx:1.27-alpine AS runtime
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
