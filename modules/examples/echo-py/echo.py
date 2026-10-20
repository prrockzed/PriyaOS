#!/usr/bin/env python3
"""The echo example module in Python, standard library only.

It behaves exactly like the Rust one (modules/examples/echo):

- echo.say {text} -> {text}, with the modules.echo.prefix setting in front;
- echo.emit {text} records the event `said {text}` -> {seq};
- echo.setting {key} -> {value}, reading any setting through priyad.

The protocol (02-core section 5.2) is JSON-RPC 2.0 both ways over stdin and
stdout, each message framed with a Content-Length header. priyad calls
initialize, ping, the module's methods, `event` and shutdown; the module
calls host.* back. Requests are handled on threads of their own, so a ping
is answered while a method waits for priyad. stderr is the module's log.
"""

import json
import sys
import threading

NAME = "echo"
VERSION = "0.1.0"
METHODS = ["echo.say", "echo.emit", "echo.setting"]
MAX_MESSAGE = 16 * 1024 * 1024
MAX_HEADERS = 4096
MAX_IN_FLIGHT = 64

# One application error code per kind, as priya_api::rpc does.
CODES = {
    "not_found": -32001,
    "invalid_input": -32002,
    "conflict": -32003,
    "permission_denied": -32004,
    "unavailable": -32005,
    "timeout": -32006,
    "cancelled": -32007,
    "provider_error": -32008,
    "internal": -32603,
}


def log(message):
    print(message, file=sys.stderr, flush=True)


class ApiError(Exception):
    """An error with a kind and a plain message, as priyad expects."""

    def __init__(self, kind, user_message, detail=None, retryable=None):
        super().__init__(user_message)
        self.data = {
            "kind": kind,
            "retryable": kind in ("unavailable", "timeout") if retryable is None else retryable,
            "user_message": user_message,
        }
        if detail is not None:
            self.data["detail"] = detail

    @classmethod
    def from_data(cls, data):
        error = cls("internal", "")
        error.data = data
        return error

    def to_rpc(self):
        return {"code": CODES.get(self.data["kind"], -32603), "message": self.data["kind"], "data": self.data}


def wrong_params(detail):
    return ApiError("invalid_input", "That request has a wrong or missing value.", detail)


class Stream:
    """Content-Length framing on stdin and stdout."""

    def __init__(self):
        self.input = sys.stdin.buffer
        self.output = sys.stdout.buffer
        self.lock = threading.Lock()

    def read(self):
        """The next message's body, None at a clean end; raises on garbage."""
        headers = b""
        while not headers.endswith(b"\r\n\r\n"):
            byte = self.input.read(1)
            if not byte:
                if headers:
                    raise ValueError("the stream ended in the middle of a message")
                return None
            headers += byte
            if len(headers) > MAX_HEADERS + 4:
                raise ValueError("the message header is over %d bytes" % MAX_HEADERS)
            if byte == b"\n" and not headers.endswith(b"\r\n"):
                raise ValueError("bad message header: header lines must end in \\r\\n, not just \\n")
        length = None
        for line in headers[:-4].split(b"\r\n"):
            name, colon, value = line.partition(b":")
            if not colon:
                raise ValueError("bad message header: `%s` isn't a header" % line[:40].decode(errors="replace"))
            if name.strip().lower() == b"content-length":
                value = value.strip()
                if not value.isdigit():
                    raise ValueError("bad message header: Content-Length isn't a number")
                if length is not None:
                    raise ValueError("bad message header: Content-Length is given twice")
                length = int(value)
        if length is None:
            raise ValueError("bad message header: there's no Content-Length")
        if length > MAX_MESSAGE:
            raise ValueError("a message of %d bytes is over the %d-byte limit" % (length, MAX_MESSAGE))
        body = self.input.read(length)
        if len(body) < length:
            raise ValueError("the stream ended in the middle of a message")
        return body

    def write(self, message):
        body = json.dumps(message, separators=(",", ":")).encode()
        with self.lock:
            self.output.write(b"Content-Length: %d\r\n\r\n" % len(body))
            self.output.write(body)
            self.output.flush()


