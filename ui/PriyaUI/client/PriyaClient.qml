import QtQuick
import "PriyaClient.js" as Rpc

// Talks to priyad: JSON-RPC over NDJSON through an injected transport
// (0.13, UX-02). The shell passes a Quickshell socket, tests a mock, so
// PriyaUI stays free of Quickshell.
//
// A transport has open() and send(line), and the signals opened(),
// closed() (after a failed open too) and received(line), one line each.
// Set it once: the client doesn't move a live connection to another one.
QtObject {
    id: client

    property var transport
    // "connecting" until connected, or until a retry fails too: a short
    // blip never shows as unavailable. Then "unavailable" until it's back.
    readonly property string state: internal.state

    // A notification that no intent or subscription watches.
    signal notified(string method, var params)

    // Sends a request; exactly one callback runs, later. onError gets
    // priyad's error data {kind, retryable, user_message}. While
    // connecting, the request waits for the connection; while
    // unavailable, it fails at once.
    function request(method, params, onResult, onError) {
        if (internal.state === "connected") {
            transport.send(internal.session.request(method, params, onResult, onError));
        } else if (internal.state === "connecting") {
            internal.waiting.push([method, params, onResult, onError]);
        } else {
            Qt.callLater(() => {
                if (typeof onError === "function")
                    onError(Rpc.unavailable());
            });
        }
    }

    // Steps of an intent (by intent_id) or events of a subscription (by
    // its id) go to handler(params, method). A watch ends by itself after
    // an intent's last step or a subscription's `ended`, and every watch
    // ends with the connection.
    function watch(key, handler) {
        internal.session.watch(key, handler);
    }

    function unwatch(key) {
        internal.session.unwatch(key);
    }

    // Starts connecting; called by itself once the transport is set.
    function start() {
        if (!transport || internal.opening || internal.state === "connected")
            return;
        internal.retry.stop();
        internal.opening = true;
        transport.open();
    }

    onTransportChanged: start()

    // Not part of the API.
    property QtObject internals: QtObject {
        id: internal

        property string state: "connecting"
        property bool opening: false
        property int attempt: 0
        property var session: new Rpc.Session()
        // Requests made while connecting, as request()'s arguments.
        property var waiting: []

        property Timer retry: Timer {
            onTriggered: client.start()
        }

        property Connections link: Connections {
            target: client.transport

            function onOpened() {
                internal.opening = false;
                internal.attempt = 0;
                internal.state = "connected";
                const waiting = internal.waiting;
                internal.waiting = [];
                waiting.forEach(args => client.request(...args));
            }

            function onClosed() {
                // A transport may report one loss twice (an error, then
                // the close); only the first counts.
                if (!internal.opening && internal.state !== "connected")
                    return;
                internal.opening = false;
                internal.retry.interval = Rpc.retryDelay(internal.attempt);
                internal.retry.restart();
                internal.state = internal.attempt === 0 ? "connecting" : "unavailable";
                internal.attempt += 1;
                // Callbacks run last, so one that asks again sees the new
                // state.
                if (internal.state === "unavailable") {
                    const waiting = internal.waiting;
                    internal.waiting = [];
                    waiting.forEach(args => client.request(...args));
                }
                internal.session.disconnected();
            }

            function onReceived(line) {
                const unwatched = internal.session.receive(line);
                if (unwatched !== null)
                    client.notified(unwatched.method, unwatched.params);
            }
        }
    }
}
