import time

import requests

from .common import ORIGIN, VERIFY, health, pkce


class API:
    def __init__(self, host, evidence=None):
        self.host = host.rstrip("/")
        self.evidence = evidence
        self.session = requests.Session()
        self.session.trust_env = False
        self.session.verify = VERIFY
        self.session.headers.update({"Origin": ORIGIN})

    def call(self, method, path, body=None, token=None, expected=200, key=None):
        headers = {}
        if token:
            headers["Authorization"] = "Bearer " + token
        if key:
            headers["Idempotency-Key"] = key
        started = time.monotonic()
        if self.evidence:
            self.evidence.emit("request_start", role="setup", method=method, path=path)

        def received_headers(response, **kwargs):
            if self.evidence:
                self.evidence.emit(
                    "response_headers",
                    role="setup",
                    name=path,
                    status=response.status_code,
                    ms=(time.monotonic() - started) * 1000,
                    health=health(response.headers.get("X-Nanacoin-Health")),
                )

        try:
            r = self.session.request(
                method,
                self.host + path,
                json=body,
                headers=headers,
                timeout=(5, 45),
                hooks={"response": received_headers},
            )
            if self.evidence:
                self.evidence.emit(
                    "request",
                    role="setup",
                    name=path,
                    status=r.status_code,
                    ms=(time.monotonic() - started) * 1000,
                    health=health(r.headers.get("X-Nanacoin-Health")),
                )
            if r.status_code != expected:
                raise RuntimeError(f"{method} {path}: HTTP {r.status_code}, expected {expected}")
            return r.json() if r.content else None
        except requests.RequestException as exc:
            if self.evidence:
                self.evidence.emit(
                    "request",
                    role="setup",
                    name=path,
                    status=0,
                    ms=(time.monotonic() - started) * 1000,
                    error=type(exc).__name__,
                )
            raise RuntimeError(f"{method} {path}: {type(exc).__name__}") from None

    def login(self, username, password):
        verifier, challenge = pkce()
        code = self.call(
            "POST",
            "/api/v1/auth/authorize",
            {
                "username": username,
                "password": password,
                "code_challenge": challenge,
                "code_challenge_method": "S256",
                "redirect_uri": ORIGIN + "/cb",
            },
        )
        result = self.call(
            "POST",
            "/api/v1/auth/token",
            {"code": code["code"], "code_verifier": verifier, "redirect_uri": ORIGIN + "/cb"},
        )
        return {"token": result["access_token"], "user": result["user"]}
