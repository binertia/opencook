# Release Checklist

> **Step-by-step process for creating a new AI Gateway release.**

---

## Pre-Release

### 1. Version Bump

- [ ] Update `version` in `Cargo.toml` workspace package
- [ ] Update version in all crate `Cargo.toml` files if not using workspace inheritance
- [ ] Update `CHANGELOG.md` — move `Unreleased` items to the new version section
- [ ] Update version references in `docs/API.md`, `docs/DEPLOYMENT.md`, and `docs/QUICKSTART.md`
- [ ] Update `helm/ai-gateway/Chart.yaml` if using Helm

### 2. Code Quality

- [ ] `cargo fmt --all` — no formatting issues
- [ ] `cargo clippy --all-targets --all-features` — no warnings
- [ ] `cargo check --all` — compiles cleanly
- [ ] `cargo test --workspace` — all Rust unit tests pass
- [ ] `cargo test --test integration -- --test-threads=1` — all E2E tests pass
- [ ] `cd frontend && pnpm test` — all frontend tests pass
- [ ] `cd frontend && pnpm build` — frontend builds without errors
- [ ] `cargo build --release --bin gateway-api` — release binary builds
- [ ] `cargo build --release --bin gateway-solo` — solo binary builds

### 3. Database Verification

- [ ] `sqlx migrate run` — all migrations apply cleanly on a fresh database
- [ ] `sqlx migrate revert` — down migrations work (spot-check)
- [ ] Verify migration files are present in `migrations/`

### 4. Documentation

- [ ] `docs/CHANGELOG.md` updated with release date and all changes
- [ ] `docs/README.md` links are valid
- [ ] `docs/QUICKSTART.md` steps verified manually
- [ ] `docs/API.md` endpoints match current router
- [ ] `docs/DEPLOYMENT.md` environment variables are current
- [ ] All external links in documentation verified (no 404s)

---

## Release

### 5. Git Tag

```bash
# Ensure you're on main and up to date
git checkout main
git pull origin main

# Create signed tag (recommended)
git tag -s v1.0.0 -m "Release v1.0.0"

# Or lightweight tag
git tag v1.0.0

# Push tag
git push origin v1.0.0
```

### 6. Docker Image Build & Push

```bash
export VERSION="1.0.0"
export REGISTRY="ghcr.io/ai-gateway"

# Build backend image
docker build \
  -f docker/Dockerfile.backend \
  -t $REGISTRY/gateway:$VERSION \
  -t $REGISTRY/gateway:latest \
  .

# Push
docker push $REGISTRY/gateway:$VERSION
docker push $REGISTRY/gateway:latest

# Build solo image (if separate)
docker build \
  -f docker/Dockerfile.solo \
  -t $REGISTRY/gateway-solo:$VERSION \
  -t $REGISTRY/gateway-solo:latest \
  .

docker push $REGISTRY/gateway-solo:$VERSION
docker push $REGISTRY/gateway-solo:latest
```

### 7. GitHub Release

- [ ] Go to GitHub → Releases → Draft new release
- [ ] Choose tag `v1.0.0`
- [ ] Title: `AI Gateway v1.0.0`
- [ ] Body: Copy relevant section from `CHANGELOG.md`
- [ ] Attach release binaries:
  - `gateway-api` (Linux x86_64)
  - `gateway-solo` (Linux x86_64)
  - `gateway-api` (Linux ARM64) if cross-compiled
- [ ] Publish release

### 8. Helm Chart Update (if applicable)

```bash
cd helm/ai-gateway

# Update Chart.yaml
sed -i "s/^version:.*/version: 1.0.0/" Chart.yaml
sed -i "s/^appVersion:.*/appVersion: \"1.0.0\"/" Chart.yaml

# Package and push
helm package .
helm push ai-gateway-1.0.0.tgz oci://ghcr.io/ai-gateway/helm
```

---

## Post-Release

### 9. Verification

- [ ] Pull and run the published Docker image locally:
  ```bash
  docker run --rm -p 8080:8080 \
    -e DATABASE_URL=... -e REDIS_URL=... \
    $REGISTRY/gateway:$VERSION
  ```
- [ ] Verify `/health` and `/ready` endpoints respond
- [ ] Verify `/v1/chat/completions` with mock response works
- [ ] Verify `/admin/` dashboard loads correctly
- [ ] Verify `/metrics` returns Prometheus data

### 10. Communication

- [ ] Post release notes to project discussion board / Slack / Discord
- [ ] Update website documentation if hosted separately
- [ ] Tweet / social media announcement (if applicable)

### 11. Monitor

- [ ] Watch error rates and latency for 24 hours after release
- [ ] Check GitHub Issues for bug reports within 48 hours
- [ ] Be prepared to execute [Rollback](#rollback-procedure) if critical issues emerge

---

## Rollback Procedure

If a critical issue is discovered post-release:

```bash
# 1. Identify last known good version
export LAST_GOOD_VERSION="v0.9.9"

# 2. Docker Compose rollback
docker compose -f docker-compose.prod.yml pull gateway:$LAST_GOOD_VERSION
docker compose -f docker-compose.prod.yml up -d gateway

# 3. Kubernetes rollback
helm rollback ai-gateway 0 -n ai-gateway
# OR
kubectl rollout undo deployment/gateway -n ai-gateway

# 4. Verify rollback
curl -f http://localhost:8080/health
curl -f http://localhost:8080/ready

# 5. Delete the bad tag (optional, prevents accidental use)
git push --delete origin v1.0.0
git tag -d v1.0.0
```

---

## Release Schedule

| Version | Target Date | Focus |
|---------|-------------|-------|
| v1.1.0 | TBD | Full API key middleware, webhook events, semantic cache |
| v1.2.0 | TBD | Billing integration, Grafana dashboards, OpenAPI docs |
| v2.0.0 | TBD | Horizontal scaling, multi-region, advanced analytics |

---

## Automation Ideas

Future improvements to this process:

- [ ] GitHub Actions workflow for automated Docker build + push on tag
- [ ] Automated changelog generation from conventional commits
- [ ] Automated Helm chart versioning and publishing
- [ ] Integration test gate before tag creation
- [ ] Automated security scan (`cargo audit`, Trivy) in CI
