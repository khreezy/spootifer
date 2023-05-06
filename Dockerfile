FROM golang:1.20
WORKDIR /spootifer
COPY go.mod go.sum ./
COPY *.go ./
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -a -o /spootifer .
EXPOSE 8080
EXPOSE 8081
ENTRYPOINT ["/spootifer/spootifer"]
