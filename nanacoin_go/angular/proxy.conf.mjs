// Dev-server proxy for the NanaCoin API.
//
// Everything under /api is forwarded to whichever NanaCoin you name, so the
// browser only ever talks to localhost:4200. That is the whole point: a
// same-origin request has no CORS check, so the board does not need to know
// this site exists and does not need reflashing to add an origin.
//
//   npm start                                  -> the Go server on :8080
//   NANACOIN=192.168.1.158 npm start           -> the board
//   $env:NANACOIN="192.168.1.158"; npm start   -> the board (PowerShell)
//
// Use this rather than entering the board's address in the site's connect
// screen when the board's origin list does not include this page. The connect
// screen is for the deployed case, where the site is served from an origin the
// board already allows.

const raw = (process.env['NANACOIN'] ?? 'localhost:8080').trim();

// Accept what someone would type: a bare host, a host with a port, or a full
// URL. Default to http, since the board serves plain HTTP.
const target = /^https?:\/\//i.test(raw) ? raw : `http://${raw}`;

console.log(`[proxy] /api -> ${target}`);

export default {
  '/api': {
    target,
    secure: false,
    // Rewrite the Host header to the target. Without this the board would see
    // "localhost:4200", which is harmless here but misleading in its logs.
    changeOrigin: true,
    // The board is slow to answer its first request after a reboot - the TCP
    // connection alone can take half a minute - and the default proxy timeout
    // would give up before it ever replied.
    proxyTimeout: 60_000,
    timeout: 60_000,
    // Say which side failed. A proxy error here means the board did not
    // answer; without this the browser just sees a bare 504 and the site
    // reports "could not reach NanaCoin", which is true but hides that the
    // proxy is the thing that gave up.
    on: {
      error(err, _req, res) {
        console.error(`[proxy] ${target} did not answer: ${err.message}`);
        if (res && 'writeHead' in res && !res.headersSent) {
          res.writeHead(504, { 'Content-Type': 'application/json' });
          res.end(
            JSON.stringify({
              error: 'unreachable',
              message: `The dev proxy could not reach NanaCoin at ${target}.`,
            }),
          );
        }
      },
    },
  },
};
