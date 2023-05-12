package main

import (
	"context"
	"crypto/ed25519"
	"fmt"
	spootiferspotify "github.com/khreezy/spootifer/spotify"
	"github.com/sashabaranov/go-openai"
	"log"
	"net/http"
	"os"
	"time"

	"github.com/bwmarrin/discordgo"
	"github.com/zmb3/spotify/v2"
	spotifyauth "github.com/zmb3/spotify/v2/auth"
	"golang.org/x/oauth2"
)

const (
	state = "abc123"
)

var discordBotPublicKey = ed25519.PublicKey(os.Getenv("DISOCRD_BOT_PUBLIC_KEY"))

var (
	ch             = make(chan *spotify.Client)
	redirectURI    = os.Getenv("SPOTIFY_REDIRECT_URI")
	openAIToken    = os.Getenv("OPENAI_TOKEN")
	chatGPTEnabled = os.Getenv("CHATGPT_ENABLED")
	auth           = spotifyauth.New(spotifyauth.WithRedirectURL(redirectURI), spotifyauth.WithScopes(spotifyauth.ScopePlaylistModifyPublic), spotifyauth.WithClientID(os.Getenv("SPOTIFY_CLIENT_ID")), spotifyauth.WithClientSecret(os.Getenv("SPOTIFY_CLIENT_SECRET")))
	emojiID        = "\u2705"
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
	//dg.Identify.Shard = []

	messageCreate := func(s *discordgo.Session, m *discordgo.MessageCreate) {
		// Check if the message contains a Spotify link
		log.Println("Received discord message")

		if spootiferspotify.ContainsSpotifyLink(m.Content) {
			log.Println("Message contained spotify link")

			ids := spootiferspotify.ExtractIDs(m.Content)

			var trackIds []spotify.ID

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
				_, err := spotifyClient.AddTracksToPlaylist(ctx, spotify.ID(os.Getenv("SPOTIFY_PLAYLIST_ID")), trackIds...)

				if err != nil {
					log.Println("Failed to add track to Spotify playlist:", err)
				} else {
					log.Println("Track added to Spotify playlist")

					err = dg.MessageReactionAdd(m.ChannelID, m.ID, emojiID)

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

func startAuthServer() {
	http.HandleFunc("/callback", completeAuth)
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		log.Println("got health check")
		w.WriteHeader(http.StatusOK)
	})
	http.HandleFunc("/bot/interactions", func(w http.ResponseWriter, r *http.Request) {
		if discordgo.VerifyInteraction(r, discordBotPublicKey) {
			w.WriteHeader(http.StatusOK)
		}

		w.WriteHeader(http.StatusUnauthorized)
	})

	log.Println("Starting auth server on port 8081")

	go func() {
		err := http.ListenAndServe(":8081", nil)
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
	client := spotify.New(auth.Client(context.Background(), tok))
	fmt.Fprintf(w, "Login Completed!")
	ch <- client
}
