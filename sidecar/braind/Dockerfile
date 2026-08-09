# Code Jam brain worker.
# Build from braind/ (Railway service root) OR from sidecar/:
#   docker build -f braind/Dockerfile -t sidecar-braind ./braind
#   docker build -f braind/Dockerfile -t sidecar-braind .   # if context is braind/
#
# No public HTTP — posts research cards to jamd over private network.
FROM golang:1.25-alpine AS build
WORKDIR /src
RUN apk add --no-cache ca-certificates git
COPY go.mod ./
COPY . ./
RUN CGO_ENABLED=0 go build -o /braind .

FROM alpine:3.20
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=build /braind /app/braind
ENV JAM_HOST=http://jamd.railway.internal:7717
ENV JAM_ID=demo
ENV BRAIN_POLL_INTERVAL=5s
ENV BRAIN_DEBOUNCE=15s
CMD ["/app/braind"]
