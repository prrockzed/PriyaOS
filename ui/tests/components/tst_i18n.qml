// Translations (0.11, DS-05): longer, accented text never gets cut, and
// API errors are described in the UI's own (translatable) words. Run with
// `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import "../../PriyaUI/ErrorMessages.js" as ErrorMessages
import "../Pseudo.js" as Pseudo
import ".."

Item {
    id: scene

    width: 900
    height: 700

    Column {
        x: 20
        y: 20
        spacing: 16

        Row {
            spacing: 8

            Button {
                id: button

                variant: "primary"
                iconName: "check"
                text: Pseudo.pseudo("Keep changes")
            }

            Button {
                id: small

                size: "small"
                text: Pseudo.pseudo("Copy answer")
            }

            Chip {
                id: chip

                text: Pseudo.pseudo("Balanced")
                selectable: true
            }
        }

        ErrorState {
            id: error

            width: 420
            title: Pseudo.pseudo("Priya can't reach the model")
            body: Pseudo.pseudo("Your internet connection seems to be off. Connect, then try again.")
            details: "kind: unavailable"
        }
    }

    TestCase {
        name: "I18n"
        when: windowShown

        // Every visible Text under `root` shows all of its text inside
        // `root`'s width.
        function clipped(root) {
            const found = [];
            const stack = [root];
            while (stack.length) {
                const item = stack.pop();
                if (!item.visible)
                    continue;
                if (item.truncated !== undefined && item.text !== undefined && item.text !== "") {
                    const left = item.mapToItem(root, 0, 0).x;
                    const shownWidth = item.wrapMode === Text.NoWrap ? item.implicitWidth : item.contentWidth;
                    if (item.truncated)
                        found.push("cut: " + item.text);
                    else if (left < -0.5 || left + shownWidth > root.width + 0.5)
                        found.push("outside: " + item.text);
                }
                for (let i = 0; i < item.children.length; ++i)
                    stack.push(item.children[i]);
            }
            return found;
        }

        // covers: DS-05
        // suite: ui
        // acceptance: 0.11
        function test_pseudo_locale_does_not_clip_button_chip_errorstate() {
            compare(Pseudo.pseudo("Undo"), "[Üñdö ~]", "accents and 40 % more");
            waitForRendering(error);
            compare(clipped(button), []);
            compare(clipped(small), []);
            compare(clipped(chip), []);
            compare(clipped(error), []);
            verify(button.width > button.implicitHeight * 3, "the button grew with its label");
        }

        // covers: DS-05
        // suite: ui
        function test_api_errors_use_the_uis_words_and_fall_back_to_user_message() {
            const kinds = ["invalid_input", "not_found", "conflict", "permission_denied", "unavailable", "timeout", "cancelled", "provider_error", "internal"];
            for (const kind of kinds) {
                const shown = ErrorMessages.forKind(kind);
                verify(shown !== null && shown.title !== "" && shown.body !== "", kind);
            }
            const unavailable = ErrorMessages.describe({
                "code": -32001,
                "message": "dns error",
                "data": {
                    "kind": "unavailable",
                    "retryable": true,
                    "user_message": "Can't reach Anthropic."
                }
            });
            compare(unavailable.title, "Priya can't reach what it needs");
            verify(unavailable.retryable);
            compare(unavailable.details, "kind: unavailable\ncode: -32001\nmessage: dns error");
            const future = ErrorMessages.describe({
                "code": -32099,
                "data": {
                    "kind": "rate_limited",
                    "user_message": "Slow down a little."
                }
            });
            compare(future.title, "Slow down a little.", "an unknown kind shows the daemon's words");
            compare(future.body, "");
            verify(!future.retryable);
            compare(ErrorMessages.describe(null).title, "Something went wrong");

            error.showApiError({
                "data": {
                    "kind": "timeout",
                    "retryable": true
                }
            });
            compare(error.title, "That took too long");
            verify(error.showRetry);
        }
    }
}
