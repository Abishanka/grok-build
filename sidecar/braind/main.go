// braind — Code Jam brain worker (Phase 4 spine).
//
//	session context → distill/query → X search/recent (+ optional x_search)
//	→ rank/digest → plane=research cards → jamd
//
//	cd sidecar && make jam   # terminal 1
//	cd sidecar && make braind  # terminal 2
//
// Env (from repo ../.env — never print secrets):
//
//	JAM_HOST                 default http://127.0.0.1:7720
//	JAM_ID                   default demo
//	JAM_TOKEN                write auth for POST /jam/:id/cards
//	X_BEARER_TOKEN           X API app bearer (missing → HOLD/skip X search)
//	XAI_API_KEY              optional chat digest + optional x_search
//	BRAIN_ENABLE_XAI_SEARCH  set 1 to enable background xAI x_search path
//	BRAIN_XAI_SEARCH_TIMEOUT default 25s
//	BRAIN_DEBOUNCE           default 10s
//	BRAIN_POLL_INTERVAL      default 3s
//	BRAIN_MAX_CARDS          default 5
package main

import (
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/config"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/worker"
)

func main() {
	cfg := config.FromEnv()
	w := worker.New(cfg)

	if err := w.Jam.Health(); err != nil {
		log.Printf("braind warning: jamd health check failed (%v) — will keep polling %s", err, cfg.JamHost)
	} else {
		log.Printf("braind connected to jamd at %s jam_id=%s", cfg.JamHost, cfg.JamID)
	}

	stop := make(chan struct{})
	go func() {
		ch := make(chan os.Signal, 1)
		signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)
		<-ch
		close(stop)
	}()

	w.Loop(stop)
	log.Printf("braind exit")
}
