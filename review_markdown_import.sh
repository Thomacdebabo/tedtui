#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  review_markdown_import.sh [--dry-run] [--vault PATH] [--tedtui-bin CMD] <source-dir>
  review_markdown_import.sh [--dry-run] <source-dir> <obsidian-vault-dir>

This script walks through all Markdown files under <source-dir>.
For each file it will show the content and let you choose:
  s = skip and keep the file
  o = create a new note in the Obsidian vault, then delete the source file
  t = open tedtui with:
      name = first Markdown heading
      goal = rest of the file
      then delete the source file
  q = quit

Options:
  --dry-run         Show what would happen without copying files or launching tedtui
  --vault PATH      Path to the Obsidian vault directory
  --tedtui-bin CMD  tedtui command to use (default: tedtui)
  -h, --help        Show this help text

Environment:
  OBSIDIAN_VAULT    Default vault path if --vault is not provided
  TEDTUI_BIN        Default tedtui command if --tedtui-bin is not provided
EOF
}

DRY_RUN=false
SOURCE_DIR=""
OBSIDIAN_VAULT="${OBSIDIAN_VAULT:-}"
TEDTUI_BIN="${TEDTUI_BIN:-tedtui}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    -n|--dry-run)
      DRY_RUN=true
      ;;
    --vault)
      shift
      [[ $# -gt 0 ]] || { echo "Missing value for --vault" >&2; exit 1; }
      OBSIDIAN_VAULT="$1"
      ;;
    --tedtui-bin)
      shift
      [[ $# -gt 0 ]] || { echo "Missing value for --tedtui-bin" >&2; exit 1; }
      TEDTUI_BIN="$1"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -* )
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
    *)
      if [[ -z "$SOURCE_DIR" ]]; then
        SOURCE_DIR="$1"
      elif [[ -z "$OBSIDIAN_VAULT" ]]; then
        OBSIDIAN_VAULT="$1"
      else
        echo "Too many arguments." >&2
        usage
        exit 1
      fi
      ;;
  esac
  shift
done

[[ -n "$SOURCE_DIR" ]] || {
  usage
  exit 1
}

expand_path() {
  local path="$1"

  case "$path" in
    "~") path="$HOME" ;;
    "~/"*) path="$HOME/${path#~/}" ;;
  esac

  printf '%s' "$path"
}

SOURCE_DIR="$(expand_path "$SOURCE_DIR")"

[[ -d "$SOURCE_DIR" ]] || {
  echo "Source directory does not exist: $SOURCE_DIR" >&2
  exit 1
}

SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"

sanitize_filename() {
  local input="$1"
  input="${input//$'\r'/}"
  input="$(printf '%s' "$input" | sed 's/[\\/:*?"<>|]/_/g; s/^[[:space:].]*//; s/[[:space:]]*$//')"
  [[ -n "$input" ]] || input="untitled"
  printf '%s' "$input"
}

extract_title() {
  local file="$1"
  local title

  title="$(awk '
    /^[[:space:]]*#/ {
      line = $0
      sub(/^[[:space:]]*#+[[:space:]]*/, "", line)
      if (length(line) > 0) {
        print line
        exit
      }
    }
  ' "$file")"

  if [[ -z "$title" ]]; then
    title="$(basename "${file%.md}")"
  fi

  printf '%s' "$title"
}

extract_body() {
  local file="$1"
  awk '
    BEGIN { found = 0 }
    /^[[:space:]]*#/ && found == 0 {
      line = $0
      sub(/^[[:space:]]*#+[[:space:]]*/, "", line)
      if (length(line) > 0) {
        found = 1
        next
      }
    }
    { print }
  ' "$file"
}

ensure_vault_dir() {
  if [[ -z "$OBSIDIAN_VAULT" ]]; then
    read -r -p "Obsidian vault directory: " OBSIDIAN_VAULT
  fi

  [[ -n "$OBSIDIAN_VAULT" ]] || {
    echo "No vault directory provided." >&2
    return 1
  }

  OBSIDIAN_VAULT="$(expand_path "$OBSIDIAN_VAULT")"
  mkdir -p "$OBSIDIAN_VAULT"
  OBSIDIAN_VAULT="$(cd "$OBSIDIAN_VAULT" && pwd)"
}

