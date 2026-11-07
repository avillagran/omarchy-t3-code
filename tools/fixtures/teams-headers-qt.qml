import QtQuick
import QtQuick.Window
import QtWebEngine
Window {
    visible: true; width: 400; height: 300
    property int stage: 0
    property var observations: []
    property string observationJson: JSON.stringify(observations)
    property var hosts: ["teams.microsoft.com", "teams.cloud.microsoft", "unrelated.invalid"]
    WebEngineView {
        id: page
        anchors.fill: parent
        profile: proofProfile
        url: "https://" + hosts[stage] + ":" + proofBase + "/header-proof"
        onCertificateError: function(error) {
            // This fixture owns its self-signed loopback certificate. Product
            // certificate handling is unchanged and never installs this handler.
            error.acceptCertificate()
        }
        onLoadingChanged: function(load) {
            if (load.status !== WebEngineView.LoadSucceededStatus) return
            runJavaScript("document.body.textContent", function(text) {
                const row = JSON.parse(text)
                observations = observations.concat([{host:hosts[stage],userAgent:row.userAgent,globalUserAgent:proofProfile.httpUserAgent}])
                const expected = stage < 2 ? "FixtureAgent/1 Chrome/144.0.0" : "FixtureAgent/1 Electron/44.0.0 Chrome/144.0.0"
                if (row.userAgent !== expected || proofProfile.httpUserAgent !== "FixtureAgent/1 Electron/44.0.0 Chrome/144.0.0") {
                    console.error("TEAMS_PHYSICAL_FAIL", JSON.stringify(row)); Qt.exit(7); return
                }
                console.error("TEAMS_PHYSICAL_PASS", hosts[stage], row.userAgent)
                if (stage === hosts.length - 1) Qt.quit(); else ++stage
            })
        }
    }
}
