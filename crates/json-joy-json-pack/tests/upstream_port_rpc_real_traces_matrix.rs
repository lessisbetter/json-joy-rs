use json_joy_json_pack::rm::RmRecordDecoder;
use json_joy_json_pack::rpc::{RpcAcceptStat, RpcMessage, RpcMessageDecoder};

const NFS3_LOOKUP_CALL_HEX: &str = "80000090eb8a42cb0000000000000002000186a30000000300000003000000010000003c00490e680000001d455042594d494e573039333554312e6d696e736b2e6570616d2e636f6d000000000001f40000000a000000020000000a000001f400000000000000000000001c9725bb51046621880c000000a68c020078286c3e00000000000000000000000568656c6c6f000000";
const NFS3_ACCESS_CALL_HEX: &str = "80000088ea8a42cb0000000000000002000186a30000000300000004000000010000003c00490e680000001d455042594d494e573039333554312e6d696e736b2e6570616d2e636f6d000000000001f40000000a000000020000000a000001f400000000000000000000001c9725bb51046621880c000000a68c020078286c3e00000000000000000000001f";
const NFS3_READDIRPLUS_REPLY_HEX: &str = "800001b4ed8a42cb0000000100000000000000000000000000000000000000000000000100000002000001ed00000002000001f400000000000000000000020000000000000008000000003c000a009700000000000000410000000000028ca651ed1cc20000000051ed1cb00000000051ed1cb0000000000000000000000f59000000010000000000028ca6000000012e000000000000000000000c0000000100000002000001ed00000002000001f400000000000000000000020000000000000008000000003c000a009700000000000000410000000000028ca651ed1cc20000000051ed1cb00000000051ed1cb000000000000000010000001c9725bb51046621880c000000a68c020078286c3e0000000000000000000000010000000000012665000000022e2e000000000000000002000000000100000002000001ff00000005000003ea000000000000000000000200000000000000080000000096000400df0000000000000041000000000001266551ec763d0000000051e69ed20000000051e69ed200000000000000010000001c9725bb51046621880c000000652601008072c43300000000000000000000000000000001";

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0, "hex length must be even");
    let mut out = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16)
            .unwrap_or_else(|e| panic!("invalid hex at offset {i}: {e}"));
        out.push(byte);
    }
    out
}

fn decode_rm_framed_rpc(hex: &str) -> RpcMessage {
    let mut rm_decoder = RmRecordDecoder::new();
    let rpc_decoder = RpcMessageDecoder::new();

    let bytes = decode_hex(hex);
    rm_decoder.push(&bytes);

    let record = rm_decoder
        .read_record()
        .unwrap_or_else(|| panic!("expected complete RM record"));
    rpc_decoder
        .decode_message(&record)
        .unwrap_or_else(|e| panic!("rpc decode failed: {e}"))
        .unwrap_or_else(|| panic!("expected complete RPC message"))
}

#[test]
fn rpc_real_trace_lookup_call_matrix() {
    let msg = decode_rm_framed_rpc(NFS3_LOOKUP_CALL_HEX);
    match msg {
        RpcMessage::Call(call) => {
            assert_eq!(call.xid, 0xeb8a42cb);
            assert_eq!(call.rpcvers, 2);
            assert_eq!(call.prog, 100_003);
            assert_eq!(call.vers, 3);
            assert_eq!(call.proc_, 3);
        }
        other => panic!("expected call, got {other:?}"),
    }
}

#[test]
fn rpc_real_trace_access_call_matrix() {
    let msg = decode_rm_framed_rpc(NFS3_ACCESS_CALL_HEX);
    match msg {
        RpcMessage::Call(call) => {
            assert_eq!(call.xid, 0xea8a42cb);
            assert_eq!(call.rpcvers, 2);
            assert_eq!(call.prog, 100_003);
            assert_eq!(call.vers, 3);
            assert_eq!(call.proc_, 4);
        }
        other => panic!("expected call, got {other:?}"),
    }
}

#[test]
fn rpc_real_trace_readdirplus_reply_matrix() {
    let msg = decode_rm_framed_rpc(NFS3_READDIRPLUS_REPLY_HEX);
    match msg {
        RpcMessage::AcceptedReply(reply) => {
            assert_eq!(reply.xid, 3_985_261_259);
            assert_eq!(reply.stat, RpcAcceptStat::Success);
            assert!(reply.results.is_some());
        }
        other => panic!("expected accepted reply, got {other:?}"),
    }
}
