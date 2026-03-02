import { Http1, Http1Opts, Http1Ctx, Http1Stream } from "./tcp1/http1";
import { Http2, Http2Opts, Http2Ctx, Http2Stream } from "./tcp1/http2";
import { WebSocket_, WebSocketOpts, WebSocketCtx, WebSocketBytes, WebSocketStream } from "./tcp1/ws";
import { Http3, Http3Opts, Http3Ctx, Http3Stream } from "./tcp2/http3";
import { WebTransport_, WebTransportOpts, WebTransportCtx, WebTransportBytes, WebTransportStream } from "./tcp2/wt";
export type { Http1Opts, Http1Ctx, Http1Stream };
export type { WebSocketOpts, WebSocketCtx, WebSocketBytes, WebSocketStream };
export type { Http2Opts, Http2Ctx, Http2Stream };
export type { Http3Opts, Http3Ctx, Http3Stream };
export type { WebTransportOpts, WebTransportCtx, WebTransportBytes, WebTransportStream };
export { Http1, WebSocket_, Http2, Http3, WebTransport_ };
//# sourceMappingURL=index.d.ts.map