// Route lists for the board's HTTP router.
//
// These live in a file with no build constraint so that a normal `go test` can
// check them, even though the adapter itself only builds under TinyGo. The
// route list is exactly the part that has already been wrong once - OPTIONS
// was missing on GET-only paths, which broke every authenticated request from
// a browser - so it is worth testing on a machine that can run tests.
package boardhttp

// APIPrefix is the version prefix every route shares.
const APIPrefix = "/api/v1"

// MethodRoutes is every method-and-path pair the board serves, in httphi
// pattern form.
//
// It mirrors internal/api/routes.go. That duplication is the price of httphi
// not having net/http's subtree matching: httphi matches patterns exactly, so
// every shape has to be named. It is a list of paths rather than a second
// implementation - the wrapped handler still does all the routing and all the
// logic.
func MethodRoutes() []string {
	return []string{
		"GET " + APIPrefix + "/status",
		"GET " + APIPrefix + "/logs",
		"GET " + APIPrefix + "/diag",
		"POST " + APIPrefix + "/provision",

		"POST " + APIPrefix + "/auth/authorize",
		"POST " + APIPrefix + "/auth/token",
		"POST " + APIPrefix + "/auth/logout",

		"GET " + APIPrefix + "/me",

		"GET " + APIPrefix + "/users",
		"POST " + APIPrefix + "/users",
		"PATCH " + APIPrefix + "/users/{id}",

		"GET " + APIPrefix + "/accounts/{id}",
		"GET " + APIPrefix + "/accounts/{id}/transactions",

		"POST " + APIPrefix + "/transfers",

		"GET " + APIPrefix + "/transactions",
		"GET " + APIPrefix + "/transactions/{id}",
		"POST " + APIPrefix + "/transactions/{id}/reverse",

		"POST " + APIPrefix + "/admin/issue",
		"POST " + APIPrefix + "/admin/retire",
		"GET " + APIPrefix + "/admin/config",
		"PATCH " + APIPrefix + "/admin/config",

		"GET " + APIPrefix + "/listings",
		"POST " + APIPrefix + "/listings",
		"GET " + APIPrefix + "/listings/{id}",
		"PATCH " + APIPrefix + "/listings/{id}",
		"POST " + APIPrefix + "/listings/{id}/purchase",
		"POST " + APIPrefix + "/listings/{id}/cancel",
		"POST " + APIPrefix + "/listings/{id}/offers",

		"GET " + APIPrefix + "/offers",
		"POST " + APIPrefix + "/offers/{id}/accept",
		"POST " + APIPrefix + "/offers/{id}/unaccept",
		"POST " + APIPrefix + "/offers/{id}/decline",
		"POST " + APIPrefix + "/offers/{id}/withdraw",

		"GET " + APIPrefix + "/quotes",
		"POST " + APIPrefix + "/quotes",
		"GET " + APIPrefix + "/quotes/{id}",
		"POST " + APIPrefix + "/quotes/{id}/take",
		"POST " + APIPrefix + "/quotes/{id}/cancel",

		"POST " + APIPrefix + "/admin/issue-usd",
	}
}

// PreflightPaths is every distinct path, for registering OPTIONS.
//
// Every path needs one, GET-only paths included: a GET carrying an
// Authorization header is not a CORS "simple request", so a browser
// preflights it. A path with no OPTIONS route answers 404, a 404 carries no
// CORS headers, and the browser reports a missing Access-Control-Allow-Origin
// - which looks like a CORS misconfiguration rather than a missing route. That
// is exactly how /me and /status broke.
//
// Deduplicated, because several paths appear under more than one method above
// and registering the same OPTIONS pattern twice is a conflict.
func PreflightPaths() []string {
	seen := make(map[string]bool, 32)
	var out []string
	for _, r := range MethodRoutes() {
		path := r
		for i := 0; i < len(r); i++ {
			if r[i] == ' ' {
				path = r[i+1:]
				break
			}
		}
		if seen[path] {
			continue
		}
		seen[path] = true
		out = append(out, path)
	}
	return out
}
