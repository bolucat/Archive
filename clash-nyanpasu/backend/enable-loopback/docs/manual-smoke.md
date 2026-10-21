# Morning GUI smoke

Run from a **non-administrator PowerShell** on an **unlocked** desktop:

```powershell
Set-Location 'G:\Programs\Rust\enable-loopback'
$env:__COMPAT_LAYER = 'RunAsInvoker'
Start-Process -FilePath '.\target\debug\enableLoopback.exe' -ArgumentList '--lang en'
```

Use `--lang zh-cn`, `zh-tw`, `ja`, `ko`, or `ru` to check the other translations.
Do not invoke Save during this smoke: saving replaces the firewall exemption table.
Undo checkbox changes before a clean close, or choose Exit in the discard dialog.

- [ ] The initial window is 960×600 DIP, has the embedded icon, Mica/theme colors, explanation bar and Learn more link. Resize down to the 640×400 minimum. Explanation, counter/toolbar, table card and status share 24-DIP page margins and 12-DIP section spacing. Toolbar uses plain icon/text buttons with 8-DIP gaps; Save uses the accent style when enabled. No command band, overflow edge or overlay covers the headers. The 280-DIP search box is at the left of the toolbar and the global counter at the right of the status bar. At the default 960 DIP width the toolbar is one row in every language; below 900 DIP the actions move to a second row, right-aligned. Toolbar, list and status remain usable.
- [ ] The table has a theme card fill, 1-DIP stroke and 8-DIP rounded corners. Row fills stay inside its rounded edges. The 32-DIP semibold header uses SubtleFillSecondary; every header cell has a 1-DIP DividerStroke separator. Rows are 40 DIP with 12-DIP text padding and horizontal dividers. Headers align exactly with rows, without inherited ListView inset, including after horizontal scrolling. Vertical scrolling keeps the header fixed. Long values are single-line ellipses; hovering a cell shows its full text. The file column receives spare width in a large window.
- [ ] Drag each header separator, including Binaries, both ways. The 8-DIP hit area shows SizeWestEast on both sides of each divider and captures the pointer; the checkbox column stays fixed and data columns stop at 48 DIP. Releasing outside the header finishes the resize; canceled/lost capture leaves a usable table. Dragging never sorts. The header resizes live and rows update on release/lost capture (260-row component measurements: ~11.1 ms/header update and ~20.9 ms/all-row commit; native layout excluded). Rows and header align after release; copy-column hit testing follows the resized widths. With 260 rows, assess resize responsiveness and check Binaries still fills spare width after maximizing.
- [ ] Scroll through the list and check that rows remain responsive. The row count in the counter stays constant; only visible/nearby WinUI containers are realized.
- [ ] Click a row's text: only that row is selected. Ctrl+click another row adds/removes it. Shift+click selects the inclusive range from the anchor, including reverse ranges. Checkbox clicks toggle exemption without changing the selection.
- [ ] Selection remains visible while another row has keyboard focus. Check light, dark and Windows high contrast. Unselected rows are transparent; hover uses SubtleFillSecondary, selection uses SubtleFillTertiary plus a 3×16-DIP accent pill. A matching current-user row has a TextSecondary dot in Users, independently of selection; copy excludes it. Keyboard focus within a row shows a 2-DIP ring, cleared when focus leaves or pointer input resumes. No opaque row bands or native selection chrome remain.
- [ ] With focus in the list, Shift+Up/Down extends and shrinks the range; plain Up/Down moves to one row. With SelectionMode=None, move across the viewport edge and confirm the focused row scrolls into view. Tab into a checkbox and confirm focus stays there; arrows restore row focus. Space toggles the focused row's exemption; Space on a focused checkbox uses its normal behavior.
- [ ] Ctrl+A selects all visible rows while focus is in the list. Ctrl+C copies selected rows in display order, with a machine-readable LoopbackExempt/LoopbackBlocked prefix and seven tab-separated fields. A single row has no trailing newline; multiple rows end with CRLF.
- [ ] Right-click a text cell in an already-selected row: the selection is preserved. Right-click an unselected row: it becomes the only selection. A WinUI MenuFlyout opens at the clicked cell (check at 100%, 150%, 200% DPI, secondary monitors and after moving between monitors). The popup contains Copy selected rows, Copy selected column, a separator, Exempt none and Exempt all. Copy selected column uses exactly the clicked column, including after horizontal scrolling, and ends each copied value with CRLF.
- [ ] Select a row near the bottom, then click each header. Ordinal sorting changes the order; clicking the same header cycles ascending (up glyph at the right of the header), descending (down glyph), then unsorted (no glyph, relevance order during search, enumeration order otherwise); another header starts ascending. Selection stays attached to SIDs and the first selected row is brought into view.
- [ ] Ctrl+F from a row or toolbar focuses Search apps. Type `store`, then a SID fragment: the list filters live across all seven columns and the localized result count appears after the global exemption counter in the status bar. Try 1–3 whitespace-separated tokens in different columns: every token must match. Upper/lowercase and Unicode lowercase work; placeholder labels never match missing fields. While typing repeatedly with a row previously selected, focus stays in the search box.
- [ ] In the search box, Ctrl+A selects text (not rows). Esc clears the query and restores all rows, hides the result count, and keeps the search box focused. Empty or whitespace-only queries show all rows. A nonsense query shows the centered localized empty-state message with the headers retained. Check the placeholder, result count and empty message in all six locales.
- [ ] Search while the selected sort column is ascending/descending: matches retain that order. Clear the three-state sort: matches rank by weighted relevance, with enumeration order breaking ties. Select several rows, narrow the query, then clear it: hidden selections are pruned and do not return. Ctrl+A and copying from the list use visible rows only, in current display order. Filtering must preserve checked states and dirty edits. Refresh retains the query.
- [ ] F5 and Alt+R refresh. While refreshing, the list and toolbar are disabled and the status bar shows a small progress ring. The completion time uses the current Windows user's long time format. Focus returns to the list.
- [ ] Toggle one checkbox: Save enables and the counter changes immediately. Toggle back: Save disables and the status clears. Without a search, Exempt all/none update every checkbox. With a search active the buttons and context-menu items read "Exempt results / Unexempt results" (localized), change only the visible rows, leave hidden rows untouched (clear the search to confirm), and show the "search results pending" status; with zero matches both are disabled. The exemption counter remains global.
- [ ] With dirty edits, F5/Refresh opens the discard dialog. Cancel is the default: Enter/Escape must preserve edits unless Refresh is explicitly chosen. Confirming Refresh discards edits and reloads the configuration.
- [ ] Ctrl+S and Alt+S trigger Save only when enabled. **Check this shortcut with Save disabled during the smoke.** Never enable it merely to test the shortcut on the real firewall. Successful writes and access-denied behavior are covered by model/dispatch tests; any live write requires a separately authorized configuration test.
- [ ] F1 and Alt+L open the Microsoft Learn loopback article. Clicking Learn more opens the same article. Hovering the link shows `enable-loopback v0.1.0 64bit Windows …` (or `arm64`); leaving restores the previous/current status.
- [ ] Alt+A and Alt+N invoke Exempt all/none. The reactor revision has no AccessKey property, so there are no Alt key-tip overlays. Revert edits with Refresh before finishing.
- [ ] With a dirty checkbox, use the title-bar ×. The window stays open and shows Exit without saving? with Cancel as the default. Cancel preserves the edits and keeps Save enabled. Repeat using Alt+F4, then Cancel again.
- [ ] Toggle back to clean state; both × and Alt+F4 now close without confirmation. Reopen, make a dirty edit and explicitly choose Exit to confirm the discard path.
- [ ] Launch with `manual --lang en`: list initially empty/disabled, Refresh available. Refresh populates it. Launch with `-registry --lang en`: it reads registry mappings and uses `?` for unavailable description/package data.
- [ ] If real orphan exemptions already exist, each SID appears as selectable text in configuration order. Unchecking Keep prompting suppresses subsequent orphan dialogs for the session. Do not create orphans for this smoke or save away existing ones.

Revision 2 validation used recording-adapter events and fork native tests without
injected desktop input. The app was launched for default/maximized screenshots;
Save was never invoked. Captures are `target/ui-rev2-default.png` and
`target/ui-rev2-max.png` (200% DPI). Pointer feel, real modifier/keyboard routing,
popup placement and light/high-contrast themes remain manual checks above.

The previous `scripts/gui-smoke.ps1` uses UIA Selection/Invoke/Toggle and assumes
Single selection. It is a revision-1 harness, not valid for SelectionMode=None.
Do not use it as revision-2 validation or run it while the user is working.
Its active UIA actions were not run during this task.

Search validation (2026-10-20): 51 unit/recording tests pass, including raw-field
weights/qualities, Unicode, null/placeholder exclusion, cross-column AND, relevance
versus active sorting, cache replacement, filtered selection/copy, bulk changes scoped to
visible rows, text-change routing and Esc. The native no-query zh-cn screenshot is
`target/ui-search-empty.png`, captured with the scratchpad `capture-window.ps1`
adapted to the owned PID and without window activation. Live UIA query verification
was initially blocked because Computer Use returned “Computer Use was not approved
to use enableloopback”; no synthesized input or Save was used.
