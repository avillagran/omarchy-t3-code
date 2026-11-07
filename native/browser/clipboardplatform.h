#pragma once
#include <QObject>
#include <QString>
#include <memory>
class ClipboardPlatform : public QObject {
  Q_OBJECT
  Q_PROPERTY(QString endpoint READ endpoint CONSTANT)
  Q_PROPERTY(bool available READ available NOTIFY availableChanged)
  Q_PROPERTY(
      bool workerAdmitted READ workerAdmitted NOTIFY workerAdmittedChanged)
public:
  explicit ClipboardPlatform(QObject *parent = nullptr);
  ~ClipboardPlatform() override;
  QString endpoint() const;
  bool available() const;
  bool workerAdmitted() const;
  Q_INVOKABLE QString bindWorker(qint64 pid);
  Q_INVOKABLE void retireWorker(const QString &binding);
signals:
  void availableChanged();
  void workerAdmittedChanged();

private:
  struct State;
  std::unique_ptr<State> state;
};
