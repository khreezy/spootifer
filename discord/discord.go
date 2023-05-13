package discord

import (
	"context"
	"fmt"
	"github.com/bwmarrin/discordgo"
	spootiferdb "github.com/khreezy/spootifer/db"
	spootiferspotify "github.com/khreezy/spootifer/spotify"
	"github.com/zmb3/spotify/v2"
	"gorm.io/gorm"
	"log"
	"time"
)

const (
	emojiID         = "\u2705"
	playlistLinkKey = "playlist-link"
)

type MessageCreateHandler func(s *discordgo.Session, m *discordgo.MessageCreate)
type InteractionCreateHandler func(s *discordgo.Session, i *discordgo.InteractionCreate)

var (
	ApplicationCommands = map[string]func(db *gorm.DB) func(s *discordgo.Session, i *discordgo.InteractionCreate){
		"authorize-spotify":         NewAuthorizeSpotifyHandler,
		"register-spotify-playlist": NewRegisterPlaylistHandler,
	}

	Commands = []*discordgo.ApplicationCommand{
		{
			Name:        "authorize-spotify",
			Description: "Generate a link to authorize Spootifer to use your spotify data.",
			Options:     []*discordgo.ApplicationCommandOption{
				//{
				//	Name:        "Spotify Playlist Link",
				//	Description: "Playlist to link after authorizing.",
				//	Type:        discordgo.ApplicationCommandOptionString,
				//},
			},
		},
		{
			Name:        "register-spotify-playlist",
			Description: "Adds an association to a playlist you would like to add tracks to",
			Options: []*discordgo.ApplicationCommandOption{
				{
					Name:        playlistLinkKey,
					Description: "Link to the playlist you would like to register",
					Type:        discordgo.ApplicationCommandOptionString,
					Required:    true,
				},
			},
		},
	}
)

func NewInteractionsHandler(db *gorm.DB) func(s *discordgo.Session, i *discordgo.InteractionCreate) {
	return func(s *discordgo.Session, i *discordgo.InteractionCreate) {
		log.Println("Received interaction: ", i.ApplicationCommandData().Name)
		if h, ok := ApplicationCommands[i.ApplicationCommandData().Name]; ok {
			log.Println("Running handler")
			h(db)(s, i)
		}
	}
}

func getUserId(i *discordgo.InteractionCreate) string {
	if i.User != nil {
		return i.User.ID
	} else {
		return i.Member.User.ID
	}
}

func NewRegisterPlaylistHandler(db *gorm.DB) func(s *discordgo.Session, i *discordgo.InteractionCreate) {
	return func(s *discordgo.Session, i *discordgo.InteractionCreate) {
		for _, opt := range i.ApplicationCommandData().Options {
			if opt.Name == playlistLinkKey {
				playlistID := spootiferspotify.ExtractPlaylistID(opt.StringValue())

				user := &spootiferdb.User{}
				tx := db.Preload("UserGuilds", "discord_guild_id = ?", i.GuildID).First(user, &spootiferdb.User{DiscordUserID: getUserId(i)})

				if tx.Error != nil {
					log.Println("error fetching user guild association", tx.Error)
					return
				}

				for _, guild := range user.UserGuilds {
					guild.SpotifyPlaylistID = playlistID
					tx := db.Save(&guild)

					if tx.Error != nil {
						log.Println("Error updating spotify guild playlist for user", tx.Error)
					} else {
						err := s.InteractionRespond(i.Interaction, &discordgo.InteractionResponse{
							Type: discordgo.InteractionResponseChannelMessageWithSource,
							Data: &discordgo.InteractionResponseData{
								Content: "Your playlist was registered for this guild.",
								Flags:   discordgo.MessageFlagsEphemeral,
							},
						})

						if err != nil {
							log.Println("error responding to interaction: ", err)
						}
					}
				}
			}
		}
	}
}

