# Changelog

## 0.1.0 (2026-11-19)


### Features

* preview a theme and apply it to the session or the config file ([0e2392e](https://github.com/fulldecent/zellij-theme-picker-ng/commit/0e2392e6496aabeee934d59f8cd71350a1df1590))

## Changelog

## Unreleased

- A floating pane lists the themes built into Zellij 0.45.1 and the themes in `~/.config/zellij/themes`. Each name is its own row.
- Moving the selection previews that theme in the current session.
- A blank line sits above the connection sentence. Under that, a table shows the session theme and the config theme for light and for dark. `[+]` marks a box that will change. All four boxes start clear. The static `theme` line in the file stays.
- Enter applies the checked boxes and closes the window. Esc leaves the file unchanged and puts the previous theme back.
- Tab moves between the list and the apply boxes. A bar at the bottom shows Tab, up, down, Esc, and Enter, and those controls can be clicked.
