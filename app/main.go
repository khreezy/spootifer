package main

import (
	"crypto/ed25519"
	"fmt"
	"github.com/bwmarrin/discordgo"
	spootiferdb "github.com/khreezy/spootifer/db"
	"github.com/khreezy/spootifer/discord"
	"github.com/zmb3/spotify/v2"
	spotifyauth "github.com/zmb3/spotify/v2/auth"
	"gorm.io/gorm"
	"log"
	"net/http"
	"os"
	"time"
)

var discordBotPublicKey = ed25519.PublicKey(os.Getenv("DISCORD_BOT_PUBLIC_KEY"))

var (
	ch          = make(chan *spotify.Client)
	redirectURI = os.Getenv("SPOTIFY_REDIRECT_URI")
	auth        = spotifyauth.New(spotifyauth.WithRedirectURL(redirectURI), spotifyauth.WithScopes(spotifyauth.ScopePlaylistModifyPublic), spotifyauth.WithClientID(os.Getenv("SPOTIFY_CLIENT_ID")), spotifyauth.WithClientSecret(os.Getenv("SPOTIFY_CLIENT_SECRET")))
)

func main() {
	// Create a new Discord session
	dg, err := discordgo.New("Bot " + os.Getenv("DISCORD_BOT_TOKEN"))

	if err != nil {
		log.Fatal("Failed to create Discord session:", err)
	}

	log.Println("Successfully authenticated with discord")

	dg.Identify.Intents = discordgo.IntentsAll
	//dg.Identify.Shard = []

	dbConn, err := spootiferdb.ConnectToDB()

	if err != nil {
		log.Fatal("Failed to connect to db")
	}

	spootiferdb.StartWriteThread()

	//allocId, err := uuid.Parse(os.Getenv("FLY_ALLOC_ID"))
	//
	//var id int
	//
	//if err == nil {
	//	id = int(allocId.ID())
	//} else {
	//	id = rand.Int()
	//}

	//dg.Identify.Shard = &([2]int{id, 2})

	startAuthServer(dbConn)

	messageCreate := discord.NewMessageCreateHandler(dbConn)
	interactionHandler := discord.NewInteractionsHandler(dbConn)
	// Register a messageCreate event handler
	dg.AddHandler(messageCreate)
	dg.AddHandler(interactionHandler)

	err = dg.Open()

	if err != nil {
		log.Fatal("Failed to open Discord connection:", err)
	}

	discord.UpdateApplicationCommands(dg)

	// Open a connection to Discord

	// Wait for the application to be terminated
	log.Println("Bot is now running. Press CTRL-C to exit.")
	<-make(chan struct{})
}

func startAuthServer(db *gorm.DB) {
	http.HandleFunc("/callback", completeAuth(db))
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

func completeAuth(db *gorm.DB) func(w http.ResponseWriter, r *http.Request) {
	return func(w http.ResponseWriter, r *http.Request) {
		st := r.FormValue("state")

		user := &spootiferdb.User{}
		tx := db.Where(&spootiferdb.User{DiscordUserID: st}).Preload("SpotifyAuthToken").First(user)

		if tx.Error != nil {
			log.Println("Error fetching user from db: ", tx.Error)
			return
		}

		tok, err := auth.Token(r.Context(), st, r)
		if err != nil {
			http.Error(w, "Couldn't get token", http.StatusForbidden)
			log.Println("Error getting token: ", err)
			return
		}

		user.CreatedAt = time.Now().Format(time.RFC3339)
		user.SpotifyAuthToken.SpotifyRefreshToken = tok.RefreshToken
		user.SpotifyAuthToken.UserId = user.ID
		user.SpotifyAuthToken.SpotifyExpiryTime = tok.Expiry.Format(time.RFC3339)
		user.SpotifyAuthToken.SpotifyAccessToken = tok.AccessToken
		user.SpotifyAuthToken.SpotifyTokenType = tok.TokenType

		// use the token to get an authenticated client

		_, err = spootiferdb.SaveSpotifyAuthToken(db, &(user.SpotifyAuthToken))

		if err != nil {
			fmt.Println("Error saving token to user: ", tx.Error)
			http.Error(w, "Internal Server Error", http.StatusInternalServerError)
		}

		w.WriteHeader(http.StatusOK)
		w.Write([]byte("Login Complete!"))
	}
}
