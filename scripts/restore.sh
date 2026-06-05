#!/usr/bin/env bash
set -euo pipefail

# ------------------------------------------------------------------------------
# Database Restore Script
#
# Downloads a backup from S3, restores it with pg_restore, and validates
# the restored database.
#
# Usage:
#   scripts/restore.sh --from-s3 <s3_uri> --to-db <database_url>
#   scripts/restore.sh --validate <database_url>
#
# Environment variables:
#   AWS_REGION  - AWS region, default: us-east-1
# ------------------------------------------------------------------------------

AWS_REGION="${AWS_REGION:-us-east-1}"

S3_URI=""
TARGET_DB=""
VALIDATE_DB=""

usage() {
    cat <<EOF
Usage:
  $(basename "$0") --from-s3 s3://bucket/prefix/file.dump.gz --to-db postgres://user:pass@host/db
  $(basename "$0") --validate postgres://user:pass@host/db
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --from-s3)
            S3_URI="$2"
            shift 2
            ;;
        --to-db)
            TARGET_DB="$2"
            shift 2
            ;;
        --validate)
            VALIDATE_DB="$2"
            shift 2
            ;;
        *)
            usage
            ;;
    esac
done

if [[ -n "$VALIDATE_DB" ]]; then
    echo "Validating database: ${VALIDATE_DB//:*@/:***@}"

    # Check connectivity
    if ! psql "${VALIDATE_DB}" -c "SELECT 1;" &>/dev/null; then
        echo "ERROR: Cannot connect to validation database" >&2
        exit 1
    fi

    # Check for invalid indexes
    invalid_indexes=$(psql "${VALIDATE_DB}" -t -c "
        SELECT COUNT(*) FROM pg_index
        WHERE NOT indisvalid;
    " | tr -d ' ')

    if [[ "${invalid_indexes}" -gt 0 ]]; then
        echo "ERROR: Found ${invalid_indexes} invalid indexes" >&2
        exit 1
    fi
    echo "  ✓ All indexes are valid"

    # Row counts for critical tables
    echo "  Row counts:"
    psql "${VALIDATE_DB}" -c "
        SELECT
            'organizations' AS table_name, COUNT(*) AS rows FROM organizations
        UNION ALL
        SELECT 'users', COUNT(*) FROM users
        UNION ALL
        SELECT 'api_keys', COUNT(*) FROM api_keys
        UNION ALL
        SELECT 'provider_configs', COUNT(*) FROM provider_configs
        UNION ALL
        SELECT 'requests', COUNT(*) FROM requests
        UNION ALL
        SELECT 'responses', COUNT(*) FROM responses;
    "

    echo "Validation complete."
    exit 0
fi

if [[ -z "$S3_URI" || -z "$TARGET_DB" ]]; then
    usage
fi

if ! command -v pg_restore &>/dev/null; then
    echo "ERROR: pg_restore is not installed" >&2
    exit 1
fi

if ! command -v aws &>/dev/null; then
    echo "ERROR: AWS CLI is not installed" >&2
    exit 1
fi

DUMP_FILE="/tmp/restore-$(date +%s).dump"
COMPRESSED_FILE="${DUMP_FILE}.gz"

echo "Downloading backup from ${S3_URI}..."
aws s3 cp "${S3_URI}" "${COMPRESSED_FILE}" --region "${AWS_REGION}"

echo "Decompressing..."
gunzip -c "${COMPRESSED_FILE}" > "${DUMP_FILE}"
rm -f "${COMPRESSED_FILE}"

# Extract database name from TARGET_DB for drop/create
DB_NAME=$(echo "${TARGET_DB}" | sed -n 's/.*\/\([^/]*\)$/\1/p')
ADMIN_URL=$(echo "${TARGET_DB}" | sed "s|/${DB_NAME}$|/postgres|")

echo "Preparing target database: ${DB_NAME}..."
psql "${ADMIN_URL}" -c "DROP DATABASE IF EXISTS ${DB_NAME};" || true
psql "${ADMIN_URL}" -c "CREATE DATABASE ${DB_NAME};"

echo "Restoring database..."
pg_restore \
    --dbname="${TARGET_DB}" \
    --jobs=4 \
    --no-owner \
    --no-privileges \
    --verbose \
    "${DUMP_FILE}"

rm -f "${DUMP_FILE}"

echo "Restore complete. Running validation..."
"$0" --validate "${TARGET_DB}"
