package boardhttp

import (
	"strings"
	"testing"
)

// The bug this guards: OPTIONS was registered only for non-GET paths, so the
// browser's preflight of an authenticated GET to /me or /status hit no route,
// got a 404 with no CORS headers, and was reported as a missing
// Access-Control-Allow-Origin. Every path needs a preflight route.
func TestEveryPathHasAPreflight(t *testing.T) {
	preflights := make(map[string]bool, len(PreflightPaths()))
	for _, p := range PreflightPaths() {
		preflights[p] = true
	}

	for _, route := range MethodRoutes() {
		method, path, ok := splitRoute(route)
		if !ok {
			t.Errorf("route %q is not %q", route, "METHOD /path")
			continue
		}
		if !preflights[path] {
			t.Errorf("%s %s has no OPTIONS route - a browser preflighting it gets a 404", method, path)
		}
	}
}

// Registering the same OPTIONS pattern twice is a conflict, and several paths
// appear under more than one method (/listings is GET and POST).
func TestPreflightPathsAreUnique(t *testing.T) {
	seen := make(map[string]bool, len(PreflightPaths()))
	for _, p := range PreflightPaths() {
		if seen[p] {
			t.Errorf("path %q appears twice in PreflightPaths", p)
		}
		seen[p] = true
	}
}

// A preflight path with no corresponding route would register OPTIONS for
// something the API does not serve.
func TestPreflightPathsAllHaveRoutes(t *testing.T) {
	paths := make(map[string]bool, len(MethodRoutes()))
	for _, route := range MethodRoutes() {
		if _, path, ok := splitRoute(route); ok {
			paths[path] = true
		}
	}
	for _, p := range PreflightPaths() {
		if !paths[p] {
			t.Errorf("PreflightPaths has %q, which no route serves", p)
		}
	}
}

func TestRoutesAreWellFormed(t *testing.T) {
	methods := map[string]bool{"GET": true, "POST": true, "PATCH": true}

	for _, route := range MethodRoutes() {
		method, path, ok := splitRoute(route)
		if !ok {
			t.Errorf("route %q is malformed", route)
			continue
		}
		if !methods[method] {
			// OPTIONS is added separately, and the API uses no other verbs.
			t.Errorf("route %q uses unexpected method %q", route, method)
		}
		if !strings.HasPrefix(path, APIPrefix+"/") {
			t.Errorf("route %q is not under %s", route, APIPrefix)
		}
		// httphi matches exactly, so a trailing slash would be a distinct
		// pattern that nothing requests.
		if strings.HasSuffix(path, "/") {
			t.Errorf("route %q has a trailing slash", route)
		}
	}
}

// The board's routes must cover the same API the desktop serves. This pins the
// count so that adding an endpoint to internal/api without adding it here
// fails a test rather than 404ing on the board only.
func TestRouteCountMatchesTheAPI(t *testing.T) {
	const want = 39
	if got := len(MethodRoutes()); got != want {
		t.Errorf("MethodRoutes has %d entries, want %d - if you added or removed an "+
			"endpoint in internal/api, update both and this count", got, want)
	}
}

func splitRoute(route string) (method, path string, ok bool) {
	i := strings.IndexByte(route, ' ')
	if i <= 0 || i == len(route)-1 {
		return "", "", false
	}
	return route[:i], route[i+1:], true
}
