#pragma once
// Current native state only. The caller must authenticate the engine connection
// and bind the real surface/window and captured frame before calling admit().
#include <QRectF>
#include <QString>
#include <QVariant>
#include <QtMath>
#include <optional>

class BrowserInputContext {
public:
  struct Binding {
    QString scope, connection, surface, tab, frame;
    bool operator==(const Binding &other) const {
      return scope == other.scope && connection == other.connection &&
             surface == other.surface && tab == other.tab && frame == other.frame;
    }
    bool valid() const {
      return !scope.isEmpty() && !connection.isEmpty() && !surface.isEmpty() &&
             !tab.isEmpty() && !frame.isEmpty();
    }
  };
  struct Geometry {
    // All native inputs are logical surface coordinates. Presentation is the
    // actual painted frame rectangle in the receiving QQuickItem's coordinates.
    QRectF content;
    QRectF presentation;
  };
  struct State {
    quint32 serial = 0;
    std::optional<QRectF> caret;
    std::optional<QByteArray> surrounding;
    int cursorByte = 0, anchorByte = 0;
    quint32 hints = 0, purpose = 0;
  };

private:
  Binding binding;
  std::optional<State> current;
  std::optional<QRectF> projected;
  QString text;
  int cursor = 0, anchor = 0;
  static bool finite(const QRectF &rect) {
    return qIsFinite(rect.x()) && qIsFinite(rect.y()) &&
           qIsFinite(rect.width()) && qIsFinite(rect.height());
  }
  static std::optional<int> utf16Offset(const QByteArray &bytes, int offset) {
    if (offset < 0 || offset > bytes.size())
      return {};
    const auto prefix = bytes.left(offset);
    const auto decoded = QString::fromUtf8(prefix);
    if (decoded.toUtf8() != prefix)
      return {};
    return int(decoded.size());
  }

public:
  void bind(const Binding &value) {
    if (!(binding == value)) {
      clear();
      binding = value;
    }
  }
  void clear() {
    current.reset();
    projected.reset();
    text.clear();
    cursor = anchor = 0;
  }
  bool admit(const Binding &source, const Geometry &geometry, const State &state) {
    if (!binding.valid() || !(binding == source))
      return false;
    if (current && quint32(state.serial-current->serial) > 0x7fffffffu)
      return false; // Older acknowledged state cannot replace the current one.
    // A stale result cannot invalidate a newer admitted lease. An invalid
    // result for the current lease must not leave its previous state queryable.
    clear();
    if (!finite(geometry.content) || !finite(geometry.presentation) ||
        geometry.content.width() <= 0 || geometry.content.height() <= 0 ||
        geometry.presentation.width() <= 0 || geometry.presentation.height() <= 0 ||
        state.purpose > 13 || (state.hints & ~quint32(1023)))
      return false;
    if (state.caret) {
      const auto &rect = *state.caret;
      if (!finite(rect) || rect.width() < 0 || rect.height() < 0)
        return false;
      const qreal sx = geometry.presentation.width() / geometry.content.width();
      const qreal sy = geometry.presentation.height() / geometry.content.height();
      projected = QRectF(geometry.presentation.x() + (rect.x() - geometry.content.x()) * sx,
                         geometry.presentation.y() + (rect.y() - geometry.content.y()) * sy,
                         rect.width() * sx, rect.height() * sy);
      if (!finite(*projected)) {
        clear();
        return false;
      }
    }
    if (state.surrounding) {
      const auto &bytes = *state.surrounding;
      const auto decoded = QString::fromUtf8(bytes);
      const auto start = utf16Offset(bytes, state.cursorByte);
      const auto end = utf16Offset(bytes, state.anchorByte);
      if (bytes.size() > 4000 || bytes.contains('\0') || decoded.toUtf8() != bytes || !start || !end) {
        clear();
        return false;
      }
      text = decoded;
      cursor = *start;
      anchor = *end;
    }
    current = state;
    return true;
  }
  bool available() const { return current.has_value(); }
  QVariant nativeHints() const { return current ? QVariant(current->hints) : QVariant(); }
  QVariant nativePurpose() const { return current ? QVariant(current->purpose) : QVariant(); }
  QVariant query(Qt::InputMethodQuery kind) const {
    if (!current)
      return {};
    if (kind == Qt::ImCursorRectangle)
      return projected ? QVariant(*projected) : QVariant();
    if (kind == Qt::ImHints) {
      // Translate only explicit equivalent constraints. Completion, spellcheck,
      // titlecase and Latin preferences remain preserved in nativeHints(); they
      // are not replaced with a guessed Qt predictive-text policy.
      Qt::InputMethodHints result = Qt::ImhNone;
      if (current->hints & 8) result |= Qt::ImhPreferLowercase;
      if (current->hints & 16) result |= Qt::ImhPreferUppercase;
      if (current->hints & 64) result |= Qt::ImhHiddenText;
      if (current->hints & 128) result |= Qt::ImhSensitiveData;
      switch (current->purpose) {
      case 2: case 9: result |= Qt::ImhDigitsOnly; break;
      case 3: result |= Qt::ImhFormattedNumbersOnly; break;
      case 4: result |= Qt::ImhDialableCharactersOnly; break;
      case 5: result |= Qt::ImhUrlCharactersOnly; break;
      case 6: result |= Qt::ImhEmailCharactersOnly; break;
      case 10: result |= Qt::ImhDate; break;
      case 11: result |= Qt::ImhTime; break;
      case 12: result |= Qt::ImhDate | Qt::ImhTime; break;
      default: break;
      }
      return int(result);
    }
    if (!current->surrounding)
      return {};
    switch (kind) {
    case Qt::ImSurroundingText: return text;
    case Qt::ImCursorPosition: return cursor;
    case Qt::ImAnchorPosition: return anchor;
    case Qt::ImCurrentSelection:
      return text.mid(qMin(cursor, anchor), qAbs(cursor - anchor));
    default: return {};
    }
  }
};
