#ifndef ARNELIFY_SERVER_HTTP1_TASK_H
#define ARNELIFY_SERVER_HTTP1_TASK_H

#include <iostream>

#include "receiver/index.h"
#include "transmitter/index.h"

#include "contracts/opts.h"

class Http1Task {
 private:
  const Http1TaskOpts opts;

 public:
  const int clientSocket;
  Http1Receiver* receiver;
  Http1Transmitter* transmitter;

  Http1Task(const int s, const Http1TaskOpts& o) : clientSocket(s), opts(o) {
    Http1TransmitterOpts transmitterOpts(this->opts.HTTP1_BLOCK_SIZE_KB,
                                         this->opts.HTTP1_CHARSET,
                                         this->opts.HTTP1_GZIP);
    this->transmitter = new Http1Transmitter(transmitterOpts);

    Http1ReceiverOpts receiverOpts(
        this->opts.HTTP1_ALLOW_EMPTY_FILES, this->opts.HTTP1_KEEP_EXTENSIONS,
        this->opts.HTTP1_MAX_FIELDS, this->opts.HTTP1_MAX_FIELDS_SIZE_TOTAL_MB,
        this->opts.HTTP1_MAX_FILES, this->opts.HTTP1_MAX_FILES_SIZE_TOTAL_MB,
        this->opts.HTTP1_MAX_FILE_SIZE_MB, this->opts.HTTP1_UPLOAD_DIR);
    this->receiver = new Http1Receiver(receiverOpts);
  };

  ~Http1Task() {
    if (this->receiver == nullptr) delete this->receiver;
    if (this->transmitter == nullptr) delete this->transmitter;
  }
};

#endif