#ifndef ARNELIFY_SERVER_HTTP1_H
#define ARNELIFY_SERVER_HTTP1_H

#include <arpa/inet.h>
#include <csignal>
#include <fcntl.h>
#include <functional>
#include <iostream>
#include <thread>
#include <unistd.h>

#include "io/index.h"

#include "contracts/opts.h"
#include "contracts/logger.h"

using Http1Req = Json::Value;
using Http1Res = Http1Transmitter *;
using Http1Handler = std::function<void(const Http1Req &, Http1Res)>;

class Http1 {
 private:
  bool isRunning;
  int serverSocket;

  Http1IO *io;
  const Http1Opts opts;

  Http1Handler handler = [](const Http1Req &req, Http1Res res) -> void {
    Json::StreamWriterBuilder writer;
    writer["indentation"] = "";
    writer["emitUTF8"] = true;

    Json::Value json;
    json["code"] = 200;
    json["success"] = "Welcome to Arnelify Server";
    res->addBody(Json::writeString(writer, json));
    res->end();
  };

  Http1Logger logger = [](const std::string &message,
                          const bool &isError) -> void {
    if (isError) {
      std::cout << "[Arnelify Server]: Error: " << message << std::endl;
      return;
    }

    std::cout << "[Arnelify Server]: " << message << std::endl;
  };

 public:
  Http1(Http1Opts &o) : isRunning(false), opts(o), serverSocket(0) {
    const int threadLimit =
        this->opts.HTTP1_THREAD_LIMIT > 0 ? this->opts.HTTP1_THREAD_LIMIT : 1;
    this->io = new Http1IO(threadLimit);
  }

  ~Http1() {
    this->stop();
    this->io->stop();
    if (this->io != nullptr) delete this->io;
  }

  void setHandler(const Http1Handler &handler) { this->handler = handler; }

  void start(const Http1Logger &logger) {
    this->isRunning = true;
    this->logger = logger;

    const std::filesystem::path uploadDir = this->opts.HTTP1_UPLOAD_DIR;
    const bool hasUploadDir = std::filesystem::exists(uploadDir);
    if (!hasUploadDir) std::filesystem::create_directory(uploadDir);

    this->serverSocket = socket(AF_INET, SOCK_STREAM, 0);
    const bool isServerSocketCreated = this->serverSocket != -1;
    if (!isServerSocketCreated) {
      this->logger("Socket creation failed.", true);
      exit(1);
    }

    int flags = fcntl(this->serverSocket, F_GETFL, 0);
    if (flags == -1) {
      this->logger("Error getting socket flags.", true);
      close(this->serverSocket);
      exit(1);
    }

    if (fcntl(this->serverSocket, F_SETFL, flags | O_NONBLOCK) == -1) {
      this->logger("Error editing the socket to non-blocking mode.", true);
      close(this->serverSocket);
      exit(1);
    }

    const int opt = 1;
    sockaddr_in serverAddr;
    serverAddr.sin_family = AF_INET;
    serverAddr.sin_addr.s_addr = INADDR_ANY;
    serverAddr.sin_port = htons(this->opts.HTTP1_PORT);
    setsockopt(this->serverSocket, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
    const bool isBindSuccess = bind(this->serverSocket, (sockaddr *)&serverAddr,
                                    sizeof(serverAddr)) != -1;
    if (!isBindSuccess) {
      this->logger("Bind failed.", true);
      close(this->serverSocket);
      exit(1);
    }

    const bool isListenSuccess =
        listen(this->serverSocket, this->opts.HTTP1_QUEUE_LIMIT) != -1;
    if (!isListenSuccess) {
      this->logger("Listen failed.", true);
      close(this->serverSocket);
      exit(1);
    }

    this->io->onRead([this](Http1Task *task) {
      ssize_t bytesRead = 0;
      const std::size_t BLOCK_SIZE = this->opts.HTTP1_BLOCK_SIZE_KB * 1024;
      char *block = new char[BLOCK_SIZE];
      int SIGNAL_ON_BLOCK = 0;
      while ((bytesRead = recv(task->clientSocket, block, BLOCK_SIZE, 0)) > 0) {
        if (bytesRead == EWOULDBLOCK) {
          delete[] block;

          this->io->addRead(task);
          return;
        }

        SIGNAL_ON_BLOCK = task->receiver->onBlock(block, bytesRead);
        if (SIGNAL_ON_BLOCK > 0) break;
      }

      delete[] block;

      const bool isFinish = SIGNAL_ON_BLOCK == 2;
      if (isFinish) {
        this->io->addHandler(task);
        return;
      }

      Json::StreamWriterBuilder writer;
      writer["indentation"] = "";
      writer["emitUTF8"] = true;

      Json::Value json;
      json["code"] = 409;
      json["error"] = task->receiver->getStatus();
      const std::string body = Json::writeString(writer, json);

      task->transmitter->setCode(409);
      task->transmitter->addBody(body);
      task->transmitter->end();

      this->io->addWrite(task);
    });

    this->io->onHandler([this](Http1Task *task) {
      task->transmitter->setLogger(this->logger);
      const std::string encoding = task->receiver->getEncoding();
      task->transmitter->setEncoding(encoding);
      const Http1Req req = task->receiver->finish();
      delete task->receiver;

      this->handler(req, task->transmitter);
      this->io->addWrite(task);
    });

    this->io->onWrite([](Http1Task *task) {
      task->transmitter->onWrite(
          [task](const char *block, const int bytesRead) {
            send(task->clientSocket, block, bytesRead, 0);
          });

      task->transmitter->write();
      delete task;
    });

    const std::string port = std::to_string(this->opts.HTTP1_PORT);
    this->logger("Server is running on port " + port, false);

    sockaddr_in clientAddr;
    socklen_t clientLen = sizeof(clientAddr);
    const Http1TaskOpts opts(
        this->opts.HTTP1_ALLOW_EMPTY_FILES, this->opts.HTTP1_BLOCK_SIZE_KB,
        this->opts.HTTP1_CHARSET, this->opts.HTTP1_GZIP,
        this->opts.HTTP1_KEEP_EXTENSIONS, this->opts.HTTP1_MAX_FIELDS,
        this->opts.HTTP1_MAX_FIELDS_SIZE_TOTAL_MB, this->opts.HTTP1_MAX_FILES,
        this->opts.HTTP1_MAX_FILES_SIZE_TOTAL_MB,
        this->opts.HTTP1_MAX_FILE_SIZE_MB, this->opts.HTTP1_UPLOAD_DIR);

    while (true) {
      const bool isStop = !this->isRunning;
      if (isStop) {
        close(this->serverSocket);
        exit(0);
      }

      const int clientSocket =
          accept(this->serverSocket, (sockaddr *)&clientAddr, &clientLen);
      if (0 > clientSocket) {
        if (errno != EWOULDBLOCK && errno != EAGAIN) {
          this->logger("Connection error", true);
          break;
        }

        continue;
      }

      Http1Task *task = new Http1Task(clientSocket, opts);
      this->io->addRead(task);
    }
  }

  void stop() {
    this->io->stop();
    this->isRunning = false;
  }
};

#endif