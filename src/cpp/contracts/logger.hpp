#ifndef ARNELIFY_SERVER_LOGGER_HPP
#define ARNELIFY_SERVER_LOGGER_HPP

#include <functional>

using ArnelifyServerLogger =
    std::function<void(const std::string &, const bool &)>;

#endif