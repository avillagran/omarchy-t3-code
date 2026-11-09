#pragma once
#include <QAudioFormat>
#include <QByteArray>
#include <QJSEngine>
#include <QObject>
#include <QString>
#include <memory>
#include <deque>

// Thread-confined consumer of the complete pinned worklet. The audio owner
// supplies the trusted resource and device format; no user expression is run.
class O3PcmDsp final : public QObject {
    Q_OBJECT
public:
    explicit O3PcmDsp(QObject *parent=nullptr);
    ~O3PcmDsp() override;
    bool prepare(const QAudioFormat &format,const QString &trustedResource,bool meterOnly=false);
    bool start();
    bool feed(const QByteArray &deviceBytes);
    bool stop();
    bool isPrepared() const;
    QString lastError() const;
    // Called only by this engine's unchanged native MessagePort adapter.
    Q_INVOKABLE void publishPcm(const QByteArray &bytes);
    Q_INVOKABLE void publishLevel(double value);
    Q_INVOKABLE void publishEvent(const QString &value);
signals:
    void pcm(const QByteArray &bytes);
    void level(double value);
    void stopped();
    void durationLimit();
    void failed(const QString &message);
private:
    bool checked(const QJSValue &result);
    bool fail(const QString &message);
    bool command(const QString &value);
    bool processFrames(qsizetype frames);
    struct Message {
        enum Kind {Pcm,Level,Stopped,Limit} kind;
        QByteArray bytes;
        double value=0;
    };
    void enqueue(Message message);
    std::unique_ptr<QJSEngine> engine;
    QJSValue processor;
    QJSValue port;
    QAudioFormat format;
    QByteArray partial;
    QString error;
    bool prepared=false;
    bool recording=false;
    bool finished=false;
    bool deliveryQueued=false;
    std::deque<Message> messages;
};
