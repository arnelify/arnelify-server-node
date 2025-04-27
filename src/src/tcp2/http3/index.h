#ifndef ARNELIFY_SERVER_HTTP_3_H
#define ARNELIFY_SERVER_HTTP_3_H

// ./ngtcp2/examples/qtlsserver 0.0.0.0 3001 server_key.pem server_cert.pem
// ./ngtcp2/examples/qtlsclient 127.0.0.1 3001 http://127.0.0.1:3001/

class Http3 {
 public:
  Http3() {}
  ~Http3() {}
};

#endif