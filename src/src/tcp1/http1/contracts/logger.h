#ifndef ARNELIFY_SERVER_HTTP1_LOGGER_H
#define ARNELIFY_SERVER_HTTP1_LOGGER_H

#include <functional>

using Http1Logger = std::function<void(const std::string &, const bool &)>;

#endif