class Module:
    def __init__(self, stream):
        self.stream = stream
        self.initialized = False
        self.closing = False
        self.state = threading.Lock()
        self.pending = {}
        self.next_id = 1
        self.in_flight = threading.BoundedSemaphore(MAX_IN_FLIGHT)
        self.workers = []
        # Set when the module should exit: told to, or priyad went away.
        self.done = threading.Event()
        self.broken = None

    # Calls to priyad.

    def call_host(self, method, params):
        with self.state:
            if self.closing:
                raise ApiError("unavailable", "priyad has gone away.")
            call_id = self.next_id
            self.next_id += 1
            waiting = {"done": threading.Event(), "answer": None}
            self.pending[call_id] = waiting
        self.stream.write({"jsonrpc": "2.0", "id": call_id, "method": method, "params": params})
        waiting["done"].wait()
        answer = waiting["answer"]
        if answer is None:
            raise ApiError("unavailable", "priyad has gone away.")
        if "error" in answer:
            raise ApiError.from_data(answer["error"]["data"])
        return answer["result"]

    def answered(self, message):
        with self.state:
            waiting = self.pending.pop(message.get("id"), None)
        if waiting is not None:
            waiting["answer"] = message
            waiting["done"].set()

    def fail_waiting(self):
        with self.state:
            self.closing = True
            waiting, self.pending = list(self.pending.values()), {}
        for call in waiting:
            call["done"].set()

    # priyad's requests.

    def dispatch(self, method, params):
        if method == "initialize":
            if self.initialized:
                raise ApiError("conflict", "The module has already started.")
            if not isinstance(params, dict):
                raise wrong_params("params must be named (an object)")
            self.initialized = True
            return {"api": "v1", "name": NAME, "version": VERSION, "methods": METHODS}
        if method == "ping":
            return {}
        if method == "shutdown":
            with self.state:
                self.closing = True
            return {}
        if not self.initialized:
            raise ApiError("unavailable", "The module is still starting.")
        if method == "event":
            return {}
        if method == "echo.say":
            text = text_param(params, "text")
            prefix = self.call_host("host.settings.get", {"key": "modules.echo.prefix"})["value"]
            if not isinstance(prefix, str):
                prefix = ""
            return {"text": prefix + text}
        if method == "echo.emit":
            text = text_param(params, "text")
            seq = self.call_host("host.emit_event", {"custom_type": "said", "payload": {"text": text}})["seq"]
            return {"seq": seq}
        if method == "echo.setting":
            key = text_param(params, "key")
            return {"value": self.call_host("host.settings.get", {"key": key})["value"]}
        raise ApiError("not_found", "This module doesn't know that request.", "no method named %s" % method)

    def handle(self, request):
        try:
            outcome = {"result": self.dispatch(request["method"], request.get("params"))}
        except ApiError as error:
            outcome = {"error": error.to_rpc()}
        except Exception as error:  # A bug here mustn't leave priyad waiting.
            outcome = {"error": ApiError("internal", "Something went wrong inside the module.", str(error)).to_rpc()}
        finally:
            self.in_flight.release()
        self.stream.write(dict({"jsonrpc": "2.0", "id": request["id"]}, **outcome))
        if request["method"] == "shutdown":
            self.done.set()

    def read(self):
        """Reads priyad's messages until the end, on a thread of its own."""
        try:
            while True:
                body = self.stream.read()
                if body is None:
                    break
                try:
                    message = json.loads(body)
                except ValueError as error:
                    raise ValueError("a message isn't JSON: %s" % error)
                if not isinstance(message, dict):
                    raise ValueError("batches aren't used between priyad and modules")
                if "method" not in message:
                    self.answered(message)
                elif self.closing:
                    continue  # Told to stop: no new work.
                elif "id" not in message:
                    log("ignored a notification: %s" % message["method"])
                elif not self.in_flight.acquire(blocking=False):
                    busy = ApiError("unavailable", "Too many requests at once; try again.")
                    self.stream.write({"jsonrpc": "2.0", "id": message["id"], "error": busy.to_rpc()})
                else:
                    worker = threading.Thread(target=self.handle, args=(message,), daemon=True)
                    self.workers.append(worker)
                    worker.start()
        except ValueError as error:
            self.broken = str(error)
        finally:
            self.done.set()

    def run(self):
        """Serves until told to stop or priyad goes away; the exit code."""
        threading.Thread(target=self.read, daemon=True).start()
        self.done.wait()
        # No answer can come any more; let the work in flight finish.
        self.fail_waiting()
        for worker in self.workers:
            worker.join(timeout=5)
        if self.broken is not None:
            log(self.broken)
            return 1
        return 0


def text_param(params, name):
    """The one text field a method takes; nothing else is allowed."""
    if params is None:
        params = {}
    if not isinstance(params, dict):
        raise wrong_params("params must be named (an object)")
    unknown = sorted(set(params) - {name})
    if unknown:
        raise wrong_params("unknown field `%s`, expected `%s`" % (unknown[0], name))
    if not isinstance(params.get(name), str):
        raise wrong_params("missing field `%s`" % name)
    return params[name]


def main():
    return Module(Stream()).run()


if __name__ == "__main__":
    sys.exit(main())
