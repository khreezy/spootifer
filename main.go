package main

import (
	"fmt"
	"log"
	"os"
	"regexp"
	"strings"

	"github.com/bwmarrin/discordgo"
	"github.com/zmb3/spotify"
	"golang.org/x/oauth2"
)

func main() {
	authConfig := &oauth2.Config{
		ClientID:     os.Getenv("CLIENT_ID"),
		ClientSecret: os.Getenv("CLIENT_SECRET"),
		RedirectURL:  "https://www.google.com",
		Scopes:       []string{spotify.ScopePlaylistModifyPublic}, // List of required scopes
		Endpoint: oauth2.Endpoint{
			AuthURL:  "https://accounts.spotify.com/authorize",
			TokenURL: "https://accounts.spotify.com/api/token",
		},
	}

	authURL := authConfig.AuthCodeURL("state", oauth2.AccessTypeOnline)

	fmt.Println("Please visit the following URL to authorize the application:")
	fmt.Println(authURL)

	fmt.Print("Enter the authorization code: ")
	var code string
	fmt.Scan(&code)

	token, err := authConfig.Exchange(oauth2.NoContext, code)

	if err != nil {
		fmt.Println("blah")
	}

	spotifyClient := spotify.Authenticator{}.NewClient(token)

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

	dg.Identify.Intents = discordgo.IntentsMessageContent

	messageCreate := func(s *discordgo.Session, m *discordgo.MessageCreate) {
		// Check if the message contains a Spotify link
		fmt.Println("received message")
		if strings.Contains(m.Content, "open.spotify.com") {
			trackID := extractTrackID(m.Content)

			fmt.Println(trackID)
			// Add the track to the Spotify playlist
			if trackID != "" {
				fmt.Println(os.Getenv("PLAYLIST_ID"))
				_, err := spotifyClient.AddTracksToPlaylist(spotify.ID(os.Getenv("PLAYLIST_ID")), spotify.ID(trackID))
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

func extractTrackID(link string) string {
	// Define a regular expression pattern to match Spotify track IDs
	// Spotify track IDs are 22 characters long and consist of uppercase letters, lowercase letters, and digits
	regexPattern := `\/track\/([A-Za-z0-9]{22})|track\/([A-Za-z0-9]{22})|track:([A-Za-z0-9]{22})`

	// Create a regular expression object
	regex := regexp.MustCompile(regexPattern)

	// Find the first match in the input link
	match := regex.FindStringSubmatch(link)

	// Extract the track ID from the match groups
	for _, group := range match {
		if group != "" {
			// Return the matched track ID
			return strings.ReplaceAll(group, "/track/", "")
		}
	}

	// Return an empty string if no track ID was found
	return ""
}
