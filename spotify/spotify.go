package spotify

import (
	"fmt"
	"regexp"
	"strings"
)

const (
	SpotifyDomain   = "open.spotify.com"
	SpotifyAlbumURI = "spotify:album:"
)

var (
	SpotifyAlbumLink = fmt.Sprintf("https://%s/album/", SpotifyDomain)
)

func IsAlbum(link string) bool {
	return strings.Contains(link, SpotifyAlbumLink) || strings.Contains(link, SpotifyAlbumURI)
}

func ContainsSpotifyLink(msg string) bool {
	return strings.Contains(msg, SpotifyDomain)
}

func ExtractIDs(link string) []string {
	// Define a regular expression pattern to match Spotify track IDs
	// Spotify track IDs are 22 characters long and consist of uppercase letters, lowercase letters, and digits
	regex := regexp.MustCompile(`(?:https?://open\.spotify\.com/track/|https?://open\.spotify\.com/album/|spotify:track:|spotify:album:)([a-zA-Z0-9]+)`)

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
