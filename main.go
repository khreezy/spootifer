package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"regexp"
	"strings"

	"github.com/bwmarrin/discordgo"
	"github.com/zmb3/spotify/v2"
	spotifyauth "github.com/zmb3/spotify/v2/auth"
	"golang.org/x/oauth2"
)

const (
	redirectURI = "https://www.google.com"
	state       = "abc123"
)

var (
	auth = spotifyauth.New(spotifyauth.WithRedirectURL(redirectURI), spotifyauth.WithScopes(spotifyauth.ScopePlaylistModifyPublic), spotifyauth.WithClientID(os.Getenv("CLIENT_ID")), spotifyauth.WithClientSecret(os.Getenv("CLIENT_SECRET")))
)

func main() {
	// authConfig := &oauth2.Config{
	// 	ClientID:     os.Getenv("CLIENT_ID"),
	// 	ClientSecret: os.Getenv("CLIENT_SECRET"),
	// 	RedirectURL:  redirectURI,
	// 	Scopes:       []string{spotify.ScopePlaylistModifyPublic}, // List of required scopes
	// 	Endpoint: oauth2.Endpoint{
	// 		AuthURL:  "https://accounts.spotify.com/authorize",
	// 		TokenURL: "https://accounts.spotify.com/api/token",
	// 	},
	// }

	authURL := auth.AuthURL(state, oauth2.AccessTypeOnline)

	fmt.Println("Please visit the following URL to authorize the application:")
	fmt.Println(authURL)

	fmt.Print("Enter the authorization code: ")
	var code string
	fmt.Scan(&code)

	token, err := auth.Exchange(context.Background(), code)

	if err != nil {
		fmt.Println("error!")
	}

	spotifyClient := spotify.New(auth.Client(context.Background(), token))
	// auth.SetAuthInfo("YOUR_CLIENT_ID", "YOUR_CLIENT_SECRET") // Replace with your client ID and client secret

	// Retrieve a token from the Spotify API
	// token, err := auth.Token("YOUR_STATE", nil)
	// if err != nil {
	// 	log.Fatal("Failed to retrieve Spotify token:", err)
	// }

	// Create a Spotify client
	// client := auth.NewClient(token)

	// Create a new Discord session
	dg, err := discordgo.New("Bot " + os.Getenv("DISCORD_BOT_TOKEN"))

	if err != nil {
		log.Fatal("Failed to create Discord session:", err)
	}

	dg.Identify.Intents = discordgo.IntentsAll

	messageCreate := func(s *discordgo.Session, m *discordgo.MessageCreate) {
		// Check if the message contains a Spotify link
		fmt.Println("received message")
		if strings.Contains(m.Content, "open.spotify.com") {
			id := extractID(m.Content)

			fmt.Println(id)
			// Add the track to the Spotify playlist

			trackIds := []spotify.ID{}

			if strings.Contains(m.Content, "https://open.spotify.com/album/") || strings.Contains(m.Content, "spotify:album:") {
				fmt.Println(id)
				album, err := spotifyClient.GetAlbum(context.Background(), spotify.ID(id))

				if err != nil {
					log.Println("error fetching album: ", err)
				}

				for _, track := range album.Tracks.Tracks {
					trackIds = append(trackIds, track.ID)
				}
			} else {
				trackIds = append(trackIds, spotify.ID(id))
			}

			if len(trackIds) > 0 {
				fmt.Println(os.Getenv("PLAYLIST_ID"))
				_, err := spotifyClient.AddTracksToPlaylist(context.Background(), spotify.ID(os.Getenv("PLAYLIST_ID")), trackIds...)
				if err != nil {
					log.Println("Failed to add track to Spotify playlist:", err)
				} else {
					log.Println("Track added to Spotify playlist")
				}
			}
		}
	}
	// Register a messageCreate event handler
	dg.AddHandler(messageCreate)

	// Open a connection to Discord
	err = dg.Open()
	if err != nil {
		log.Fatal("Failed to open Discord connection:", err)
	}

	// Wait for the application to be terminated
	fmt.Println("Bot is now running. Press CTRL-C to exit.")
	<-make(chan struct{})
}

func extractID(link string) string {
	// Define a regular expression pattern to match Spotify track IDs
	// Spotify track IDs are 22 characters long and consist of uppercase letters, lowercase letters, and digits
	regex := regexp.MustCompile(`(?:https?://open\.spotify\.com/track/|https?://open\.spotify\.com/album/|spotify:track:|spotify:album:)([a-zA-Z0-9]+)`)
	// Create a regular expression object

	// Find the first match in the input link
	matches := regex.FindStringSubmatch(link)

	fmt.Println(matches)
	if len(matches) > 1 {
		return matches[1]
	}

	// Return an empty string if no track ID was found
	return ""
}
