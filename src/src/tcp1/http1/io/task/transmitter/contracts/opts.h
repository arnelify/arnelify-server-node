#ifndef ARNELIFY_SERVER_HTTP1_TRANSMITTER_OPTS_H
#define ARNELIFY_SERVER_HTTP1_TRANSMITTER_OPTS_H

#include <iostream>

struct Http1TransmitterOpts final {
  const std::size_t HTTP1_BLOCK_SIZE_KB;
  const std::string HTTP1_CHARSET;
  const bool HTTP1_GZIP;

  Http1TransmitterOpts(const std::size_t &bs, const std::string &ch,
                       const bool &g)
      : HTTP1_BLOCK_SIZE_KB(bs), HTTP1_CHARSET(ch), HTTP1_GZIP(g) {};
};

#endif