import QtQuick

// A PriyaClient transport for tests (0.13): records what the client sends
// and lets a test play priyad's side.
QtObject {
    id: mock

    // Whether open() succeeds.
    property bool accepting: true
    // Whether open() reports at once, as Quickshell's Socket can (it
    // connects while the shell is still loading), or later.
    property bool synchronous: false
    property bool isOpen: false
    // Every open() call, as Date.now() times.
    property var opens: []
    // Every line sent, parsed.
    property var sent: []

    signal opened
    signal closed
    signal received(string line)

    function open() {
        opens = opens.concat([Date.now()]);
        if (synchronous)
            report();
        else
            Qt.callLater(report);
    }

    function report() {
        isOpen = accepting;
        if (accepting)
            opened();
        else
            closed();
    }

    function send(line) {
        sent = sent.concat([JSON.parse(line)]);
    }

    // The last request sent with this method.
    function lastRequest(method) {
        for (let i = sent.length - 1; i >= 0; i--)
            if (sent[i].method === method)
                return sent[i];
        return null;
    }

    function reply(id, result) {
        received(JSON.stringify({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        }));
    }

    function fail(id, data) {
        received(JSON.stringify({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32000,
                "message": data.kind,
                "data": data
            }
        }));
    }

    function notify(method, params) {
        received(JSON.stringify({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }));
    }

    // priyad goes away.
    function drop() {
        isOpen = false;
        closed();
    }

    function reset() {
        accepting = true;
        opens = [];
        sent = [];
    }
}
