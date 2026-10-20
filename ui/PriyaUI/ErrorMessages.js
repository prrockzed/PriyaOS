.pragma library

// What the UI says about a failed request (0.11, DS-05). priyad's errors
// carry a stable `data.kind` (crates/priya-api/src/error.rs); the UI shows
// its own translated words for each kind and falls back to the daemon's
// English `data.user_message` for kinds it doesn't know. Daemon strings stay
// English until DS-20.

// {title, body} for a kind, or null.
function forKind(kind) {
    switch (kind) {
    case "invalid_input":
        return {
            "title": qsTr("That request didn't work"),
            "body": qsTr("Something in it wasn't right. Check it, then try again.")
        };
    case "not_found":
        return {
            "title": qsTr("Priya couldn't find that"),
            "body": qsTr("It may have been moved or removed.")
        };
    case "conflict":
        return {
            "title": qsTr("Something changed in the meantime"),
            "body": qsTr("Try again to use the latest version.")
        };
    case "permission_denied":
        return {
            "title": qsTr("Priya isn't allowed to do that"),
            "body": qsTr("Check the permissions, then try again.")
        };
    case "unavailable":
        return {
            "title": qsTr("Priya can't reach what it needs"),
            "body": qsTr("Check your connection, then try again.")
        };
    case "timeout":
        return {
            "title": qsTr("That took too long"),
            "body": qsTr("Try again in a moment.")
        };
    case "cancelled":
        return {
            "title": qsTr("Stopped"),
            "body": qsTr("Nothing more will happen.")
        };
    case "provider_error":
        return {
            "title": qsTr("The model ran into a problem"),
            "body": qsTr("Try again, or pick another model.")
        };
    case "internal":
        return {
            "title": qsTr("Something went wrong inside Priya"),
            "body": qsTr("Try again. If it keeps happening, copy the details and report it.")
        };
    }
    return null;
}

// A JSON-RPC error object ({code, message, data: {kind, retryable,
// user_message}}) as {title, body, details, retryable} for ErrorState.
function describe(error) {
    const data = (error && error.data) || {};
    const known = forKind(data.kind);
    const details = [];
    if (data.kind)
        details.push("kind: " + data.kind);
    if (error && error.code !== undefined)
        details.push("code: " + error.code);
    if (error && error.message)
        details.push("message: " + error.message);
    return {
        "title": known ? known.title : (data.user_message || qsTr("Something went wrong")),
        "body": known ? known.body : "",
        "details": details.join("\n"),
        "retryable": data.retryable === true
    };
}
