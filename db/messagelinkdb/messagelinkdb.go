package messagelinkdb

import (
	"errors"
	"github.com/bwmarrin/discordgo"
	spootiferdb "github.com/khreezy/spootifer/db"
	spootiferspotify "github.com/khreezy/spootifer/spotify"
	"gorm.io/gorm"
	"log"
)

const (
	EmojiID = "\u2705"
)

func AcknowledgeMessageLink(db *gorm.DB, m *discordgo.MessageCreate, s *discordgo.Session) {
	spootiferdb.WriteAsync(func() error {
		return db.Transaction(func(tx *gorm.DB) error {
			r := tx.Model(&spootiferdb.MessageLink{}).Where("message_id = ?", m.ID).Update("acknowledged", true)

			if r.Error != nil {
				return r.Error
			}

			err := s.MessageReactionAdd(m.ChannelID, m.ID, EmojiID)

			if err != nil {
				return err
			}

			return nil
		})
	})
}

func SaveMessageLinksFromMessage(db *gorm.DB, m *discordgo.MessageCreate) {
	links := spootiferspotify.GetSpotifyLinks(m.Content)

	if len(links) > 0 {

		spootiferdb.WriteAsync(func() error {
			var err error

			for _, link := range links {
				link := &spootiferdb.MessageLink{
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

func CreateSpotifyTrackAddsForMessage(db *gorm.DB, m *discordgo.MessageCreate) ([]spootiferdb.SpotifyTrackAdd, error) {
	var messageLinks []spootiferdb.MessageLink

	tx := db.Where(&spootiferdb.MessageLink{MessageID: m.ID}).Find(messageLinks)

	if tx.Error != nil {
		return nil, tx.Error
	}

	//var trackAdds []spootiferdb.SpotifyTrackAdd

	return nil, nil
}
