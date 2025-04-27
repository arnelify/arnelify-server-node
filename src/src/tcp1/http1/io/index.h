#ifndef ARNELIFY_SERVER_HTTP1_IO_H
#define ARNELIFY_SERVER_HTTP1_IO_H

#include <iostream>
#include <mutex>
#include <thread>
#include <queue>

#include "task/index.h"

class Http1IO {
 private:
  bool isRunning;
  const int threadLimit;

  std::mutex mtx;
  std::queue<Http1Task*> read;
  std::queue<Http1Task*> handler;
  std::queue<Http1Task*> write;

  bool hasRead;
  bool hasHandler;
  bool hasWrite;

  void safeClear(std::queue<Http1Task*> queue) {
    while (!queue.empty()) {
      Http1Task* task = queue.front();
      if (task != nullptr) delete task;
      queue.pop();
    }
  }

 public:
  Http1IO(const int t)
      : hasRead(false),
        hasHandler(false),
        hasWrite(false),
        isRunning(true),
        threadLimit(t) {}

  ~Http1IO() {
    this->safeClear(this->read);
    this->safeClear(this->handler);
    this->safeClear(this->write);
  }

  void addRead(Http1Task* task) { this->read.push(task); }
  void addHandler(Http1Task* task) { this->handler.push(task); }
  void addWrite(Http1Task* task) { this->write.push(task); }

  void onRead(const std::function<void(Http1Task*)>& callback) {
    if (this->hasRead) return;
    this->hasRead = true;

    for (int i = 0; this->threadLimit > i; i++) {
      std::thread thread([this, callback]() {
        while (true) {
          if (!this->isRunning) break;
          Http1Task* task = nullptr;
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

  void onHandler(const std::function<void(Http1Task*)>& callback) {
    if (this->hasHandler) return;
    this->hasHandler = true;

    for (int i = 0; this->threadLimit > i; i++) {
      std::thread thread([this, callback]() {
        while (true) {
          if (!this->isRunning) break;
          Http1Task* task = nullptr;
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

  void onWrite(const std::function<void(Http1Task*)>& callback) {
    if (this->hasWrite) return;
    this->hasWrite = true;

    for (int i = 0; this->threadLimit > i; i++) {
      std::thread thread([this, callback]() {
        while (true) {
          if (!this->isRunning) break;
          Http1Task* task = nullptr;
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