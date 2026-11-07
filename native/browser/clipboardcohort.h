#pragma once
#include "waylandoffers.h"
#include <QMimeData>
#include <QString>
#include <functional>
#include <memory>
class ClipboardCohort {
public:
  using Publish = std::function<void(const QString &, QMimeData *)>;
  // Takes all three descriptors, including on failure. The caller has already
  // authenticated the worker and correlated the physical engine/owned X server.
  static std::shared_ptr<ClipboardCohort>
  create(QString id, int socket, int authority, int pidfd, qint64 engine,
         std::shared_ptr<NativeWaylandOffers> offers, Publish publish);
  ~ClipboardCohort();
  void retire();
  bool alive() const;
  void offerChanged(const QString &currentSource);

private:
  struct State;
  std::shared_ptr<State> state;
  explicit ClipboardCohort(std::shared_ptr<State> state);
};
