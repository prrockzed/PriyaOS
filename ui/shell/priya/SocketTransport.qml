import Quickshell
import Quickshell.Io

// PriyaClient's transport in the shell (0.13): priyad's socket, one JSON
// value per line. A failed connect reports `closed` too, as PriyaClient
// expects; a second report of the same loss is ignored there.
Socket {
    id: socket

    signal opened
    signal closed
    signal received(string line)

    function open() {
        connected = true;
    }

    function send(line) {
        write(line + "\n");
        flush();
    }

    path: Quickshell.env("XDG_RUNTIME_DIR") + "/priya/priyad.sock"
    onConnectedChanged: connected ? opened() : closed()
    onError: closed()

    parser: SplitParser {
        onRead: data => socket.received(data)
    }
}
