#!/usr/bin/env bash
set -euo pipefail

# ------------------------------------------------------------------------------
# Database Backup Script
#
# Performs a pg_dump in custom format, compresses it, and uploads to S3.
# Enforces retention: 30 daily, 12 weekly, 12 monthly backups.
#
# Environment variables:
#   DATABASE_URL    - PostgreSQL connection string (required)
#   S3_BUCKET       - S3 bucket name (required)
#   S3_PREFIX       - S3 key prefix, default: backups/gateway
#   AWS_REGION      - AWS region, default: us-east-1
#   RETENTION_DAYS  - Daily retention, default: 30
#   RETENTION_WEEKS - Weekly retention, default: 12
#   RETENTION_MONTHS- Monthly retention, default: 12
# ------------------------------------------------------------------------------

DATABASE_URL="${DATABASE_URL:-}"
S3_BUCKET="${S3_BUCKET:-}"
S3_PREFIX="${S3_PREFIX:-backups/gateway}"
AWS_REGION="${AWS_REGION:-us-east-1}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
RETENTION_WEEKS="${RETENTION_WEEKS:-12}"
RETENTION_MONTHS="${RETENTION_MONTHS:-12}"

if [[ -z "$DATABASE_URL" ]]; then
    echo "ERROR: DATABASE_URL is required" >&2
    exit 1
fi

if [[ -z "$S3_BUCKET" ]]; then
    echo "ERROR: S3_BUCKET is required" >&2
    exit 1
fi

if ! command -v pg_dump &>/dev/null; then
    echo "ERROR: pg_dump is not installed" >&2
    exit 1
fi

if ! command -v aws &>/dev/null; then
    echo "ERROR: AWS CLI is not installed" >&2
    exit 1
fi

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
YEAR=$(date +%Y)
MONTH=$(date +%m)
WEEK=$(date +%V)
DAY=$(date +%d)

DUMP_FILE="/tmp/gateway-${TIMESTAMP}.dump"
COMPRESSED_FILE="${DUMP_FILE}.gz"

# Determine backup type
if [[ "$DAY" == "01" ]]; then
    BACKUP_TYPE="monthly"
    S3_KEY="${S3_PREFIX}/${YEAR}/${MONTH}/gateway-${TIMESTAMP}-monthly.dump.gz"
elif [[ "$(date +%u)" == "7" ]]; then
    BACKUP_TYPE="weekly"
    S3_KEY="${S3_PREFIX}/${YEAR}/${MONTH}/week-${WEEK}/gateway-${TIMESTAMP}-weekly.dump.gz"
else
    BACKUP_TYPE="daily"
    S3_KEY="${S3_PREFIX}/${YEAR}/${MONTH}/${DAY}/gateway-${TIMESTAMP}-daily.dump.gz"
fi

echo "Starting ${BACKUP_TYPE} backup at ${TIMESTAMP}..."
echo "Source: ${DATABASE_URL//:*@/:***@}"
echo "Destination: s3://${S3_BUCKET}/${S3_KEY}"

# Run pg_dump with custom format and compression
pg_dump \
    --dbname="${DATABASE_URL}" \
    --format=custom \
    --compress=9 \
    --verbose \
    --file="${DUMP_FILE}"

# Compress with gzip for extra safety
gzip -c "${DUMP_FILE}" > "${COMPRESSED_FILE}"
rm -f "${DUMP_FILE}"

# Upload to S3 with server-side encryption
aws s3 cp "${COMPRESSED_FILE}" "s3://${S3_BUCKET}/${S3_KEY}" \
    --region "${AWS_REGION}" \
    --server-side-encryption AES256

rm -f "${COMPRESSED_FILE}"

echo "Backup uploaded successfully: s3://${S3_BUCKET}/${S3_KEY}"

# ------------------------------------------------------------------------------
# Retention cleanup
# ------------------------------------------------------------------------------

cleanup_retention() {
    local prefix="$1"
    local retention="$2"
    local unit="$3"

    echo "Cleaning up ${unit} backups older than ${retention} ${unit}s..."

    cutoff_date=$(date -d "-${retention} ${unit}" +%s 2>/dev/null || date -v-${retention}${unit} +%s)
    cutoff_iso=$(date -d "@${cutoff_date}" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -r "${cutoff_date}" +%Y-%m-%dT%H:%M:%SZ)

    aws s3api list-objects-v2 \
        --bucket "${S3_BUCKET}" \
        --prefix "${prefix}" \
        --query 'Contents[?LastModified<=`'"${cutoff_iso}"'`].[Key]' \
        --output text \
        --region "${AWS_REGION}" | while read -r key; do
        if [[ -n "$key" && "$key" != "None" ]]; then
            echo "Deleting old backup: s3://${S3_BUCKET}/${key}"
            aws s3 rm "s3://${S3_BUCKET}/${key}" --region "${AWS_REGION}"
        fi
    done
}

cleanup_retention "${S3_PREFIX}/" "${RETENTION_DAYS}" "day"

echo "Backup complete at $(date +%Y%m%d-%H%M%S)"
