#ifndef ARNELIFY_UNIX_DOMAIN_SOCKET_CLIENT_OPTS_H
#define ARNELIFY_UNIX_DOMAIN_SOCKET_CLIENT_OPTS_H

#include <filesystem>
#include <iostream>

struct UDSOpts final {
  const std::size_t UDS_BLOCK_SIZE_KB;
  const std::filesystem::path UDS_SOCKET_PATH;

  UDSOpts(const int b, const std::string &s = "/tmp/arnelify.sock")
      : UDS_BLOCK_SIZE_KB(b), UDS_SOCKET_PATH(s) {};
};

#endif