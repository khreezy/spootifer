package main

import (
	"context"
	"github.com/bwmarrin/discordgo"
	"github.com/sashabaranov/go-openai"
	"log"
)

func generateChatGptResponse(_ context.Context, chatClient *openai.Client, discordClient *discordgo.Session, discordMessage *discordgo.MessageCreate) error {
	log.Println("Sending chatGPT response")
	resp, err := chatClient.CreateChatCompletion(context.Background(), openai.ChatCompletionRequest{
		Model: openai.GPT3Dot5Turbo,
		Messages: []openai.ChatCompletionMessage{
			{
				Role:    openai.ChatMessageRoleSystem,
				Content: "You're a potty-mouthed record store owner.",
			},
			{
				Role:    openai.ChatMessageRoleSystem,
				Content: "Someone has a sent a song to you. Choose how you feel about it at random, then response to it in 1-3 sentences.",
			},
			{
				Role:    openai.ChatMessageRoleSystem,
				Content: "Don't prefix the response with any content as if you were anything but the record store owner.",
			},
			{
				Role:    openai.ChatMessageRoleSystem,
				Content: "Do not tell me you understand the request before performing the request and do not tell me you are randomly choosing something.",
			},
		},
	})

	if err != nil {
		return err
	} else {
		msg := resp.Choices[0].Message.Content

		log.Println("Sending discord reply")

		_, err := discordClient.ChannelMessageSendReply(discordMessage.ChannelID, msg, &discordgo.MessageReference{ChannelID: discordMessage.ChannelID, MessageID: discordMessage.ID})

		if err != nil {
			return err
		}
	}

	return nil
}
