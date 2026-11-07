#pragma once
// Receives actual Qt input-method events on the owning GUI connection. Engine
// surrounding-text/caret queries are deliberately unavailable until supplied by
// a faithful native engine boundary; no DOM-derived context is invented here.
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
  Q_PROPERTY(bool contextAvailable READ contextAvailable CONSTANT)
  bool enabled = false, composing = false;
  QString token;

public:
  explicit BrowserInputItem(QQuickItem *parent = nullptr) : QQuickItem(parent) {
    setFlag(ItemAcceptsInputMethod, true);
  }
  bool inputEnabled() const { return enabled; }
  QString scopeToken() const { return token; }
  bool contextAvailable() const { return false; }
  void setInputEnabled(bool value) {
    if (enabled == value)
      return;
    enabled = value;
    setActiveFocusOnTab(enabled);
    if (!enabled)
      composing = false;
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
    emit scopeTokenChanged();
  }
  QVariant inputMethodQuery(Qt::InputMethodQuery query) const override {
    if (query == Qt::ImEnabled)
      return enabled && isVisible() && window() && window()->isVisible() &&
             window()->isActive() && !token.isEmpty();
    // Invalid variants distinguish an unavailable native query from empty text,
    // a guessed caret rectangle, a password hint or an invented selection.
    return {};
  }

signals:
  void inputEnabledChanged();
  void scopeTokenChanged();
  void input(const QString &scopeToken, const QVariantMap &event);
  void inputRejected(const QString &reason);
  void contextRequested(const QString &scopeToken);

protected:
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
