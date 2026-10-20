.pragma library

// The JSON-RPC side of PriyaClient (0.13, UX-02): request ids, replies, and
// notifications routed to whoever watches their intent or subscription.
// Pure logic with no transport, so it runs the same in the shell and in
// tests. Wire format: docs/claude/architecture/02-core.md §1 and §4.

// Reconnect waits: 250 ms, doubling, at most 5 s.
var FIRST_RETRY_MS = 250;
var MAX_RETRY_MS = 5000;

function retryDelay(attempt) {
    return Math.min(FIRST_RETRY_MS * Math.pow(2, Math.max(0, attempt)), MAX_RETRY_MS);
}

// The error given to callers when priyad can't be reached, shaped like
// priyad's own error data.
function unavailable() {
    return {
        "kind": "unavailable",
        "retryable": true,
        "user_message": qsTr("Priya's background service isn't running.")
    };
}

// The step kinds after which an intent sends nothing more.
var LAST_STEPS = ["done", "error", "cancelled"];

// The kinds of the standard JSON-RPC codes, for an error without data
// (crates/priya-api/src/rpc.rs, kind_for).
var CODE_KINDS = {
    "-32700": "invalid_input",
    "-32600": "invalid_input",
    "-32602": "invalid_input",
    "-32601": "not_found"
};

function Session() {
    this.nextId = 1;
    // Keyed by ids from the wire, so no inherited keys such as "constructor".
    // id → {onResult, onError}
    this.pending = Object.create(null);
    // intent_id or subscription id → handler(params, method)
    this.watchers = Object.create(null);
}

// Returns the line to send. Exactly one of the callbacks runs later.
Session.prototype.request = function (method, params, onResult, onError) {
    const id = this.nextId++;
    this.pending[id] = {
        "onResult": onResult,
        "onError": onError
    };
    const message = {
        "jsonrpc": "2.0",
        "id": id,
        "method": method
    };
    if (params !== undefined && params !== null)
        message.params = params;
    return JSON.stringify(message);
};

Session.prototype.watch = function (key, handler) {
    this.watchers[key] = handler;
};

Session.prototype.unwatch = function (key) {
    delete this.watchers[key];
};

// Handles one received line. Returns a notification nobody watches, as
// {method, params}, so the caller can pass it on; else null.
Session.prototype.receive = function (line) {
    let message;
    try {
        message = JSON.parse(line);
    } catch (e) {
        console.warn("PriyaClient: ignoring a line that isn't JSON");
        return null;
    }
    if (message === null || typeof message !== "object")
        return null;
    if (message.method !== undefined)
        return this.notify(message.method, message.params || {});
    const waiting = this.pending[message.id];
    if (waiting === undefined)
        return null;
    delete this.pending[message.id];
    if (message.error !== undefined)
        call(waiting.onError, errorData(message.error));
    else
        call(waiting.onResult, message.result);
    return null;
};

Session.prototype.notify = function (method, params) {
    // Intent steps carry `intent_id`; event notifications `subscription`.
    const isIntent = params.intent_id !== undefined;
    const key = isIntent ? params.intent_id : params.subscription;
    const handler = key === undefined ? undefined : this.watchers[key];
    if (handler === undefined)
        return {
            "method": method,
            "params": params
        };
    // After an intent's last step, or a subscription's `ended`, nothing
    // more comes for that key.
    if (isIntent ? LAST_STEPS.indexOf(params.kind) >= 0 : params.reason !== undefined)
        delete this.watchers[key];
    call(handler, params, method);
    return null;
};

// The connection is gone: every waiting request fails, and watched intents
// and subscriptions end, since priyad drops them with the connection.
Session.prototype.disconnected = function () {
    const pending = this.pending;
    this.pending = Object.create(null);
    this.watchers = Object.create(null);
    for (const id in pending)
        call(pending[id].onError, unavailable());
};

// Error data, made up for an error that comes without any: `message` is a
// code's standard text, never plain language, so it only goes in detail.
function errorData(error) {
    if (error && error.data && error.data.kind)
        return error.data;
    const code = error && error.code !== undefined ? String(error.code) : "";
    return {
        "kind": CODE_KINDS[code] || "internal",
        "retryable": false,
        "user_message": qsTr("Something went wrong."),
        "detail": error ? code + " " + (error.message || "") : ""
    };
}

// One broken callback mustn't stop the others, or the client's own
// bookkeeping.
function call(callback, a, b) {
    if (typeof callback !== "function")
        return;
    try {
        callback(a, b);
    } catch (e) {
        console.warn("PriyaClient: a callback failed:", e);
    }
}
