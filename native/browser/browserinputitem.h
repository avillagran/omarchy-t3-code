#pragma once
// Receives actual Qt input-method events on the owning GUI connection. Engine
// surrounding-text/caret queries remain unavailable unless a trusted native
// boundary supplies a matching connection/surface/tab/frame lease.
#include "browserinputcontext.h"
#include <QGuiApplication>
#include <QInputMethod>
#include <QInputMethodEvent>
#include <QQuickItem>
#include <QQuickWindow>
#include <QVariantMap>

class BrowserInputItem : public QQuickItem {
  Q_OBJECT
  Q_PROPERTY(bool inputEnabled READ inputEnabled WRITE setInputEnabled NOTIFY
                 inputEnabledChanged)
  Q_PROPERTY(QString scopeToken READ scopeToken WRITE setScopeToken NOTIFY
                 scopeTokenChanged)
  Q_PROPERTY(bool contextAvailable READ contextAvailable NOTIFY contextChanged)
  bool enabled = false, composing = false;
  QString token;
  BrowserInputContext context;
  QMetaObject::Connection windowContextConnection;
  QMetaObject::Connection windowVisibilityConnection;

public:
  explicit BrowserInputItem(QQuickItem *parent = nullptr) : QQuickItem(parent) {
    setFlag(ItemAcceptsInputMethod, true);
    connect(this, &QQuickItem::windowChanged, this, [this](QQuickWindow *value) {
      disconnect(windowContextConnection);
      disconnect(windowVisibilityConnection);
      clearNativeContext();
      if (value) {
        windowContextConnection = connect(value, &QWindow::activeChanged, this,
            [this, value] { if (!value->isActive()) clearNativeContext(); });
        windowVisibilityConnection = connect(value, &QWindow::visibleChanged, this,
            [this](bool visible) { if (!visible) clearNativeContext(); });
      }
    });
  }
  ~BrowserInputItem() override {
    disconnect(windowContextConnection);
    disconnect(windowVisibilityConnection);
    disconnect(this, nullptr, this, nullptr);
  }
  bool inputEnabled() const { return enabled; }
  QString scopeToken() const { return token; }
  bool contextAvailable() const { return context.available(); }
  // C++ owner entry points, intentionally not invokable by page JavaScript.
  void bindNativeContext(const BrowserInputContext::Binding &binding) {
    context.bind(binding);
    if (binding.scope != token)
      context.clear();
    notifyContext();
  }
  bool admitNativeContext(const BrowserInputContext::Binding &binding,
                          const BrowserInputContext::Geometry &geometry,
                          const BrowserInputContext::State &state) {
    if (!enabled || binding.scope != token)
      return false;
    const bool admitted = context.admit(binding, geometry, state);
    notifyContext();
    return admitted;
  }
  bool retireNativeContext(const BrowserInputContext::Binding &binding) {
    if (!context.matches(binding))
      return false;
    clearNativeContext();
    return true;
  }
  void clearNativeContext() {
    context.clear();
    notifyContext();
  }
  void setInputEnabled(bool value) {
    if (enabled == value)
      return;
    enabled = value;
    setActiveFocusOnTab(enabled);
    if (!enabled) {
      composing = false;
      clearNativeContext();
    }
    emit inputEnabledChanged();
    if (hasActiveFocus() && window() && window()->isVisible() &&
        window()->isActive())
      QGuiApplication::inputMethod()->update(Qt::ImEnabled);
  }
  void setScopeToken(const QString &value) {
    if (token == value)
      return;
    token = value;
    composing = false;
    clearNativeContext();
    emit scopeTokenChanged();
  }
  QVariant inputMethodQuery(Qt::InputMethodQuery query) const override {
    if (query == Qt::ImEnabled)
      return enabled && isVisible() && window() && window()->isVisible() &&
             window()->isActive() && !token.isEmpty();
    if (!enabled || !isVisible() || !hasActiveFocus() || !window() ||
        !window()->isVisible() || !window()->isActive() || token.isEmpty())
      return {};
    return context.query(query);
  }

signals:
  void inputEnabledChanged();
  void scopeTokenChanged();
  void contextChanged();
  void input(const QString &scopeToken, const QVariantMap &event);
  void inputRejected(const QString &reason);
  void contextRequested(const QString &scopeToken);

protected:
  void notifyContext() {
    emit contextChanged();
    if (hasActiveFocus() && window() && window()->isActive())
      QGuiApplication::inputMethod()->update(Qt::ImQueryInput | Qt::ImHints);
  }
  void focusOutEvent(QFocusEvent *event) override {
    clearNativeContext();
    QQuickItem::focusOutEvent(event);
  }
  void itemChange(ItemChange change, const ItemChangeData &value) override {
    if (change == ItemVisibleHasChanged || change == ItemSceneChange)
      clearNativeContext();
    QQuickItem::itemChange(change, value);
  }
  void focusInEvent(QFocusEvent *event) override {
    QQuickItem::focusInEvent(event);
    if (enabled && window() && window()->isVisible() && window()->isActive() &&
        !token.isEmpty())
      emit contextRequested(token);
  }
  void inputMethodEvent(QInputMethodEvent *event) override {
    if (!enabled || !isVisible() || !hasActiveFocus() || !window() ||
        !window()->isVisible() || !window()->isActive() || token.isEmpty()) {
      event->ignore();
      return;
    }
    if (event->replacementStart() || event->replacementLength()) {
      emit inputRejected(QStringLiteral("browser_ime_context_unavailable"));
      event->ignore();
      return;
    }
    const auto commit = event->commitString(), preedit = event->preeditString();
    if (commit.size() > 4096 || preedit.size() > 4096) {
      emit inputRejected(QStringLiteral("invalid_browser_input"));
      event->ignore();
      return;
    }
    int cursor = int(preedit.size());
    for (const auto &attribute : event->attributes()) {
      if (attribute.type == QInputMethodEvent::Selection) {
        emit inputRejected(QStringLiteral("browser_ime_context_unavailable"));
        event->ignore();
        return;
      }
      if (attribute.type == QInputMethodEvent::Cursor)
        cursor = qBound(0, attribute.start, int(preedit.size()));
    }
    // Every emission carries the original mount token. A callback replacing
    // the surface cannot redirect the remainder of an event into another tab.
    const auto admitted = token;
    const bool wasComposing = composing;
    if (!commit.isEmpty()) {
      composing = false;
      emit input(admitted, {{"type", "text"}, {"text", commit}});
    }
    if (token != admitted || !enabled)
      return;
    if (!preedit.isEmpty() || (wasComposing && commit.isEmpty())) {
      composing = !preedit.isEmpty();
      emit input(admitted, {{"type", "composition"},
                            {"preedit", preedit},
                            {"selectionStart", cursor},
                            {"selectionEnd", cursor}});
    }
    event->accept();
  }
};
