package spootiferdb

import (
	_ "embed"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
	"log"
)

func ConnectToDB() (*gorm.DB, error) {
	db, err := gorm.Open(sqlite.Open("/litefs/spootifer.db?_journal_mode=WAL"), &gorm.Config{
		Logger: logger.Default.LogMode(logger.Info),
	})

	if err != nil {
		return nil, err
	}

	db.AutoMigrate(&User{}, &UserGuild{}, &SpotifyAuthToken{})

	users := &[]*User{}
	db.Find(users)

	log.Println("Users: ", users)
	return db, nil
}
