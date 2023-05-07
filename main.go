package main

import (
	"context"
	"fmt"
	"github.com/sashabaranov/go-openai"
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
	ch             = make(chan *spotify.Client)
	redirectURI    = os.Getenv("SPOTIFY_REDIRECT_URI")
	openAIToken    = os.Getenv("OPENAI_TOKEN")
	chatGPTEnabled = os.Getenv("CHATGPT_ENABLED")
	auth           = spotifyauth.New(spotifyauth.WithRedirectURL(redirectURI), spotifyauth.WithScopes(spotifyauth.ScopePlaylistModifyPublic), spotifyauth.WithClientID(os.Getenv("SPOTIFY_CLIENT_ID")), spotifyauth.WithClientSecret(os.Getenv("SPOTIFY_CLIENT_SECRET")))
	emojiID        = "1025618781206216744"
)

func main() {
	startAuthServer()

	chatClient := openai.NewClient(openAIToken)

	authURL := auth.AuthURL(state, oauth2.AccessTypeOnline)

	fmt.Println("Please visit the following URL to authorize the application:")
	fmt.Println(authURL)

	err := sendAuthEmail(authURL)

	if err != nil {
		log.Println("Error sending auth email: ", err)
	}

	spotifyClient := <-ch

	log.Println("Received spotify authorization!")

	// Create a new Discord session
	dg, err := discordgo.New("Bot " + os.Getenv("DISCORD_BOT_TOKEN"))

	if err != nil {
		log.Fatal("Failed to create Discord session:", err)
	}

	log.Println("Successfully authenticated with discord")

	dg.Identify.Intents = discordgo.IntentsAll

	messageCreate := func(s *discordgo.Session, m *discordgo.MessageCreate) {
		// Check if the message contains a Spotify link
		log.Println("Received discord message")

		if strings.Contains(m.Content, "open.spotify.com") {
			log.Println("Message contained spotify link")

			ids := extractIDs(m.Content)

			var trackIds []spotify.ID

			if strings.Contains(m.Content, "https://open.spotify.com/album/") || strings.Contains(m.Content, "spotify:album:") {
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
				_, err := spotifyClient.AddTracksToPlaylist(context.Background(), spotify.ID(os.Getenv("SPOTIFY_PLAYLIST_ID")), trackIds...)
				if err != nil {
					log.Println("Failed to add track to Spotify playlist:", err)
				} else {
					log.Println("Track added to Spotify playlist")

					err = dg.MessageReactionAdd(m.ChannelID, m.MessageReference.MessageID, emojiID)

					if err != nil {
						log.Println("Error adding react emoji: ", err)
					}
				}

				if chatGPTEnabled == "true" {
					log.Println("generating chatGPT response")

					err := generateChatGptResponse(context.Background(), chatClient, s, m)

					if err != nil {
						log.Println("error generating chatGPT response: ", err)
					}
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
	log.Println("Bot is now running. Press CTRL-C to exit.")
	<-make(chan struct{})
}

func extractIDs(link string) []string {
	// Define a regular expression pattern to match Spotify track IDs
	// Spotify track IDs are 22 characters long and consist of uppercase letters, lowercase letters, and digits
	regex := regexp.MustCompile(`(?:https?://open\.spotify\.com/track/|https?://open\.spotify\.com/album/|spotify:track:|spotify:album:)([a-zA-Z0-9]+)`)
	// Create a regular expression object

	// Find the first match in the input link
	matches := regex.FindAllStringSubmatch(link, -1)

	fmt.Println(matches)

	ids := []string{}

	for _, match := range matches {
		if len(match) > 1 {
			ids = append(ids, match[1])
		}
	}

	// Return an empty string if no track ID was found
	return ids
}

func startAuthServer() {
	http.HandleFunc("/callback", completeAuth)
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		log.Println("got health check")
		w.WriteHeader(http.StatusOK)
	})

	log.Println("Starting auth server on port 8080")

	go func() {
		err := http.ListenAndServe(":8080", nil)
		if err != nil {
			log.Fatal(err)
		}
	}()
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
