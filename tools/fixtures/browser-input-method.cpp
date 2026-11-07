// Actual GUI item and QInputMethodEvent production; no simulated engine
// context.
#include <QElapsedTimer>
#include <QGuiApplication>
#include <QInputMethodEvent>
#include <QJSValue>
#include <QJsonArray>
#include <QJsonObject>
#include <QProcess>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QThread>
#include <cstdio>
#include <cstdlib>
#define CHECK(value)                                                           \
  do {                                                                         \
    if (!(value)) {                                                            \
      fprintf(stderr, "BROWSER_IME_ASSERT line=%d\n", __LINE__);               \
      std::exit(2);                                                            \
    }                                                                          \
  } while (0)
static bool waitFor(const std::function<bool()> &ready) {
  QElapsedTimer clock;
  clock.start();
  while (!ready() && clock.elapsed() < 5000) {
    QCoreApplication::processEvents();
    QThread::msleep(2);
  }
  return ready();
}
int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  QQmlEngine engine;
  engine.addImportPath("/fixture/qml");
  QQmlComponent component(&engine);
  component.setData(R"(import QtQuick
import O3.Browser 1.0
BrowserInputItem {
 width:320;height:240;inputEnabled:true;scopeToken:"first"
 property var records:[]
 property var rejections:[]
 property var queries:[]
 onInput:function(token,event){records=records.concat([{token:token,event:event}]);if(event.text==="rotate")scopeToken="replacement"}
 onInputRejected:function(reason){rejections=rejections.concat([reason])}
 onContextRequested:function(token){queries=queries.concat([token])}
})",
                    QUrl("file:///fixture/input.qml"));
  CHECK(waitFor([&] { return component.status() != QQmlComponent::Loading; }));
  auto *item = qobject_cast<QQuickItem *>(component.create());
  if (!item) {
    fprintf(stderr, "%s\n", qPrintable(component.errorString()));
    return 2;
  }
  QQuickWindow window;
  window.setTitle("Owned Qt native IME producer");
  window.resize(320, 240);
  item->setParentItem(window.contentItem());
  window.show();
  item->forceActiveFocus();
  window.requestActivate();
  CHECK(waitFor([&] { return window.isActive() && item->hasActiveFocus(); }));
  auto records = [&] {
    return QJsonArray::fromVariantList(
        item->property("records").value<QJSValue>().toVariant().toList());
  };
  auto rejections = [&] {
    return QJsonArray::fromVariantList(
        item->property("rejections").value<QJSValue>().toVariant().toList());
  };
  CHECK(!item->property("contextAvailable").toBool());
  QInputMethodQueryEvent query(Qt::ImEnabled | Qt::ImSurroundingText |
                               Qt::ImCursorRectangle | Qt::ImCursorPosition |
                               Qt::ImAnchorPosition | Qt::ImHints);
  QCoreApplication::sendEvent(item, &query);
  CHECK(query.value(Qt::ImEnabled).toBool());
  for (auto kind : {Qt::ImSurroundingText, Qt::ImCursorRectangle,
                    Qt::ImCursorPosition, Qt::ImAnchorPosition, Qt::ImHints})
    CHECK(!query.value(kind).isValid());
  QInputMethodEvent commit;
  commit.setCommitString(QString::fromUtf8("拼音🙂"));
  QCoreApplication::sendEvent(item, &commit);
  CHECK(commit.isAccepted());
  CHECK(records().size() == 1);
  CHECK(records()[0].toObject()["token"] == "first");
  CHECK(records()[0].toObject()["event"].toObject()["text"] ==
        QString::fromUtf8("拼音🙂"));
  QInputMethodEvent preedit(QString::fromUtf8("にほん"),
                            {{QInputMethodEvent::Cursor, 1, 1, {}}});
  QCoreApplication::sendEvent(item, &preedit);
  CHECK(records().size() == 2);
  CHECK(records()[1].toObject()["event"].toObject()["selectionStart"] == 1);
  QInputMethodEvent next(QString::fromUtf8("ご"), {});
  next.setCommitString(QString::fromUtf8("日本"));
  QCoreApplication::sendEvent(item, &next);
  CHECK(records().size() == 4);
  CHECK(records()[2].toObject()["event"].toObject()["type"] == "text");
  CHECK(records()[3].toObject()["event"].toObject()["type"] == "composition");
  QInputMethodEvent cancel;
  QCoreApplication::sendEvent(item, &cancel);
  CHECK(records().size() == 5);
  CHECK(records()[4].toObject()["event"].toObject()["preedit"] == "");
  QInputMethodEvent replace;
  replace.setCommitString("x", -1, 1);
  QCoreApplication::sendEvent(item, &replace);
  CHECK(!replace.isAccepted());
  CHECK(records().size() == 5);
  CHECK(rejections().size() == 1);
  QInputMethodEvent selection("", {{QInputMethodEvent::Selection, 1, 2, {}}});
  QCoreApplication::sendEvent(item, &selection);
  CHECK(!selection.isAccepted());
  CHECK(records().size() == 5);
  CHECK(rejections().size() == 2);
  QInputMethodEvent oversized;
  oversized.setCommitString(QString(4097, QChar('x')));
  QCoreApplication::sendEvent(item, &oversized);
  CHECK(!oversized.isAccepted());
  CHECK(records().size() == 5);
  CHECK(rejections().size() == 3);
  QInputMethodEvent rotate("new-preedit", {});
  rotate.setCommitString("rotate");
  QCoreApplication::sendEvent(item, &rotate);
  CHECK(records().size() == 6);
  CHECK(records()[5].toObject()["token"] == "first");
  CHECK(item->property("scopeToken") == "replacement");
  {
    QQuickWindow foreign;
    foreign.setTitle("Owned inactive IME peer");
    foreign.resize(120, 120);
    foreign.show();
    foreign.requestActivate();
    CHECK(waitFor([&] { return foreign.isActive() && !window.isActive(); }));
    QInputMethodEvent inactive;
    inactive.setCommitString("discard-inactive-window");
    QCoreApplication::sendEvent(item, &inactive);
    CHECK(!inactive.isAccepted());
    CHECK(records().size() == 6);
    foreign.hide();
  }
  window.requestActivate();
  item->forceActiveFocus();
  CHECK(waitFor([&] { return window.isActive() && item->hasActiveFocus(); }));
  item->setProperty("inputEnabled", false);
  QInputMethodEvent disabled;
  disabled.setCommitString("discard");
  QCoreApplication::sendEvent(item, &disabled);
  CHECK(!disabled.isAccepted());
  CHECK(records().size() == 6);
  item->setParentItem(nullptr);
  delete item;
  QQmlComponent pageComponent(&engine);
  pageComponent.setData(R"(import QtQuick
import "page" as Product
Product.T3BrowserPage {
 width:320;height:240;interactive:true
 page:({requestId:"page-first",nativeInput:true})
 property var records:[]
 onInput:function(token,event){records=records.concat([{token:token,event:event}]);if(event.text==="rotate")page=Object.assign({},page,{requestId:"page-replacement"})}
})",
                        QUrl("file:///fixture/proof.qml"));
  CHECK(waitFor(
      [&] { return pageComponent.status() != QQmlComponent::Loading; }));
  auto *page = qobject_cast<QQuickItem *>(pageComponent.create());
  if (!page) {
    fprintf(stderr, "%s\n", qPrintable(pageComponent.errorString()));
    return 2;
  }
  page->setParentItem(window.contentItem());
  QQuickItem *native = nullptr;
  const bool mounted = waitFor([&] {
    native = qvariant_cast<QQuickItem *>(page->property("nativeInputItem"));
    return native;
  });
  if (!mounted) {
    fprintf(stderr,
            "INPUT_MOUNT_DIAGNOSTIC propertyType=%s native=%p enabled=%d "
            "pageFlag=%d request=%s\n",
            page->property("nativeInputItem").typeName(),
            page->property("nativeInputItem").value<QObject *>(),
            page->property("interactive").toBool(),
            page->property("page")
                .value<QJSValue>()
                .property("nativeInput")
                .toBool(),
            qPrintable(page->property("requestId").toString()));
    for (auto *child : page->findChildren<QObject *>()) {
      if (auto *failed = qobject_cast<QQmlComponent *>(child))
        fprintf(stderr, "LOADER_COMPONENT_ERROR %s\n",
                qPrintable(failed->errorString()));
      fprintf(
          stderr,
          "INPUT_CHILD type=%s name=%s active=%d source=%s status=%d item=%p\n",
          child->metaObject()->className(), qPrintable(child->objectName()),
          child->property("active").toBool(),
          qPrintable(child->property("source").toUrl().toString()),
          child->property("status").toInt(),
          static_cast<void *>(child->property("item").value<QObject *>()));
    }
  }
  CHECK(mounted);
  native->forceActiveFocus();
  CHECK(waitFor([&] { return native->hasActiveFocus(); }));
  QInputMethodEvent pagePreedit(QString::fromUtf8("かな"), {});
  QCoreApplication::sendEvent(native, &pagePreedit);
  CHECK(pagePreedit.isAccepted());
  QInputMethodEvent pageCommit;
  pageCommit.setCommitString(QString::fromUtf8("仮名🙂"));
  QCoreApplication::sendEvent(native, &pageCommit);
  CHECK(pageCommit.isAccepted());
  auto pageRecords = [&] {
    return QJsonArray::fromVariantList(
        page->property("records").value<QJSValue>().toVariant().toList());
  };
  CHECK(pageRecords().size() == 2);
  CHECK(pageRecords()[0].toObject()["token"] == "page-first");
  CHECK(pageRecords()[1].toObject()["event"].toObject()["text"] ==
        QString::fromUtf8("仮名🙂"));
  QInputMethodEvent pageRotate("discard-new-composition", {});
  pageRotate.setCommitString("rotate");
  QCoreApplication::sendEvent(native, &pageRotate);
  CHECK(pageRecords().size() == 3);
  CHECK(page->property("requestId") == "page-replacement");
  page->setProperty("interactive", false);
  QInputMethodEvent pageDisabled;
  pageDisabled.setCommitString("discard");
  QCoreApplication::sendEvent(native, &pageDisabled);
  CHECK(!pageDisabled.isAccepted());
  CHECK(pageRecords().size() == 3);
  if (qEnvironmentVariableIsSet("O3_TEST_COMPOSITOR_IME")) {
    page->setProperty("interactive", true);
    native->forceActiveFocus();
    CHECK(
        waitFor([&] { return native->hasActiveFocus() && window.isActive(); }));
    QProcess method;
    method.setProgram("/fixture/input-method-peer");
    method.start();
    CHECK(method.waitForStarted(2000));
    bool ready = false, active = false;
    auto drain = [&] {
      while (method.canReadLine()) {
        const auto line = method.readLine().trimmed();
        if (line == "ready")
          ready = true;
        else if (line.startsWith("active "))
          active = true;
        else if (line.startsWith("inactive "))
          active = false;
      }
    };
    const bool activated = waitFor([&] {
      drain();
      return ready && active;
    });
    if (!activated)
      fprintf(stderr, "COMPOSITOR_IME_NOT_ACTIVE state=%d error=%s\n",
              int(method.state()), method.readAllStandardError().constData());
    CHECK(activated);
    const auto before = pageRecords().size();
    method.write("preedit\n");
    CHECK(method.waitForBytesWritten(1000));
    CHECK(waitFor([&] {
      drain();
      return pageRecords().size() > before;
    }));
    CHECK(pageRecords().last().toObject()["event"].toObject()["preedit"] ==
          QString::fromUtf8("にほん"));
    const auto composition =
        pageRecords().last().toObject()["event"].toObject();
    CHECK(composition["preedit"].toString().size() == 3);
    CHECK(composition["selectionStart"].toInt(-1) == 3);
    CHECK(composition["selectionEnd"].toInt(-1) == 3);
    CHECK(!composition.contains("replacementStart"));
    CHECK(!composition.contains("replacementEnd"));
    method.write("commit\n");
    CHECK(method.waitForBytesWritten(1000));
    CHECK(waitFor([&] {
      drain();
      for (const auto &record : pageRecords())
        if (record.toObject()["event"].toObject()["text"] ==
            QString::fromUtf8("日本"))
          return true;
      return false;
    }));
    CHECK(pageRecords().last().toObject()["token"] == "page-replacement");
    method.write("quit\n");
    CHECK(method.waitForBytesWritten(1000));
    CHECK(method.waitForFinished(2000) && method.exitCode() == 0);
    puts("BROWSER_COMPOSITOR_IME_PASS input-method-v2 text-input-v3 "
         "actual-T3BrowserPage unicode-preedit-commit");
  }
  page->setParentItem(nullptr);
  delete page;
  window.hide();
  puts("BROWSER_QT_INPUT_METHOD_PASS real-Qt-events actual-T3BrowserPage "
       "unicode-preedit-commit-cancel unavailable-context replacement-refusal "
       "stale-scope disabled bound");
  return 0;
}
