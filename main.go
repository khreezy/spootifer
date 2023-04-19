package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"os"
	"regexp"
	"strings"

	"github.com/bwmarrin/discordgo"
	"github.com/zmb3/spotify/v2"
	spotifyauth "github.com/zmb3/spotify/v2/auth"
	"golang.org/x/oauth2"
)

const (
	redirectURI = "http://localhost:8080/callback"
	state       = "abc123"
)

var (
	auth = spotifyauth.New(spotifyauth.WithRedirectURL(redirectURI), spotifyauth.WithScopes(spotifyauth.ScopePlaylistModifyPublic), spotifyauth.WithClientID(os.Getenv("CLIENT_ID")), spotifyauth.WithClientSecret(os.Getenv("CLIENT_SECRET")))
	ch   = make(chan *spotify.Client)
)

func main() {
	http.HandleFunc("/callback", completeAuth)
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		log.Println("Got request for:", r.URL.String())
	})
	go func() {
		err := http.ListenAndServe(":8080", nil)
		if err != nil {
			log.Fatal(err)
		}
	}()

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

	spotifyClient := <-ch

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
			trackID := extractTrackID(m.Content)

			fmt.Println(trackID)
			// Add the track to the Spotify playlist
			if trackID != "" {
				fmt.Println(os.Getenv("PLAYLIST_ID"))
				_, err := spotifyClient.AddTracksToPlaylist(context.Background(), spotify.ID(os.Getenv("PLAYLIST_ID")), spotify.ID(trackID))
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

func completeAuth(w http.ResponseWriter, r *http.Request) {
	tok, err := auth.Token(r.Context(), state, r)
	if err != nil {
		http.Error(w, "Couldn't get token", http.StatusForbidden)
		log.Fatal(err)
	}
	if st := r.FormValue("state"); st != state {
		http.NotFound(w, r)
		log.Fatalf("State mismatch: %s != %s\n", st, state)
	}

	// use the token to get an authenticated client
	client := spotify.New(auth.Client(r.Context(), tok))
	fmt.Fprintf(w, "Login Completed!")
	ch <- client
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
