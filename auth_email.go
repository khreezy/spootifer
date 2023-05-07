package main

import (
	"github.com/sendgrid/sendgrid-go"
	"github.com/sendgrid/sendgrid-go/helpers/mail"
	"log"
	"os"
)

const (
	linkKey = "link"
)

func sendAuthEmail(authURL string) error {
	log.Println("Constructing email")
	m := mail.NewV3Mail()

	from := mail.NewEmail("Spooty", os.Getenv("SPOOTIFER_FROM_EMAIL"))
	subject := "Your Spootifer Authorization Link!"
	to := mail.NewEmail("User", os.Getenv("SPOOTIFER_TO_EMAIL"))

	m.SetFrom(from)
	m.SetTemplateID(os.Getenv("SENDGRID_TEMPLATE_ID"))

	p := mail.NewPersonalization()
	p.AddTos(to)

	p.Subject = subject
	p.To = []*mail.Email{to}
	p.From = from

	p.SetDynamicTemplateData(linkKey, authURL)

	m.AddPersonalizations(p)

	request := sendgrid.GetRequest(os.Getenv("SENDGRID_API_KEY"), "/v3/mail/send", "https://api.sendgrid.com")
	request.Method = "POST"

	client := sendgrid.NewSendClient(os.Getenv("SENDGRID_API_KEY"))

	_, err := client.Send(m)

	if err != nil {
		return err
	}

	log.Println("Successfully sent email")

	return nil
}
