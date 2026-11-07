#include "browserinputitem.h"
#include <QElapsedTimer>
#include <QThread>
#include <QInputMethodQueryEvent>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QFile>
#include <cstdio>
#include <limits>
#include <functional>

static int checks = 0;
#define CHECK(value) do { if (!(value)) { fprintf(stderr, "check failed line %d\n", __LINE__); return 1; } ++checks; } while(false)
static bool waitFor(const std::function<bool()> &condition) {
  QElapsedTimer timer;
  timer.start();
  while (!condition() && timer.elapsed() < 3000) {
    QCoreApplication::processEvents();
    QThread::msleep(2);
  }
  return condition();
}

int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  QQuickWindow window;
  window.setTitle("Owned native context query");
  window.resize(320, 240);
  BrowserInputItem item;
  item.setParentItem(window.contentItem());
  item.setSize(QSizeF(320, 240));
  item.setScopeToken("owned-scope");
  item.setInputEnabled(true);
  window.show();
  item.forceActiveFocus();
  window.requestActivate();
  CHECK(waitFor([&] { return window.isActive() && item.hasActiveFocus(); }));
  BrowserInputContext::Binding lease{"owned-scope", "physical-connection-1",
      "physical-surface-3", "owned-tab", "owned-frame"};
  BrowserInputContext::Geometry geometry{QRectF(0,29,1280,995), QRectF(10,20,640,497.5)};
  QFile receipt("/fixture/native-state.json");
  CHECK(receipt.open(QIODevice::ReadOnly));
  const auto native = QJsonDocument::fromJson(receipt.readAll()).object();
  const auto actual = native["state"].toObject();
  const auto actualRect = actual["rectangle"].toArray();
  const auto outer = native["bounds"].toObject();
  const auto content = native["contentBounds"].toObject();
  geometry.content = QRectF(content["x"].toDouble()-outer["x"].toDouble(),
      content["y"].toDouble()-outer["y"].toDouble(),
      content["width"].toDouble(),content["height"].toDouble());
  BrowserInputContext::State state;
  state.serial = quint32(actual["serial"].toInt());
  CHECK(actualRect.size()==4);
  state.caret = QRectF(actualRect[0].toDouble(),actualRect[1].toDouble(),
                      actualRect[2].toDouble(),actualRect[3].toDouble());
  state.surrounding = QByteArray("A\xf0\x9f\x98\x80\xe6\x97\xa5\xe6\x9c\xacZQ");
  state.cursorByte = actual["cursorByte"].toInt();
  state.anchorByte = actual["anchorByte"].toInt();
  state.hints = quint32(actual["contentHints"].toInt());
  state.purpose = quint32(actual["contentPurpose"].toInt());
  CHECK(!item.contextAvailable());
  CHECK(!item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  CHECK(!item.admitNativeContext(lease, geometry, state));
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  QInputMethodQueryEvent query(Qt::ImSurroundingText | Qt::ImCursorRectangle |
                             Qt::ImCursorPosition | Qt::ImAnchorPosition);
  QCoreApplication::sendEvent(&item, &query);
  CHECK(query.value(Qt::ImSurroundingText).toString().size() == 7);
  CHECK(query.value(Qt::ImCursorPosition).toInt() == 7);
  CHECK(query.value(Qt::ImAnchorPosition).toInt() == 7);
  CHECK(query.value(Qt::ImCursorRectangle).toRectF() == QRectF(55,37,0,9));
  auto older = state;
  older.serial--;
  older.caret = QRectF(1,2,0,18);
  CHECK(!item.admitNativeContext(lease, geometry, older));
  CHECK(item.inputMethodQuery(Qt::ImCursorRectangle).toRectF() == QRectF(55,37,0,9));
  state.anchorByte = 5;
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(item.inputMethodQuery(Qt::ImAnchorPosition).toInt() == 3);
  CHECK(item.inputMethodQuery(Qt::ImCurrentSelection).toString() == QString::fromUtf8("日本ZQ"));
  auto foreign = lease;
  for (auto member : {&BrowserInputContext::Binding::scope, &BrowserInputContext::Binding::connection,
                     &BrowserInputContext::Binding::surface, &BrowserInputContext::Binding::tab,
                     &BrowserInputContext::Binding::frame}) {
    foreign = lease;
    foreign.*member = "foreign";
    CHECK(!item.admitNativeContext(foreign, geometry, state));
    CHECK(!item.retireNativeContext(foreign));
    CHECK(item.contextAvailable());
  }
  state.cursorByte = 2;
  CHECK(!item.admitNativeContext(lease, geometry, state));
  CHECK(!item.contextAvailable());
  state.cursorByte = 13;
  state.surrounding = QByteArray();
  state.cursorByte = state.anchorByte = 0;
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  CHECK(item.inputMethodQuery(Qt::ImSurroundingText).toString().isEmpty());
  state.surrounding.reset();
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(!item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  CHECK(item.inputMethodQuery(Qt::ImCursorRectangle).isValid());
  state.hints = 64 | 128;
  state.purpose = 9;
  CHECK(item.admitNativeContext(lease, geometry, state));
  const auto hints = item.inputMethodQuery(Qt::ImHints).toInt();
  CHECK(hints & Qt::ImhHiddenText);
  CHECK(hints & Qt::ImhSensitiveData);
  CHECK(hints & Qt::ImhDigitsOnly);
  auto invalid = geometry;
  invalid.content.setWidth(0);
  CHECK(!item.admitNativeContext(lease, invalid, state));
  CHECK(!item.contextAvailable());
  state.caret = QRectF(-5,-10,0,18);
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(item.inputMethodQuery(Qt::ImCursorRectangle).toRectF().x() == 7.5);
  state.caret = QRectF(0,0,-1,18);
  CHECK(!item.admitNativeContext(lease, geometry, state));
  state.caret = QRectF(std::numeric_limits<double>::infinity(),0,0,18);
  CHECK(!item.admitNativeContext(lease, geometry, state));
  state.caret = QRectF(90,63,0,18);
  CHECK(item.admitNativeContext(lease, geometry, state));
  item.setScopeToken("replacement-scope");
  CHECK(!item.contextAvailable());
  CHECK(!item.admitNativeContext(lease, geometry, state));
  item.setScopeToken("owned-scope");
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  item.setInputEnabled(false);
  CHECK(!item.contextAvailable());
  CHECK(!item.inputMethodQuery(Qt::ImCursorRectangle).isValid());
  item.setInputEnabled(true);
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  item.setVisible(false);
  CHECK(!item.contextAvailable());
  item.setVisible(true);
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  window.hide();
  QCoreApplication::processEvents();
  CHECK(!item.contextAvailable());
  CHECK(!item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  printf("BROWSER_NATIVE_CONTEXT_QUERY_PASS checks=%d realQtQueryEvent=true productWaylandRouteSelected=false\n", checks);
  return 0;
}
