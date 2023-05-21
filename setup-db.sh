#!/usr/bin/env sh

set -e

echo "$DATABASE_PATH"

dbPath="$(dirname "$DATABASE_PATH")"
if [ ! -d "$dbPath" ]
then
  echo "Initializing data dir"
  mkdir -p "$dbPath"
  echo $?
fi

if [ ! -f "$DATABASE_PATH" ]
then
  echo "Initializing db file"
  touch "$DATABASE_PATH"
  echo $?
fi

chmod a+rw "$DATABASE_PATH"