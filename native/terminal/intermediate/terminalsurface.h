#pragma once
#include <QColor>
#include <QFont>
#include <QPoint>
#include <QQuickPaintedItem>
#include <QTimer>
#include <QVariantMap>
#include <deque>
#include <vector>
#include <vterm.h>

// A VT display/input encoder only. Rust owns terminal RPCs and process lifetime.
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
    Q_PROPERTY(int scrollOffset READ scrollOffset WRITE setScrollOffset NOTIFY scrollChanged)
    Q_PROPERTY(int scrollbackLines READ scrollbackLines NOTIFY scrollChanged)
    Q_PROPERTY(QString title READ title NOTIFY titleChanged)
    Q_PROPERTY(int cursorRow READ cursorRow NOTIFY cursorChanged)
    Q_PROPERTY(int cursorColumn READ cursorColumn NOTIFY cursorChanged)
    Q_PROPERTY(bool alternateScreen READ alternateScreen NOTIFY alternateScreenChanged)
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
    int scrollbackLines() const { return int(m_history.size()); }
    QString title() const { return m_title; }
    int cursorRow() const { return m_cursor.row; }
    int cursorColumn() const { return m_cursor.col; }
    bool alternateScreen() const { return m_alternate; }
    bool hasSelection() const { return m_selected; }
    QString selectedText() const;
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
    Q_INVOKABLE void paste(const QString &text);
    // End column is exclusive; row coordinates are relative to the visible viewport.
    Q_INVOKABLE void selectRange(int firstRow, int firstColumn, int lastRow, int lastColumn);
    Q_INVOKABLE QString rowText(int row) const;
    Q_INVOKABLE QVariantMap cellAt(int row, int column) const;
signals:
    void input(const QString &data);
    void gridChanged(int columns, int rows);
    void copyRequested(const QString &text);
    void pasteRequested();
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
    void mousePressEvent(QMouseEvent *) override;
    void mouseMoveEvent(QMouseEvent *) override;
    void mouseReleaseEvent(QMouseEvent *) override;
    void mouseDoubleClickEvent(QMouseEvent *) override;
    void wheelEvent(QWheelEvent *) override;
    void inputMethodEvent(QInputMethodEvent *) override;
    QVariant inputMethodQuery(Qt::InputMethodQuery) const override;
private:
    struct Position { qint64 row = 0; int column = 0; };
    struct HistoryLine { std::vector<VTermScreenCell> cells; bool continuation = false; };
    VTerm *m_terminal = nullptr;
    VTermState *m_state = nullptr;
    VTermScreen *m_screen = nullptr;
    int m_columns = 80, m_rows = 24, m_scrollOffset = 0, m_mouseMode = 0;
    qint64 m_pruned = 0;
    VTermPos m_cursor{0, 0};
    int m_cursorShape = VTERM_PROP_CURSORSHAPE_BLOCK;
    bool m_cursorVisible = true, m_cursorBlink = true, m_blinkPhase = true;
    bool m_alternate = false, m_selected = false, m_dragging = false;
    Position m_anchor, m_end;
    std::deque<HistoryLine> m_history;
    QFont m_font;
    qreal m_cellWidth = 8, m_cellHeight = 16, m_ascent = 12;
    QColor m_foreground = QColor("#d8dee9"), m_background = QColor("#111318");
    QColor m_selectionColor = QColor(88, 130, 190, 130);
    QString m_title, m_preedit;
    QTimer m_blink;
    void updateMetrics();
    void updateGrid();
    void updateDefaultColors();
    qint64 visibleStart() const;
    Position positionAt(const QPointF &) const;
    VTermScreenCell cell(qint64 row, int column) const;
    bool continuation(qint64 row) const;
    QColor color(VTermColor, bool foreground) const;
    bool selected(qint64 row, int column) const;
    void writeUnicode(const QString &, VTermModifier);
    static VTermModifier modifiers(Qt::KeyboardModifiers);
    static int damaged(VTermRect, void *);
    static int moved(VTermRect, VTermRect, void *);
    static int cursorMoved(VTermPos, VTermPos, int, void *);
    static int propertyChanged(VTermProp, VTermValue *, void *);
    static int rangBell(void *);
    static int resized(int, int, void *);
    static int pushed(int, const VTermScreenCell *, void *);
    static int popped(int, VTermScreenCell *, void *);
    static int historyCleared(void *);
    static void output(const char *, size_t, void *);
};
