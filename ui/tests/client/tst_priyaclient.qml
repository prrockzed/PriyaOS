// PriyaClient (0.13). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import "../"
import "../../PriyaUI/api/Api.js" as Api
import "../../PriyaUI/client/PriyaClient.js" as Rpc

TestCase {
    id: test

    name: "PriyaClient"

    // Intent steps aren't in main's Api.js until priyad serves them.
    readonly property string intentStream: "v1.intent.stream"

    property MockTransport transport
    property PriyaClient client

    Component {
        id: transportComponent

        MockTransport {}
    }

    Component {
        id: clientComponent

        PriyaClient {}
    }

    function init() {
        transport = createTemporaryObject(transportComponent, test);
        client = createTemporaryObject(clientComponent, test, {
            "transport": transport
        });
    }

    function connected() {
        tryCompare(client, "state", "connected");
    }

    function notify(session, method, params) {
        return session.receive(JSON.stringify({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }));
    }

    function step(intent, seq, kind, text) {
        return {
            "intent_id": intent,
            "seq": seq,
            "kind": kind,
            "text": text
        };
    }

    // covers: UX-02
    // suite: ui
    // acceptance: 0.13
    function test_priyaclient_routes_notifications_by_intent_id() {
        const session = new Rpc.Session();
        const seen = [];
        const unwatched = [];
        const send = (method, params) => {
            const rest = notify(session, method, params);
            if (rest !== null)
                unwatched.push(rest.method + ":" + (rest.params.intent_id || rest.params.subscription));
        };
        session.watch("a", params => seen.push("a" + params.seq + params.kind));
        session.watch("b", params => seen.push("b" + params.seq + params.kind));
        session.watch("c", () => {
            throw new Error("a broken handler");
        });
        session.watch("d", params => seen.push("d" + params.seq + params.kind));
        session.watch("sub-1", (params, method) => seen.push(method + ":" + (params.event ? params.event.type : params.reason)));

        send(intentStream, step("a", 1, "started"));
        send(intentStream, step("b", 1, "started"));
        send(intentStream, step("c", 1, "started"));
        send(intentStream, step("a", 2, "delta", "Hi"));
        send(intentStream, step("a", 3, "usage"));
        send(Api.notifications.eventsEvent, {
            "subscription": "sub-1",
            "event": {
                "type": "setting.changed"
            }
        });
        send(intentStream, step("x", 1, "started"));
        send(intentStream, step("constructor", 1, "started"));
        send(intentStream, step("a", 4, "done"));
        send(intentStream, step("a", 5, "delta", "late"));
        send(intentStream, step("b", 2, "error"));
        send(Api.notifications.eventsEnded, {
            "subscription": "sub-1",
            "reason": "cancelled"
        });
        send(Api.notifications.eventsEvent, {
            "subscription": "sub-1",
            "event": {
                "type": "late"
            }
        });
        session.unwatch("d");
        send(intentStream, step("d", 1, "started"));

        compare(seen, ["a1started", "b1started", "a2delta", "a3usage", "v1.events.event:setting.changed", "a4done", "b2error", "v1.events.ended:cancelled"]);
        compare(unwatched, ["v1.intent.stream:x", "v1.intent.stream:constructor", "v1.intent.stream:a", "v1.events.event:sub-1", "v1.intent.stream:d"], "after an intent's last step or a subscription's end, nothing more is routed");

        // A lost connection ends every watch.
        session.watch("e", () => fail("a dropped watch gets nothing"));
        session.disconnected();
        compare(notify(session, intentStream, step("e", 1, "started")).params.intent_id, "e");
    }

    // covers: UX-02
    // suite: ui
    function test_priyaclient_replies_reach_their_request() {
        const session = new Rpc.Session();
        const results = [];
        const sent = [session.request(Api.methods.settingsGet, {
                "key": "ui.theme"
            }, r => results.push("theme=" + r.value), e => results.push("bad")), session.request(Api.methods.moduleCall, {
                "x": 1
            }, r => results.push("bad"), e => results.push(e.kind + ":" + e.user_message)), session.request(Api.methods.systemStatus, null, r => results.push("bad"), e => results.push(e.kind + ":" + e.detail)), session.request(Api.methods.systemStatus, null, r => results.push("bad"), e => results.push("lost:" + e.kind))].map(line => JSON.parse(line));

        compare(sent[0].jsonrpc, "2.0");
        compare(sent[0].method, "v1.settings.get");
        compare(sent[0].params.key, "ui.theme");
        verify(sent[2].params === undefined, "no params is sent without params");
        verify(new Set(sent.map(m => m.id)).size === sent.length, "every request has its own id");

        const reply = message => session.receive(JSON.stringify(Object.assign({
                "jsonrpc": "2.0"
            }, message)));
        session.receive("not json");
        reply({
            "id": 999,
            "result": {}
        });
        reply({
            "id": sent[1].id,
            "error": {
                "code": -32001,
                "message": "not_found",
                "data": {
                    "kind": "not_found",
                    "retryable": false,
                    "user_message": "No such module."
                }
            }
        });
        reply({
            "id": sent[0].id,
            "result": {
                "value": "dark"
            }
        });
        reply({
            "id": sent[0].id,
            "result": {
                "value": "twice"
            }
        });
        // An error without data, as a module might send.
        reply({
            "id": sent[2].id,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        });
        session.disconnected();
        compare(results, ["not_found:No such module.", "theme=dark", "not_found:-32601 Method not found", "lost:unavailable"]);
        ["unavailable", "internal", "not_found", "invalid_input"].forEach(kind => verify(Api.errorKinds.indexOf(kind) >= 0, kind + " is an API error kind"));
    }

    // covers: UX-02
    // suite: ui
    // acceptance: 0.13
    function test_priyaclient_reconnect_backoff() {
        compare([0, 1, 2, 3, 4, 5, 9].map(Rpc.retryDelay), [250, 500, 1000, 2000, 4000, 5000, 5000]);

        connected();
        transport.accepting = false;
        const dropped = Date.now();
        transport.drop();
        transport.drop();
        compare(client.state, "connecting", "a short blip, even reported twice, doesn't show as unavailable");
        tryVerify(() => transport.opens.length >= 2, 1000);
        tryCompare(client, "state", "unavailable", 100, "a failed retry shows as unavailable");
        tryVerify(() => transport.opens.length >= 4, 3000);
        const opens = transport.opens;
        const gaps = [opens[1] - dropped, opens[2] - opens[1], opens[3] - opens[2]];
        // Timers don't fire early; a busy CI machine may make them late.
        [Rpc.FIRST_RETRY_MS, 500, 1000].forEach((wait, i) => verify(gaps[i] >= wait - 10 && gaps[i] < wait * 1.5 + 100, "retry " + i + " waits " + wait + " ms, not " + gaps[i]));

        let failed = null;
        client.request(Api.methods.systemStatus, null, null, e => failed = e);
        compare(transport.lastRequest("v1.system.status"), null, "nothing is sent while unavailable");
        tryVerify(() => failed !== null);
        compare(failed.kind, "unavailable");

        transport.accepting = true;
        tryCompare(client, "state", "connected", 3000);

        // A dropped connection fails what was sent, and a request made
        // from that failure, or while reconnecting, waits for the
        // connection to come back.
        const outcomes = [];
        client.request(Api.methods.systemStatus, null, null, e => {
            outcomes.push("first " + e.kind + " " + client.state);
            client.request(Api.methods.systemStatus, null, r => outcomes.push("retried"));
        });
        client.watch("a", () => fail("a dropped watch gets nothing"));
        const before = transport.opens.length;
        transport.drop();
        compare(outcomes, ["first unavailable connecting"]);
        tryCompare(client, "state", "connected", 600);
        compare(transport.opens.length, before + 1);
        const retried = transport.lastRequest("v1.system.status");
        transport.reply(retried.id, {});
        compare(outcomes, ["first unavailable connecting", "retried"]);
        transport.notify(intentStream, step("a", 2, "delta", "x"));
    }

    // covers: UX-02
    // suite: ui
    function test_priyaclient_first_connect_failing_shows_unavailable() {
        const down = createTemporaryObject(transportComponent, test, {
            "accepting": false
        });
        const fresh = createTemporaryObject(clientComponent, test, {
            "transport": down
        });
        fresh.start();
        compare(down.opens.length, 1, "one open at a time");
        let failed = null;
        fresh.request(Api.methods.settingsGet, {
            "key": "ui.theme"
        }, null, e => failed = e);
        wait(50);
        compare(fresh.state, "connecting", "the first failure is retried before showing as unavailable");
        verify(failed === null, "a request made while connecting waits");
        tryCompare(fresh, "state", "unavailable", 1000);
        tryVerify(() => failed !== null);
        compare(failed.kind, "unavailable");
        compare(down.sent.length, 0);
    }

    // covers: UX-02
    // suite: ui
    function test_priyaclient_passes_on_what_the_transport_brings() {
        connected();
        const seen = [];
        client.notified.connect((method, params) => seen.push("notified " + params.intent_id));
        client.watch("a", params => seen.push("a " + params.kind));
        client.request(Api.methods.systemStatus, null, r => seen.push("status " + r.version));
        transport.notify(intentStream, step("a", 1, "started"));
        transport.notify(intentStream, step("b", 1, "started"));
        transport.reply(transport.lastRequest("v1.system.status").id, {
            "version": "0.13"
        });
        compare(seen, ["a started", "notified b", "status 0.13"]);
    }

    Component {
        id: inlineComponent

        PriyaClient {
            transport: MockTransport {
                synchronous: true
            }
        }
    }

    // covers: UX-02
    // suite: ui
    // A transport that opens while the client is still being made, as the
    // shell's socket does, still connects it (the journey tests found it
    // stuck at "connecting").
    function test_priyaclient_connects_when_the_transport_opens_at_once() {
        const inline = createTemporaryObject(inlineComponent, test);
        tryCompare(inline, "state", "connected", 1000);
        compare(inline.transport.opens.length, 1);
    }
}
