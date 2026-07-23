# Terminal viewport navigation

## Problem

Moving through the file explorer or a long diff changes the selected item, but the visible viewport does not follow it. File navigation also wraps at the ends, which makes the list feel infinite.

## Chosen behavior

- `j` and Down stop at the last item; `k` and Up stop at the first item.
- The file explorer keeps the active file visible as selection moves.
- The diff keeps the active row visible as the cursor moves.
- Changing files resets the diff cursor and viewport to the first row.
- Existing shortcuts and persisted session fields remain compatible.

## Approaches considered

1. Keep wraparound and add scrollbars. This improves orientation but retains the infinite-loop feeling.
2. Clamp navigation and synchronize each viewport with its selection. This is the selected approach because it matches editor behavior and requires only local state/rendering changes.
3. Add independent scrolling separate from selection. This is more flexible but introduces extra commands and state that are unnecessary for the current workflow.

## Design

The reducer will clamp file and diff selection at their boundaries. Viewport positioning will be derived from the selected index and the panel's visible height during rendering, keeping the selected item within the usable inner area. The file explorer will use Ratatui list state to position its viewport, while the diff will calculate a bounded vertical offset before rendering the paragraph.

## Verification

Reducer tests will prove navigation does not wrap. TUI tests will prove selections below the first screen remain visible in both panels and snapshot tests will cover the visual result. A release build and a real PR session will validate keyboard behavior end to end.
