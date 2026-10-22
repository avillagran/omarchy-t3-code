-- Default agent only. Ctrl+Enter is handled inside ot3; Omarchy owns Super+Enter.
-- The app runs on its own (bin/ot3), so the shortcut works even when the bar is stuck.
local ok, info = pcall(function() return debug.getinfo(1, "S").source end)
local here = ok and type(info) == "string" and info:match("^@(.*)/integrations/hyprland/bindings%.lua$")
  or (os.getenv("HOME") .. "/.config/omarchy/plugins/omarchy-t3-code")
local launcher = "'" .. (here .. "/bin/ot3"):gsub("'", "'\\''") .. "'"
hl.unbind("SUPER + SHIFT + A")
o.bind("SUPER + SHIFT + A", "AI agent: T3 Code", launcher .. " toggle")
