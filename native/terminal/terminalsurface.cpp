#include "terminalsurface.h"
#include <QFontMetricsF>
#include <QGuiApplication>
#include <QHoverEvent>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QPainter>
#include <QWheelEvent>
#include <QTouchEvent>
#include <QRegularExpression>
#include <algorithm>
#include <cmath>
#include <functional>
#include <linux/input-event-codes.h>

namespace {
static_assert(GHOSTTY_CELL_WIDE_NARROW == 0 && GHOSTTY_CELL_WIDE_WIDE == 1 && GHOSTTY_CELL_WIDE_SPACER_TAIL == 2 && GHOSTTY_CELL_WIDE_SPACER_HEAD == 3);
constexpr auto EngineRevision = "9f62873bf195e4d8a762d768a1405a5f2f7b1697";
QColor color(GhosttyColorRgb c) { return QColor(c.r, c.g, c.b); }
GhosttyColorRgb rgb(QColor c) { return {uint8_t(c.red()), uint8_t(c.green()), uint8_t(c.blue())}; }
void require(GhosttyResult result, const char *operation) {
    if (result != GHOSTTY_SUCCESS) qFatal("Ghostty %s failed (%d)", operation, int(result));
}
QByteArray encoded(const std::function<GhosttyResult(char *, size_t, size_t *)> &encode) {
    size_t written = 0;
    auto result = encode(nullptr, 0, &written);
    if (result != GHOSTTY_OUT_OF_SPACE || !written) return {};
    QByteArray output(qsizetype(written), Qt::Uninitialized);
    result = encode(output.data(), size_t(output.size()), &written);
    if (result != GHOSTTY_SUCCESS) return {};
    output.resize(qsizetype(written)); return output;
}
// Qt Wayland, XCB and evdev report XKB keycodes (Linux evdev + 8).
// Physical keys remain independent of the active keyboard layout.
GhosttyKey physicalKey(const QKeyEvent &event) {
    const QString platform = QGuiApplication::platformName();
    if ((platform == "xcb" || platform.startsWith("wayland") || platform == "eglfs") && event.nativeScanCode() >= 8) {
        switch (event.nativeScanCode() - 8) {
#define NATIVE(key, value) case KEY_##key: return GHOSTTY_KEY_##value;
        NATIVE(ESC, ESCAPE) NATIVE(1, DIGIT_1) NATIVE(2, DIGIT_2) NATIVE(3, DIGIT_3)
        NATIVE(4, DIGIT_4) NATIVE(5, DIGIT_5) NATIVE(6, DIGIT_6) NATIVE(7, DIGIT_7)
        NATIVE(8, DIGIT_8) NATIVE(9, DIGIT_9) NATIVE(0, DIGIT_0) NATIVE(MINUS, MINUS)
        NATIVE(EQUAL, EQUAL) NATIVE(BACKSPACE, BACKSPACE) NATIVE(TAB, TAB)
        NATIVE(Q, Q) NATIVE(W, W) NATIVE(E, E) NATIVE(R, R) NATIVE(T, T) NATIVE(Y, Y)
        NATIVE(U, U) NATIVE(I, I) NATIVE(O, O) NATIVE(P, P) NATIVE(LEFTBRACE, BRACKET_LEFT)
        NATIVE(RIGHTBRACE, BRACKET_RIGHT) NATIVE(ENTER, ENTER) NATIVE(LEFTCTRL, CONTROL_LEFT)
        NATIVE(A, A) NATIVE(S, S) NATIVE(D, D) NATIVE(F, F) NATIVE(G, G) NATIVE(H, H)
        NATIVE(J, J) NATIVE(K, K) NATIVE(L, L) NATIVE(SEMICOLON, SEMICOLON)
        NATIVE(APOSTROPHE, QUOTE) NATIVE(GRAVE, BACKQUOTE) NATIVE(LEFTSHIFT, SHIFT_LEFT)
        NATIVE(BACKSLASH, BACKSLASH) NATIVE(Z, Z) NATIVE(X, X) NATIVE(C, C) NATIVE(V, V)
        NATIVE(B, B) NATIVE(N, N) NATIVE(M, M) NATIVE(COMMA, COMMA) NATIVE(DOT, PERIOD)
        NATIVE(SLASH, SLASH) NATIVE(RIGHTSHIFT, SHIFT_RIGHT) NATIVE(KPASTERISK, NUMPAD_MULTIPLY)
        NATIVE(LEFTALT, ALT_LEFT) NATIVE(SPACE, SPACE) NATIVE(CAPSLOCK, CAPS_LOCK)
        NATIVE(F1, F1) NATIVE(F2, F2) NATIVE(F3, F3) NATIVE(F4, F4) NATIVE(F5, F5)
        NATIVE(F6, F6) NATIVE(F7, F7) NATIVE(F8, F8) NATIVE(F9, F9) NATIVE(F10, F10)
        NATIVE(NUMLOCK, NUM_LOCK) NATIVE(KP7, NUMPAD_7) NATIVE(KP8, NUMPAD_8)
        NATIVE(KP9, NUMPAD_9) NATIVE(KPMINUS, NUMPAD_SUBTRACT) NATIVE(KP4, NUMPAD_4)
        NATIVE(KP5, NUMPAD_5) NATIVE(KP6, NUMPAD_6) NATIVE(KPPLUS, NUMPAD_ADD)
        NATIVE(KP1, NUMPAD_1) NATIVE(KP2, NUMPAD_2) NATIVE(KP3, NUMPAD_3)
        NATIVE(KP0, NUMPAD_0) NATIVE(KPDOT, NUMPAD_DECIMAL) NATIVE(102ND, INTL_BACKSLASH)
        NATIVE(F11, F11) NATIVE(F12, F12) NATIVE(KPENTER, NUMPAD_ENTER)
        NATIVE(RIGHTCTRL, CONTROL_RIGHT) NATIVE(KPSLASH, NUMPAD_DIVIDE)
        NATIVE(SYSRQ, PRINT_SCREEN) NATIVE(RIGHTALT, ALT_RIGHT) NATIVE(HOME, HOME)
        NATIVE(UP, ARROW_UP) NATIVE(PAGEUP, PAGE_UP) NATIVE(LEFT, ARROW_LEFT)
        NATIVE(RIGHT, ARROW_RIGHT) NATIVE(END, END) NATIVE(DOWN, ARROW_DOWN)
        NATIVE(PAGEDOWN, PAGE_DOWN) NATIVE(INSERT, INSERT) NATIVE(DELETE, DELETE)
        NATIVE(PAUSE, PAUSE) NATIVE(LEFTMETA, META_LEFT) NATIVE(RIGHTMETA, META_RIGHT)
        NATIVE(COMPOSE, CONTEXT_MENU) NATIVE(RO, INTL_RO) NATIVE(YEN, INTL_YEN)
        NATIVE(HENKAN, CONVERT) NATIVE(MUHENKAN, NON_CONVERT) NATIVE(KATAKANAHIRAGANA, KANA_MODE)
#undef NATIVE
        default: break;
        }
    }
    const int key = event.key();
    if (key >= Qt::Key_A && key <= Qt::Key_Z) return GhosttyKey(GHOSTTY_KEY_A + key - Qt::Key_A);
    if (key >= Qt::Key_0 && key <= Qt::Key_9) return GhosttyKey(GHOSTTY_KEY_DIGIT_0 + key - Qt::Key_0);
    if (key >= Qt::Key_F1 && key <= Qt::Key_F25) return GhosttyKey(GHOSTTY_KEY_F1 + key - Qt::Key_F1);
    switch (key) {
#define KEY(qt, ghostty) case Qt::Key_##qt: return GHOSTTY_KEY_##ghostty;
    KEY(Escape, ESCAPE) KEY(Backspace, BACKSPACE) KEY(Return, ENTER) KEY(Enter, NUMPAD_ENTER)
    KEY(Tab, TAB) KEY(Backtab, TAB) KEY(Space, SPACE) KEY(Left, ARROW_LEFT) KEY(Right, ARROW_RIGHT)
    KEY(Up, ARROW_UP) KEY(Down, ARROW_DOWN) KEY(Home, HOME) KEY(End, END) KEY(PageUp, PAGE_UP)
    KEY(PageDown, PAGE_DOWN) KEY(Insert, INSERT) KEY(Delete, DELETE) KEY(Minus, MINUS)
    KEY(Equal, EQUAL) KEY(QuoteLeft, BACKQUOTE) KEY(BracketLeft, BRACKET_LEFT)
    KEY(BracketRight, BRACKET_RIGHT) KEY(Backslash, BACKSLASH) KEY(Semicolon, SEMICOLON)
    KEY(Apostrophe, QUOTE) KEY(Comma, COMMA) KEY(Period, PERIOD) KEY(Slash, SLASH)
    KEY(Shift, SHIFT_LEFT) KEY(Control, CONTROL_LEFT) KEY(Alt, ALT_LEFT) KEY(Meta, META_LEFT)
    KEY(CapsLock, CAPS_LOCK) KEY(NumLock, NUM_LOCK) KEY(ScrollLock, SCROLL_LOCK) KEY(Menu, CONTEXT_MENU)
    KEY(Print, PRINT_SCREEN) KEY(Pause, PAUSE) KEY(Copy, COPY) KEY(Cut, CUT) KEY(Paste, PASTE)
#undef KEY
    default: return GHOSTTY_KEY_UNIDENTIFIED;
    }
}
uint32_t unshifted(const QString &text, Qt::KeyboardModifiers modifiers) {
    const auto points = text.toUcs4(); if (points.size() != 1) return 0;
    const char32_t cp = points[0];
    if (cp >= 'A' && cp <= 'Z') return cp + 32;
    if (!(modifiers & Qt::ShiftModifier)) return cp;
    const QString shifted = QStringLiteral("~!@#$%^&*()_+{}|:\"<>?");
    const QString plain = QStringLiteral("`1234567890-=[]\\;',./");
    const auto index = shifted.indexOf(QString::fromUcs4(&cp, 1));
    if (index >= 0) return plain[index].unicode();
    const auto lowercase = text.toLower().toUcs4();
    return lowercase.size() == 1 && lowercase[0] != cp ? lowercase[0] : 0;
}
GhosttyMouseButton button(Qt::MouseButton value) {
    switch (value) {
    case Qt::LeftButton: return GHOSTTY_MOUSE_BUTTON_LEFT;
    case Qt::RightButton: return GHOSTTY_MOUSE_BUTTON_RIGHT;
    case Qt::MiddleButton: return GHOSTTY_MOUSE_BUTTON_MIDDLE;
    default: return GHOSTTY_MOUSE_BUTTON_UNKNOWN;
    }
}
}

