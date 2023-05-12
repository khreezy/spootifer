package main

import (
	"database/sql"
	"log"
)

func connectToDB() (*sql.DB, error) {
	db, err := sql.Open("sqlite", "/litefs/spootifer.db")

	if err != nil {
		return nil, err
	}

	log.Println("Successfully opened DB connection")

	return db, nil
}