show_file() {
  local file="$1"
  local current="$2"
  local total="$3"

  clear 2>/dev/null || true
  printf '\n===== [%s/%s] %s =====\n\n' "$current" "$total" "$file"

  if command -v bat >/dev/null 2>&1; then
    bat --paging=never --style=plain --language=md "$file"
  else
    cat "$file"
  fi

  printf '\n'
}

delete_source_file() {
  local file="$1"

  if $DRY_RUN; then
    printf '[dry-run] Would delete source file: %s\n' "$file"
    return 0
  fi

  rm -f -- "$file"
  printf 'Deleted source file: %s\n' "$file"
}

create_obsidian_note() {
  local source_file="$1"
  local title="$2"

  ensure_vault_dir

  local safe_title target suffix
  safe_title="$(sanitize_filename "$title")"
  target="$OBSIDIAN_VAULT/$safe_title.md"
  suffix=2

  while [[ -e "$target" ]]; do
    target="$OBSIDIAN_VAULT/${safe_title}_$suffix.md"
    ((suffix++))
  done

  if $DRY_RUN; then
    printf '[dry-run] Would create Obsidian note: %s\n' "$target"
    return 0
  fi

  cp "$source_file" "$target"
  printf 'Created Obsidian note: %s\n' "$target"
}

open_in_tedtui() {
  local title="$1"
  local body="$2"

  if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required to build the JSON payload." >&2
    return 1
  fi

  local json_payload
  json_payload="$(TITLE="$title" BODY="$body" python3 - <<'PY'
import json
import os

payload = {
    "name": os.environ.get("TITLE", ""),
    "goal": os.environ.get("BODY", ""),
}
print(json.dumps(payload, ensure_ascii=False))
PY
)"

  if $DRY_RUN; then
    printf '[dry-run] Would run: %s --json %q\n' "$TEDTUI_BIN" "$json_payload"
    return 0
  fi

  if ! command -v "$TEDTUI_BIN" >/dev/null 2>&1; then
    echo "tedtui command not found: $TEDTUI_BIN" >&2
    return 1
  fi

  "$TEDTUI_BIN" --json "$json_payload"
}

mapfile -d '' FILES < <(find "$SOURCE_DIR" -type f \( -iname '*.md' \) -print0 | sort -z)
TOTAL="${#FILES[@]}"

if [[ "$TOTAL" -eq 0 ]]; then
  echo "No Markdown files found in: $SOURCE_DIR"
  exit 0
fi

skipped=0
obsidian_created=0
tedtui_opened=0
deleted_files=0

for i in "${!FILES[@]}"; do
  file="${FILES[$i]}"
  display_num=$((i + 1))

  show_file "$file" "$display_num" "$TOTAL"

  title="$(extract_title "$file")"
  body="$(extract_body "$file")"

  printf 'Detected title: %s\n' "$title"

  while true; do
    read -r -p "Choose [s]kip, [o]bsidian, [t]edtui, [q]uit: " choice
    choice="${choice,,}"

    case "$choice" in
      ""|s|skip)
        ((skipped+=1))
        break
        ;;
      o|obsidian)
        create_obsidian_note "$file" "$title"
        delete_source_file "$file"
        ((obsidian_created+=1))
        ((deleted_files+=1))
        break
        ;;
      t|tedtui)
        open_in_tedtui "$title" "$body"
        delete_source_file "$file"
        ((tedtui_opened+=1))
        ((deleted_files+=1))
        break
        ;;
      q|quit)
        printf '\nStopped early.\n'
        printf 'Summary: skipped=%s, obsidian=%s, tedtui=%s, deleted=%s\n' "$skipped" "$obsidian_created" "$tedtui_opened" "$deleted_files"
        exit 0
        ;;
      *)
        echo "Please enter s, o, t, or q."
        ;;
    esac
  done
done

printf '\nDone. Summary: skipped=%s, obsidian=%s, tedtui=%s, deleted=%s\n' "$skipped" "$obsidian_created" "$tedtui_opened" "$deleted_files"
