# Deployment Guide

> **Deploy the AI Gateway to production in Docker, Kubernetes, or bare metal.**

---

## Table of Contents

1. [Docker Compose Deployment](#1-docker-compose-deployment)
2. [Kubernetes Deployment](#2-kubernetes-deployment)
3. [Environment Variables](#3-environment-variable-reference)
4. [SSL/TLS Configuration](#4-ssltls-configuration)
5. [Health Checks & Monitoring](#5-health-checks--monitoring)
6. [Backup & Recovery](#6-backup--recovery)
7. [Rollback Procedure](#7-rollback-procedure)

---

## 1. Docker Compose Deployment

### 1.1 Production Compose File

Create `docker-compose.prod.yml`:

```yaml
version: "3.9"

services:
  postgres:
    image: postgres:16-alpine
    container_name: gateway-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: ${POSTGRES_USER:-gateway}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: ${POSTGRES_DB:-gateway}
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U $$POSTGRES_USER -d $$POSTGRES_DB"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - gateway

  redis:
    image: redis:7.2-alpine
    container_name: gateway-redis
    restart: unless-stopped
    command: redis-server --appendonly yes --maxmemory-policy allkeys-lru
    volumes:
      - redis_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 5
    networks:
      - gateway

  gateway:
    image: ghcr.io/ai-gateway/gateway:${VERSION:-latest}
    container_name: gateway-api
    restart: unless-stopped
    environment:
      DATABASE_URL: postgres://${POSTGRES_USER:-gateway}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB:-gateway}
      REDIS_URL: redis://redis:6379
      RUST_LOG: ${RUST_LOG:-info}
      RUST_BACKTRACE: ${RUST_BACKTRACE:-0}
      APP_ENV: production
    ports:
      - "8080:8080"
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    networks:
      - gateway
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 15s
      timeout: 5s
      retries: 3
      start_period: 30s

volumes:
  postgres_data:
  redis_data:

networks:
  gateway:
    driver: bridge
```

### 1.2 Deploy

```bash
# Set required secrets
export POSTGRES_PASSWORD=$(openssl rand -hex 32)

# Pull and start
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d

# Verify
curl http://localhost:8080/health
curl http://localhost:8080/ready
```

### 1.3 Build Custom Image

```bash
# Multi-stage build (backend + frontend)
docker build -f docker/Dockerfile.backend -t gateway:latest .

# Or use the dev Dockerfile as a starting point
cat docker/Dockerfile.backend.dev
```

### 1.4 SOLO Mode (No PostgreSQL/Redis)

For personal/local use without multi-tenancy:

```bash
# Build the solo binary
cargo build --release --bin gateway-solo

# Run with SQLite (auto-creates gateway-solo.db)
./target/release/gateway-solo

# Configure interactively
./target/release/gateway-solo config
```

See [CURRENT_STATE.md](CURRENT_STATE.md) for SOLO mode features.

---

## 2. Kubernetes Deployment

### 2.1 Helm Chart Structure

```
helm/ai-gateway/
├── Chart.yaml
├── values.yaml
├── values-production.yaml
└── templates/
    ├── namespace.yaml
    ├── configmap.yaml
    ├── secret.yaml
    ├── postgres-statefulset.yaml
    ├── redis-deployment.yaml
    ├── gateway-deployment.yaml
    ├── gateway-service.yaml
    ├── gateway-ingress.yaml
    └── hpa.yaml
```

### 2.2 Minimal Helm Values

```yaml
# values.yaml
replicaCount: 2

image:
  repository: ghcr.io/ai-gateway/gateway
  tag: "v0.1.0"
  pullPolicy: IfNotPresent

service:
  type: ClusterIP
  port: 8080

ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
  hosts:
    - host: gateway.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: gateway-tls
      hosts:
        - gateway.example.com

resources:
  requests:
    memory: "256Mi"
    cpu: "250m"
  limits:
    memory: "512Mi"
    cpu: "1000m"

postgres:
  enabled: true
  image: postgres:16-alpine
  storage: 10Gi
  user: gateway
  password: ""
  database: gateway

redis:
  enabled: true
  image: redis:7.2-alpine
  storage: 5Gi

autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
```

### 2.3 Deploy with Helm

```bash
# Add namespace
kubectl create namespace ai-gateway

# Install with Helm
helm upgrade --install ai-gateway ./helm/ai-gateway \
  --namespace ai-gateway \
  --values values-production.yaml \
  --set postgres.password=$(openssl rand -hex 32)

# Verify
kubectl get pods -n ai-gateway
kubectl logs -n ai-gateway deployment/gateway
```

### 2.4 Kubernetes Resources

```yaml
# gateway-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: gateway
  namespace: ai-gateway
spec:
  replicas: 2
  selector:
    matchLabels:
      app: gateway
  template:
    metadata:
      labels:
        app: gateway
    spec:
      containers:
        - name: gateway
          image: ghcr.io/ai-gateway/gateway:v0.1.0
          ports:
            - containerPort: 8080
          env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: gateway-secrets
                  key: database-url
            - name: REDIS_URL
              valueFrom:
                secretKeyRef:
                  name: gateway-secrets
                  key: redis-url
            - name: RUST_LOG
              value: "info"
          resources:
            requests:
              memory: "256Mi"
              cpu: "250m"
            limits:
              memory: "512Mi"
              cpu: "1000m"
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 15
          readinessProbe:
            httpGet:
              path: /ready
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: gateway
  namespace: ai-gateway
spec:
  selector:
    app: gateway
  ports:
    - port: 80
      targetPort: 8080
```

---

## 3. Environment Variable Reference

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | `postgres://user:pass@localhost:5432/gateway` |
| `REDIS_URL` | Redis connection string | `redis://localhost:6379` |

### Optional Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log verbosity (`trace`, `debug`, `info`, `warn`, `error`) |
| `RUST_BACKTRACE` | `0` | Enable backtraces (`1`, `full`) |
| `APP_ENV` | `production` | `development` skips static file serving, enables hot reload |
| `PORT` | `8080` | HTTP server port |
| `JWT_SECRET` | *(generated)* | RS256 private key for JWT signing |
| `API_KEY_PREFIX` | `sk_gw_` | Prefix for generated API keys |

### SOLO Mode Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SOLO_DB_PATH` | `gateway-solo.db` | SQLite database file path |
| `SOLO_CONFIG_PATH` | `gateway-solo.toml` | TOML config file path |

---

## 4. SSL/TLS Configuration

### 4.1 With Nginx Reverse Proxy (Recommended)

```nginx
# /etc/nginx/sites-available/gateway
server {
    listen 80;
    server_name gateway.example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name gateway.example.com;

    ssl_certificate /etc/letsencrypt/live/gateway.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/gateway.example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256';
    ssl_prefer_server_ciphers on;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;

    # Body size limit (match gateway's 10MB)
    client_max_body_size 10M;

    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # SSE streaming support
        proxy_buffering off;
        proxy_cache off;
    }
}
```

### 4.2 Let's Encrypt with Certbot

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Obtain certificate
sudo certbot --nginx -d gateway.example.com

# Auto-renewal is enabled by default
sudo certbot renew --dry-run
```

### 4.3 With Traefik (Docker Compose)

```yaml
# Add to docker-compose.prod.yml
  traefik:
    image: traefik:v3.0
    command:
      - --providers.docker=true
      - --entrypoints.web.address=:80
      - --entrypoints.websecure.address=:443
      - --certificatesresolvers.letsencrypt.acme.tlschallenge=true
      - --certificatesresolvers.letsencrypt.acme.email=admin@example.com
      - --certificatesresolvers.letsencrypt.acme.storage=/letsencrypt/acme.json
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - letsencrypt:/letsencrypt
    networks:
      - gateway

# Add labels to gateway service:
#    labels:
#      - traefik.enable=true
#      - traefik.http.routers.gateway.rule=Host(`gateway.example.com`)
#      - traefik.http.routers.gateway.tls.certresolver=letsencrypt
```

---

## 5. Health Checks & Monitoring

### 5.1 Endpoints

| Endpoint | Auth | Description |
|----------|------|-------------|
| `GET /health` | No | Liveness probe — returns `{"status":"healthy"}` |
| `GET /ready` | No | Readiness probe — checks DB and Redis connectivity |
| `GET /metrics` | No | Prometheus metrics (`metrics-exporter-prometheus`) |

### 5.2 Prometheus Scraping

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'ai-gateway'
    static_configs:
      - targets: ['gateway:8080']
    metrics_path: /metrics
    scrape_interval: 15s
```

### 5.3 Key Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `gateway_requests_total` | Counter | Total requests by status code |
| `gateway_request_duration_seconds` | Histogram | Request latency |
| `gateway_provider_latency_seconds` | Histogram | Provider API latency |
| `gateway_cache_hits_total` | Counter | Cache hit count |
| `gateway_quota_checks_total` | Counter | Quota enforcement count |

---

## 6. Backup & Recovery

### 6.1 PostgreSQL Backup

```bash
# Automated daily backup via cron
0 2 * * * pg_dump -h localhost -U gateway gateway | gzip > /backups/gateway-$(date +\%Y\%m\%d).sql.gz

# Restore
zcat /backups/gateway-20250601.sql.gz | psql -h localhost -U gateway gateway
```

### 6.2 Redis Backup

```bash
# Trigger RDB save
redis-cli BGSAVE

# Copy dump.rdb
cp /data/dump.rdb /backups/redis-$(date +%Y%m%d).rdb
```

---

## 7. Rollback Procedure

```bash
# 1. Identify last known good version
export LAST_GOOD_VERSION="v0.1.0"

# 2. Scale down current deployment
docker compose -f docker-compose.prod.yml stop gateway

# 3. Restore database (if needed)
# zcat /backups/gateway-YYYYMMDD.sql.gz | psql ...

# 4. Deploy previous version
docker compose -f docker-compose.prod.yml pull gateway:$LAST_GOOD_VERSION
docker compose -f docker-compose.prod.yml up -d gateway

# 5. Verify health
curl -f http://localhost:8080/health
curl -f http://localhost:8080/ready
```

For Kubernetes:

```bash
# Rollback to previous revision
helm rollback ai-gateway 0 -n ai-gateway

# Or with kubectl
kubectl rollout undo deployment/gateway -n ai-gateway
```

---

## Resource Requirements

| Component | CPU | Memory | Storage | Notes |
|-----------|-----|--------|---------|-------|
| Gateway (TEAM) | 0.5–1 core | 256–512 MB | None (stateless) | Scale horizontally |
| PostgreSQL | 0.5–1 core | 512 MB–1 GB | 10 GB+ | SSD recommended |
| Redis | 0.25 core | 256 MB | 5 GB | Optional persistence |
| **Total (single node)** | **1.5–2 cores** | **1–2 GB** | **15 GB** | Handles 1000+ req/min |

---

## Next Steps

- **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** — Common deployment issues
- **[OBSERVABILITY.md](OBSERVABILITY.md)** — Full monitoring setup
- **[RELEASE_CHECKLIST.md](../RELEASE_CHECKLIST.md)** — Production release process
