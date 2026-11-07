// Isolated request-interceptor proof. No application profiles or credentials.
#include "../../ui/browser/teams-request-headers.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QFile>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QTimer>
#include <cstdio>
int main(int argc, char **argv) {
    if (argc != 4) return 2;
    QFile input(argv[1]);
    if (!input.open(QIODevice::ReadOnly)) return 3;
    const auto fixture = QJsonDocument::fromJson(input.readAll()).object();
    int count = 0;
    for (const auto &row : fixture["cases"].toArray()) {
        const auto value = row.toObject();
        auto headers = value["requestHeaders"].toObject();
        QJsonObject projected;
        for (auto header = headers.begin(); header != headers.end(); ++header) {
            if (header.key().compare("user-agent", Qt::CaseInsensitive) != 0) continue;
            const auto original = header.value().toString();
            const auto adapted = TeamsRequestHeaders::withoutElectron(original);
            if (TeamsRequestHeaders::withoutElectronBytes(original.toUtf8()) != adapted.toUtf8()) return 8;
            header.value() = adapted;
            projected["requestHeaders"] = headers;
            break;
        }
        if (projected != value["expected"].toObject()) { fprintf(stderr, "HEADER_CASE_FAILED %d\n", count); return 4; }
        ++count;
    }
    const QByteArray opaque = QByteArray::fromHex("ff") + " Electron/44 suffix";
    if (TeamsRequestHeaders::withoutElectronBytes(opaque) != QByteArray::fromHex("ff") + " suffix") return 9;
    fprintf(stdout, "NATIVE_HEADER_CASES_PASS %d\n", count); fflush(stdout);
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QtWebEngineQuick::initialize();
    QGuiApplication app(argc, argv);
    TeamsRequestHeaders::Interceptor interceptor;
    QQuickWebEngineProfile profile;
    profile.setOffTheRecord(true);
    profile.setHttpUserAgent("FixtureAgent/1 Electron/44.0.0 Chrome/144.0.0");
    profile.setUrlRequestInterceptor(&interceptor);
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("proofProfile", &profile);
    engine.rootContext()->setContextProperty("proofBase", QString::fromUtf8(argv[3]));
    engine.load(QUrl::fromLocalFile(argv[2]));
    if (engine.rootObjects().isEmpty()) return 5;
    QTimer::singleShot(15000, &app, [&app] { app.exit(6); });
    const auto result = app.exec();
    fprintf(stdout, "PHYSICAL_HEADERS %s\n", engine.rootObjects().first()->property("observationJson").toString().toUtf8().constData());
    return result;
}
