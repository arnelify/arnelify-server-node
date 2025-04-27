#ifndef ARNELIFY_SERVER_HTTP1_OPTS_H
#define ARNELIFY_SERVER_HTTP1_OPTS_H

#include <filesystem>
#include <iostream>

struct Http1Opts final {
  const bool HTTP1_ALLOW_EMPTY_FILES;
  const std::size_t HTTP1_BLOCK_SIZE_KB;
  const std::string HTTP1_CHARSET;
  const bool HTTP1_GZIP;
  const bool HTTP1_KEEP_EXTENSIONS;
  const int HTTP1_MAX_FIELDS;
  const std::size_t HTTP1_MAX_FIELDS_SIZE_TOTAL_MB;
  const int HTTP1_MAX_FILES;
  const std::size_t HTTP1_MAX_FILES_SIZE_TOTAL_MB;
  const std::size_t HTTP1_MAX_FILE_SIZE_MB;
  const int HTTP1_PORT;
  const int HTTP1_THREAD_LIMIT;
  const int HTTP1_QUEUE_LIMIT;
  const std::filesystem::path HTTP1_UPLOAD_DIR;

  Http1Opts(const bool &a, const int b, const std::string &c, const bool &g,
            const bool &k, const int mfd, const int mfdst, const int mfl,
            const int mflst, const int mfls, const int p, const int tl,
            const int q, const std::string &u = "storage/upload")
      : HTTP1_ALLOW_EMPTY_FILES(a),
        HTTP1_BLOCK_SIZE_KB(b),
        HTTP1_CHARSET(c),
        HTTP1_GZIP(g),
        HTTP1_KEEP_EXTENSIONS(k),
        HTTP1_MAX_FIELDS(mfd),
        HTTP1_MAX_FIELDS_SIZE_TOTAL_MB(mfdst),
        HTTP1_MAX_FILES(mfl),
        HTTP1_MAX_FILES_SIZE_TOTAL_MB(mflst),
        HTTP1_MAX_FILE_SIZE_MB(mfls),
        HTTP1_PORT(p),
        HTTP1_THREAD_LIMIT(tl),
        HTTP1_QUEUE_LIMIT(q),
        HTTP1_UPLOAD_DIR(u) {};
};

#endif