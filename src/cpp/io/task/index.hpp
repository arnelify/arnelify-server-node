#ifndef ARNELIFY_SERVER_TASK_HPP
#define ARNELIFY_SERVER_TASK_HPP

#include <iostream>

#include "receiver/index.hpp"
#include "transmitter/index.hpp"

#include "contracts/opts.hpp"

class ArnelifyServerTask {
 private:
  const ArnelifyServerTaskOpts opts;

 public:
  const int clientSocket;
  ArnelifyReceiver* receiver;
  ArnelifyTransmitter* transmitter;

  ArnelifyServerTask(const int s, const ArnelifyServerTaskOpts &o)
      : clientSocket(s), opts(o) {
    ArnelifyTransmitterOpts transmitterOpts(this->opts.SERVER_BLOCK_SIZE_KB,
                                            this->opts.SERVER_CHARSET,
                                            this->opts.SERVER_GZIP);
    this->transmitter = new ArnelifyTransmitter(transmitterOpts);

    ArnelifyReceiverOpts receiverOpts(
        this->opts.SERVER_ALLOW_EMPTY_FILES, this->opts.SERVER_KEEP_EXTENSIONS,
        this->opts.SERVER_MAX_FIELDS,
        this->opts.SERVER_MAX_FIELDS_SIZE_TOTAL_MB, this->opts.SERVER_MAX_FILES,
        this->opts.SERVER_MAX_FILES_SIZE_TOTAL_MB,
        this->opts.SERVER_MAX_FILE_SIZE_MB, this->opts.SERVER_UPLOAD_DIR);
    this->receiver = new ArnelifyReceiver(receiverOpts);
  };

  ~ArnelifyServerTask() {
    if (this->receiver == nullptr) delete this->receiver;
    if (this->transmitter == nullptr) delete this->transmitter;
  }
};

#endif