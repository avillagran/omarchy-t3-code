pragma Singleton
import QtQuick
import Quickshell

// Show/hide transition chosen in Preferences > Animations, shared by every
// T3Reveal. The panel binds these from the saved view state.
Singleton {
  // "nebula" | "warp" | "fold" | "spring" | "instant"
  property string style: "nebula"
  // Duration multiplier: slow 1.6, normal 1, fast 0.6.
  property real pace: 1
}
