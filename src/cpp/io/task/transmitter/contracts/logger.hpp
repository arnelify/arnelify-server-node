#ifndef ARNELIFY_TRANSMITTER_LOGGER_HPP
#define ARNELIFY_TRANSMITTER_LOGGER_HPP

#include <functional>

using ArnelifyTransmitterLogger =
    std::function<void(const std::string&, const bool&)>;

#endif