TerminalSurface::TerminalSurface(QQuickItem *parent) : QQuickPaintedItem(parent) {
    setAcceptedMouseButtons(Qt::AllButtons); setAcceptHoverEvents(true); setAcceptTouchEvents(true);
    setFlag(ItemAcceptsInputMethod, true); setClip(true); setAntialiasing(false);
    m_font.setFamily("monospace"); m_font.setStyleHint(QFont::Monospace);
    m_font.setFixedPitch(true); m_font.setPixelSize(12); updateMetrics(); m_clock.start();
    GhosttyTerminalOptions options{uint16_t(m_columns), uint16_t(m_rows), 10000};
    require(ghostty_terminal_new(nullptr, &m_terminal, options), "terminal_new");
    require(ghostty_render_state_new(nullptr, &m_render), "render_state_new");
    require(ghostty_render_state_row_iterator_new(nullptr, &m_iterator), "row_iterator_new");
    require(ghostty_render_state_row_cells_new(nullptr, &m_cells), "row_cells_new");
    require(ghostty_key_encoder_new(nullptr, &m_keyEncoder), "key_encoder_new");
    require(ghostty_key_event_new(nullptr, &m_keyEvent), "key_event_new");
    require(ghostty_mouse_encoder_new(nullptr, &m_mouseEncoder), "mouse_encoder_new");
    require(ghostty_mouse_event_new(nullptr, &m_mouseEvent), "mouse_event_new");
    require(ghostty_selection_gesture_new(nullptr, &m_gesture), "selection_gesture_new");
    require(ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_USERDATA, this), "userdata");
    require(ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_WRITE_PTY, reinterpret_cast<void *>(output)), "write_pty");
    require(ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_BELL,
        reinterpret_cast<void *>(+[](GhosttyTerminal, void *data) { emit static_cast<TerminalSurface *>(data)->bell(); })), "bell");
    bool blink = true;
    require(ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_DEFAULT_CURSOR_BLINK, &blink), "cursor_blink");
    updateDefaultColors(); refresh();
    m_blink.setInterval(500);
    connect(&m_blink, &QTimer::timeout, this, [this] {
        if (isVisible() && hasActiveFocus() && m_cursorBlink && !mode(2026)) { m_blinkPhase = !m_blinkPhase; update(); }
    });
    connect(this, &QQuickItem::activeFocusChanged, this, [this] { m_blinkPhase = true; update(); });
    m_blink.start();
    m_selectionScroll.setInterval(80);
    connect(&m_selectionScroll, &QTimer::timeout, this, [this] {
        const qreal distance = m_dragPoint.y() < 0 ? m_dragPoint.y() : m_dragPoint.y() > height() ? m_dragPoint.y() - height() : 0;
        if (!m_dragging || distance == 0) { m_selectionScroll.stop(); return; }
        const qreal ramp = qBound<qreal>(-1, distance / 50, 1);
        const int rows = (ramp < 0 ? -1 : 1) + int(std::floor(ramp * 14 + .5));
        setScrollOffset(m_scrollOffset - rows);
        selectionEvent(GHOSTTY_SELECTION_GESTURE_EVENT_TYPE_DRAG, m_dragPoint);
    });
    m_synchronized.setSingleShot(true); m_synchronized.setInterval(500);
    connect(&m_synchronized, &QTimer::timeout, this, [this] {
        require(ghostty_terminal_mode_set(m_terminal, GHOSTTY_MODE_SYNC_OUTPUT, false), "synchronized_output_end");
        refresh();
    });
}
TerminalSurface::~TerminalSurface() {
    ghostty_selection_gesture_free(m_gesture, m_terminal);
    ghostty_mouse_event_free(m_mouseEvent); ghostty_mouse_encoder_free(m_mouseEncoder);
    ghostty_key_event_free(m_keyEvent); ghostty_key_encoder_free(m_keyEncoder);
    ghostty_render_state_row_cells_free(m_cells); ghostty_render_state_row_iterator_free(m_iterator);
    ghostty_render_state_free(m_render); ghostty_terminal_free(m_terminal);
}
QString TerminalSurface::engineRevision() const { return QString::fromLatin1(EngineRevision); }
void TerminalSurface::updateMetrics() {
    QFontMetricsF metrics(m_font);
    m_cellWidth = std::max<qreal>(1, metrics.horizontalAdvance(QLatin1Char('M')));
    const auto bounds = metrics.tightBoundingRect(QStringLiteral("Mg"));
    const qreal glyphHeight = bounds.height();
    m_cellHeight = std::max<qreal>({1, qreal(qRound(m_font.pixelSize() * 1.35)), std::ceil(glyphHeight)});
    m_ascent = qRound((m_cellHeight - glyphHeight) / 2 - bounds.top());
}
void TerminalSurface::setFontFamily(const QString &family) {
    if (family.isEmpty() || family == m_font.family()) return;
    m_font.setFamily(family); updateMetrics(); updateGrid(); emit fontChanged(); update();
}
void TerminalSurface::setFontPixelSize(qreal size) {
    if (!std::isfinite(size)) return;
    int pixels = qBound(6, qRound(size), 32); if (pixels == m_font.pixelSize()) return;
    m_font.setPixelSize(pixels); updateMetrics(); updateGrid(); emit fontChanged(); update();
}
void TerminalSurface::updateDefaultColors() {
    auto fg = rgb(m_foreground), bg = rgb(m_background);
    require(ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_COLOR_FOREGROUND, &fg), "foreground");
    require(ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_COLOR_BACKGROUND, &bg), "background");
    require(ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_COLOR_CURSOR, &fg), "cursor_color");
}
void TerminalSurface::setForeground(const QColor &value) {
    if (!value.isValid() || value == m_foreground) return;
    m_foreground = value; updateDefaultColors(); emit colorsChanged(); refreshOrDefer();
}
void TerminalSurface::setBackground(const QColor &value) {
    if (!value.isValid() || value == m_background) return;
    m_background = value; updateDefaultColors(); emit colorsChanged(); refreshOrDefer();
}
void TerminalSurface::setSelectionColor(const QColor &value) {
    if (!value.isValid() || value == m_selectionColor) return;
    m_selectionColor = value; emit colorsChanged(); update();
}
void TerminalSurface::setGrid(int columns, int rows) {
    columns = qBound(1, columns, 1000); rows = qBound(1, rows, 500);
    const bool changed = columns != m_columns || rows != m_rows;
    m_columns = columns; m_rows = rows;
    require(ghostty_terminal_resize(m_terminal, uint16_t(columns), uint16_t(rows), uint32_t(std::round(m_cellWidth)), uint32_t(std::round(m_cellHeight))), "terminal_resize");
    if (changed) { emit dimensionsChanged(); emit gridChanged(columns, rows); }
    refreshOrDefer();
}
void TerminalSurface::updateGrid() {
    if (width() > 0 && height() > 0) setGrid(int(width() / m_cellWidth), int(height() / m_cellHeight));
}
void TerminalSurface::geometryChange(const QRectF &current, const QRectF &previous) {
    QQuickPaintedItem::geometryChange(current, previous);
    if (current.size() != previous.size() && m_terminal) updateGrid();
}
bool TerminalSurface::mode(uint16_t code) const {
    bool value = false; ghostty_terminal_mode_get(m_terminal, ghostty_mode_new(code, false), &value); return value;
}
void TerminalSurface::feed(const QString &text) { feedBytes(text.toUtf8()); }
void TerminalSurface::feedBytes(const QByteArray &bytes) {
    if (bytes.isEmpty()) return;
    bool synchronized = mode(2026);
    for (qsizetype start = 0; start < bytes.size();) {
        qsizetype end = synchronized ? bytes.indexOf('\x1b', start + 1) : -1;
        if (end < 0) end = bytes.size();
        ghostty_terminal_vt_write(m_terminal, reinterpret_cast<const uint8_t *>(bytes.constData() + start), size_t(end - start));
        start = end;
        if (synchronized && !mode(2026)) { synchronized = false; m_synchronized.stop(); refresh(); }
    }
    refreshOrDefer();
}
void TerminalSurface::refreshOrDefer() {
    if (mode(2026)) { if (!m_synchronized.isActive()) m_synchronized.start(); return; }
    m_synchronized.stop(); refresh();
}
void TerminalSurface::reset() {
    m_synchronized.stop(); m_selectionScroll.stop();
    ghostty_selection_gesture_reset(m_gesture, m_terminal);
    ghostty_terminal_reset(m_terminal); m_dragging = false; m_encodedKeys.clear();
    bool blink = true; ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_DEFAULT_CURSOR_BLINK, &blink);
    updateDefaultColors(); refresh();
}
void TerminalSurface::refresh() {
    require(ghostty_render_state_update(m_render, m_terminal), "render_state_update");
    GhosttyRenderStateColors colors = GHOSTTY_INIT_SIZED(GhosttyRenderStateColors);
    require(ghostty_render_state_colors_get(m_render, &colors), "render_state_colors_get");
    m_frameForeground = color(colors.foreground); m_frameBackground = color(colors.background);
    m_cursorColor = colors.cursor_has_value ? color(colors.cursor) : m_frameForeground;
    ghostty_render_state_get(m_render, GHOSTTY_RENDER_STATE_DATA_CURSOR_VISIBLE, &m_cursorVisible);
    ghostty_render_state_get(m_render, GHOSTTY_RENDER_STATE_DATA_CURSOR_BLINKING, &m_cursorBlink);
    ghostty_render_state_get(m_render, GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE, &m_cursorInViewport);
    ghostty_render_state_get(m_render, GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_X, &m_cursorX);
    ghostty_render_state_get(m_render, GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_Y, &m_cursorY);
    ghostty_render_state_get(m_render, GHOSTTY_RENDER_STATE_DATA_CURSOR_VISUAL_STYLE, &m_cursorStyle);
    GhosttyTerminalScreen screen = GHOSTTY_TERMINAL_SCREEN_PRIMARY;
    ghostty_terminal_get(m_terminal, GHOSTTY_TERMINAL_DATA_ACTIVE_SCREEN, &screen);
    const bool alternate = screen == GHOSTTY_TERMINAL_SCREEN_ALTERNATE;
    if (m_alternate != alternate) { m_alternate = alternate; emit alternateScreenChanged(); }
    GhosttyString title{}; ghostty_terminal_get(m_terminal, GHOSTTY_TERMINAL_DATA_TITLE, &title);
    const QString nextTitle = QString::fromUtf8(reinterpret_cast<const char *>(title.ptr), qsizetype(title.len));
    if (nextTitle != m_title) { m_title = nextTitle; emit titleChanged(); }
    GhosttyTerminalScrollbar scrollbar{};
    ghostty_terminal_get(m_terminal, GHOSTTY_TERMINAL_DATA_SCROLLBAR, &scrollbar);
    const int history = int(scrollbar.total > scrollbar.len ? scrollbar.total - scrollbar.len : 0);
    const int offset = std::max(0, history - int(scrollbar.offset));
    if (history != m_scrollback || offset != m_scrollOffset) {
        m_scrollback = history; m_scrollOffset = offset; emit scrollChanged();
    }
    require(ghostty_render_state_get(m_render, GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR, &m_iterator), "row_iterator");
    m_frame.clear(); m_frame.reserve(size_t(m_rows)); m_rowWraps.clear();
    while (int(m_frame.size()) < m_rows && ghostty_render_state_row_iterator_next(m_iterator)) {
        require(ghostty_render_state_row_get(m_iterator, GHOSTTY_RENDER_STATE_ROW_DATA_CELLS, &m_cells), "row_cells");
        GhosttyRow rawRow{}; bool wraps = false, continuation = false;
        require(ghostty_render_state_row_get(m_iterator, GHOSTTY_RENDER_STATE_ROW_DATA_RAW, &rawRow), "raw_row");
        require(ghostty_row_get(rawRow, GHOSTTY_ROW_DATA_WRAP, &wraps), "row_wrap");
        require(ghostty_row_get(rawRow, GHOSTTY_ROW_DATA_WRAP_CONTINUATION, &continuation), "row_continuation");
        m_rowWraps.emplace_back(wraps, continuation);
        std::vector<Cell> row; row.reserve(size_t(m_columns));
        while (int(row.size()) < m_columns && ghostty_render_state_row_cells_next(m_cells)) {
            Cell cell; cell.foreground = m_frameForeground; cell.background = m_frameBackground;
            GhosttyStyle style = GHOSTTY_INIT_SIZED(GhosttyStyle);
            require(ghostty_render_state_row_cells_get(m_cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE, &style), "cell_style");
            GhosttyColorRgb fg{}, bg{};
            if (ghostty_render_state_row_cells_get(m_cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_FG_COLOR, &fg) == GHOSTTY_SUCCESS) cell.foreground = color(fg);
            if (ghostty_render_state_row_cells_get(m_cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_BG_COLOR, &bg) == GHOSTTY_SUCCESS) cell.background = color(bg);
            if (style.inverse) std::swap(cell.foreground, cell.background);
            if (style.faint) cell.foreground = QColor((cell.foreground.red() * 155 + cell.background.red() * 100) / 255, (cell.foreground.green() * 155 + cell.background.green() * 100) / 255, (cell.foreground.blue() * 155 + cell.background.blue() * 100) / 255);
            cell.bold = style.bold; cell.italic = style.italic; cell.invisible = style.invisible;
            cell.strike = style.strikethrough; cell.overline = style.overline; cell.underline = style.underline;
            cell.underlineColor = style.underline_color.tag == GHOSTTY_STYLE_COLOR_RGB ? color(style.underline_color.value.rgb)
                : style.underline_color.tag == GHOSTTY_STYLE_COLOR_PALETTE ? color(colors.palette[style.underline_color.value.palette]) : cell.foreground;
            ghostty_render_state_row_cells_get(m_cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_SELECTED, &cell.selected);
            uint32_t length = 0;
            require(ghostty_render_state_row_cells_get(m_cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_LEN, &length), "graphemes_len");
            if (length) {
                std::vector<char32_t> points(length);
                require(ghostty_render_state_row_cells_get(m_cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_BUF, points.data()), "graphemes_buf");
                cell.text = QString::fromUcs4(points.data(), qsizetype(points.size()));
            }
            GhosttyCell raw{}; GhosttyCellWide wide = GHOSTTY_CELL_WIDE_NARROW;
            require(ghostty_render_state_row_cells_get(m_cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_RAW, &raw), "cell_raw");
            require(ghostty_cell_get(raw, GHOSTTY_CELL_DATA_WIDE, &wide), "cell_wide");
            cell.rawWide = int(wide);
            cell.width = wide == GHOSTTY_CELL_WIDE_WIDE ? 2 : wide == GHOSTTY_CELL_WIDE_SPACER_TAIL || wide == GHOSTTY_CELL_WIDE_SPACER_HEAD ? 0 : 1;
            row.push_back(std::move(cell));
        }
        m_frame.push_back(std::move(row));
    }
    m_blinkPhase = true; emit cursorChanged(); emit selectionChanged(); update();
}
void TerminalSurface::setScrollOffset(int offset) {
    offset = qBound(0, offset, m_scrollback); if (offset == m_scrollOffset) return;
    GhosttyTerminalScrollViewport scroll{};
    scroll.tag = GHOSTTY_SCROLL_VIEWPORT_DELTA; scroll.value.delta = m_scrollOffset - offset;
    ghostty_terminal_scroll_viewport(m_terminal, scroll); refreshOrDefer();
}
QString TerminalSurface::rowText(int row) const {
    if (row < 0 || row >= int(m_frame.size())) return {};
    QString text;
    for (const auto &cell : m_frame[size_t(row)]) if (cell.width) text += cell.text.isEmpty() ? QStringLiteral(" ") : cell.text;
    while (text.endsWith(QLatin1Char(' '))) text.chop(1); return text;
}
QVariantMap TerminalSurface::cellAt(int row, int column) const {
    if (row < 0 || row >= int(m_frame.size()) || column < 0 || column >= int(m_frame[size_t(row)].size())) return {};
    const auto &cell = m_frame[size_t(row)][size_t(column)];
    return {{"text", cell.text}, {"width", cell.width}, {"foreground", cell.foreground}, {"background", cell.background},
        {"bold", cell.bold}, {"italic", cell.italic}, {"underline", cell.underline != 0}, {"selected", cell.selected}};
}
void TerminalSurface::paint(QPainter *painter) {
    painter->fillRect(boundingRect(), m_frameBackground); painter->setRenderHint(QPainter::TextAntialiasing, true);
    const auto powerline = [](const QString &text) { return text == QStringLiteral("\ue0b0") || text == QStringLiteral("\ue0b2"); };
    const auto drawPowerline = [&](const QString &text, const QRectF &box) {
        const qreal base = text == QStringLiteral("\ue0b0") ? box.left() : box.right();
        const qreal point = text == QStringLiteral("\ue0b0") ? box.right() : box.left();
        painter->setPen(Qt::NoPen);
        painter->drawPolygon(QPolygonF{QPointF(base, box.top()), QPointF(point, box.center().y()), QPointF(base, box.bottom())});
    };
    for (size_t row = 0; row < m_frame.size(); ++row) {
        const auto &cells = m_frame[row];
        for (size_t col = 0; col < cells.size(); ++col) {
            const auto &cell = cells[col];
            const QRectF box(qreal(col) * m_cellWidth, qreal(row) * m_cellHeight, m_cellWidth, m_cellHeight);
            painter->fillRect(box, cell.background); if (cell.selected) painter->fillRect(box, m_selectionColor);
        }
        // T3 draws adjacent cells in one styled run so ligatures and fallback advances survive selection.
        for (size_t start = 0; start < cells.size();) {
            const auto &first = cells[start];
            if (first.text.isEmpty()) { ++start; continue; }
            const QRectF firstBox(qreal(start) * m_cellWidth, qreal(row) * m_cellHeight, m_cellWidth, m_cellHeight);
            if (powerline(first.text)) {
                if (!first.invisible) { painter->setBrush(first.foreground); drawPowerline(first.text, firstBox); }
                ++start; continue;
            }
            size_t end = start + 1;
            while (end < cells.size()) {
                const auto &next = cells[end];
                if (!next.width && end > 0 && cells[end - 1].width == 2) { ++end; continue; }
                if (next.text.isEmpty() || powerline(next.text) || next.foreground != first.foreground || next.bold != first.bold || next.italic != first.italic || next.invisible != first.invisible) break;
                ++end;
            }
            QString text; for (size_t index = start; index < end; ++index) text += cells[index].text;
            if (!first.invisible && !text.trimmed().isEmpty()) {
                const QRectF box(firstBox.topLeft(), QSizeF(qreal(end - start) * m_cellWidth, m_cellHeight));
                QFont font = m_font; font.setBold(first.bold); font.setItalic(first.italic);
                painter->save(); painter->setClipRect(box); painter->setFont(font); painter->setPen(first.foreground);
                const qreal advance = QFontMetricsF(font).horizontalAdvance(text);
                const qreal scale = advance > box.width() ? box.width() / advance : 1;
                painter->translate(box.topLeft()); painter->scale(scale, 1);
                painter->drawText(QPointF(0, m_ascent), text); painter->restore();
            }
            start = end;
        }
        for (size_t col = 0; col < cells.size(); ++col) {
            const auto &cell = cells[col]; const qreal x = qreal(col) * m_cellWidth, y = qreal(row) * m_cellHeight;
            if (cell.underline) painter->fillRect(QRectF(x, y + m_cellHeight - 2, m_cellWidth, 1), cell.foreground);
            if (cell.strike) painter->fillRect(QRectF(x, y + std::floor(m_cellHeight * .55), m_cellWidth, 1), cell.foreground);
            if (cell.overline) painter->fillRect(QRectF(x, y + 1, m_cellWidth, 1), cell.foreground);
        }
    }
    if (m_cursorVisible && m_cursorInViewport && (!m_cursorBlink || m_blinkPhase || !hasActiveFocus())) {
        QRectF cursor(m_cursorX * m_cellWidth, m_cursorY * m_cellHeight, m_cellWidth, m_cellHeight);
        painter->setPen(m_cursorColor);
        if (!hasActiveFocus() || m_cursorStyle == GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK_HOLLOW) { painter->setBrush(Qt::NoBrush); painter->drawRect(cursor.adjusted(.5, .5, -.5, -.5)); }
        else if (m_cursorStyle == GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_UNDERLINE) painter->fillRect(QRectF(cursor.x(), cursor.bottom() - 2, cursor.width(), 2), m_cursorColor);
        else if (m_cursorStyle == GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BAR) painter->fillRect(QRectF(cursor.x(), cursor.y(), 2, cursor.height()), m_cursorColor);
        else {
            painter->fillRect(cursor, m_cursorColor);
            if (m_cursorY < m_frame.size() && m_cursorX < m_frame[m_cursorY].size()) {
                const auto &cell = m_frame[m_cursorY][m_cursorX];
                if (powerline(cell.text)) { painter->setBrush(m_frameBackground); drawPowerline(cell.text, cursor); }
                else if (!cell.text.isEmpty()) {
                    QFont font = m_font; font.setBold(cell.bold); font.setItalic(cell.italic); painter->setFont(font); painter->setPen(m_frameBackground);
                    painter->save(); painter->setClipRect(cursor); painter->drawText(QPointF(cursor.x(), cursor.y() + m_ascent), cell.text); painter->restore();
                }
            }
        }
        if (!m_preedit.isEmpty()) { painter->setPen(m_frameForeground); painter->setFont(m_font); painter->drawText(QPointF(cursor.x(), cursor.y() + m_ascent), m_preedit); }
    }
}
bool TerminalSurface::hasSelection() const { return !selectedText().isEmpty(); }
QVariantMap TerminalSurface::selectionRange() const {
    if (selectedText().isEmpty()) return {};
    GhosttySelection selection = GHOSTTY_INIT_SIZED(GhosttySelection);
    if (ghostty_terminal_get(m_terminal, GHOSTTY_TERMINAL_DATA_SELECTION, &selection) != GHOSTTY_SUCCESS) return {};
    GhosttyPointCoordinate start{}, end{};
    if (ghostty_terminal_point_from_grid_ref(m_terminal, &selection.start, GHOSTTY_POINT_TAG_SCREEN, &start) != GHOSTTY_SUCCESS ||
        ghostty_terminal_point_from_grid_ref(m_terminal, &selection.end, GHOSTTY_POINT_TAG_SCREEN, &end) != GHOSTTY_SUCCESS) return {};
    if (start.y > end.y || (start.y == end.y && start.x > end.x)) std::swap(start, end);
    // ThreadTerminalDrawer.terminalSelectionLineRange uses one-based screen rows.
    return {{"lineStart", qulonglong(start.y) + 1}, {"lineEnd", qulonglong(end.y) + 1},
        {"startColumn", int(start.x)}, {"endColumn", int(end.x)}};
}
QString TerminalSurface::selectedText() const {
    auto options = GHOSTTY_INIT_SIZED(GhosttyTerminalSelectionFormatOptions);
    static_assert(GHOSTTY_FORMATTER_FORMAT_PLAIN == 0);
    options.unwrap = true; options.trim = true;
    return QString::fromUtf8(encoded([&](char *data, size_t capacity, size_t *written) {
        return ghostty_terminal_selection_format_buf(m_terminal, options, reinterpret_cast<uint8_t *>(data), capacity, written);
    }));
}
bool TerminalSurface::gridRef(int row, int column, GhosttyGridRef &ref) const {
    ref = GHOSTTY_INIT_SIZED(GhosttyGridRef);
    GhosttyPoint point{}; point.tag = GHOSTTY_POINT_TAG_VIEWPORT;
    point.value.coordinate.x = uint16_t(qBound(0, column, m_columns - 1));
    point.value.coordinate.y = uint32_t(qBound(0, row, m_rows - 1));
    return ghostty_terminal_grid_ref(m_terminal, point, &ref) == GHOSTTY_SUCCESS;
}
void TerminalSurface::clearSelection() {
    ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_SELECTION, nullptr); emit selectionChanged(); refreshOrDefer();
}
void TerminalSurface::selectRange(int firstRow, int firstColumn, int lastRow, int lastColumn) {
    if (firstRow == lastRow && firstColumn == lastColumn) { clearSelection(); return; }
    if (firstRow > lastRow || (firstRow == lastRow && firstColumn > lastColumn)) {
        std::swap(firstRow, lastRow); std::swap(firstColumn, lastColumn);
    }
    if (!lastColumn && lastRow > firstRow) { --lastRow; lastColumn = m_columns; }
    GhosttySelection selection = GHOSTTY_INIT_SIZED(GhosttySelection);
    if (gridRef(firstRow, firstColumn, selection.start) && gridRef(lastRow, lastColumn - 1, selection.end)) {
        ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_SELECTION, &selection); refreshOrDefer();
    }
}
void TerminalSurface::copySelection() { if (hasSelection()) emit copyRequested(selectedText()); }
void TerminalSurface::selectAll() {
    GhosttySelection selection = GHOSTTY_INIT_SIZED(GhosttySelection);
    if (ghostty_terminal_select_all(m_terminal, &selection) == GHOSTTY_SUCCESS) {
        ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_SELECTION, &selection); refreshOrDefer();
    }
}
QString TerminalSurface::hyperlinkAt(int row, int column) const {
    GhosttyGridRef ref{}; if (!gridRef(row, column, ref)) return {};
    return QString::fromUtf8(encoded([&](char *data, size_t capacity, size_t *written) {
        return ghostty_grid_ref_hyperlink_uri(&ref, reinterpret_cast<uint8_t *>(data), capacity, written);
    }));
}
// Materialize only the hovered logical line. Other viewport entries stay null,
// preserving source row coordinates without allocating maps for the entire grid.
QVariantList TerminalSurface::linkRows(int row) const {
    if (row < 0 || row >= int(m_frame.size())) return {};
    int first = row, last = row;
    while (first > 0 && m_rowWraps[size_t(first)].second) --first;
    while (last + 1 < int(m_frame.size()) && m_rowWraps[size_t(last + 1)].second) ++last;
    QVariantList result;
    for (int index = 0; index < int(m_frame.size()); ++index) {
        if (index < first || index > last) { result.push_back(QVariant()); continue; }
        QVariantList cells;
        for (const auto &cell : m_frame[size_t(index)]) cells.push_back(QVariantMap{{"text", cell.text}, {"wide", cell.rawWide}});
        result.push_back(QVariantMap{{"cells", cells}, {"wrapsToNext", m_rowWraps[size_t(index)].first}, {"isWrapContinuation", m_rowWraps[size_t(index)].second}});
    }
    return result;
}
// Qt JS lacks matchAll/named groups. Keep the native patterns and algorithm,
// supplying PCRE matches with the same UTF16 positions and capture shape.
QVariantList TerminalSurface::matchTerminalPattern(const QString &text, const QString &pattern, const QString &flags) const {
    QRegularExpression::PatternOptions options = QRegularExpression::NoPatternOption;
    if (flags.contains(QLatin1Char('i'))) options |= QRegularExpression::CaseInsensitiveOption;
    const QRegularExpression expression(pattern, options);
    if (!expression.isValid()) { qWarning("Invalid terminal link pattern"); return {}; }
    QVariantList result;
    auto iterator = expression.globalMatch(text);
    const auto names = expression.namedCaptureGroups();
    while (iterator.hasNext()) {
        const auto match = iterator.next();
        QVariantMap groups;
        for (int index = 1; index < names.size(); ++index) if (!names[index].isEmpty() && match.hasCaptured(index)) groups.insert(names[index], match.captured(index));
        result.push_back(QVariantMap{{"0", match.captured()}, {"index", match.capturedStart()}, {"groups", groups}});
    }
    return result;
}
void TerminalSurface::emitBytes(const QByteArray &bytes) { if (!bytes.isEmpty()) emit input(QString::fromUtf8(bytes)); }
void TerminalSurface::output(GhosttyTerminal, void *data, const uint8_t *bytes, size_t length) {
    auto *surface = static_cast<TerminalSurface *>(data);
    // The engine forbids reentrant VT writes. Queue responses beyond the parser callback.
    const QByteArray copy(reinterpret_cast<const char *>(bytes), qsizetype(length));
    QMetaObject::invokeMethod(surface, [surface, copy] { surface->emitBytes(copy); }, Qt::QueuedConnection);
}
GhosttyMods TerminalSurface::modifiers(Qt::KeyboardModifiers value) {
    return (value & Qt::ShiftModifier ? GHOSTTY_MODS_SHIFT : 0) | (value & Qt::ControlModifier ? GHOSTTY_MODS_CTRL : 0)
        | (value & Qt::AltModifier ? GHOSTTY_MODS_ALT : 0) | (value & Qt::MetaModifier ? GHOSTTY_MODS_SUPER : 0);
}
void TerminalSurface::encodeKey(QKeyEvent *event, GhosttyKeyAction action) {
    ghostty_key_encoder_setopt_from_terminal(m_keyEncoder, m_terminal);
    ghostty_key_event_set_action(m_keyEvent, action);
    ghostty_key_event_set_key(m_keyEvent, physicalKey(*event));
    auto mods = modifiers(event->modifiers());
    if (event->nativeModifiers() & 2) mods |= GHOSTTY_MODS_CAPS_LOCK;
    if (event->nativeModifiers() & 16) mods |= GHOSTTY_MODS_NUM_LOCK;
    ghostty_key_event_set_mods(m_keyEvent, mods);
    QString text = event->text();
    if (text.size() == 1 && (text[0].unicode() < 32 || text[0].unicode() == 127)) {
        text.clear();
        if (event->key() >= Qt::Key_A && event->key() <= Qt::Key_Z)
            text = QChar(event->key() + ((mods & GHOSTTY_MODS_SHIFT) ? 0 : 32));
        else if (event->key() >= Qt::Key_Space && event->key() <= Qt::Key_AsciiTilde) text = QChar(event->key());
    }
    const auto points = text.toUcs4();
    const auto consumed = (mods == GHOSTTY_MODS_SHIFT && points.size() == 1) ? GHOSTTY_MODS_SHIFT : 0;
    ghostty_key_event_set_consumed_mods(m_keyEvent, GhosttyMods(consumed));
    ghostty_key_event_set_composing(m_keyEvent, !m_preedit.isEmpty());
    ghostty_key_event_set_unshifted_codepoint(m_keyEvent, unshifted(text, event->modifiers()));
    const QByteArray utf8 = text.toUtf8();
    ghostty_key_event_set_utf8(m_keyEvent, utf8.constData(), size_t(utf8.size()));
    const auto bytes = encoded([&](char *data, size_t capacity, size_t *written) {
        return ghostty_key_encoder_encode(m_keyEncoder, m_keyEvent, data, capacity, written);
    });
    if (action != GHOSTTY_KEY_ACTION_RELEASE && !bytes.isEmpty()) m_encodedKeys.insert(int(physicalKey(*event)));
    emitBytes(bytes);
    event->accept();
}
void TerminalSurface::keyPressEvent(QKeyEvent *event) {
    const auto mods = event->modifiers();
    if (!event->isAutoRepeat()) m_encodedKeys.remove(int(physicalKey(*event)));
    if (mods == Qt::ShiftModifier && !m_alternate && (event->key() == Qt::Key_PageUp || event->key() == Qt::Key_PageDown)) {
        setScrollOffset(m_scrollOffset + (event->key() == Qt::Key_PageUp ? m_rows : -m_rows)); event->accept(); return;
    }
    if (mods == (Qt::ControlModifier | Qt::ShiftModifier) && !m_alternate && (event->key() == Qt::Key_Home || event->key() == Qt::Key_End)) {
        setScrollOffset(event->key() == Qt::Key_Home ? m_scrollback : 0); event->accept(); return;
    }
    if (mods == Qt::ControlModifier && event->key() == Qt::Key_C && hasSelection()) { copySelection(); clearSelection(); event->accept(); return; }
    if (mods == (Qt::ControlModifier | Qt::ShiftModifier)) {
        if (event->key() == Qt::Key_C) { copySelection(); event->accept(); return; }
        if (event->key() == Qt::Key_V) { emit pasteRequested(); event->accept(); return; }
        if (event->key() == Qt::Key_A) { selectAll(); event->accept(); return; }
    }
    if (mods == Qt::ShiftModifier && event->key() == Qt::Key_Insert) { emit pasteRequested(); event->accept(); return; }
    if (mods == Qt::ControlModifier && event->key() == Qt::Key_Insert) { copySelection(); event->accept(); return; }
    setScrollOffset(0); clearSelection();
    encodeKey(event, event->isAutoRepeat() ? GHOSTTY_KEY_ACTION_REPEAT : GHOSTTY_KEY_ACTION_PRESS);
}
void TerminalSurface::keyReleaseEvent(QKeyEvent *event) {
    if (!m_encodedKeys.remove(int(physicalKey(*event)))) { event->accept(); return; }
    encodeKey(event, GHOSTTY_KEY_ACTION_RELEASE);
}
void TerminalSurface::paste(const QString &text) {
    QByteArray bytes = text.toUtf8(); if (bytes.isEmpty()) return;
    const bool bracketed = mode(2004);
    const auto output = encoded([&](char *data, size_t capacity, size_t *written) {
        return ghostty_paste_encode(bytes.data(), size_t(bytes.size()), bracketed, data, capacity, written);
    });
    setScrollOffset(0); clearSelection(); emitBytes(output);
}
bool TerminalSurface::mouseTracking() const {
    GhosttyMouseTrackingMode tracking = GHOSTTY_MOUSE_TRACKING_NONE;
    ghostty_terminal_get(m_terminal, GHOSTTY_TERMINAL_DATA_MOUSE_TRACKING, &tracking);
    return tracking != GHOSTTY_MOUSE_TRACKING_NONE;
}
QPoint TerminalSurface::positionAt(const QPointF &point) const {
    return {qBound(0, int(point.x() / m_cellWidth), m_columns - 1), qBound(0, int(point.y() / m_cellHeight), m_rows - 1)};
}
void TerminalSurface::sendMouse(GhosttyMouseAction action, GhosttyMouseButton mouseButton, bool hasButton,
                               const QPointF &point, Qt::KeyboardModifiers mods, bool anyPressed) {
    ghostty_mouse_encoder_setopt_from_terminal(m_mouseEncoder, m_terminal);
    auto size = GHOSTTY_INIT_SIZED(GhosttyMouseEncoderSize);
    size.screen_width = uint32_t(std::max<qreal>(1, width())); size.screen_height = uint32_t(std::max<qreal>(1, height()));
    size.cell_width = uint32_t(std::max(1, qRound(m_cellWidth))); size.cell_height = uint32_t(std::max(1, qRound(m_cellHeight)));
    ghostty_mouse_encoder_setopt(m_mouseEncoder, GHOSTTY_MOUSE_ENCODER_OPT_SIZE, &size);
    ghostty_mouse_encoder_setopt(m_mouseEncoder, GHOSTTY_MOUSE_ENCODER_OPT_ANY_BUTTON_PRESSED, &anyPressed);
    bool track = true; ghostty_mouse_encoder_setopt(m_mouseEncoder, GHOSTTY_MOUSE_ENCODER_OPT_TRACK_LAST_CELL, &track);
    ghostty_mouse_event_set_action(m_mouseEvent, action);
    if (hasButton) ghostty_mouse_event_set_button(m_mouseEvent, mouseButton); else ghostty_mouse_event_clear_button(m_mouseEvent);
    ghostty_mouse_event_set_mods(m_mouseEvent, modifiers(mods));
    ghostty_mouse_event_set_position(m_mouseEvent, {float(point.x()), float(point.y())});
    emitBytes(encoded([&](char *data, size_t capacity, size_t *written) {
        return ghostty_mouse_encoder_encode(m_mouseEncoder, m_mouseEvent, data, capacity, written);
    }));
}
bool TerminalSurface::reportMouse(Qt::KeyboardModifiers mods) const {
    return mouseTracking() && !(mods & (Qt::ShiftModifier | Qt::ControlModifier | Qt::MetaModifier));
}
void TerminalSurface::selectionEvent(GhosttySelectionGestureEventType type, const QPointF &point) {
    GhosttySelectionGestureEvent event = nullptr;
    require(ghostty_selection_gesture_event_new(nullptr, &event, type), "selection_gesture_event_new");
    GhosttyGridRef ref{}; const auto cell = positionAt(point);
    if (!gridRef(cell.y(), cell.x(), ref)) { ghostty_selection_gesture_event_free(event); return; }
    require(ghostty_selection_gesture_event_set(event, GHOSTTY_SELECTION_GESTURE_EVENT_OPT_REF, &ref), "selection_ref");
    if (type != GHOSTTY_SELECTION_GESTURE_EVENT_TYPE_RELEASE) {
        GhosttySurfacePosition position{point.x(), point.y()};
        ghostty_selection_gesture_event_set(event, GHOSTTY_SELECTION_GESTURE_EVENT_OPT_POSITION, &position);
        if (type == GHOSTTY_SELECTION_GESTURE_EVENT_TYPE_PRESS) {
            const uint64_t now = uint64_t(m_clock.nsecsElapsed()), interval = 500000000;
            const double distance = 4;
            ghostty_selection_gesture_event_set(event, GHOSTTY_SELECTION_GESTURE_EVENT_OPT_TIME_NS, &now);
            ghostty_selection_gesture_event_set(event, GHOSTTY_SELECTION_GESTURE_EVENT_OPT_REPEAT_INTERVAL_NS, &interval);
            ghostty_selection_gesture_event_set(event, GHOSTTY_SELECTION_GESTURE_EVENT_OPT_REPEAT_DISTANCE, &distance);
        } else {
            GhosttySelectionGestureGeometry geometry{uint32_t(m_columns), uint32_t(qRound(m_cellWidth)), 0, uint32_t(std::max<qreal>(1, height()))};
            ghostty_selection_gesture_event_set(event, GHOSTTY_SELECTION_GESTURE_EVENT_OPT_GEOMETRY, &geometry);
        }
    }
    GhosttySelection selection = GHOSTTY_INIT_SIZED(GhosttySelection);
    const auto result = ghostty_selection_gesture_event(m_gesture, m_terminal, event, &selection);
    if (result == GHOSTTY_SUCCESS) ghostty_terminal_set(m_terminal, GHOSTTY_TERMINAL_OPT_SELECTION, &selection);
    else if (result != GHOSTTY_NO_VALUE) require(result, "selection_gesture_event");
    ghostty_selection_gesture_event_free(event); refreshOrDefer();
}
void TerminalSurface::mousePressEvent(QMouseEvent *event) {
    forceActiveFocus();
    if (reportMouse(event->modifiers())) {
        sendMouse(GHOSTTY_MOUSE_ACTION_PRESS, button(event->button()), true, event->position(), event->modifiers(), true);
    } else if (event->button() == Qt::LeftButton) {
        m_anchor = positionAt(event->position()); m_dragging = true; clearSelection();
        selectionEvent(GHOSTTY_SELECTION_GESTURE_EVENT_TYPE_PRESS, event->position());
    } else if (event->button() == Qt::MiddleButton) Q_EMIT primaryPasteRequested();
    event->accept();
}
void TerminalSurface::mouseMoveEvent(QMouseEvent *event) {
    if (reportMouse(event->modifiers())) {
        Qt::MouseButton pressed = event->buttons() & Qt::LeftButton ? Qt::LeftButton : event->buttons() & Qt::RightButton ? Qt::RightButton : Qt::MiddleButton;
        sendMouse(GHOSTTY_MOUSE_ACTION_MOTION, button(pressed), event->buttons() != Qt::NoButton, event->position(), event->modifiers(), event->buttons() != Qt::NoButton);
    } else if (m_dragging) {
        m_dragPoint = event->position(); selectionEvent(GHOSTTY_SELECTION_GESTURE_EVENT_TYPE_DRAG, m_dragPoint);
        if ((m_dragPoint.y() < 0 || m_dragPoint.y() > height()) && !m_selectionScroll.isActive()) m_selectionScroll.start();
    }
    event->accept();
}
void TerminalSurface::hoverMoveEvent(QHoverEvent *event) {
    if (reportMouse(event->modifiers()) && mode(1003)) sendMouse(GHOSTTY_MOUSE_ACTION_MOTION, GHOSTTY_MOUSE_BUTTON_UNKNOWN, false, event->position(), event->modifiers(), false);
    event->accept();
}
void TerminalSurface::mouseReleaseEvent(QMouseEvent *event) {
    if (reportMouse(event->modifiers())) {
        sendMouse(GHOSTTY_MOUSE_ACTION_RELEASE, button(event->button()), true, event->position(), event->modifiers(), event->buttons() != Qt::NoButton);
    } else if (event->button() == Qt::LeftButton) {
        if (m_dragging) selectionEvent(GHOSTTY_SELECTION_GESTURE_EVENT_TYPE_RELEASE, event->position());
        const auto point = positionAt(event->position());
        if (point == m_anchor && !hasSelection()) {
            const auto uri = hyperlinkAt(point.y(), point.x()); if (!uri.isEmpty() && !(event->modifiers() & Qt::ShiftModifier)) Q_EMIT linkActivated(uri);
        }
    }
    m_dragging = false; m_selectionScroll.stop(); event->accept();
}
void TerminalSurface::mouseDoubleClickEvent(QMouseEvent *event) { mousePressEvent(event); }
void TerminalSurface::wheelEvent(QWheelEvent *event) {
    const qreal delta = event->pixelDelta().y() ? -event->pixelDelta().y() / m_cellHeight : -event->angleDelta().y() / 40.0;
    m_wheelRemainder += delta; const int rows = int(m_wheelRemainder); m_wheelRemainder -= rows;
    scrollRows(rows, event->position(), event->modifiers()); event->accept();
}
void TerminalSurface::scrollRows(int rows, const QPointF &point, Qt::KeyboardModifiers mods) {
    if (!rows) return;
    if (reportMouse(mods)) {
        for (int index = 0; index < std::abs(rows); ++index) sendMouse(GHOSTTY_MOUSE_ACTION_PRESS,
            rows < 0 ? GHOSTTY_MOUSE_BUTTON_FOUR : GHOSTTY_MOUSE_BUTTON_FIVE, true, point, mods, false);
    } else if (m_alternate) {
        const QByteArray sequence = rows < 0 ? (mode(1) ? "\x1bOA" : "\x1b[A") : (mode(1) ? "\x1bOB" : "\x1b[B");
        emitBytes(sequence.repeated(std::abs(rows)));
    } else setScrollOffset(m_scrollOffset - rows);
}
void TerminalSurface::touchEvent(QTouchEvent *event) {
    if (event->points().isEmpty()) { m_touchActive = false; event->accept(); return; }
    const auto &point = event->points().first();
    if (event->type() == QEvent::TouchBegin) {
        m_touchActive = true; m_touchMoved = false; m_touchOrigin = m_touchPrevious = point.position(); m_touchRemainder = 0;
    } else if (event->type() == QEvent::TouchUpdate && m_touchActive) {
        const auto position = point.position();
        m_touchMoved = m_touchMoved || std::hypot(position.x() - m_touchOrigin.x(), position.y() - m_touchOrigin.y()) > 4;
        if (m_touchMoved) {
            const qreal delta = m_touchRemainder + (m_touchPrevious.y() - position.y()) / m_cellHeight;
            const int rows = int(delta); m_touchRemainder = delta - rows; m_touchPrevious = position;
            scrollRows(rows, position, event->modifiers());
        }
    } else if (event->type() == QEvent::TouchEnd || event->type() == QEvent::TouchCancel) m_touchActive = false;
    event->accept();
}

void TerminalSurface::inputMethodEvent(QInputMethodEvent *event) {
    if (!event->commitString().isEmpty()) { setScrollOffset(0); clearSelection(); emit input(event->commitString()); }
    m_preedit = event->preeditString(); event->accept(); update();
}
QVariant TerminalSurface::inputMethodQuery(Qt::InputMethodQuery query) const {
    if (query == Qt::ImEnabled) return true;
    if (query == Qt::ImCursorRectangle) return QRectF(m_cursorX * m_cellWidth, m_cursorY * m_cellHeight, m_cellWidth, m_cellHeight);
    if (query == Qt::ImFont) return m_font;
    return QQuickPaintedItem::inputMethodQuery(query);
}
