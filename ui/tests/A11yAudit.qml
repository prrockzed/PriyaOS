import QtQuick

// The runtime accessibility check every component test runs (0.11, DS-06):
// every item a user can Tab to has a role and a spoken name, and Tab visits
// them in reading order (top to bottom, then left to right).
//
//     A11yAudit { id: audit }
//     compare(audit.problems(scene), [])
QtObject {
    // Items closer than this vertically count as one row.
    property real rowTolerance: 4

    // Every visible, enabled item under `root` that Tab can reach.
    function focusable(root) {
        const found = [];
        const walk = item => {
            if (!item.visible || !item.enabled)
                return;
            if (item.activeFocusOnTab)
                found.push(item);
            for (let i = 0; i < item.children.length; ++i)
                walk(item.children[i]);
        };
        walk(root);
        return found;
    }

    // What is wrong, as readable lines; empty when all is well.
    function problems(root) {
        const out = [];
        const items = focusable(root);
        for (const item of items) {
            const label = String(item);
            if (!item.Accessible.name)
                out.push(label + " has no Accessible.name");
            if (item.Accessible.role === Accessible.NoRole)
                out.push(label + " has no Accessible.role");
        }
        if (items.length < 2)
            return out;

        const position = item => item.mapToItem(root, 0, 0);
        const readingOrder = items.slice().sort((a, b) => {
            const pa = position(a);
            const pb = position(b);
            if (Math.abs(pa.y - pb.y) > rowTolerance)
                return pa.y - pb.y;
            return pa.x - pb.x;
        });
        // Follow Tab from the first item in reading order.
        const tabOrder = [readingOrder[0]];
        let next = readingOrder[0].nextItemInFocusChain(true);
        while (next && next !== readingOrder[0] && tabOrder.length <= items.length) {
            if (items.indexOf(next) >= 0)
                tabOrder.push(next);
            next = next.nextItemInFocusChain(true);
        }
        for (let i = 0; i < readingOrder.length; ++i) {
            if (tabOrder[i] !== readingOrder[i]) {
                out.push("Tab order differs from reading order at step " + (i + 1) + ": Tab goes to " + tabOrder[i] + ", reading order expects " + readingOrder[i]);
                break;
            }
        }
        return out;
    }
}
