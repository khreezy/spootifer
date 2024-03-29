package spotify

import (
	"context"
	"fmt"
	spootiferdb "github.com/khreezy/spootifer/db"
	"github.com/zmb3/spotify/v2"
	spotifyauth "github.com/zmb3/spotify/v2/auth"
	"golang.org/x/oauth2"
	"golang.org/x/oauth2/clientcredentials"
	"log"
	"net/http"
	"os"
	"regexp"
	"strings"
	"time"
)

const (
	SpotifyDomain          = "open.spotify.com"
	SpotifyShortenedDomain = "spotify.link"
	SpotifyAlbumURI        = "spotify:album:"
	State                  = "abc123"
	MaxRedirectDepth       = 5
)

var (
	SpotifyAlbumLink = fmt.Sprintf("https://%s/album/", SpotifyDomain)
	redirectURI      = os.Getenv("SPOTIFY_REDIRECT_URI")
	auth             = spotifyauth.New(spotifyauth.WithRedirectURL(redirectURI), spotifyauth.WithScopes(spotifyauth.ScopePlaylistModifyPublic), spotifyauth.WithClientID(os.Getenv("SPOTIFY_CLIENT_ID")), spotifyauth.WithClientSecret(os.Getenv("SPOTIFY_CLIENT_SECRET")))
)

func IsAlbum(link string) bool {
	return strings.Contains(link, SpotifyAlbumLink) || strings.Contains(link, SpotifyAlbumURI)
}

func ContainsSpotifyLink(msg string) bool {
	return strings.Contains(msg, SpotifyDomain) || strings.Contains(msg, SpotifyShortenedDomain)
}

func ExtractIDs(link string) []string {
	// Define a regular expression pattern to match Spotify track IDs
	// Spotify track IDs are 22 characters long and consist of uppercase letters, lowercase letters, and digits
	regex := regexp.MustCompile(`(((?:https?://open\.spotify\.com/track/|https?://open\.spotify\.com/album/|spotify:track:|spotify:album:)([a-zA-Z0-9]+))|https?://spotify.link/[a-zA-Z0-9]+)`)

	// Find the first match in the input link
	matches := regex.FindAllStringSubmatch(link, -1)

	ids := []string{}

	for _, match := range matches {
		if len(match) > 1 {
			if strings.Contains(match[1], "spotify.link") {
				fullURI, err := ExpandSpotifyShortLink(match[1], 0)

				if err != nil {
					log.Println(err.Error())
					continue
				}

				ids = append(ids, ExtractIDs(fullURI)...)
			} else {
				ids = append(ids, match[3])
			}
		}
	}

	log.Println("Got spotify ids: ", ids)

	return ids
}

func GetSpotifyLinks(msg string) []string {
	regex := regexp.MustCompile(`(https?://open\.spotify\.com/track/[a-zA-Z0-9]+|https?://open\.spotify\.com/album/[a-zA-Z0-9]+|spotify:track:|spotify:album:[a-zA-Z0-9]+)`)

	return regex.FindAllString(msg, -1)
}

func ExtractPlaylistID(link string) string {
	// Define a regular expression pattern to match Spotify track IDs
	// Spotify track IDs are 22 characters long and consist of uppercase letters, lowercase letters, and digits
	regex := regexp.MustCompile(`(?:https:\/\/open\.spotify\.com\/playlist\/([a-zA-Z0-9]+))`)

	// Find the first match in the input link
	matches := regex.FindStringSubmatch(link)

	if len(matches) > 1 {
		return matches[1]
	}

	// Return an empty string if no track ID was found
	return ""
}

func GenerateAuthURL(state string) string {
	return auth.AuthURL(state, oauth2.AccessTypeOnline)
}

func ClientFromClientCreds(ctx context.Context) (*spotify.Client, error) {
	config := &clientcredentials.Config{
		ClientID:     os.Getenv("SPOTIFY_CLIENT_ID"),
		ClientSecret: os.Getenv("SPOTIFY_CLIENT_SECRET"),
		TokenURL:     spotifyauth.TokenURL,
	}
	token, err := config.Token(ctx)

	if err != nil {
		return nil, err
	}

	httpClient := spotifyauth.New().Client(ctx, token)

	return spotify.New(httpClient), nil
}

func ClientFromDBToken(token spootiferdb.SpotifyAuthToken) (*spotify.Client, error) {
	expiry, err := time.Parse(time.RFC3339, token.SpotifyExpiryTime)

	if err != nil {
		expiry, err = time.Parse(time.DateTime, token.SpotifyExpiryTime)

		if err != nil {

		}

		return nil, err
	}

	tok := oauth2.Token{
		RefreshToken: token.SpotifyRefreshToken,
		AccessToken:  token.SpotifyAccessToken,
		TokenType:    token.SpotifyTokenType,
		Expiry:       expiry,
	}

	return spotify.New(auth.Client(context.Background(), &tok)), nil
}

func ExpandSpotifyShortLink(link string, depth int) (string, error) {
	if depth >= MaxRedirectDepth {
		return link, nil
	}

	expandedUrl := link

	req, err := http.NewRequest(http.MethodHead, link, nil)

	if err != nil {
		return "", err
	}

	req.Header.Add("User-Agent", "python-requests/2.31.0")
	req.Header.Add("Accept-Encoding", "gzip, deflate")
	req.Header.Add("Accept", "*/*")
	req.Header.Add("Connection", "keep-alive")

	client := &http.Client{}

	resp, err := client.Do(req)

	if err != nil {
		return expandedUrl, err
	}

	expandedUrl = resp.Request.URL.String()

	defer resp.Body.Close()

	if !strings.Contains(expandedUrl, SpotifyDomain) {
		return ExpandSpotifyShortLink(expandedUrl, depth+1)
	}

	return expandedUrl, nil
}
