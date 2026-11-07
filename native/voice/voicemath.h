#pragma once
#include <QObject>

// A scoped dependency of the unchanged native PCM source, never a global Math patch.
class O3VoiceMath : public QObject {
    Q_OBJECT
public:
    explicit O3VoiceMath(QObject *parent=nullptr) : QObject(parent) {}
    Q_INVOKABLE double sin(double value) const;
    Q_INVOKABLE double cos(double value) const;
    Q_INVOKABLE double exp(double value) const;
};
