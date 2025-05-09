#ifndef ARNELIFY_SERVER_HTTP1_IO_H
#define ARNELIFY_SERVER_HTTP1_IO_H

#include <iostream>
#include <mutex>
#include <thread>
#include <queue>

#include "task/index.h"

#include <iostream>
#include <vector>
#include <thread>
#include <functional>
#include <queue>
#include <mutex>
#include <condition_variable>
#include <atomic>

class Http1IO {
 private:
  bool isRunning;
  int threadLimit;

  std::queue<Http1Task*> queue;
  std::vector<std::thread> threads;
  std::mutex mtx;
  std::condition_variable cv;
  std::function<void(Http1Task*)> callback;

 public:
  Http1IO(int t) : isRunning(false), threadLimit(t) {}
  ~Http1IO() { this->stop(); }

  void addTask(Http1Task* task) {
    std::lock_guard<std::mutex> lock(this->mtx);
    this->queue.push(task);
    this->cv.notify_one();
  }

  void handler(std::function<void(Http1Task*)> cb) { this->callback = cb; }

  void start() {
    if (this->isRunning) return;
    this->isRunning = true;
    
    for (int i = 0; this->threadLimit > i; i++) {
      std::thread thread([this]() {
        while (true) {
          Http1Task* task = nullptr;
          {
            std::unique_lock<std::mutex> lock(this->mtx);
            cv.wait(lock, [this]() { return !this->queue.empty() || !this->isRunning; });
            if (!this->isRunning) break;

            task = this->queue.front();
            this->queue.pop();
          }

          if (task && this->callback) {
            this->callback(task);
          }
        }
      });

      this->threads.push_back(std::move(thread));
    }
  }

  void stop() {
    this->isRunning = false;
    this->cv.notify_all();
    for (auto& thread : this->threads) {
      if (thread.joinable()) {
        thread.join();
      }
    }
  }
};

#endif