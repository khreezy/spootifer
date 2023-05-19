#!/usr/bin/env sh

dbPath="$(dirname "$DATABASE_PATH")"
if [ ! -d "$dbPath" ]
then
  mkdir -p "$dbPath"
fi

if [ ! -f "$DATABASE_PATH" ]
then
  touch "$DATABASE_PATH"
fi

chmod a+rw "$DATABASE_PATH"