FROM golang:1.20 AS builder
WORKDIR /spootifer
COPY . ./
#COPY **/*.go ./
RUN go build -buildvcs=false -ldflags "-s -w -extldflags '-static'" -tags osusergo,netgo -o /spootifer .
EXPOSE 8080
EXPOSE 8081
# Copy binaries from the previous build stages.
FROM alpine
COPY --from=flyio/litefs:main /usr/local/bin/litefs /usr/local/bin/litefs
COPY --from=builder /spootifer/spootifer /spootifer/spootifer
RUN apk add bash fuse3 sqlite ca-certificates curl

# Copy our LiteFS configuration.
ADD litefs.yml litefs.yml
RUN apk add bash fuse3 sqlite ca-certificates curl
ENTRYPOINT litefs mount
