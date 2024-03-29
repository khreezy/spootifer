package discord

import (
	"context"
	"fmt"
	"github.com/bwmarrin/discordgo"
	spootiferdb "github.com/khreezy/spootifer/db"
	"github.com/khreezy/spootifer/db/messagelinkdb"
	spootiferspotify "github.com/khreezy/spootifer/spotify"
	"github.com/zmb3/spotify/v2"
	"gorm.io/gorm"
	"log"
	"time"
)

const (
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

func UpdateApplicationCommands(s *discordgo.Session) {
	for _, v := range Commands {
		_, err := s.ApplicationCommandCreate(s.State.User.ID, "", v)

		if err != nil {
			log.Println("Error registering application command: ", err)
		}
	}

	existingCmds, err := s.ApplicationCommands(s.State.User.ID, "")

	if err != nil {
		log.Println("Error listing existing application commands")
		return
	}

	for _, cmd := range existingCmds {
		if _, ok := ApplicationCommands[cmd.Name]; !ok {
			err := s.ApplicationCommandDelete(s.State.User.ID, "", cmd.ID)

			if err != nil {
				log.Println("Error deleting application command ", cmd.Name, ":", err)
			}
		}
	}
}

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

					_, err := spootiferdb.SaveUserGuild(db, &guild)

					if err != nil {
						log.Println("Error updating spotify guild playlist for user", tx.Error)
					} else {
						go respondToMessage(s, i.Interaction, "Your playlist was registered for this server.")
					}
				}
			}
		}
	}
}

func respondToMessage(s *discordgo.Session, i *discordgo.Interaction, msg string) {
	err := s.InteractionRespond(i, &discordgo.InteractionResponse{
		Type: discordgo.InteractionResponseChannelMessageWithSource,
		Data: &discordgo.InteractionResponseData{
			Content: msg,
			Flags:   discordgo.MessageFlagsEphemeral,
		},
	})

	if err != nil {
		log.Println("error responding to interaction: ", err)
	}
}

func NewAuthorizeSpotifyHandler(db *gorm.DB) func(s *discordgo.Session, i *discordgo.InteractionCreate) {
	return func(s *discordgo.Session, i *discordgo.InteractionCreate) {
		userId := getUserId(i)

		user, err := spootiferdb.FirstOrCreateUserWithDiscordID(db, userId)

		if err != nil {
			log.Println("error getting or creating user: ", err)
			return
		}

		userGuild, err := spootiferdb.FirstOrCreateUserGuildWithGuildID(db, user.ID, i.GuildID)

		if err != nil {
			log.Println("Error creating user guild: ", err)
			return
		}

		log.Printf("Got user guild for user ID %d and guild ID %s", userGuild.UserID, userGuild.DiscordGuildID)

		authUrl := spootiferspotify.GenerateAuthURL(userId)

		go respondToMessage(s, i.Interaction, fmt.Sprintf("Please click this link to authorizer with spotify.\n%s", authUrl))
	}

}
func NewMessageCreateHandler(db *gorm.DB) func(s *discordgo.Session, m *discordgo.MessageCreate) {
	return func(s *discordgo.Session, m *discordgo.MessageCreate) {
		// Check if the message contains a Spotify link
		log.Println("Received discord message")

		if spootiferspotify.ContainsSpotifyLink(m.Content) {
			messagelinkdb.SaveMessageLinksFromMessage(db, m)

			var userGuilds []spootiferdb.UserGuild

			tx := db.Where("discord_guild_id = ?", m.GuildID).Preload("User").Preload("User.SpotifyAuthToken").Find(&userGuilds)

			if tx.Error != nil {
				log.Println("error fetching guild/playlist associations")
			}

			log.Println("Message contained spotify link")

			ids := spootiferspotify.ExtractIDs(m.Content)

			var trackIds []spotify.ID

			if spootiferspotify.IsAlbum(m.Content) {
				spotifyClient, err := spootiferspotify.ClientFromClientCreds(context.Background())

				if err != nil {
					log.Println("Error getting spotify client: ", err)
				}

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

			for _, guild := range userGuilds {
				spotifyClient, err := spootiferspotify.ClientFromDBToken(guild.User.SpotifyAuthToken)

				if err != nil {
					log.Println("Error getting client: ", err)
					continue
				}

				if len(trackIds) == 0 {

				}

				go FinishAddTrackToPlaylist(db, s, spotifyClient, trackIds, guild, m)
			}
		}
	}
}

func FinishAddTrackToPlaylist(db *gorm.DB, s *discordgo.Session, spotifyClient *spotify.Client, trackIDs []spotify.ID, guild spootiferdb.UserGuild, m *discordgo.MessageCreate) {
	if len(trackIDs) > 0 {
		ctx, _ := context.WithTimeout(context.Background(), 5*time.Second)

		if guild.SpotifyPlaylistID == "" {
			log.Println("Playlist ID was empty")
			return
		}

		playlistID := spotify.ID(guild.SpotifyPlaylistID)

		_, err := spotifyClient.AddTracksToPlaylist(ctx, playlistID, trackIDs...)

		if err != nil {
			log.Println("Failed to add track to Spotify playlist:", err)
		} else {
			log.Println("Track added to Spotify playlist")

			messagelinkdb.AcknowledgeMessageLink(db, m, s)
		}
	}
}
