FROM golang:1.20 AS builder
WORKDIR /spootifer
COPY app/go.mod app/go.sum ./
RUN go mod download
COPY app/ ./
RUN go build -buildvcs=false -ldflags "-s -w -extldflags '-static'" -tags osusergo,netgo -o /spootifer .
EXPOSE 8080
EXPOSE 8081
# Copy binaries from the previous build stages.
FROM registry.fly.io/spootifer:orchestra-latest AS worker

FROM alpine
COPY --from=flyio/litefs:0.5 /usr/local/bin/litefs /usr/local/bin/litefs
COPY --from=builder /spootifer/spootifer /spootifer/spootifer
COPY --from=worker /app/worker/worker /spootifer/worker
RUN apk add bash fuse3 sqlite ca-certificates curl

# Copy our LiteFS configuration.
ADD litefs.app.yml litefs.app.yml
ADD litefs.worker.yml litefs.worker.yml
RUN apk add bash fuse3 sqlite ca-certificates curl
ENTRYPOINT ["litefs", "mount", "-config"]