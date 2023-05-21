package spootiferdb

import (
	_ "embed"
	"fmt"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
	"log"
	"os"
)

func ConnectToDB() (*gorm.DB, error) {
	dbPath := os.Getenv("DATABASE_PATH")
	log.Println("Using database at: ", dbPath)
	db, err := gorm.Open(sqlite.Open(fmt.Sprintf("%s?_journal_mode=WAL", dbPath)), &gorm.Config{
		Logger: logger.Default.LogMode(logger.Info),
	})

	if err != nil {
		return nil, err
	}

	db.AutoMigrate(&User{}, &UserGuild{}, &SpotifyAuthToken{}, &MessageLink{})

	return db, nil
}

func FirstOrCreateUserWithDiscordID(db *gorm.DB, discordUserID string) (*User, error) {
	user := &User{}

	log.Println("Looking up user")

	err := WriteSync(func() error {
		tx := db.Where(&User{DiscordUserID: discordUserID}).FirstOrCreate(user)

		if tx.Error != nil {
			return tx.Error
		}

		log.Println("Successfully got user")

		return nil
	})

	if err != nil && err != gorm.ErrRecordNotFound {
		return nil, err
	}

	if err == gorm.ErrRecordNotFound {
		log.Println("record was not found")
	}

	log.Println("Found user")
	return user, nil
}

func FirstOrCreateUserGuildWithGuildID(db *gorm.DB, userID int, discordGuildID string) (*UserGuild, error) {
	guild := &UserGuild{}

	err := WriteSync(func() error {
		tx := db.Where(&UserGuild{UserID: userID, DiscordGuildID: discordGuildID}).Attrs(&UserGuild{DiscordGuildID: discordGuildID}).FirstOrCreate(guild)

		if tx.Error != nil && tx.Error != gorm.ErrRecordNotFound {
			return tx.Error
		}

		return nil
	})

	if err != nil {
		return nil, err
	}

	return guild, nil
}

func SaveUserGuild(db *gorm.DB, userGuild *UserGuild) (*UserGuild, error) {
	err := WriteSync(func() error {
		tx := db.Save(userGuild)

		if tx.Error != nil {
			return tx.Error
		}

		return nil
	})

	if err != nil {
		return nil, err
	}

	return userGuild, nil
}

func SaveSpotifyAuthToken(db *gorm.DB, auth *SpotifyAuthToken) (*SpotifyAuthToken, error) {
	err := WriteSync(func() error {
		tx := db.Save(auth)
		if tx.Error != nil {
			return tx.Error
		}
		return nil
	})

	if err != nil {
		return nil, err
	}

	return auth, nil
}
