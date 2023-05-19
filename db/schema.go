package spootiferdb

import (
	"gorm.io/gorm"
)

type User struct {
	gorm.Model
	ID               int
	DiscordUserID    string
	UserGuilds       []UserGuild
	SpotifyAuthToken SpotifyAuthToken
	DeletedAt        *string
	CreatedAt        string
	UpdatedAt        string
}

// Represents a table of spotify auth tokens associated with users

type SpotifyAuthToken struct {
	gorm.Model
	UserId              int
	SpotifyRefreshToken string
	SpotifyAccessToken  string
	SpotifyExpiryTime   string
	SpotifyTokenType    string
	DeletedAt           *string
	CreatedAt           string
	UpdatedAt           string
}

type UserGuild struct {
	gorm.Model
	UserID            int
	DiscordGuildID    string
	SpotifyPlaylistID string
	User              User
	DeletedAt         *string
	CreatedAt         string
	UpdatedAt         string
}

type MessageLink struct {
	gorm.Model
	Link         string
	MessageID    string
	GuildID      string
	ChannelID    string
	Acknowledged bool
	LinkType     string
	TrackAdds    []SpotifyTrackAdd
}

type SpotifyTrackAdd struct {
	gorm.Model
	SpotifyTrackID    string
	SpotifyPlaylistID string
	MessageLinkID     int
	MessageLink       MessageLink
}
