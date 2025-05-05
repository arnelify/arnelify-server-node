#ifndef ARNELIFY_SERVER_TEST_CPP
#define ARNELIFY_SERVER_TEST_CPP

#include <iostream>

#include "json.h"

#include "../index.h"

int main(int argc, char* argv[]) {
  Http1Opts http1Opts(true, 64, "UTF-8", true, true, 1024, 20, 1, 60, 60, 3001,
                      3, 1024, "./storage/upload");

  Http1 http1(http1Opts);
  http1.handler([](const Http1Req& req, Http1Res res) {
    Json::StreamWriterBuilder writer;
    writer["indentation"] = "";
    writer["emitUTF8"] = true;

    res->setCode(200);
    res->addBody(Json::writeString(writer, req));
    res->end();
  });

  http1.start([](const std::string& message, const bool& isError) {
    if (isError) {
      std::cout << "[Arnelify Server]: Error: " << message << std::endl;
      exit(1);
    }

    std::cout << "[Arnelify Server]: " << message << std::endl;
  });

  return 0;
}

#endif