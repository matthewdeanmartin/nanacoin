"""Failure injection for the evidence collector; no board or credentials needed."""
import asyncio
import json
from pathlib import Path
import tempfile
import unittest

from bottleneck import Evidence, Experiment, Wire, health, parser, summarize
from evidence_report import growth_rows, report


class WireTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.evidence = Evidence(Path(self.tmp.name) / "run")
        self.servers = []
        self.connections = []

    async def asyncTearDown(self):
        for writer in self.connections:
            writer.close()
        for server in self.servers:
            server.close()
            await server.wait_closed()
        self.evidence.file.close()
        self.tmp.cleanup()

    async def wire(self, reply):
        async def handle(reader, writer):
            self.connections.append(writer)
            await reader.readuntil(b"\r\n\r\n")
            await reply(reader, writer)
        server = await asyncio.start_server(handle, "127.0.0.1", 0)
        self.servers.append(server)
        return Wire("127.0.0.1", server.sockets[0].getsockname()[1], self.evidence, .15)

    def events(self):
        return [json.loads(line) for line in
                (self.evidence.directory / "events.jsonl").read_text().splitlines()]

    async def test_health_survives_body_timeout_and_socket_closes(self):
        closed = asyncio.Event()

        async def reply(reader, writer):
            writer.write(b"HTTP/1.1 200 OK\r\nX-Nanacoin-Health: free 012345 obj 00042 gc 00003\r\nContent-Length: 99\r\n\r\n{")
            await writer.drain()
            await reader.read()
            closed.set()
        wire = await self.wire(reply)
        r = await wire.request("GET", "/api/v1/status")
        self.assertFalse(r["ok"])
        self.assertEqual((r["status"], r["stage"], r["bytes"]), (200, "body", 1))
        self.assertEqual(r["health"]["free"], 12345)
        await asyncio.wait_for(closed.wait(), 1)
        events = self.events()
        self.assertEqual(events[1]["kind"], "response_headers")
        self.assertEqual(events[1]["health"]["obj"], 42)

    async def test_truncated_length_not_success(self):
        async def reply(reader, writer):
            writer.write(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n{}")
            await writer.drain()
            writer.close()
        r = await (await self.wire(reply)).request("GET", "/")
        self.assertFalse(r["ok"])
        self.assertIn("truncated", r["error"])

    async def test_auth_secrets_not_saved(self):
        async def reply(reader, writer):
            writer.write(b'HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{"access_token":"secret-response"}')
            await writer.drain()
            writer.close()
        r = await (await self.wire(reply)).request("POST", "/api/v1/auth/token",
            token="secret-header", body={"password": "secret-password"})
        self.assertTrue(r["ok"])
        self.assertEqual(r["json"]["access_token"], "secret-response")
        self.assertNotIn("secret-", json.dumps(self.events()))

    async def test_chunked_and_expected_refusal(self):
        async def reply(reader, writer):
            writer.write(b'HTTP/1.1 404 Not Found\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n')
            await writer.drain()
            writer.close()
        r = await (await self.wire(reply)).request("GET", "/missing", expected=(404,))
        self.assertTrue(r["ok"])
        self.assertEqual(r["json"], {})

    async def test_malformed_json_is_anomaly(self):
        async def reply(reader, writer):
            writer.write(b'HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{')
            await writer.drain()
        r = await (await self.wire(reply)).request("GET", "/")
        self.assertFalse(r["ok"])
        self.assertEqual(r["error"], "invalid JSON response")

    async def test_empty_success_is_not_valid_json(self):
        async def reply(reader, writer):
            writer.write(b'HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n')
            await writer.drain()
        r = await (await self.wire(reply)).request("GET", "/")
        self.assertFalse(r["ok"])
        self.assertEqual(r["error"], "empty JSON response")

    async def test_final_snapshot_failure_fails_run(self):
        exp = Experiment(parser().parse_args([]), self.evidence)
        class FakeWire:
            async def request(self, *args, **kwargs):
                return {"ok": False, "id": 1}
        exp.wire = FakeWire()
        await exp.snapshot("after_quiet")
        self.assertEqual(exp.anomaly["reason"], "snapshot_unavailable")

    async def test_low_headroom_stops_load(self):
        async def reply(reader, writer):
            writer.write(b'HTTP/1.1 200 OK\r\nX-Nanacoin-Health: free 000512\r\nContent-Length: 2\r\n\r\n{}')
            await writer.drain()
        exp = Experiment(parser().parse_args([]), self.evidence)
        exp.wire = await self.wire(reply)
        await exp.call("GET", "/")
        self.assertTrue(exp.stop.is_set())
        self.assertEqual(exp.anomaly["reason"], "low_headroom")

    async def test_uptime_drop_detected(self):
        exp = Experiment(parser().parse_args([]), self.evidence)
        exp.last_uptime = 500
        class FakeWire:
            async def request(self, *args, **kwargs):
                return {"ok": True, "id": 1, "json": {"uptime_seconds": 2}}
        exp.wire = FakeWire()
        await exp.snapshot("test")
        self.assertEqual(exp.anomaly["reason"], "uptime_decreased")

    async def test_listing_corruption_is_not_success(self):
        exp = Experiment(parser().parse_args([]), self.evidence)
        async def call(*args, **kwargs):
            return {"ok": True, "id": 1, "json": {"id": "listing-1", "description": ""}}
        exp.call = call
        await exp.operation("listing-churn", 0)
        self.assertEqual(exp.anomaly["reason"], "listing_roundtrip_mismatch")


class SummaryTests(unittest.TestCase):
    def test_growth_boundary_decodes_target_uintptr_hex(self):
        rows = growth_rows([
            "ledger-growth txns before len 128 cap 128 elem 0x00000038 buffer-bytes 0x00003800 free 42000",
            "ledger-growth txns after len 129 cap 256 elem 0x00000038 buffer-bytes 0x00003800 free 27000",
            "ledger-growth txns before len 256 cap 256 elem 0x00000038 buffer-bytes 0x00007000 free 37000",
        ])
        self.assertEqual(rows[0]["after"], 256)
        self.assertEqual(rows[1]["bytes"], 28672)
        self.assertEqual(rows[1]["element"], 56)
        self.assertEqual(rows[1]["after"], "no after marker")

    def test_zero_and_unknown_are_different(self):
        self.assertEqual(health("free 000000 blk 0100 gc 00002"), {"free": 0, "blk": 100, "gc": 2})
        self.assertEqual(health(""), {})
        self.assertIsNone(summarize([])["min_free"])

    def test_report_recovers_partial_host_log(self):
        with tempfile.TemporaryDirectory() as directory:
            events = Path(directory) / "events.jsonl"
            events.write_text(json.dumps({"kind": "response_headers", "id": 7,
                "phase": "issue-new/c1", "stage": "body", "health": {"free": 12345}})
                + '\n{"kind":', encoding="utf-8")
            text = report(directory).read_text(encoding="utf-8")
            self.assertIn('"id": 7', text)
            self.assertIn('12345', text)
            self.assertIn('Ignored 1 malformed event', text)


if __name__ == "__main__":
    unittest.main()
