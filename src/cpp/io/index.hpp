#ifndef ARNELIFY_SERVER_IO_HPP
#define ARNELIFY_SERVER_IO_HPP

#include <iostream>
#include <mutex>
#include <thread>
#include <queue>

#include "task/index.hpp"

class ArnelifyServerIO {
 private:
  bool isRunning;
  const int threadLimit;

  std::mutex mtx;
  std::queue<ArnelifyServerTask*> read;
  std::queue<ArnelifyServerTask*> handler;
  std::queue<ArnelifyServerTask*> write;

  bool hasRead;
  bool hasHandler;
  bool hasWrite;

  void safeClear(std::queue<ArnelifyServerTask*> queue) {
    while (!queue.empty()) {
      ArnelifyServerTask* task = queue.front();
      if (task != nullptr) delete task;
      queue.pop();
    }
  }

 public:
  ArnelifyServerIO(const int& t)
      : hasRead(false),
        hasHandler(false),
        hasWrite(false),
        isRunning(true),
        threadLimit(t) {}

  ~ArnelifyServerIO() {
    this->safeClear(this->read);
    this->safeClear(this->handler);
    this->safeClear(this->write);
  }

  void addRead(ArnelifyServerTask* task) { this->read.push(task); }
  void addHandler(ArnelifyServerTask* task) { this->handler.push(task); }
  void addWrite(ArnelifyServerTask* task) { this->write.push(task); }

  void onRead(const std::function<void(ArnelifyServerTask*)>& callback) {
    if (this->hasRead) return;
    this->hasRead = true;

    for (int i = 0; this->threadLimit > i; i++) {
      std::thread thread([this, callback]() {
        while (true) {
          if (!this->isRunning) break;
          ArnelifyServerTask* task = nullptr;
          this->mtx.lock();
          if (!this->read.empty()) {
            task = this->read.front();
            this->read.pop();
          }

          this->mtx.unlock();
          if (task) callback(task);
        }
      });

      thread.detach();
    }
  }

  void onHandler(const std::function<void(ArnelifyServerTask*)>& callback) {
    if (this->hasHandler) return;
    this->hasHandler = true;

    for (int i = 0; this->threadLimit > i; i++) {
      std::thread thread([this, callback]() {
        while (true) {
          if (!this->isRunning) break;
          ArnelifyServerTask* task = nullptr;
          this->mtx.lock();
          if (!this->handler.empty()) {
            task = this->handler.front();
            this->handler.pop();
          }

          this->mtx.unlock();
          if (task) callback(task);
        }
      });

      thread.detach();
    }
  }

  void onWrite(const std::function<void(ArnelifyServerTask*)>& callback) {
    if (this->hasWrite) return;
    this->hasWrite = true;

    for (int i = 0; this->threadLimit > i; i++) {
      std::thread thread([this, callback]() {
        while (true) {
          if (!this->isRunning) break;
          ArnelifyServerTask* task = nullptr;
          this->mtx.lock();
          if (!this->write.empty()) {
            task = this->write.front();
            this->write.pop();
          }

          this->mtx.unlock();
          if (task) callback(task);
        }
      });

      thread.detach();
    }
  }

  void stop() {
    this->isRunning = false;
    this->safeClear(this->read);
    this->safeClear(this->handler);
    this->safeClear(this->write);
  }
};

#endif