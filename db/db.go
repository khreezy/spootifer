package spootiferdb

import (
	_ "embed"
	"errors"
	"github.com/bwmarrin/discordgo"
	"github.com/khreezy/spootifer/discord"
	spootiferspotify "github.com/khreezy/spootifer/spotify"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
	"log"
	"time"
)

func ConnectToDB() (*gorm.DB, error) {
	db, err := gorm.Open(sqlite.Open("/litefs/spootifer.db?_journal_mode=WAL"), &gorm.Config{
		Logger: logger.Default.LogMode(logger.Info),
	})

	if err != nil {
		return nil, err
	}

	db.AutoMigrate(&User{}, &UserGuild{}, &SpotifyAuthToken{})

	var authTokens []SpotifyAuthToken

	db.Find(&authTokens)

	for _, token := range authTokens {
		exp, err := time.Parse(time.DateTime, token.SpotifyExpiryTime)

		if err != nil {
			log.Println("err: ", err)
			continue
		}

		token.SpotifyExpiryTime = exp.Format(time.RFC3339)

		db.Save(&token)
	}
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

func SaveMessageLinks(db *gorm.DB, m *discordgo.MessageCreate) {
	links := spootiferspotify.GetSpotifyLinks(m.Content)

	if len(links) > 0 {

		WriteAsync(func() error {
			var err error

			for _, link := range links {
				link := &MessageLink{
					MessageID: m.ID,
					GuildID:   m.GuildID,
					ChannelID: m.ChannelID,
					Link:      link,
				}

				tx := db.Save(link)

				if tx.Error != nil {
					log.Println("Error saving MessageLink: ", err)
					err = errors.New("error saving some message link")
				}
			}

			if err != nil {
				return err
			}

			return nil
		})

	}
}

func AcknowledgeMessageLink(db *gorm.DB, m *discordgo.MessageCreate, s *discordgo.Session) {
	WriteAsync(func() error {
		return db.Transaction(func(tx *gorm.DB) error {
			r := tx.Model(&MessageLink{}).Where("message_id = ?", m.ID).Update("acknowledged", true)

			if r.Error != nil {
				return r.Error
			}

			err := s.MessageReactionAdd(m.ChannelID, m.ID, discord.EmojiID)

			if err != nil {
				return err
			}

			return nil
		})
	})
}
