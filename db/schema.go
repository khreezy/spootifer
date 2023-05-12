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
