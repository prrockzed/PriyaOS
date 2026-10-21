import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import PriyaUI

// The Priya bar (0.13, UX-02): a layer-shell panel at the top centre. It is
// made at start and only shown or hidden, to open in under 100 ms. What it
// shows is PriyaBarContent; what it does is BarSession.
//
// Quickshell's type info for qmllint lacks a few things that exist at run
// time: PanelWindow is made through its backend, and `margins` and the
// screen's size aren't described. Those lines say so.
PanelWindow { // qmllint disable uncreatable-type
    id: bar

    // Clear room around the bar for its overlay shadow (blur 40, 12 down
    // in dark); clicks there go through to what is behind.
    readonly property real sideRoom: Theme.space48
    readonly property real topRoom: Theme.space16
    readonly property real bottomRoom: Theme.space48 + Theme.space16
    // When an open was asked for, until its first frame is drawn.
    property real openAsked: 0

    function open() {
        if (visible)
            return;
        openAsked = Date.now();
        visible = true;
        content.input.editor.forceActiveFocus();
    }

    function close() {
        visible = false;
    }

    // Hidden until Super+Space; the previous answer stays for next time.
    visible: false
    anchors.top: true
    // 96 px from the top to the bar itself, as in the mockup.
    margins.top: Theme.space48 * 2 - topRoom // qmllint disable missing-property
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: Math.min(session.barWidth, (screen ? screen.width : session.barWidth) * 0.6) + 2 * sideRoom // qmllint disable missing-property
    implicitHeight: topRoom + content.implicitHeight + bottomRoom
    color: "transparent"
    mask: Region {
        item: content
    }
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "priya-bar"
    // Always exclusive: a hidden panel has no surface to take focus with,
    // and asking from the start is what compositors honour on mapping.
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

    PriyaBarContent {
        id: content

        x: bar.sideRoom
        y: bar.topRoom
        width: bar.width - 2 * bar.sideRoom
        maxHeight: (bar.screen ? bar.screen.height : 900) * 0.7 // qmllint disable missing-property
    }

    PriyaClient {
        id: priyad

        transport: SocketTransport {}
    }

    BarSession {
        id: session

        client: priyad
        view: content
        onCloseRequested: bar.close()
    }

    // A click outside closes the bar; an answer still streaming goes on
    // and is there when it opens again.
    HyprlandFocusGrab {
        windows: [bar]
        active: bar.visible
        onCleared: bar.close()
    }

    // `qs -p /usr/share/priya/shell ipc call priya-bar toggle`
    IpcHandler {
        target: "priya-bar"

        function toggle(): void {
            if (bar.visible)
                bar.close();
            else
                bar.open();
        }

        function open(): void {
            bar.open();
        }

        function close(): void {
            bar.close();
        }

        // What the bar shows, as JSON, for the journey tests
        // (ui/tests/journeys) to check against.
        function state(): string {
            return JSON.stringify({
                "visible": bar.visible,
                "connected": priyad.state === "connected",
                "phase": content.phase,
                "question": content.question,
                "answer": content.answer,
                "input": content.input.text,
                "theme": Theme.mode
            });
        }
    }

    // At debug level (0.13 perf): open_requested → first_frame.
    Connections {
        target: bar.contentItem.Window.window
        enabled: bar.openAsked > 0

        function onFrameSwapped() {
            console.debug(perf, "open_requested → first_frame:", Date.now() - bar.openAsked, "ms");
            bar.openAsked = 0;
        }
    }

    LoggingCategory {
        id: perf

        name: "priya.bar.perf"
        defaultLogLevel: LoggingCategory.Warning
    }
}
