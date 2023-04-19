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
	state = "abc123"
)

var (
	auth        = spotifyauth.New(spotifyauth.WithRedirectURL(redirectURI), spotifyauth.WithScopes(spotifyauth.ScopePlaylistModifyPublic), spotifyauth.WithClientID(os.Getenv("CLIENT_ID")), spotifyauth.WithClientSecret(os.Getenv("CLIENT_SECRET")))
	ch          = make(chan *spotify.Client)
	redirectURI = os.Getenv("REDIRECT_URI")
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

	authURL := auth.AuthURL(state, oauth2.AccessTypeOnline)

	fmt.Println("Please visit the following URL to authorize the application:")
	fmt.Println(authURL)

	spotifyClient := <-ch

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
