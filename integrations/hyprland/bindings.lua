-- SUPER+RETURN sends only while the T3 panel is open; otherwise the normal terminal.
local source = debug.getinfo(1, "S").source
local plugin_dir = source:match("^@(.+)/integrations/hyprland/bindings%.lua$")
  or ((os.getenv("HOME") or "") .. "/.config/omarchy/plugins/omarchy-t3-code")
hl.unbind("SUPER + RETURN")
o.bind("SUPER + RETURN", "T3 send / Terminal", plugin_dir .. "/bin/omarchy-t3-client super-enter")
