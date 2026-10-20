-- PriyaOS's lines for Hyprland 0.55 and later (Lua config), installed to
-- /usr/share/priya/desktop/hyprland.lua. Add this to
-- ~/.config/hypr/hyprland.lua:
--   pcall(require, "/usr/share/priya/desktop/hyprland.lua")
-- (require, as Hyprland's wiki says, runs it in its own scope; pcall keeps
-- the rest of the config working if PriyaOS is removed.)

-- Super+Space opens and closes the Priya bar.
hl.bind("SUPER + SPACE", hl.dsp.exec_cmd("qs -p /usr/share/priya/shell ipc call priya-bar toggle"))

-- The bar's shell needs the session's Wayland and Hyprland details, which
-- systemd learns here; then it starts.
hl.on("hyprland.start", function()
	hl.exec_cmd(
		"systemctl --user import-environment WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP"
			.. " && systemctl --user start priya-shell"
	)
end)

-- No layer rule: Hyprland doesn't blur layers unless asked, and the bar
-- draws its own shadow.
