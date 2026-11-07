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
#include <QHash>
#include <QPointer>
#include <QList>
#include <functional>

class BrowserInputItem : public QQuickItem {
  Q_OBJECT
  Q_PROPERTY(bool inputEnabled READ inputEnabled WRITE setInputEnabled NOTIFY
                 inputEnabledChanged)
  Q_PROPERTY(QString scopeToken READ scopeToken WRITE setScopeToken NOTIFY
                 scopeTokenChanged)
  Q_PROPERTY(bool contextAvailable READ contextAvailable NOTIFY contextChanged)
  Q_PROPERTY(QVariantMap nativeBinding READ nativeBinding WRITE setNativeBinding NOTIFY nativeBindingChanged)
  Q_PROPERTY(QRectF nativeContentRect READ nativeContentRect WRITE setNativeContentRect NOTIFY nativeGeometryChanged)
  Q_PROPERTY(QRectF framePresentation READ framePresentation WRITE setFramePresentation NOTIFY nativeGeometryChanged)
  bool enabled = false, composing = false;
  QString token;
  BrowserInputContext context;
  std::function<bool()> currentOwner;
  QMetaObject::Connection windowContextConnection;
  QMetaObject::Connection windowVisibilityConnection;
  QVariantMap bindingMetadata;
  BrowserInputContext::Geometry nativeGeometry;
  bool acceptsContext() const {
    return enabled && isVisible() && hasActiveFocus() && window() &&
      window()->isVisible() && window()->isActive() && !token.isEmpty();
  }
  static QHash<QString, QList<QPointer<BrowserInputItem>>> &mounts() {
    static QHash<QString, QList<QPointer<BrowserInputItem>>> values;
    return values;
  }
  void unmount() {
    auto found = mounts().find(token);
    if (found == mounts().end()) return;
    found->removeAll(this);
    const auto remaining=*found;
    if (remaining.isEmpty()) mounts().erase(found);
    else if (remaining.size()==1 && remaining.first() && remaining.first()->acceptsContext())
      emit remaining.first()->contextRequested(remaining.first()->token);
  }
  void bindMetadata() {
    BrowserInputContext::Binding value;
    value.scope = token;
    if (bindingMetadata.size() == 4) {
      auto field = [&](const char *key) {
        const auto entry = bindingMetadata.value(QString::fromLatin1(key));
        const auto text = entry.toString();
        return entry.metaType().id() == QMetaType::QString && text.size() <= 256 ? text : QString();
      };
      value.connection=field("connection");value.surface=field("surface");
      value.tab=field("tab");value.frame=field("frame");
    }
    bindNativeContext(value);
    if (acceptsContext()) emit contextRequested(token);
  }

public:
  explicit BrowserInputItem(QQuickItem *parent = nullptr) : QQuickItem(parent) {
    setFlag(ItemAcceptsInputMethod, true);
    connect(this, &QQuickItem::windowChanged, this, [this](QQuickWindow *value) {
      disconnect(windowContextConnection);
      disconnect(windowVisibilityConnection);
      clearNativeContext();
      if (value) {
        windowContextConnection = connect(value, &QWindow::activeChanged, this,
            [this, value] { if (!value->isActive()) clearNativeContext();
              else if (acceptsContext()) emit contextRequested(token); });
        windowVisibilityConnection = connect(value, &QWindow::visibleChanged, this,
            [this](bool visible) { if (!visible) clearNativeContext(); });
      }
    });
  }
  ~BrowserInputItem() override {
    unmount();
    disconnect(windowContextConnection);
    disconnect(windowVisibilityConnection);
    disconnect(this, nullptr, this, nullptr);
  }
  bool inputEnabled() const { return enabled; }
  QString scopeToken() const { return token; }
  bool contextAvailable() const { return context.available() && (!currentOwner || currentOwner()); }
  bool nativeContextInteractive() const { return acceptsContext(); }
  static BrowserInputItem *nativeMount(const QString &scope) {
    const auto values = mounts().value(scope);
    BrowserInputItem *found = nullptr;
    for (const auto &value : values) {
      if (value && value->scopeToken() == scope) {
        if (found) return nullptr; // Ambiguous GUI mounts are never last-writer-wins.
        found = value;
      }
    }
    return found;
  }
  QVariantMap nativeBinding() const { return bindingMetadata; }
  void setNativeBinding(const QVariantMap &value) {
    QVariantMap admitted;
    if (value.size() == 4) {
      for (const auto *key : {"connection","surface","tab","frame"}) {
        const auto entry = value.value(QString::fromLatin1(key));
        const auto text = entry.toString();
        if (entry.metaType().id() != QMetaType::QString || text.isEmpty() || text.size() > 256) {
          admitted.clear();
          break;
        }
        admitted.insert(QString::fromLatin1(key),text);
      }
    }
    if (bindingMetadata == admitted) return;
    bindingMetadata = admitted;
    bindMetadata();
    emit nativeBindingChanged();
  }
  QRectF nativeContentRect() const { return nativeGeometry.content; }
  QRectF framePresentation() const { return nativeGeometry.presentation; }
  void setNativeContentRect(const QRectF &value) {
    if (nativeGeometry.content == value) return;
    nativeGeometry.content = value;
    clearNativeContext();
    emit nativeGeometryChanged();
    if (acceptsContext()) emit contextRequested(token);
  }
  void setFramePresentation(const QRectF &value) {
    if (nativeGeometry.presentation == value) return;
    nativeGeometry.presentation = value;
    clearNativeContext();
    emit nativeGeometryChanged();
    if (acceptsContext()) emit contextRequested(token);
  }
  bool admitBoundNativeContext(const BrowserInputContext::Binding &binding,
                              const BrowserInputContext::State &state,
                              std::function<bool()> owner) {
    if (!owner) return false;
    return admitNativeContext(binding, nativeGeometry, state,std::move(owner));
  }
  // C++ owner entry points, intentionally not invokable by page JavaScript.
  void bindNativeContext(const BrowserInputContext::Binding &binding) {
    if (!context.matches(binding)) currentOwner={};
    context.bind(binding);
    if (binding.scope != token)
      context.clear();
    notifyContext();
  }
  bool admitNativeContext(const BrowserInputContext::Binding &binding,
                          const BrowserInputContext::Geometry &geometry,
                          const BrowserInputContext::State &state,
                          std::function<bool()> owner={}) {
    if (!acceptsContext() || binding.scope != token)
      return false;
    const bool admitted = context.admit(binding, geometry, state);
    if (admitted) currentOwner=std::move(owner);
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
    currentOwner={};
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
    if (acceptsContext()) emit contextRequested(token);
    if (hasActiveFocus() && window() && window()->isVisible() &&
        window()->isActive())
      QGuiApplication::inputMethod()->update(Qt::ImEnabled);
  }
  void setScopeToken(const QString &value) {
    if (token == value)
      return;
    unmount();
    token = value;
    if (!token.isEmpty()) {
      mounts()[token].append(this);
      const auto peers=mounts().value(token);
      if (peers.size()>1)
        for (const auto &peer : peers) if (peer) peer->clearNativeContext();
    }
    composing = false;
    clearNativeContext();
    if (!bindingMetadata.isEmpty()) bindMetadata();
    emit scopeTokenChanged();
  }
  QVariant inputMethodQuery(Qt::InputMethodQuery query) const override {
    if (query == Qt::ImEnabled)
      return enabled && isVisible() && window() && window()->isVisible() &&
             window()->isActive() && !token.isEmpty();
    if (!enabled || !isVisible() || !hasActiveFocus() || !window() ||
        !window()->isVisible() || !window()->isActive() || token.isEmpty())
      return {};
    return contextAvailable()?context.query(query):QVariant();
  }

signals:
  void inputEnabledChanged();
  void scopeTokenChanged();
  void contextChanged();
  void nativeBindingChanged();
  void nativeGeometryChanged();
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
