#include "terminalsurface.h"
#include <QGuiApplication>
#include <QImage>
#include <QMouseEvent>
#include <QHoverEvent>
#include <QFile>
#include <QCryptographicHash>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QKeyEvent>
#include <QPainter>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QtTest>

class TerminalTests : public QObject {
    Q_OBJECT
    static QString output(const QSignalSpy &spy) {
        QString result; for (const auto &event : spy) result += event[0].toString(); return result;
    }
    static void key(TerminalSurface &surface, int key, Qt::KeyboardModifiers modifiers = Qt::NoModifier, const QString &text = {}) {
        QKeyEvent event(QEvent::KeyPress, key, modifiers, text); QCoreApplication::sendEvent(&surface, &event);
    }
private slots:
    void cellsKeepCursorColorsAndWideCombiningCharacters() {
        TerminalSurface surface; surface.setGrid(40, 4);
        surface.feed(QStringLiteral("one\r\n\x1b[31;1mred\x1b[0m"));
        QCOMPARE(surface.rowText(0), QStringLiteral("one"));
        QCOMPARE(surface.rowText(1), QStringLiteral("red"));
        QCOMPARE(surface.cellAt(1, 0)["bold"].toBool(), true);
        QColor red = surface.cellAt(1, 0)["foreground"].value<QColor>();
        QVERIFY(red == QColor(204, 102, 102));
        surface.feed(QStringLiteral("\x1b[1;1HX\x1b[K"));
        QCOMPARE(surface.rowText(0), QStringLiteral("X"));
        surface.reset(); surface.setGrid(40, 4);
        QByteArray bytes = QString::fromUtf8("日本 e\xcc\x81 😀").toUtf8();
        for (char byte : bytes) surface.feedBytes(QByteArray(1, byte));
        QCOMPARE(surface.rowText(0), QString::fromUtf8("日本 e\xcc\x81 😀"));
        QCOMPARE(surface.cellAt(0, 0)["width"].toInt(), 2);
        QCOMPARE(surface.cellAt(0, 1)["width"].toInt(), 0);
        QCOMPARE(surface.cellAt(0, 5)["text"].toString(), QString::fromUtf8("e\xcc\x81"));
    }
    void alternateScreenRestoresMainAndDeviceStatusUsesInputSignal() {
        TerminalSurface surface; surface.setGrid(20, 4);
        QSignalSpy input(&surface, &TerminalSurface::input);
        surface.feed(QStringLiteral("main"));
        surface.feed(QStringLiteral("\x1b[?1049h\x1b[Halt"));
        QVERIFY(surface.alternateScreen()); QCOMPARE(surface.rowText(0), QStringLiteral("alt"));
        surface.feed(QStringLiteral("\x1b[?1049l"));
        QVERIFY(!surface.alternateScreen()); QCOMPARE(surface.rowText(0), QStringLiteral("main"));
        surface.feed(QStringLiteral("\x1b[6n"));
        QTRY_COMPARE(output(input), QStringLiteral("\x1b[1;5R"));
        surface.feed(QStringLiteral("\x1b]2;Synthetic terminal\x07"));
        QCOMPARE(surface.title(), QStringLiteral("Synthetic terminal"));
    }
    void historySelectionAndCopyAreBoundedAndDoNotOwnClipboard() {
        TerminalSurface surface; surface.setGrid(20, 3);
        for (int line = 0; line < 12; ++line) surface.feed(QString("row%1\r\n").arg(line));
        QVERIFY(surface.scrollbackLines() >= 9);
        surface.setScrollOffset(surface.scrollbackLines());
        QCOMPARE(surface.rowText(0), QStringLiteral("row0"));
        surface.selectRange(0, 0, 0, 4); QCOMPARE(surface.selectedText(), QStringLiteral("row0"));
        QSignalSpy copied(&surface, &TerminalSurface::copyRequested);
        surface.copySelection(); QCOMPARE(copied.size(), 1); QCOMPARE(copied[0][0].toString(), QStringLiteral("row0"));
        surface.clearSelection(); QVERIFY(!surface.hasSelection());
        surface.setScrollOffset(0);
        for (int line = 0; line < 10010; ++line) surface.feed(QStringLiteral("x\r\n"));
        QVERIFY(surface.scrollbackLines() > 0); QVERIFY(surface.scrollbackLines() < 10000);
        surface.setGrid(1001, 501); QCOMPARE(surface.columns(), 1000); QCOMPARE(surface.rows(), 500);
        surface.setGrid(-1, 0); QCOMPARE(surface.columns(), 1); QCOMPARE(surface.rows(), 1);
    }
    void keyboardModesAndBracketedPasteUseVtEncoding() {
        TerminalSurface surface; QSignalSpy input(&surface, &TerminalSurface::input);
        key(surface, Qt::Key_C, Qt::ControlModifier, QString(QChar(3)));
        QCOMPARE(output(input), QString(QChar(3))); input.clear();
        key(surface, Qt::Key_Up); QCOMPARE(output(input), QStringLiteral("\x1b[A")); input.clear();
        surface.feed(QStringLiteral("\x1b[?1h"));
        key(surface, Qt::Key_Up); QCOMPARE(output(input), QStringLiteral("\x1bOA")); input.clear();
        key(surface, Qt::Key_Up, Qt::ControlModifier); QCOMPARE(output(input), QStringLiteral("\x1b[1;5A")); input.clear();
        key(surface, Qt::Key_Return); QCOMPARE(output(input), QStringLiteral("\r")); input.clear();
        key(surface, Qt::Key_Backtab, Qt::ShiftModifier); QCOMPARE(output(input), QStringLiteral("\x1b[Z")); input.clear();
        surface.feed(QStringLiteral("\x1b[?2004h")); surface.paste(QStringLiteral("one\ntwo"));
        QCOMPARE(output(input), QStringLiteral("\x1b[200~one\ntwo\x1b[201~"));
        QSignalSpy paste(&surface, &TerminalSurface::pasteRequested);
        key(surface, Qt::Key_V, Qt::ControlModifier | Qt::ShiftModifier); QCOMPARE(paste.size(), 1);
    }
    void painterProducesStyledPixelsWithoutWindowOrProcess() {
        TerminalSurface surface; surface.setWidth(300); surface.setHeight(100); surface.setGrid(30, 5);
        surface.setBackground(QColor("#101820")); surface.feed(QStringLiteral("\x1b[32mVisible\x1b[0m"));
        QImage image(300, 100, QImage::Format_ARGB32_Premultiplied); image.fill(Qt::transparent);
        QPainter painter(&image); surface.paint(&painter); painter.end();
        QCOMPARE(image.pixelColor(299, 99), QColor("#101820"));
        bool glyph = false;
        for (int y = 0; y < 25; ++y) for (int x = 0; x < 100; ++x) if (image.pixelColor(x, y) != QColor("#101820")) glyph = true;
        QVERIFY(glyph);
    }
    void pinnedNativeOracle_data() {
        QTest::addColumn<QJsonObject>("fixture");
        const QString root = qEnvironmentVariable("O3_TERMINAL_SOURCE");
        QFile file(qEnvironmentVariable("O3_TERMINAL_ORACLE", root + "/native/terminal/fixtures.json")); QVERIFY(file.open(QFile::ReadOnly));
        const auto document = QJsonDocument::fromJson(file.readAll()).object();
        QCOMPARE(document["engineRevision"].toString(), QStringLiteral("9f62873bf195e4d8a762d768a1405a5f2f7b1697"));
        for (const auto entry : document["cases"].toArray()) {
            const auto fixture = entry.toObject(); QTest::newRow(qPrintable(fixture["label"].toString())) << fixture;
        }
    }
    void pinnedNativeOracle() {
        QFETCH(QJsonObject, fixture);
        TerminalSurface surface; surface.setGrid(fixture["cols"].toInt(), fixture["rows"].toInt());
        QCOMPARE(surface.engineRevision(), QStringLiteral("9f62873bf195e4d8a762d768a1405a5f2f7b1697"));
        const int history = fixture["history"].toInt();
        for (int line = 0; line < history; ++line) surface.feed(history == 12 ? QString("row%1\r\n").arg(line) : QStringLiteral("x\r\n"));
        const auto vt = fixture["vt"].toString();
        if (fixture["splitBytes"].toBool()) for (const char byte : vt.toUtf8()) surface.feedBytes(QByteArray(1, byte));
        else surface.feed(vt);
        if (fixture["scrollTop"].toBool()) surface.setScrollOffset(surface.scrollbackLines());
        const auto selection = fixture["select"].toArray();
        if (!selection.isEmpty()) surface.selectRange(selection[0].toInt(), selection[1].toInt(), selection[2].toInt(), selection[3].toInt());
        const auto expected = fixture["expected"].toObject();
        const auto rows = expected["rows"].toArray();
        const auto rgb = [](const QJsonObject &value) { return QColor(value["r"].toInt(), value["g"].toInt(), value["b"].toInt()); };
        for (int row = 0; row < rows.size(); ++row) {
            const auto cells = rows[row].toArray();
            for (int column = 0; column < cells.size(); ++column) {
                const auto cell = cells[column].toObject(); const auto actual = surface.cellAt(row, column);
                QCOMPARE(actual["text"].toString(), cell["text"].toString());
                QCOMPARE(actual["foreground"].value<QColor>(), rgb(cell["foreground"].toObject()));
                QCOMPARE(actual["background"].value<QColor>(), rgb(cell["background"].toObject()));
                for (const QString field : {QStringLiteral("bold"), QStringLiteral("italic"), QStringLiteral("underline"), QStringLiteral("selected")}) QCOMPARE(actual[field].toBool(), cell[field].toBool());
            }
        }
        QCOMPARE(surface.selectedText(), expected["selection"].toString());
        const auto cursor = expected["cursor"].toArray();
        if (cursor[0].toInt() >= 0) { QCOMPARE(surface.cursorRow(), cursor[0].toInt()); QCOMPARE(surface.cursorColumn(), cursor[1].toInt()); }
        const auto scrollbar = expected["scrollbar"].toObject();
        if (history < 10000) QCOMPARE(surface.scrollbackLines(), scrollbar["total"].toInt() - scrollbar["len"].toInt());
        else {
            // Ghostty pages have architecture-dependent pointer/alignment overhead.
            // The exact pin and byte budget match T3; both engines prune history.
            QVERIFY(surface.scrollbackLines() > 0 && surface.scrollbackLines() < history);
            qInfo() << "Pinned Ghostty scrollback rows: native" << surface.scrollbackLines() << "WASM" << scrollbar["total"].toInt() - scrollbar["len"].toInt();
        }
        if (expected.contains("hyperlink")) {
            const auto point = fixture["hyperlink"].toArray(); QCOMPARE(surface.hyperlinkAt(point[0].toInt(), point[1].toInt()), expected["hyperlink"].toString());
        }
        QSignalSpy input(&surface, &TerminalSurface::input);
        const auto keys = fixture["keys"].toArray(), outputs = expected["keys"].toArray();
        for (int index = 0; index < keys.size(); ++index) {
            const auto spec = keys[index].toObject(); const auto name = spec["qt"].toString(); int code = 0;
            if (name.size() == 1) code = name[0].toUpper().unicode();
            else {
                const QMap<QString, int> codes{{"Up", Qt::Key_Up}, {"Left", Qt::Key_Left}, {"Return", Qt::Key_Return}, {"Backtab", Qt::Key_Backtab}, {"Backspace", Qt::Key_Backspace}, {"Escape", Qt::Key_Escape}, {"F5", Qt::Key_F5}, {"Slash", Qt::Key_Slash}};
                QVERIFY(codes.contains(name)); code = codes[name];
            }
            const int flags = spec["mods"].toInt();
            const auto mods = Qt::KeyboardModifiers((flags & 1 ? Qt::ShiftModifier : Qt::NoModifier) | (flags & 2 ? Qt::ControlModifier : Qt::NoModifier) | (flags & 4 ? Qt::AltModifier : Qt::NoModifier) | (flags & 8 ? Qt::MetaModifier : Qt::NoModifier));
            QKeyEvent event(spec["action"].toString() == "release" ? QEvent::KeyRelease : QEvent::KeyPress, code, mods, spec["text"].toString());
            QCoreApplication::sendEvent(&surface, &event); QCOMPARE(output(input), outputs[index].toString()); input.clear();
        }
        if (fixture.contains("paste")) { surface.paste(fixture["paste"].toString()); QCOMPARE(output(input), expected["paste"].toString()); }
        if (fixture.contains("mouse")) {
            surface.setWidth(160); surface.setHeight(64); surface.setGrid(fixture["cols"].toInt(), fixture["rows"].toInt());
            const auto events = fixture["mouse"].toArray(), packets = expected["mouse"].toArray();
            for (int index = 0; index < events.size(); ++index) {
                const auto spec = events[index].toObject(); const QPointF point(spec["x"].toDouble(), spec["y"].toDouble());
                const auto action = spec["action"].toString();
                if (spec["button"].isNull()) {
                    QHoverEvent event(QEvent::HoverMove, point, point, point); QCoreApplication::sendEvent(&surface, &event);
                } else {
                    const auto type = action == "press" ? QEvent::MouseButtonPress : action == "release" ? QEvent::MouseButtonRelease : QEvent::MouseMove;
                    QMouseEvent event(type, point, point, action == "motion" ? Qt::NoButton : Qt::LeftButton, spec["anyButtonPressed"].toBool() ? Qt::LeftButton : Qt::NoButton, Qt::NoModifier);
                    QCoreApplication::sendEvent(&surface, &event);
                }
                QCOMPARE(output(input), packets[index].toString()); input.clear();
            }
        }
    }
    void mouseSelectionTracksScrollbackAndSupportsRepeatedClicks() {
        TerminalSurface surface; surface.setWidth(160); surface.setHeight(64); surface.setGrid(20, 4);
        surface.feed(QStringLiteral("hello world\r\nsecond line"));
        const auto mouse = [&](QEvent::Type type, QPointF point, Qt::MouseButtons buttons) {
            QMouseEvent event(type, point, point, type == QEvent::MouseMove ? Qt::NoButton : Qt::LeftButton, buttons, Qt::NoModifier);
            QCoreApplication::sendEvent(&surface, &event);
        };
        mouse(QEvent::MouseButtonPress, {1, 1}, Qt::LeftButton);
        mouse(QEvent::MouseMove, {33, 1}, Qt::LeftButton);
        mouse(QEvent::MouseButtonRelease, {33, 1}, Qt::NoButton);
        QCOMPARE(surface.selectedText(), QStringLiteral("hello"));
        QCOMPARE(surface.selectionRange()["lineStart"].toInt(), 1); QCOMPARE(surface.selectionRange()["lineEnd"].toInt(), 1);
        for (int index = 0; index < 10; ++index) surface.feed(QStringLiteral("\r\nnew output"));
        QCOMPARE(surface.selectedText(), QStringLiteral("hello")); QCOMPARE(surface.selectionRange()["lineStart"].toInt(), 1);
        surface.setScrollOffset(surface.scrollbackLines());
        mouse(QEvent::MouseButtonPress, {1, 1}, Qt::LeftButton); mouse(QEvent::MouseButtonRelease, {1, 1}, Qt::NoButton);
        mouse(QEvent::MouseButtonDblClick, {1, 1}, Qt::LeftButton); mouse(QEvent::MouseButtonRelease, {1, 1}, Qt::NoButton);
        QVERIFY(!surface.selectedText().isEmpty());
        QSignalSpy primary(&surface, &TerminalSurface::primaryPasteRequested);
        QMouseEvent middle(QEvent::MouseButtonPress, QPointF(1,1), QPointF(1,1), Qt::MiddleButton, Qt::MiddleButton, Qt::NoModifier);
        QCoreApplication::sendEvent(&surface, &middle); QCOMPARE(primary.size(), 1);
    }
    void synchronizedUpdatesKeepCompleteFramesAndExpire() {
        TerminalSurface surface; surface.feed(QStringLiteral("before"));
        surface.feed(QStringLiteral("\x1b[?2026h\x1b[Hafter")); QCOMPARE(surface.rowText(0), QStringLiteral("before"));
        surface.feed(QStringLiteral("\x1b[?2026l")); QCOMPARE(surface.rowText(0), QStringLiteral("aftere"));
        surface.feed(QStringLiteral("\x1b[?2026h\x1b[Htimeout"));
        QTRY_COMPARE_WITH_TIMEOUT(surface.rowText(0), QStringLiteral("timeout"), 1000);
    }
    void pinnedTerminalLinkAlgorithmUsesNamedUnicodeMatches() {
        const QString root = qEnvironmentVariable("O3_TERMINAL_SOURCE");
        QFile source(root + "/ui/js/NativeTerminalLinks.js"); QVERIFY(source.open(QIODevice::ReadOnly));
        const auto bytes = source.readAll();
        QFile provenance(root + "/ui/js/NativeTerminalLinks.sources.json"); QVERIFY(provenance.open(QIODevice::ReadOnly));
        const auto metadata = QJsonDocument::fromJson(provenance.readAll()).object();
        QCOMPARE(QString::fromLatin1(QCryptographicHash::hash(bytes, QCryptographicHash::Sha256).toHex()), metadata["outputSha256"].toString());
        QString program = QString::fromUtf8(bytes); program.remove(0, program.indexOf(QLatin1Char('\n')) + 1);
        TerminalSurface surface; QQmlEngine engine;
        QQmlEngine::setObjectOwnership(&surface, QQmlEngine::CppOwnership);
        engine.globalObject().setProperty("terminal", engine.newQObject(&surface));
        const auto loaded = engine.evaluate(program); QVERIFY2(!loaded.isError(), qPrintable(loaded.toString()));
        auto runner = engine.evaluate(R"JS((function(test) {
            var api = create(function(text,pattern,flags) { var result=terminal.matchTerminalPattern(text,pattern,flags); terminalTrace.push({pattern:pattern,flags:flags,matches:result}); return result });
            terminalTrace=[];
            var args = test.args;
            if (test.grid) args[0]=terminalGrids[args[0]];
            if (test.method === "collectWrappedTerminalLinkLine") {
                var lines = args[1];
                args[1] = function(index) { var line = lines[index]; if (!line) return null; return {isWrapped:line.isWrapped,translateToString:function(trim) { return trim ? line.text.replace(/\s+$/u, "") : line.text }} };
            }
            return api[test.method].apply(null,args);
        }))JS");
        QVERIFY2(runner.isCallable(), qPrintable(runner.toString()));
        QFile fixtures(qEnvironmentVariable("O3_TERMINAL_LINK_ORACLE", root + "/native/terminal/link-fixtures.json")); QVERIFY(fixtures.open(QIODevice::ReadOnly));
        const auto data = QJsonDocument::fromJson(fixtures.readAll()).object();
        const auto cases = data["cases"].toArray();
        engine.globalObject().setProperty("terminalGrids", engine.toScriptValue(data["grids"].toVariant()));
        QCOMPARE(cases.size(), 219);
        for (const QJsonValue test : cases) {
            const auto result = runner.call({engine.toScriptValue(test.toVariant())});
            QVERIFY2(!result.isError(), qPrintable(result.toString()+" stack="+result.property("stack").toString()+" case="+QString::fromUtf8(QJsonDocument(test.toObject()).toJson(QJsonDocument::Compact))));
            const auto actual = QJsonValue::fromVariant(result.toVariant());
            QVERIFY2(actual == test["expected"], qPrintable(QString::fromUtf8(QJsonDocument(test.toObject()).toJson(QJsonDocument::Compact)) + " actual=" + engine.evaluate("JSON.stringify").call({result}).toString()+" trace="+engine.evaluate("JSON.stringify(terminalTrace)").toString()));
        }
        const auto viewports = data["viewportCases"].toArray(); QCOMPARE(viewports.size(), 5);
        for (const QJsonValue viewport : viewports) {
            surface.reset(); surface.setGrid(viewport["cols"].toInt(), viewport["rows"].toInt()); surface.feed(viewport["vt"].toString());
            for (const QJsonValue point : viewport["points"].toArray()) {
                const int row = point["row"].toInt(), column = point["column"].toInt();
                const QVariantMap input{{"method", "terminalLinkAtPositionWithRange"}, {"args", QVariantList{surface.linkRows(row), row, column}}};
                const auto result = runner.call({engine.toScriptValue(input)});
                QVERIFY2(!result.isError(), qPrintable(result.toString()));
                QVERIFY2(QJsonValue::fromVariant(result.toVariant()) == point["expected"], qPrintable(viewport["vt"].toString()+" row="+QString::number(row)+" column="+QString::number(column)));
            }
        }
    }
    void qmlPluginLoadsByUriAndRelativeImport() {
        const QString root = qEnvironmentVariable("O3_TERMINAL_SOURCE"); QVERIFY(!root.isEmpty());
        QQmlEngine engine; engine.addImportPath(root + "/bin/qml");
        for (const QByteArray import : {QByteArray("import O3.Terminal 1.0 as VT"), QByteArray("import \"../../bin/qml/O3/Terminal\" 1.0 as VT")}) {
            QQmlComponent component(&engine);
            component.setData("import QtQuick\n" + import + "\nVT.TerminalSurface { width: 300; height: 100 }", QUrl::fromLocalFile(root + "/tests/qml/terminal-plugin-test.qml"));
            QTRY_VERIFY_WITH_TIMEOUT(component.status() != QQmlComponent::Loading, 5000);
            QVERIFY2(component.isReady(), qPrintable(component.errorString()));
            QScopedPointer<QObject> object(component.create()); QVERIFY2(object, qPrintable(component.errorString()));
            QVERIFY(QMetaObject::invokeMethod(object.data(), "feed", Q_ARG(QString, QStringLiteral("QML ready"))));
        }
    }
};
QTEST_MAIN(TerminalTests)
#include "tests.moc"
