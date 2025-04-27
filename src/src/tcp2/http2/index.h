#ifndef ARNELIFY_SERVER_HTTP_2_H
#define ARNELIFY_SERVER_HTTP_2_H

// ./ngtcp2/examples/qtlsserver 0.0.0.0 3001 server_key.pem server_cert.pem
// ./ngtcp2/examples/qtlsclient 127.0.0.1 3001 http://127.0.0.1:3001/

class Http2 {
 public:
  Http2() {}
  ~Http2() {}
};

#endif