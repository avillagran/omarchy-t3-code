#include "terminalsurface.h"
#include <QFontMetricsF>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QPainter>
#include <QWheelEvent>
#include <algorithm>
#include <cmath>

namespace {
constexpr int ScrollbackRows = 10000; // Pinned Ghostty terminal/core.ts limit.
QString characters(const VTermScreenCell &cell) {
    if (cell.chars[0] == UINT32_MAX) return {};
    int length = 0;
    while (length < VTERM_MAX_CHARS_PER_CELL && cell.chars[length]) ++length;
    return QString::fromUcs4(reinterpret_cast<const char32_t *>(cell.chars), length);
}
bool precedes(qint64 ar, int ac, qint64 br, int bc) { return ar < br || (ar == br && ac < bc); }
}

TerminalSurface::TerminalSurface(QQuickItem *parent) : QQuickPaintedItem(parent) {
    setAcceptedMouseButtons(Qt::AllButtons);
    setFlag(ItemAcceptsInputMethod, true);
    setClip(true);
    setAntialiasing(false);
    m_font.setFamily("monospace");
    m_font.setStyleHint(QFont::Monospace);
    m_font.setFixedPitch(true);
    m_font.setPixelSize(13);
    updateMetrics();
    m_terminal = vterm_new(m_rows, m_columns);
    vterm_set_utf8(m_terminal, 1);
    m_state = vterm_obtain_state(m_terminal);
    m_screen = vterm_obtain_screen(m_terminal);
    static const VTermScreenCallbacks callbacks = {damaged, moved, cursorMoved, propertyChanged,
        rangBell, resized, pushed, popped, historyCleared};
    vterm_screen_set_callbacks(m_screen, &callbacks, this);
    vterm_output_set_callback(m_terminal, output, this);
    vterm_screen_enable_altscreen(m_screen, 1);
    vterm_screen_enable_reflow(m_screen, true);
    vterm_screen_set_damage_merge(m_screen, VTERM_DAMAGE_ROW);
    updateDefaultColors();
    vterm_screen_reset(m_screen, 1);
    m_blink.setInterval(500);
    connect(&m_blink, &QTimer::timeout, this, [this] {
        if (isVisible() && hasActiveFocus() && m_cursorBlink) { m_blinkPhase = !m_blinkPhase; update(); }
    });
    m_blink.start();
}
TerminalSurface::~TerminalSurface() { vterm_free(m_terminal); }
void TerminalSurface::updateMetrics() {
    const QFontMetricsF metrics(m_font);
    m_cellWidth = std::max<qreal>(1, metrics.horizontalAdvance(QLatin1Char('M')));
    m_cellHeight = std::max<qreal>(1, std::ceil(metrics.height()));
    m_ascent = metrics.ascent();
}
void TerminalSurface::setFontFamily(const QString &family) {
    if (family.isEmpty() || family == m_font.family()) return;
    m_font.setFamily(family); updateMetrics(); updateGrid(); emit fontChanged(); update();
}
void TerminalSurface::setFontPixelSize(qreal size) {
    if (!std::isfinite(size)) return;
    int pixels = qBound(6, qRound(size), 96);
    if (pixels == m_font.pixelSize()) return;
    m_font.setPixelSize(pixels); updateMetrics(); updateGrid(); emit fontChanged(); update();
}
void TerminalSurface::updateDefaultColors() {
    VTermColor fg, bg;
    vterm_color_rgb(&fg, m_foreground.red(), m_foreground.green(), m_foreground.blue());
    vterm_color_rgb(&bg, m_background.red(), m_background.green(), m_background.blue());
    vterm_screen_set_default_colors(m_screen, &fg, &bg);
}
void TerminalSurface::setForeground(const QColor &value) {
    if (!value.isValid() || value == m_foreground) return;
    m_foreground = value; updateDefaultColors(); emit colorsChanged(); update();
}
void TerminalSurface::setBackground(const QColor &value) {
    if (!value.isValid() || value == m_background) return;
    m_background = value; updateDefaultColors(); emit colorsChanged(); update();
}
void TerminalSurface::setSelectionColor(const QColor &value) {
    if (!value.isValid() || value == m_selectionColor) return;
    m_selectionColor = value; emit colorsChanged(); update();
}
void TerminalSurface::setGrid(int columns, int rows) {
    columns = qBound(1, columns, 1000); rows = qBound(1, rows, 500);
    if (columns == m_columns && rows == m_rows) return;
    clearSelection();
    m_columns = columns; m_rows = rows;
    vterm_set_size(m_terminal, rows, columns);
    vterm_screen_flush_damage(m_screen);
    emit dimensionsChanged(); emit gridChanged(columns, rows); update();
}
void TerminalSurface::updateGrid() {
    if (width() > 0 && height() > 0) setGrid(int(width() / m_cellWidth), int(height() / m_cellHeight));
}
void TerminalSurface::geometryChange(const QRectF &current, const QRectF &previous) {
    QQuickPaintedItem::geometryChange(current, previous);
    if (current.size() != previous.size() && m_terminal) updateGrid();
}
void TerminalSurface::feed(const QString &text) { feedBytes(text.toUtf8()); }
void TerminalSurface::feedBytes(const QByteArray &bytes) {
    if (bytes.isEmpty()) return;
    vterm_input_write(m_terminal, bytes.constData(), size_t(bytes.size()));
    vterm_screen_flush_damage(m_screen);
    update();
}
void TerminalSurface::reset() {
    clearSelection(); m_history.clear(); m_pruned = 0; m_scrollOffset = 0;
    vterm_screen_reset(m_screen, 1); vterm_screen_flush_damage(m_screen);
    m_title.clear(); emit titleChanged(); emit scrollChanged(); update();
}
qint64 TerminalSurface::visibleStart() const {
    return m_pruned + qint64(m_history.size()) - (m_alternate ? 0 : m_scrollOffset);
}
void TerminalSurface::setScrollOffset(int rows) {
    rows = m_alternate ? 0 : qBound(0, rows, int(m_history.size()));
    if (rows == m_scrollOffset) return;
    m_scrollOffset = rows; emit scrollChanged(); update();
}
VTermScreenCell TerminalSurface::cell(qint64 row, int column) const {
    VTermScreenCell value{}; value.width = 1;
    value.fg.type = VTERM_COLOR_DEFAULT_FG; value.bg.type = VTERM_COLOR_DEFAULT_BG;
    if (column < 0) return value;
    const qint64 index = row - m_pruned;
    if (index < 0) return value;
    if (index < qint64(m_history.size())) {
        const auto &line = m_history[size_t(index)].cells;
        if (column < int(line.size())) value = line[size_t(column)];
    } else if (index - qint64(m_history.size()) < m_rows && column < m_columns) {
        vterm_screen_get_cell(m_screen, VTermPos{int(index - qint64(m_history.size())), column}, &value);
    }
    // libvterm exposes a wide glyph's continuation as UINT32_MAX, not a scalar.
    if (value.chars[0] == UINT32_MAX) { value.chars[0] = 0; value.width = 0; }
    return value;
}
bool TerminalSurface::continuation(qint64 row) const {
    const qint64 index = row - m_pruned;
    if (index < 0) return false;
    if (index < qint64(m_history.size())) return m_history[size_t(index)].continuation;
    int screenRow = int(index - qint64(m_history.size()));
    const auto *info = screenRow < m_rows ? vterm_state_get_lineinfo(m_state, screenRow) : nullptr;
    return info && info->continuation;
}
QColor TerminalSurface::color(VTermColor value, bool foreground) const {
    if (VTERM_COLOR_IS_DEFAULT_FG(&value)) return m_foreground;
    if (VTERM_COLOR_IS_DEFAULT_BG(&value)) return m_background;
    vterm_screen_convert_color_to_rgb(m_screen, &value);
    if (!VTERM_COLOR_IS_RGB(&value)) return foreground ? m_foreground : m_background;
    return QColor(value.rgb.red, value.rgb.green, value.rgb.blue);
}
bool TerminalSurface::selected(qint64 row, int column) const {
    if (!m_selected) return false;
    Position first = m_anchor, last = m_end;
    if (precedes(last.row, last.column, first.row, first.column)) std::swap(first, last);
    return !precedes(row, column, first.row, first.column) && precedes(row, column, last.row, last.column);
}
void TerminalSurface::paint(QPainter *painter) {
    painter->fillRect(boundingRect(), m_background);
    painter->setRenderHint(QPainter::TextAntialiasing, true);
    const qint64 first = visibleStart();
    for (int row = 0; row < m_rows; ++row) for (int column = 0; column < m_columns; ++column) {
        auto value = cell(first + row, column);
        if (value.width == 0) continue;
        const QRectF bounds(column * m_cellWidth, row * m_cellHeight, std::max(1, int(value.width)) * m_cellWidth, m_cellHeight);
        QColor fg = color(value.fg, true), bg = color(value.bg, false);
        if (value.attrs.reverse) std::swap(fg, bg);
        if (bg != m_background) painter->fillRect(bounds, bg);
        if (selected(first + row, column)) painter->fillRect(bounds, m_selectionColor);
        if (value.attrs.conceal) continue;
        const QString text = characters(value);
        if (text.isEmpty()) continue;
        QFont font = m_font; font.setBold(value.attrs.bold); font.setItalic(value.attrs.italic);
        font.setUnderline(value.attrs.underline != 0); font.setStrikeOut(value.attrs.strike);
        painter->setFont(font); painter->setPen(fg); painter->save(); painter->setClipRect(bounds);
        painter->drawText(QPointF(bounds.x(), bounds.y() + m_ascent), text); painter->restore();
    }
    if (!m_scrollOffset && m_cursorVisible && (!m_cursorBlink || m_blinkPhase || !hasActiveFocus())) {
        QRectF cursor(m_cursor.col * m_cellWidth, m_cursor.row * m_cellHeight, m_cellWidth, m_cellHeight);
        painter->setPen(m_foreground);
        if (!hasActiveFocus()) painter->drawRect(cursor.adjusted(.5, .5, -.5, -.5));
        else if (m_cursorShape == VTERM_PROP_CURSORSHAPE_UNDERLINE) painter->fillRect(QRectF(cursor.x(), cursor.bottom() - 2, cursor.width(), 2), m_foreground);
        else if (m_cursorShape == VTERM_PROP_CURSORSHAPE_BAR_LEFT) painter->fillRect(QRectF(cursor.x(), cursor.y(), 2, cursor.height()), m_foreground);
        else { QColor fill = m_foreground; fill.setAlpha(100); painter->fillRect(cursor, fill); }
        if (!m_preedit.isEmpty()) { painter->setFont(m_font); painter->drawText(QPointF(cursor.x(), cursor.y() + m_ascent), m_preedit); }
    }
}
QString TerminalSurface::rowText(int row) const {
    if (row < 0 || row >= m_rows) return {};
    QString text;
    for (int column = 0; column < m_columns; ++column) {
        auto value = cell(visibleStart() + row, column);
        if (value.width == 0) continue;
        QString part = characters(value); text += part.isEmpty() ? QStringLiteral(" ") : part;
    }
    while (text.endsWith(QLatin1Char(' '))) text.chop(1);
    return text;
}
QVariantMap TerminalSurface::cellAt(int row, int column) const {
    if (row < 0 || row >= m_rows || column < 0 || column >= m_columns) return {};
    const auto value = cell(visibleStart() + row, column);
    QColor fg = color(value.fg, true), bg = color(value.bg, false);
    if (value.attrs.reverse) std::swap(fg, bg);
    return {{"text", characters(value)}, {"width", int(value.width)}, {"foreground", fg}, {"background", bg},
        {"bold", bool(value.attrs.bold)}, {"italic", bool(value.attrs.italic)}, {"underline", bool(value.attrs.underline)}};
}
void TerminalSurface::clearSelection() {
    if (!m_selected) return;
    m_selected = false; emit selectionChanged(); update();
}
void TerminalSurface::selectRange(int firstRow, int firstColumn, int lastRow, int lastColumn) {
    m_anchor = {visibleStart() + qBound(0, firstRow, m_rows - 1), qBound(0, firstColumn, m_columns)};
    m_end = {visibleStart() + qBound(0, lastRow, m_rows - 1), qBound(0, lastColumn, m_columns)};
    m_selected = m_anchor.row != m_end.row || m_anchor.column != m_end.column;
    emit selectionChanged(); update();
}
QString TerminalSurface::selectedText() const {
    if (!m_selected) return {};
    Position first = m_anchor, last = m_end;
    if (precedes(last.row, last.column, first.row, first.column)) std::swap(first, last);
    QString result;
    for (qint64 row = std::max(first.row, m_pruned); row <= last.row; ++row) {
        QString line;
        int begin = row == first.row ? first.column : 0, end = row == last.row ? last.column : m_columns;
        for (int column = begin; column < end; ++column) {
            auto value = cell(row, column); if (value.width == 0) continue;
            QString text = characters(value); line += text.isEmpty() ? QStringLiteral(" ") : text;
        }
        while (line.endsWith(QLatin1Char(' '))) line.chop(1);
        result += line;
        if (row != last.row && !continuation(row + 1)) result += QLatin1Char('\n');
    }
    return result;
}
void TerminalSurface::copySelection() { if (m_selected) emit copyRequested(selectedText()); }
void TerminalSurface::paste(const QString &text) {
    if (text.isEmpty()) return;
    clearSelection(); setScrollOffset(0);
    QString normalized = text; normalized.replace(QStringLiteral("\r\n"), QStringLiteral("\n")); normalized.replace(QLatin1Char('\n'), QLatin1Char('\r'));
    vterm_keyboard_start_paste(m_terminal); emit input(normalized); vterm_keyboard_end_paste(m_terminal);
}
VTermModifier TerminalSurface::modifiers(Qt::KeyboardModifiers mods) {
    int result = VTERM_MOD_NONE;
    if (mods.testFlag(Qt::ShiftModifier)) result |= VTERM_MOD_SHIFT;
    if (mods.testFlag(Qt::AltModifier)) result |= VTERM_MOD_ALT;
    if (mods.testFlag(Qt::ControlModifier)) result |= VTERM_MOD_CTRL;
    return VTermModifier(result);
}
void TerminalSurface::writeUnicode(const QString &text, VTermModifier mods) {
    for (char32_t point : text.toUcs4()) vterm_keyboard_unichar(m_terminal, uint32_t(point), mods);
}
void TerminalSurface::keyPressEvent(QKeyEvent *event) {
    const auto mods = event->modifiers();
    bool clipboardModifier = mods.testFlag(Qt::ControlModifier) && mods.testFlag(Qt::ShiftModifier);
#ifdef Q_OS_MACOS
    clipboardModifier = clipboardModifier || mods.testFlag(Qt::MetaModifier);
#endif
    if (clipboardModifier && event->key() == Qt::Key_C) { copySelection(); event->accept(); return; }
    if (clipboardModifier && event->key() == Qt::Key_V) { emit pasteRequested(); event->accept(); return; }
    VTermKey key = VTERM_KEY_NONE;
    switch (event->key()) {
    case Qt::Key_Return: case Qt::Key_Enter: key = VTERM_KEY_ENTER; break;
    case Qt::Key_Tab: case Qt::Key_Backtab: key = VTERM_KEY_TAB; break;
    case Qt::Key_Backspace: key = VTERM_KEY_BACKSPACE; break;
    case Qt::Key_Escape: key = VTERM_KEY_ESCAPE; break;
    case Qt::Key_Up: key = VTERM_KEY_UP; break; case Qt::Key_Down: key = VTERM_KEY_DOWN; break;
    case Qt::Key_Left: key = VTERM_KEY_LEFT; break; case Qt::Key_Right: key = VTERM_KEY_RIGHT; break;
    case Qt::Key_Home: key = VTERM_KEY_HOME; break; case Qt::Key_End: key = VTERM_KEY_END; break;
    case Qt::Key_Insert: key = VTERM_KEY_INS; break; case Qt::Key_Delete: key = VTERM_KEY_DEL; break;
    case Qt::Key_PageUp: key = VTERM_KEY_PAGEUP; break; case Qt::Key_PageDown: key = VTERM_KEY_PAGEDOWN; break;
    default: if (event->key() >= Qt::Key_F1 && event->key() <= Qt::Key_F35) key = VTermKey(VTERM_KEY_FUNCTION(event->key() - Qt::Key_F1 + 1));
    }
    clearSelection(); setScrollOffset(0); m_blinkPhase = true;
    auto modifier = modifiers(mods);
    if (event->key() == Qt::Key_Backtab) modifier = VTermModifier(modifier | VTERM_MOD_SHIFT);
    if (key != VTERM_KEY_NONE) vterm_keyboard_key(m_terminal, key, modifier);
    else if (!event->text().isEmpty()) {
        // Qt already turns Ctrl+C into U+0003. libvterm needs the original
        // printable key plus Ctrl, otherwise it encodes a CSI-u control scalar.
        if ((modifier & VTERM_MOD_CTRL) && event->text()[0].unicode() < 0x20 && event->key() >= 0x20 && event->key() <= 0x7e) {
            uint32_t point = uint32_t(event->key());
            if (point >= 'A' && point <= 'Z') point += 'a' - 'A';
            vterm_keyboard_unichar(m_terminal, point, modifier);
        } else writeUnicode(event->text(), mods.testFlag(Qt::GroupSwitchModifier) ? VTERM_MOD_NONE : modifier);
    }
    else { event->ignore(); return; }
    event->accept(); update();
}
TerminalSurface::Position TerminalSurface::positionAt(const QPointF &point) const {
    return {visibleStart() + qBound(0, int(point.y() / m_cellHeight), m_rows - 1), qBound(0, int(point.x() / m_cellWidth + .5), m_columns)};
}
void TerminalSurface::mousePressEvent(QMouseEvent *event) {
    forceActiveFocus(Qt::MouseFocusReason);
    if (event->button() == Qt::MiddleButton) { emit pasteRequested(); event->accept(); return; }
    if (m_mouseMode && !event->modifiers().testFlag(Qt::ShiftModifier)) {
        vterm_mouse_move(m_terminal, qBound(0, int(event->position().y() / m_cellHeight), m_rows - 1), qBound(0, int(event->position().x() / m_cellWidth), m_columns - 1), modifiers(event->modifiers()));
        vterm_mouse_button(m_terminal, event->button() == Qt::LeftButton ? 1 : event->button() == Qt::RightButton ? 3 : 2, true, modifiers(event->modifiers()));
        event->accept(); return;
    }
    if (event->button() != Qt::LeftButton) { event->ignore(); return; }
    m_anchor = m_end = positionAt(event->position()); m_dragging = true; clearSelection(); event->accept();
}
void TerminalSurface::mouseMoveEvent(QMouseEvent *event) {
    if (m_mouseMode && !event->modifiers().testFlag(Qt::ShiftModifier)) {
        vterm_mouse_move(m_terminal, qBound(0, int(event->position().y() / m_cellHeight), m_rows - 1), qBound(0, int(event->position().x() / m_cellWidth), m_columns - 1), modifiers(event->modifiers()));
    } else if (m_dragging) {
        m_end = positionAt(event->position()); m_selected = m_end.row != m_anchor.row || m_end.column != m_anchor.column; emit selectionChanged(); update();
    }
    event->accept();
}
void TerminalSurface::mouseReleaseEvent(QMouseEvent *event) {
    if (m_mouseMode && !event->modifiers().testFlag(Qt::ShiftModifier)) vterm_mouse_button(m_terminal, event->button() == Qt::LeftButton ? 1 : event->button() == Qt::RightButton ? 3 : 2, false, modifiers(event->modifiers()));
    m_dragging = false; event->accept();
}
void TerminalSurface::mouseDoubleClickEvent(QMouseEvent *event) {
    if (m_mouseMode && !event->modifiers().testFlag(Qt::ShiftModifier)) { mousePressEvent(event); return; }
    const Position position = positionAt(event->position());
    auto word = [&](int col) { QString text = characters(cell(position.row, col)); return !text.isEmpty() && !text[0].isSpace() && !QStringLiteral("()[]{}<>;:'\"").contains(text[0]); };
    int first = qBound(0, position.column, m_columns - 1), last = first;
    while (first > 0 && word(first - 1)) --first;
    while (last < m_columns && word(last)) ++last;
    m_anchor = {position.row, first}; m_end = {position.row, last}; m_selected = first != last; emit selectionChanged(); update(); event->accept();
}
void TerminalSurface::wheelEvent(QWheelEvent *event) {
    int delta = event->angleDelta().y() ? event->angleDelta().y() / 40 : qRound(event->pixelDelta().y() / m_cellHeight);
    if (!delta) { event->accept(); return; }
    if (m_mouseMode && !event->modifiers().testFlag(Qt::ShiftModifier)) {
        for (int count = 0; count < std::min(1000, std::abs(delta)); ++count) vterm_mouse_button(m_terminal, delta > 0 ? 4 : 5, true, modifiers(event->modifiers()));
    } else if (m_alternate) {
        for (int count = 0; count < std::min(1000, std::abs(delta)); ++count) vterm_keyboard_key(m_terminal, delta > 0 ? VTERM_KEY_UP : VTERM_KEY_DOWN, VTERM_MOD_NONE);
    } else setScrollOffset(m_scrollOffset + delta);
    event->accept();
}
void TerminalSurface::inputMethodEvent(QInputMethodEvent *event) {
    if (!event->commitString().isEmpty()) { clearSelection(); setScrollOffset(0); emit input(event->commitString()); }
    m_preedit = event->preeditString(); event->accept(); update();
}
QVariant TerminalSurface::inputMethodQuery(Qt::InputMethodQuery query) const {
    if (query == Qt::ImEnabled) return true;
    if (query == Qt::ImFont) return m_font;
    if (query == Qt::ImCursorRectangle) return QRectF(m_cursor.col * m_cellWidth, m_cursor.row * m_cellHeight, m_cellWidth, m_cellHeight);
    return QQuickPaintedItem::inputMethodQuery(query);
}
int TerminalSurface::damaged(VTermRect rect, void *user) {
    auto *self = static_cast<TerminalSurface *>(user);
    const qint64 first = self->m_pruned + qint64(self->m_history.size());
    if (self->m_selected && ((self->m_anchor.row >= first + rect.start_row && self->m_anchor.row < first + rect.end_row) || (self->m_end.row >= first + rect.start_row && self->m_end.row < first + rect.end_row))) self->clearSelection();
    self->update(); return 1;
}
int TerminalSurface::moved(VTermRect, VTermRect, void *user) { static_cast<TerminalSurface *>(user)->update(); return 1; }
int TerminalSurface::cursorMoved(VTermPos current, VTermPos, int visible, void *user) {
    auto *self = static_cast<TerminalSurface *>(user); self->m_cursor = current; self->m_cursorVisible = visible; self->m_blinkPhase = true; emit self->cursorChanged(); self->update(); return 1;
}
int TerminalSurface::propertyChanged(VTermProp property, VTermValue *value, void *user) {
    auto *self = static_cast<TerminalSurface *>(user);
    switch (property) {
    case VTERM_PROP_CURSORVISIBLE: self->m_cursorVisible = value->boolean; break;
    case VTERM_PROP_CURSORBLINK: self->m_cursorBlink = value->boolean; break;
    case VTERM_PROP_CURSORSHAPE: self->m_cursorShape = value->number; break;
    case VTERM_PROP_ALTSCREEN: self->m_alternate = value->boolean; self->setScrollOffset(0); self->clearSelection(); emit self->alternateScreenChanged(); break;
    case VTERM_PROP_MOUSE: self->m_mouseMode = value->number; break;
    case VTERM_PROP_TITLE:
        if (value->string.initial) self->m_title.clear();
        self->m_title += QString::fromUtf8(value->string.str, qsizetype(value->string.len));
        if (value->string.final) emit self->titleChanged();
        break;
    default: break;
    }
    self->update(); return 1;
}
int TerminalSurface::rangBell(void *user) { emit static_cast<TerminalSurface *>(user)->bell(); return 1; }
int TerminalSurface::resized(int, int, void *user) { static_cast<TerminalSurface *>(user)->update(); return 1; }
int TerminalSurface::pushed(int columns, const VTermScreenCell *cells, void *user) {
    auto *self = static_cast<TerminalSurface *>(user);
    if (self->m_alternate) return 1;
    HistoryLine line;
    const auto *info = vterm_state_get_lineinfo(self->m_state, 0); line.continuation = info && info->continuation;
    int used = columns;
    while (used > 0) {
        const auto &cell = cells[used - 1];
        if ((cell.chars[0] != 0 && cell.chars[0] != ' ') || cell.attrs.underline || cell.attrs.reverse || cell.attrs.strike || !VTERM_COLOR_IS_DEFAULT_BG(&cell.bg)) break;
        --used;
    }
    line.cells.assign(cells, cells + used); self->m_history.push_back(std::move(line));
    if (self->m_scrollOffset) ++self->m_scrollOffset;
    if (self->m_history.size() > ScrollbackRows) { self->m_history.pop_front(); ++self->m_pruned; if (self->m_selected && (self->m_anchor.row < self->m_pruned || self->m_end.row < self->m_pruned)) self->clearSelection(); }
    self->m_scrollOffset = std::min(self->m_scrollOffset, int(self->m_history.size())); emit self->scrollChanged(); return 1;
}
int TerminalSurface::popped(int columns, VTermScreenCell *cells, void *user) {
    auto *self = static_cast<TerminalSurface *>(user);
    if (self->m_history.empty() || self->m_alternate) return 0;
    const auto line = std::move(self->m_history.back()); self->m_history.pop_back();
    for (int column = 0; column < columns; ++column) {
        VTermScreenCell value{}; value.width = 1; value.fg.type = VTERM_COLOR_DEFAULT_FG; value.bg.type = VTERM_COLOR_DEFAULT_BG;
        if (column < int(line.cells.size())) value = line.cells[size_t(column)];
        cells[column] = value;
    }
    self->m_scrollOffset = std::min(self->m_scrollOffset, int(self->m_history.size())); emit self->scrollChanged(); return 1;
}
int TerminalSurface::historyCleared(void *user) {
    auto *self = static_cast<TerminalSurface *>(user); self->m_history.clear(); self->m_scrollOffset = 0; self->clearSelection(); emit self->scrollChanged(); return 1;
}
void TerminalSurface::output(const char *bytes, size_t length, void *user) { emit static_cast<TerminalSurface *>(user)->input(QString::fromUtf8(bytes, qsizetype(length))); }
