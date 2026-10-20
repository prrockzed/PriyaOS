// PriyaClient (0.13). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import "../../PriyaUI/api/Api.js" as Api
import "../../PriyaUI/client/PriyaClient.js" as Rpc

TestCase {
    id: test

    name: "PriyaClient"

    // Intent steps aren't in main's Api.js until priyad serves them.
    readonly property string intentStream: "v1.intent.stream"

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
}
