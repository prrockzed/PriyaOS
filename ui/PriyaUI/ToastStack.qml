pragma ComponentBehavior: Bound

import QtQuick

// The toasts on screen, newest at the bottom, at most `maxVisible` (11 §1:
// calm, at most 3). More wait in a queue until one leaves.
Item {
    id: root

    property int maxVisible: 3
    property int timeout: 4000
    readonly property alias count: shown.count
    readonly property int queued: waiting.count

    signal actionTriggered(string id)

    implicitWidth: Theme.space48 * 8 - Theme.space4
    implicitHeight: column.implicitHeight

    // Shows a toast: {id, kind, title, body, actionText}.
    function show(toast) {
        const entry = {
            "toastId": toast.id ?? String(Date.now() + Math.random()),
            "kind": toast.kind ?? "info",
            "title": toast.title ?? "",
            "body": toast.body ?? "",
            "actionText": toast.actionText ?? ""
        };
        if (shown.count < maxVisible)
            shown.append(entry);
        else
            waiting.append(entry);
    }

    function dismiss(index) {
        shown.remove(index);
        if (waiting.count > 0) {
            shown.append(waiting.get(0));
            waiting.remove(0);
        }
    }

    ListModel {
        id: shown
    }

    ListModel {
        id: waiting
    }

    Column {
        id: column

        width: root.width
        spacing: Theme.space8

        Repeater {
            model: shown

            delegate: Toast {
                required property int index
                required property var model

                width: column.width
                kind: model.kind
                title: model.title
                body: model.body
                actionText: model.actionText
                timeout: root.timeout
                onDismissed: root.dismiss(index)
                onActionTriggered: {
                    root.actionTriggered(model.toastId);
                    root.dismiss(index);
                }
            }
        }
    }
}
