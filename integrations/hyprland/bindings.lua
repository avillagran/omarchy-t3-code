-- Default agent shortcut, plus contextual send/terminal behavior.
local source = debug.getinfo(1, "S").source
local plugin_dir = source:match("^@(.+)/integrations/hyprland/bindings%.lua$")
  or ((os.getenv("HOME") or "") .. "/.config/omarchy/plugins/omarchy-t3-code")
hl.unbind("SUPER + RETURN")
o.bind("SUPER + RETURN", "T3 send / Terminal", plugin_dir .. "/bin/omarchy-t3-client super-enter")

-- Replace Omarchy's default ChatGPT binding (and earlier Ares overrides).
hl.unbind("SUPER + SHIFT + A")
o.bind("SUPER + SHIFT + A", "AI agent: T3 Code", "omarchy-shell shell summon io.github.avillagran.omarchy-t3-code '{}'")
