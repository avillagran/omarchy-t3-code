-- SUPER+RETURN sends only while the T3 panel is open; otherwise the normal terminal.
hl.unbind("SUPER + RETURN")
o.bind("SUPER + RETURN", "T3 send / Terminal", (os.getenv("HOME") or "") .. "/.config/omarchy/plugins/omarchy-t3-code/bin/omarchy-t3-client super-enter")
