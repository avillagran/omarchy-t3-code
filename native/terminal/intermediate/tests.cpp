#include "terminalsurface.h"
#include <QGuiApplication>
#include <QImage>
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
        QVERIFY(red.red() > 100 && red.green() < 100);
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
        QCOMPARE(output(input), QStringLiteral("\x1b[1;5R"));
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
        QCOMPARE(surface.scrollbackLines(), 10000);
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
        QCOMPARE(output(input), QStringLiteral("\x1b[200~one\rtwo\x1b[201~"));
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
