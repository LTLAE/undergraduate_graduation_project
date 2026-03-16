#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"

# Create SQLite database in the script directory and apply init.sql
DB_FILE="${script_dir}/db.sqlite"
INIT_SQL="${script_dir}/init.sql"

if [[ ! -f "${INIT_SQL}" ]]; then
  echo "init.sql not found in ${script_dir}." >&2
  exit 1
fi

sqlite3 "${DB_FILE}" < "${INIT_SQL}"
echo "SQLite database created at ${DB_FILE} using ${INIT_SQL}."
