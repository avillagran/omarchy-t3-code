#pragma once
#include <QColor>
#include <QFont>
#include <QElapsedTimer>
#include <QQuickPaintedItem>
#include <QTimer>
#include <QSet>
#include <QVariantMap>
#pragma push_macro("emit")
#undef emit
#include <ghostty/vt.h>
#pragma pop_macro("emit")
#include <vector>

// Display and encode VT data only. Rust owns RPCs, the clipboard and process lifetime.
class TerminalSurface : public QQuickPaintedItem {
    Q_OBJECT
    Q_PROPERTY(int columns READ columns NOTIFY dimensionsChanged)
    Q_PROPERTY(int rows READ rows NOTIFY dimensionsChanged)
    Q_PROPERTY(QString fontFamily READ fontFamily WRITE setFontFamily NOTIFY fontChanged)
    Q_PROPERTY(qreal fontPixelSize READ fontPixelSize WRITE setFontPixelSize NOTIFY fontChanged)
    Q_PROPERTY(QColor foreground READ foreground WRITE setForeground NOTIFY colorsChanged)
    Q_PROPERTY(QColor background READ background WRITE setBackground NOTIFY colorsChanged)
    Q_PROPERTY(QColor selectionColor READ selectionColor WRITE setSelectionColor NOTIFY colorsChanged)
    Q_PROPERTY(QString selectedText READ selectedText NOTIFY selectionChanged)
    Q_PROPERTY(bool hasSelection READ hasSelection NOTIFY selectionChanged)
    Q_PROPERTY(QVariantMap selectionRange READ selectionRange NOTIFY selectionChanged)
    Q_PROPERTY(int scrollOffset READ scrollOffset WRITE setScrollOffset NOTIFY scrollChanged)
    Q_PROPERTY(int scrollbackLines READ scrollbackLines NOTIFY scrollChanged)
    Q_PROPERTY(QString title READ title NOTIFY titleChanged)
    Q_PROPERTY(int cursorRow READ cursorRow NOTIFY cursorChanged)
    Q_PROPERTY(int cursorColumn READ cursorColumn NOTIFY cursorChanged)
    Q_PROPERTY(bool alternateScreen READ alternateScreen NOTIFY alternateScreenChanged)
    Q_PROPERTY(QString engineRevision READ engineRevision CONSTANT)
public:
    explicit TerminalSurface(QQuickItem *parent = nullptr);
    ~TerminalSurface() override;
    void paint(QPainter *) override;
    int columns() const { return m_columns; }
    int rows() const { return m_rows; }
    QString fontFamily() const { return m_font.family(); }
    qreal fontPixelSize() const { return m_font.pixelSize(); }
    QColor foreground() const { return m_foreground; }
    QColor background() const { return m_background; }
    QColor selectionColor() const { return m_selectionColor; }
    int scrollOffset() const { return m_scrollOffset; }
    int scrollbackLines() const { return m_scrollback; }
    QString title() const { return m_title; }
    int cursorRow() const { return m_cursorY; }
    int cursorColumn() const { return m_cursorX; }
    bool alternateScreen() const { return m_alternate; }
    bool hasSelection() const;
    QString selectedText() const;
    QVariantMap selectionRange() const;
    QString engineRevision() const;
    void setFontFamily(const QString &);
    void setFontPixelSize(qreal);
    void setForeground(const QColor &);
    void setBackground(const QColor &);
    void setSelectionColor(const QColor &);
    void setScrollOffset(int);
    Q_INVOKABLE void feed(const QString &text);
    Q_INVOKABLE void feedBytes(const QByteArray &bytes);
    Q_INVOKABLE void setGrid(int columns, int rows);
    Q_INVOKABLE void reset();
    Q_INVOKABLE void clearSelection();
    Q_INVOKABLE void copySelection();
    Q_INVOKABLE void selectAll();
    Q_INVOKABLE void paste(const QString &text);
    // End column is exclusive; row coordinates are relative to the viewport.
    Q_INVOKABLE void selectRange(int firstRow, int firstColumn, int lastRow, int lastColumn);
    Q_INVOKABLE QString rowText(int row) const;
    Q_INVOKABLE QVariantMap cellAt(int row, int column) const;
    Q_INVOKABLE QString hyperlinkAt(int row, int column) const;
    Q_INVOKABLE QVariantList linkRows(int row) const;
    Q_INVOKABLE QVariantList matchTerminalPattern(const QString &text, const QString &pattern, const QString &flags) const;
signals:
    void input(const QString &data);
    void gridChanged(int columns, int rows);
    void copyRequested(const QString &text);
    void pasteRequested();
    void primaryPasteRequested();
    void linkActivated(const QString &uri);
    void dimensionsChanged();
    void fontChanged();
    void colorsChanged();
    void selectionChanged();
    void scrollChanged();
    void titleChanged();
    void cursorChanged();
    void alternateScreenChanged();
    void bell();
protected:
    void geometryChange(const QRectF &, const QRectF &) override;
    void keyPressEvent(QKeyEvent *) override;
    void keyReleaseEvent(QKeyEvent *) override;
    void mousePressEvent(QMouseEvent *) override;
    void mouseMoveEvent(QMouseEvent *) override;
    void mouseReleaseEvent(QMouseEvent *) override;
    void mouseDoubleClickEvent(QMouseEvent *) override;
    void hoverMoveEvent(QHoverEvent *) override;
    void wheelEvent(QWheelEvent *) override;
    void touchEvent(QTouchEvent *) override;
    void inputMethodEvent(QInputMethodEvent *) override;
    QVariant inputMethodQuery(Qt::InputMethodQuery) const override;
private:
    struct Cell {
        QString text;
        QColor foreground, background, underlineColor;
        int width = 1, rawWide = 0;
        bool bold = false, italic = false, invisible = false, strike = false;
        bool overline = false, selected = false;
        int underline = 0;
    };
    GhosttyTerminal m_terminal = nullptr;
    GhosttyRenderState m_render = nullptr;
    GhosttyRenderStateRowIterator m_iterator = nullptr;
    GhosttyRenderStateRowCells m_cells = nullptr;
    GhosttyKeyEncoder m_keyEncoder = nullptr;
    GhosttyKeyEvent m_keyEvent = nullptr;
    GhosttyMouseEncoder m_mouseEncoder = nullptr;
    GhosttyMouseEvent m_mouseEvent = nullptr;
    GhosttySelectionGesture m_gesture = nullptr;
    int m_columns = 80, m_rows = 24, m_scrollOffset = 0, m_scrollback = 0;
    uint16_t m_cursorX = 0, m_cursorY = 0;
    GhosttyRenderStateCursorVisualStyle m_cursorStyle = GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK;
    bool m_cursorVisible = true, m_cursorInViewport = true, m_cursorBlink = true, m_blinkPhase = true;
    bool m_alternate = false, m_dragging = false;
    QPoint m_anchor;
    QFont m_font;
    qreal m_cellWidth = 8, m_cellHeight = 16, m_ascent = 12, m_wheelRemainder = 0;
    QColor m_foreground = QColor("#d8dee9"), m_background = QColor("#111318");
    QColor m_frameForeground, m_frameBackground, m_cursorColor;
    QColor m_selectionColor = QColor(72, 122, 191, 89);
    QString m_title, m_preedit;
    QTimer m_blink, m_synchronized, m_selectionScroll;
    QElapsedTimer m_clock;
    QPointF m_dragPoint, m_touchOrigin, m_touchPrevious;
    bool m_touchActive = false, m_touchMoved = false;
    qreal m_touchRemainder = 0;
    std::vector<std::vector<Cell>> m_frame;
    std::vector<std::pair<bool, bool>> m_rowWraps;
    QSet<int> m_encodedKeys;
    void updateMetrics();
    void updateGrid();
    void updateDefaultColors();
    void refresh();
    void refreshOrDefer();
    bool mode(uint16_t) const;
    bool mouseTracking() const;
    QPoint positionAt(const QPointF &) const;
    bool gridRef(int row, int column, GhosttyGridRef &) const;
    void encodeKey(QKeyEvent *, GhosttyKeyAction);
    void selectionEvent(GhosttySelectionGestureEventType, const QPointF &);
    bool reportMouse(Qt::KeyboardModifiers) const;
    void sendMouse(GhosttyMouseAction, GhosttyMouseButton, bool hasButton,
                   const QPointF &, Qt::KeyboardModifiers, bool anyPressed);
    void emitBytes(const QByteArray &);
    void scrollRows(int, const QPointF &, Qt::KeyboardModifiers);
    static GhosttyMods modifiers(Qt::KeyboardModifiers);
    static void output(GhosttyTerminal, void *, const uint8_t *, size_t);
};
