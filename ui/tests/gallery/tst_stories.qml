// Every gallery story: it loads, and its controls pass the accessibility
// audit (names, roles, Tab order), so each story works with the keyboard
// alone (0.11). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import "../../gallery/Catalog.js" as Catalog
import ".."

Item {
    id: scene

    width: 800
    height: 700

    A11yAudit {
        id: audit
    }

    TestCase {
        name: "Stories"
        when: windowShown

        function init() {
            Theme.reducedMotion = true;
            Theme.mode = "dark";
        }

        function cleanup() {
            Theme.reducedMotion = false;
        }

        function test_a11y_audit_all_stories_data() {
            return Catalog.components.map(name => ({
                        "tag": name,
                        "component": name
                    }));
        }

        // covers: DS-06, QA-09
        // suite: ui
        // acceptance: 0.11
        function test_a11y_audit_all_stories(row) {
            const component = Qt.createComponent(Qt.resolvedUrl("../../gallery/stories/" + row.component + ".stories.qml"));
            compare(component.status, Component.Ready, component.errorString());
            const book = component.createObject(scene);
            verify(book !== null);
            compare(book.component, row.component);
            verify(book.stories.length > 0, "has stories");
            let controls = 0;
            for (let i = 0; i < book.stories.length; ++i) {
                const story = book.stories[i];
                story.parent = scene;
                waitForRendering(story);
                compare(audit.problems(story), [], row.component + "/" + story.name);
                // Tab reaches every control from outside the story.
                const focusable = audit.focusable(story);
                controls += focusable.length;
                for (const item of focusable) {
                    scene.forceActiveFocus();
                    let reached = false;
                    for (let step = 0; step <= focusable.length && !reached; ++step) {
                        keyClick(Qt.Key_Tab);
                        reached = item.activeFocus;
                    }
                    verify(reached, row.component + "/" + story.name + ": Tab never reaches " + item);
                }
                story.parent = null;
            }
            book.destroy();
            // Only these have nothing to operate.
            verify(controls > 0 || ["Icon", "Skeleton"].includes(row.component), "the audit saw no controls");
        }

        // covers: DS-04
        // suite: ui
        function test_every_stories_file_is_in_the_catalog() {
            // Every component the gallery lists has a stories file with a
            // matching name; the shot harness checks the other direction.
            for (const name of Catalog.components) {
                const component = Qt.createComponent(Qt.resolvedUrl("../../gallery/stories/" + name + ".stories.qml"));
                compare(component.status, Component.Ready, name + ": " + component.errorString());
            }
            compare(Catalog.parseArgs(["qml", "--story", "Button/hover", "--all", "--theme", "dark"]), {
                "story": "Button/hover",
                "all": true,
                "theme": "dark"
            });
        }
    }
}
