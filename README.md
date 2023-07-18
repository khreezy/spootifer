# Spootifer

Spootifer is a discord bot intended to create archival lists of content links posted in a Discord server or channel

Currently, Spootifer only supports adding to Spotify playlists. In order to develop on Spootifer, you should:

1. Create your own Discord application. 
2. Generate a Bot Token
3. Invite the Bot to a test server you own
4. Create your own Spotify application
5. Set your app .env based on [the example](.env.example) using values from step 2 and 4
6. Set your worker .env.worker based on [the example](.env.worker.example)
7. Run `./run-spooty.sh`

If you need to view Spootifer's logs run `./spootifer-logs.sh`