pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Window
import QtQuick.Controls.Basic as T
import PriyaUI
import "Catalog.js" as Catalog

// The PriyaUI gallery (0.11): every component in every state, in light and
// dark. `cargo xtask gallery` opens it.
//
// Shot mode renders stories to PNG files and quits, with animations off:
//   --story <Component>/<state> --theme light|dark --out <file.png>
//   --all --out-dir <dir>       every story in light and dark, as
//                               <dir>/<Component>/<state>/<theme>.png
//   --list                      prints every story id and quits
// Scale 2.0 comes from QT_SCALE_FACTOR=2, set by the caller.
Window {
    id: window

    readonly property var args: Catalog.parseArgs(Qt.application.arguments)
    readonly property bool shotMode: args.story !== undefined || args.all === true || args.list === true
    property string current: Catalog.components[0]
    property bool hindi: false

    width: 1280
    height: 860
    visible: true
    color: Theme.bgBase
    title: "PriyaUI gallery"

    Component.onCompleted: {
        // Motion off first: a theme change would otherwise fade, and a
        // picture could catch it halfway.
        if (shotMode)
            Theme.reducedMotion = true;
        if (args.theme)
            Theme.mode = args.theme;
        if (shotMode)
            shooter.start();
    }

    function loadBook(name) {
        const component = Qt.createComponent(Qt.resolvedUrl("stories/" + name + ".stories.qml"));
        if (component.status !== Component.Ready) {
            console.error("can't load the stories of " + name + ": " + component.errorString());
            return null;
        }
        // Parented to a hidden holder, so items have a window from the start.
        return component.createObject(holder, {
            "hindi": window.hindi
        });
    }

    // ---- Shot mode ------------------------------------------------------

    QtObject {
        id: shooter

        // [{component, state, theme, out}]
        property var queue: []
        property var book: null
        property var story: null
        property var job: null
        property int failures: 0
        // Frames drawn since the current story was placed.
        property int frames: -1

        function start() {
            if (window.args.list) {
                for (const name of Catalog.components) {
                    const book = window.loadBook(name);
                    if (book === null) {
                        Qt.exit(1);
                        return;
                    }
                    for (let i = 0; i < book.stories.length; ++i)
                        console.info("story: " + name + "/" + book.stories[i].name);
                    book.destroy();
                }
                Qt.exit(0);
                return;
            }
            const jobs = [];
            if (window.args.all) {
                const themes = window.args.theme ? [window.args.theme] : ["light", "dark"];
                for (const name of Catalog.components) {
                    const book = window.loadBook(name);
                    if (book === null) {
                        Qt.exit(1);
                        return;
                    }
                    for (let i = 0; i < book.stories.length; ++i)
                        for (const theme of themes)
                            jobs.push({
                                "component": name,
                                "state": book.stories[i].name,
                                "theme": theme,
                                "out": window.args["out-dir"] + "/" + name + "/" + book.stories[i].name + "/" + theme + ".png"
                            });
                    book.destroy();
                }
            } else {
                const [name, state] = String(window.args.story).split("/");
                jobs.push({
                    "component": name,
                    "state": state,
                    "theme": window.args.theme || "light",
                    "out": window.args.out
                });
            }
            queue = jobs;
            next();
        }

        function next() {
            if (book !== null)
                book.destroy();
            book = null;
            story = null;
            if (queue.length === 0) {
                Qt.exit(failures > 0 ? 1 : 0);
                return;
            }
            job = queue[0];
            queue = queue.slice(1);
            Theme.mode = job.theme;
            book = window.loadBook(job.component);
            story = book ? book.story(job.state) : null;
            if (story === null) {
                console.error("no story " + job.component + "/" + job.state);
                ++failures;
                Qt.callLater(next);
                return;
            }
            story.parent = shotFrame;
            story.width = story.implicitWidth;
            window.width = Math.ceil(story.width);
            window.height = Math.ceil(story.implicitHeight);
            // Two frames: layouts settle in the first, the second shows them.
            frames = 0;
            window.requestUpdate();
        }

        function capture() {
            const done = shooter.job;
            // The whole window, so popups (tooltips) are in the picture.
            window.contentItem.grabToImage(result => {
                if (!result.saveToFile(done.out)) {
                    console.error("can't write " + done.out);
                    ++shooter.failures;
                }
                shooter.next();
            }, Qt.size(window.width, window.height));
        }
    }

    Connections {
        target: window

        function onFrameSwapped() {
            if (shooter.frames < 0)
                return;
            shooter.frames += 1;
            if (shooter.frames >= 2) {
                shooter.frames = -1;
                shooter.capture();
            } else {
                window.requestUpdate();
            }
        }
    }

    Item {
        id: holder

        visible: false
    }

    Rectangle {
        id: shotFrame

        visible: window.shotMode
        width: window.width
        height: window.height
        color: Theme.bgBase
    }

    // ---- Interactive gallery --------------------------------------------

    Item {
        anchors.fill: parent
        visible: !window.shotMode

        Rectangle {
            id: sidebar

            width: Theme.space48 * 4
            height: parent.height
            color: Theme.bgSurface1

            Column {
                x: Theme.space12
                y: Theme.space16
                width: parent.width - Theme.space24
                spacing: Theme.space2

                Text {
                    text: "PriyaUI"
                    font: Theme.title
                    color: Theme.textPrimary
                    bottomPadding: Theme.space12
                    Accessible.role: Accessible.Heading
                    Accessible.name: text
                }

                Repeater {
                    model: Catalog.components

                    delegate: T.ItemDelegate {
                        id: entry

                        required property string modelData

                        width: parent.width
                        height: Theme.space32
                        text: modelData
                        font: Theme.body
                        focusPolicy: Qt.StrongFocus
                        Accessible.name: modelData
                        Accessible.role: Accessible.ListItem
                        onClicked: window.current = modelData

                        contentItem: Text {
                            leftPadding: Theme.space8
                            verticalAlignment: Text.AlignVCenter
                            text: entry.text
                            font: entry.font
                            color: window.current === entry.modelData ? Theme.textPrimary : Theme.textSecondary
                        }

                        background: Rectangle {
                            radius: Theme.radiusControl
                            color: window.current === entry.modelData ? Theme.accentSubtle : entry.hovered ? Theme.bgSurface3 : "transparent"
                        }

                        FocusRing {
                            shown: entry.visualFocus
                        }
                    }
                }
            }
        }

        Column {
            id: toolbar

            x: sidebar.width + Theme.space24
            y: Theme.space16
            spacing: Theme.space8

            Row {
                spacing: Theme.space8

                Chip {
                    selectable: true
                    selected: Theme.mode === "light"
                    text: "Light"
                    onClicked: Theme.mode = "light"
                }

                Chip {
                    selectable: true
                    selected: Theme.mode === "dark"
                    text: "Dark"
                    onClicked: Theme.mode = "dark"
                }

                Chip {
                    selectable: true
                    selected: Theme.scale === 2
                    text: "Text size 200 %"
                    onClicked: Theme.scale = Theme.scale === 2 ? 1 : 2
                }

                Chip {
                    selectable: true
                    selected: window.hindi
                    text: "Hindi samples"
                    onClicked: window.hindi = !window.hindi
                }

                Chip {
                    selectable: true
                    selected: Theme.reducedMotion
                    text: "Reduced motion"
                    onClicked: Theme.reducedMotion = !Theme.reducedMotion
                }
            }
        }

        Flickable {
            id: page

            x: toolbar.x
            y: toolbar.y + toolbar.height + Theme.space16
            width: parent.width - x - Theme.space16
            height: parent.height - y
            contentHeight: shelf.height + Theme.space32
            clip: true

            T.ScrollBar.vertical: T.ScrollBar {}

            Loader {
                id: shelf

                width: page.width
                sourceComponent: shelfComponent
                // Reload when the component or the sample language changes.
                property string key: window.current + (window.hindi ? "/hi" : "")
                onKeyChanged: {
                    active = false;
                    active = true;
                }
            }
        }
    }

    Component {
        id: shelfComponent

        Column {
            id: shelfColumn

            property var book: window.loadBook(window.current)

            spacing: Theme.space16
            Component.onDestruction: {
                if (book)
                    book.destroy();
            }

            Text {
                text: window.current
                font: Theme.headline
                color: Theme.textPrimary
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }

            Text {
                width: shelfColumn.width - Theme.space16
                text: shelfColumn.book ? shelfColumn.book.summary : ""
                wrapMode: Text.Wrap
                font: Theme.small
                color: Theme.textSecondary
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Repeater {
                model: shelfColumn.book ? shelfColumn.book.stories.length : 0

                delegate: Row {
                    id: storyRow

                    required property int index
                    readonly property var story: shelfColumn.book.stories[index]

                    spacing: Theme.space16

                    Text {
                        width: Theme.space48 * 3
                        topPadding: Theme.space16 + Theme.space8
                        text: storyRow.story.title || storyRow.story.name
                        font: Theme.caption
                        color: Theme.textTertiary
                        wrapMode: Text.Wrap
                    }

                    Item {
                        id: slot

                        width: storyRow.story.implicitWidth
                        height: storyRow.story.implicitHeight

                        Component.onCompleted: {
                            storyRow.story.parent = slot;
                            storyRow.story.width = Qt.binding(() => slot.width);
                        }
                    }
                }
            }
        }
    }
}