func NewAuthorizeSpotifyHandler(db *gorm.DB) func(s *discordgo.Session, i *discordgo.InteractionCreate) {
	return func(s *discordgo.Session, i *discordgo.InteractionCreate) {
		user := &spootiferdb.User{}

		log.Println("Looking up user")

		userId := getUserId(i)

		tx := db.Where(&spootiferdb.User{DiscordUserID: userId}).FirstOrCreate(user)

		if tx.Error != nil && tx.Error != gorm.ErrRecordNotFound {
			log.Println("Error querying db: ", tx.Error)
			return
		}

		if tx.Error == gorm.ErrRecordNotFound {
			log.Println("record was not found")
		}

		guild := &spootiferdb.UserGuild{}

		tx = db.Where(&spootiferdb.UserGuild{UserID: user.ID, DiscordGuildID: i.GuildID}).Attrs(&spootiferdb.UserGuild{DiscordGuildID: i.GuildID}).FirstOrCreate(guild)

		authUrl := spootiferspotify.GenerateAuthURL(userId)
		err := s.InteractionRespond(i.Interaction, &discordgo.InteractionResponse{
			Type: discordgo.InteractionResponseChannelMessageWithSource,
			Data: &discordgo.InteractionResponseData{
				Content: fmt.Sprintf("Please click this link to authorizer with spotify.\n%s", authUrl),
				Flags:   discordgo.MessageFlagsEphemeral,
			},
		})

		if err != nil {
			log.Println("error responding to interaction: ", err)
		}

		return
	}

}
func NewMessageCreateHandler(db *gorm.DB) func(s *discordgo.Session, m *discordgo.MessageCreate) {

	//spotifyClient := spootiferspotify.InitiateAuth(spotifyAuthChannel)

	return func(s *discordgo.Session, m *discordgo.MessageCreate) {
		// Check if the message contains a Spotify link
		log.Println("Received discord message")

		if spootiferspotify.ContainsSpotifyLink(m.Content) {
			var userGuilds []spootiferdb.UserGuild

			tx := db.Where("discord_guild_id = ?", m.GuildID).Preload("User").Preload("User.SpotifyAuthToken").Find(&userGuilds)

			if tx.Error != nil {
				log.Println("error fetching guild/playlist associations")
			}

			log.Println("Message contained spotify link")

			ids := spootiferspotify.ExtractIDs(m.Content)

			var trackIds []spotify.ID

			for _, guild := range userGuilds {
				log.Println("User auth: ", guild.User.SpotifyAuthToken)
				spotifyClient, err := spootiferspotify.ClientFromDBToken(guild.User.SpotifyAuthToken)

				if err != nil {
					log.Println("Error getting client: ", err)
				}

				if spootiferspotify.IsAlbum(m.Content) {

					album, err := spotifyClient.GetAlbum(context.Background(), spotify.ID(ids[0]))

					if err != nil {
						log.Println("error fetching album: ", err)
					}

					for _, track := range album.Tracks.Tracks {
						trackIds = append(trackIds, track.ID)
					}
				} else {
					for _, id := range ids {
						trackIds = append(trackIds, spotify.ID(id))
					}
				}

				if len(trackIds) > 0 {
					ctx, _ := context.WithTimeout(context.Background(), 5*time.Second)

					if guild.SpotifyPlaylistID == "" {
						log.Println("Playlist ID was empty")
						return
					}

					playlistID := spotify.ID(guild.SpotifyPlaylistID)

					_, err := spotifyClient.AddTracksToPlaylist(ctx, playlistID, trackIds...)

					if err != nil {
						log.Println("Failed to add track to Spotify playlist:", err)
					} else {
						log.Println("Track added to Spotify playlist")

						err = s.MessageReactionAdd(m.ChannelID, m.ID, emojiID)

						if err != nil {
							log.Println("Error adding react emoji: ", err)
						}
					}
				}
			}
		}
	}
}
