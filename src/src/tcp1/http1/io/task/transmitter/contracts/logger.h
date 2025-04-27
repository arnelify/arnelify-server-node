#ifndef ARNELIFY_SERVER_HTTP1_TRANSMITTER_LOGGER_H
#define ARNELIFY_SERVER_HTTP1_TRANSMITTER_LOGGER_H

#include <functional>

using Http1TransmitterLogger =
    std::function<void(const std::string&, const bool&)>;

#endif