import QtQuick 6.11
import qs.Commons
// An environment's machine, as T3 draws it (EnvironmentMachineIcon): the
// kind is T3's own choice for that environment (catalog.machine, from its
// settings.environmentIcon, else environment.platform.machine).
T3Icon {
 id:root
 property string kind:"server"
 readonly property var icons:({server:"server",cloud:"cloud",linux:"linux",desktop:"monitor",laptop:"laptop","mac-mini":"mac-mini","mac-studio":"mac-studio"})
 // T3 ENVIRONMENT_MACHINE_KIND_LABELS; English source strings.
 readonly property string label:({server:"Server",cloud:"Cloud VM",linux:"Linux/WSL",desktop:"Desktop",laptop:"Laptop","mac-mini":"Mini PC","mac-studio":"Workstation"})[kind] || "Server"
 name:icons[kind] || "server"
}
