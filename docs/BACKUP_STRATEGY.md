# Database Backup and Migration Strategy

## Overview

This document defines the backup, restore, and migration strategy for the AI Gateway PostgreSQL database. It applies to both managed RDS deployments and self-hosted PostgreSQL instances.

---

## Backup Strategy

### Automated Daily Snapshots

**RDS (AWS):**
- RDS automated backups are enabled with a 7-day retention window.
- Backup window: `03:00–04:00 UTC` (low-traffic period).
- Multi-AZ is enabled for production to ensure failover during maintenance.

**Self-hosted / On-premise:**
- A cron job runs `scripts/backup.sh` daily at `03:00 UTC`.
- Backups are stored in S3 (or MinIO-compatible object storage).

### Point-in-Time Recovery (PITR)

- **RDS:** PITR is available for the last 7 days via automated backups and transaction logs.
- **Self-hosted:** WAL archiving must be configured (e.g., `archive_command` to S3) to enable PITR.

### Weekly Full Backups to S3

- Every Sunday at `03:00 UTC`, a full logical backup is created using `pg_dump`.
- Backups use the custom format (`--format=custom`) with compression level 9.
- Files are uploaded to S3 with server-side encryption (AES-256).
- Path convention: `s3://<bucket>/backups/gateway/<year>/<month>/<week>/gateway-<timestamp>.dump`

### Backup Retention

| Type     | Frequency | Retention | Storage         |
|----------|-----------|-----------|-----------------|
| Snapshot | Daily     | 7 days    | RDS / local     |
| Full     | Weekly    | 12 weeks  | S3 (encrypted)  |
| Full     | Monthly   | 12 months | S3 ( Glacier )  |

### Monthly Test Restore

- On the first Monday of each month, a test restore is performed on a staging database.
- Validation steps:
  1. Restore the latest weekly backup to `gateway-restore-test`.
  2. Run `ANALYZE` and verify all indexes are valid (`pg_index` check).
  3. Compare row counts for critical tables against production (within 1% tolerance).
  4. Run a subset of application smoke tests against the restored database.
- Results are logged in the infrastructure runbook.

---

## Migration Safety

### Backward-Compatible Migrations

All migrations must be backward-compatible with the currently running application version. This means:

1. **Add columns** — always use `ADD COLUMN IF NOT EXISTS`.
2. **Add indexes** — always use `CREATE INDEX IF NOT EXISTS`.
3. **Add tables** — always use `CREATE TABLE IF NOT EXISTS`.
4. **Never drop columns** in the same release that adds them.
   - Release N:   Add new column, start writing to both old and new.
   - Release N+1: Stop writing to old column, migrate data.
   - Release N+2: Drop old column in a separate migration.
5. **Never drop tables** without a deprecation period.
6. **Renames** are implemented as: add new → migrate → drop old (3-release cycle).

### Atomic Migrations

- All DDL migrations run inside transactions where possible.
- PostgreSQL does not support transactional DDL for all operations (e.g., `CREATE INDEX CONCURRENTLY` cannot run in a transaction).
- For non-transactional operations, migrations are run during a maintenance window with the application in read-only mode.

### Migration Ordering

- Migrations are ordered by filename prefix: `####_description.up.sql`.
- The migration tool (`sqlx migrate`) enforces strict sequential execution.
- Never modify a migration that has already been applied to production.

### Migration Verification

- Every migration has a corresponding `.down.sql` rollback script.
- Rollback scripts are tested in staging before production deployment.
- The CI pipeline runs migrations on both a fresh database and a database at the current production state.

---

## Restore Procedures

### Full Restore from S3 Backup

```bash
# 1. Download the backup
aws s3 cp s3://<bucket>/backups/gateway/2024/06/week-23/gateway-20240609-030000.dump /tmp/restore.dump

# 2. Create a new database (or drop and recreate)
psql -c "CREATE DATABASE gateway_restore;"

# 3. Restore
pg_restore --dbname=gateway_restore --jobs=4 --no-owner /tmp/restore.dump

# 4. Validate
scripts/restore.sh --validate gateway_restore
```

### Point-in-Time Recovery (RDS)

1. Navigate to the RDS console.
2. Select the database instance.
3. Choose "Restore to point in time".
4. Specify the desired time and target instance name.
5. Update the application `DATABASE_URL` to point to the restored instance.

### Rollback Plan

If a migration fails in production:

1. **Halt deployments** immediately.
2. **Assess** whether the migration was partially applied.
3. **Run `sqlx migrate revert`** to apply the `.down.sql` script (only if safe to do so).
4. If rollback is not safe, restore from the most recent backup + WAL to just before the migration.
5. Document the incident and update the migration before retrying.

---

## Disaster Recovery (DR)

### RPO / RTO Targets

| Environment | RPO     | RTO      |
|-------------|---------|----------|
| Production  | 1 hour  | 4 hours  |
| Staging     | 24 hours| 8 hours  |

### Cross-Region Replication (RDS)

- For production, enable cross-region read replicas in a secondary AWS region.
- In a regional disaster, promote the read replica to a standalone instance.
- Update DNS / `DATABASE_URL` to point to the promoted instance.

---

## Tools and Scripts

| Script              | Purpose                                    |
|---------------------|--------------------------------------------|
| `scripts/backup.sh` | Daily `pg_dump` + S3 upload with retention |
| `scripts/restore.sh`| Download from S3 + `pg_restore` + validate |

---

## Responsibilities

- **SRE / Platform:** Configure and monitor backup jobs, run monthly test restores.
- **Backend Engineers:** Write backward-compatible migrations, provide `.down.sql` scripts.
- **On-call Engineer:** Execute restore procedures during incidents.
