// Command nanacoin runs the NanaCoin server.
//
// This is the desktop/local build. It is the reference implementation: the
// behaviour defined here is what the eventual on-device build must match, and
// the conformance tests in internal/api run against exactly this handler.
package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/api"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/flashlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
)

func main() {
	var (
		addr     = flag.String("addr", ":8080", "listen address")
		journal  = flag.String("journal", "nanacoin.journal", "path to the append-only journal; empty for in-memory")
		capacity = flag.Int64("capacity", 0, "journal capacity in bytes; 0 for unbounded. Set this to the target flash partition size to rehearse a full journal locally.")
		// 4200 is `ng serve` for the Angular client, 8080 this server serving
		// the vanilla one, 5173 Vite. A browser origin that is missing here
		// gets a 200 with no Access-Control-Allow-Origin header and reports
		// the server as unreachable, so the defaults cover the local clients.
		origins  = flag.String("origins", "http://localhost:4200,http://localhost:8080,http://localhost:5173", "comma-separated allowed CORS origins")
		webDir   = flag.String("web", "web", "directory of static client files; empty to serve API only")
		tokenTTL = flag.Duration("token-ttl", 8*time.Hour, "access token lifetime")
		// Desktop RAM is not the constraint the board's is, so logging stays
		// on by default here. The flag exists so the no-logs client path -
		// a hidden Logs tab, a /logs that 404s - can be exercised without
		// reflashing a board to see it.
		//
		// This is a flag and the board's equivalent is a build tag on
		// purpose: there, turning the ring off has to remove the array at
		// compile time to reclaim anything at all.
		logs = flag.Bool("logs", true, "record server events and serve /api/v1/logs")
	)
	flag.Parse()

	svc, err := openService(*journal, *capacity)
	if err != nil {
		log.Fatalf("nanacoin: %v", err)
	}
	defer svc.Close()

	sessions := auth.NewStore(auth.Options{TokenTTL: *tokenTTL})

	status := svc.Status()
	srv := api.NewServer(svc, sessions, api.Config{
		AllowedOrigins: splitOrigins(*origins),
		NoLog:          !*logs,
		// Provisioning stays open until a Nana exists. There is no window
		// in which a provisioned household can be re-provisioned: the
		// service refuses that regardless of this flag.
		AllowProvision: true,
	})

	mux := http.NewServeMux()
	mux.Handle("/api/", srv.Handler())
	if *webDir != "" {
		// The client is served from here for convenience in development.
		// Production may host it anywhere - that is what the CORS config is
		// for - and the API does not depend on being the page's origin.
		mux.Handle("/", http.FileServer(http.Dir(*webDir)))
	}

	httpSrv := &http.Server{
		Addr:    *addr,
		Handler: mux,
		// A microcontroller cannot afford a connection held open by a slow
		// or dead client, and neither can a laptop running the same code.
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       30 * time.Second,
		WriteTimeout:      30 * time.Second,
		IdleTimeout:       60 * time.Second,
	}

	log.Printf("nanacoin: %s, %d users, %d transactions, %d in circulation",
		status.Household, status.Users, status.Transactions, status.Circulation)
	if !status.Provisioned {
		log.Printf("nanacoin: not provisioned - POST /api/v1/provision to create the first Nana")
	}
	log.Printf("nanacoin: allowed browser origins: %s", strings.Join(splitOrigins(*origins), ", "))
	log.Printf("nanacoin: listening on %s", *addr)

	// Shut down on a signal so the journal is closed cleanly. An unclean
	// exit is survivable by design, but there is no reason to rely on that
	// when the user pressed Ctrl-C.
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	go func() {
		if err := httpSrv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("nanacoin: %v", err)
		}
	}()

	<-ctx.Done()
	log.Printf("nanacoin: shutting down")

	shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := httpSrv.Shutdown(shutdownCtx); err != nil {
		log.Printf("nanacoin: shutdown: %v", err)
	}
}

func openService(path string, capacity int64) (*core.Service, error) {
	var j storage.Journal
	if path == "" {
		log.Printf("nanacoin: in-memory journal - nothing will be saved")
		j = memory.New()
	} else {
		f, err := flashlog.Open(path, capacity)
		if err != nil {
			return nil, fmt.Errorf("opening journal %s: %w", path, err)
		}
		j = f
	}
	return core.New(j, core.Options{})
}

func splitOrigins(s string) []string {
	var out []string
	for _, o := range strings.Split(s, ",") {
		if o = strings.TrimSpace(o); o != "" {
			out = append(out, o)
		}
	}
	return out
}
