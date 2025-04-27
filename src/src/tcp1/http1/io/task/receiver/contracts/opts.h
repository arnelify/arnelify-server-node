#ifndef ARNELIFY_SERVER_HTTP1_RECEIVER_OPTS_H
#define ARNELIFY_SERVER_HTTP1_RECEIVER_OPTS_H

#include <filesystem>
#include <iostream>

struct Http1ReceiverOpts final {
  const bool HTTP1_ALLOW_EMPTY_FILES;
  const bool HTTP1_KEEP_EXTENSIONS;
  const int HTTP1_MAX_FIELDS;
  const std::size_t HTTP1_MAX_FIELDS_SIZE_TOTAL_MB;
  const int HTTP1_MAX_FILES;
  const std::size_t HTTP1_MAX_FILES_SIZE_TOTAL_MB;
  const std::size_t HTTP1_MAX_FILE_SIZE_MB;
  const std::filesystem::path HTTP1_UPLOAD_DIR;

  Http1ReceiverOpts(const bool &a, const bool &k, const int mfd,
                    const std::size_t &mfdst, const int mfl,
                    const std::size_t &mflst, const std::size_t &mfls,
                    const std::string &u = "storage/upload")
      : HTTP1_ALLOW_EMPTY_FILES(a),
        HTTP1_KEEP_EXTENSIONS(k),
        HTTP1_MAX_FIELDS(mfd),
        HTTP1_MAX_FIELDS_SIZE_TOTAL_MB(mfdst),
        HTTP1_MAX_FILES(mfl),
        HTTP1_MAX_FILES_SIZE_TOTAL_MB(mflst),
        HTTP1_MAX_FILE_SIZE_MB(mfls),
        HTTP1_UPLOAD_DIR(u) {};
};

#endif