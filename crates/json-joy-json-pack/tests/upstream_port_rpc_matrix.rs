use json_joy_json_pack::rpc::{
    RpcAcceptStat, RpcAuthFlavor, RpcAuthStat, RpcDecodeError, RpcMessage, RpcMessageDecoder,
    RpcMessageEncoder, RpcOpaqueAuth, RpcRejectStat,
};

#[derive(Clone, Copy)]
enum FixtureKind {
    Call {
        xid: u32,
        rpcvers: u32,
        prog: u32,
        vers: u32,
        proc_: u32,
        cred_flavor: RpcAuthFlavor,
        verf_flavor: RpcAuthFlavor,
        cred_body_len: usize,
        params_len: usize,
    },
    Accepted {
        xid: u32,
        stat: RpcAcceptStat,
        mismatch: Option<(u32, u32)>,
    },
    Rejected {
        xid: u32,
        stat: RpcRejectStat,
        mismatch: Option<(u32, u32)>,
        auth_stat: Option<RpcAuthStat>,
    },
}

#[derive(Clone, Copy)]
struct Fixture {
    name: &'static str,
    bytes: &'static [u8],
    kind: FixtureKind,
}

const NFS_NULL_CALL: &[u8] = &[
    0x00, 0x00, 0x00, 0x01, // XID
    0x00, 0x00, 0x00, 0x00, // CALL
    0x00, 0x00, 0x00, 0x02, // rpcvers
    0x00, 0x01, 0x86, 0xa3, // prog=100003
    0x00, 0x00, 0x00, 0x03, // vers=3
    0x00, 0x00, 0x00, 0x00, // proc=0
    0x00, 0x00, 0x00, 0x00, // cred flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // cred len
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
];

const PORTMAP_GETPORT: &[u8] = &[
    0x00, 0x00, 0x00, 0x9c, // XID
    0x00, 0x00, 0x00, 0x00, // CALL
    0x00, 0x00, 0x00, 0x02, // rpcvers
    0x00, 0x01, 0x86, 0xa0, // prog=100000
    0x00, 0x00, 0x00, 0x02, // vers=2
    0x00, 0x00, 0x00, 0x03, // proc=3
    0x00, 0x00, 0x00, 0x00, // cred flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // cred len
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
    0x00, 0x01, 0x86, 0xa3, // params: prog=100003
    0x00, 0x00, 0x00, 0x03, // params: vers=3
    0x00, 0x00, 0x00, 0x11, // params: protocol=17
    0x00, 0x00, 0x00, 0x00, // params: port=0
];

const CALL_WITH_AUTH_UNIX: &[u8] = &[
    0x00, 0x00, 0x04, 0xd2, // XID
    0x00, 0x00, 0x00, 0x00, // CALL
    0x00, 0x00, 0x00, 0x02, // rpcvers
    0x00, 0x01, 0x86, 0xa3, // prog=100003
    0x00, 0x00, 0x00, 0x03, // vers=3
    0x00, 0x00, 0x00, 0x01, // proc=1
    0x00, 0x00, 0x00, 0x01, // cred flavor AUTH_UNIX
    0x00, 0x00, 0x00, 0x18, // cred len=24
    0x00, 0x00, 0x00, 0x00, // stamp
    0x00, 0x00, 0x00, 0x04, // machine len
    0x74, 0x65, 0x73, 0x74, // "test"
    0x00, 0x00, 0x03, 0xe8, // uid=1000
    0x00, 0x00, 0x03, 0xe8, // gid=1000
    0x00, 0x00, 0x00, 0x00, // gids len=0
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
];

const SUCCESS_REPLY: &[u8] = &[
    0x00, 0x00, 0x00, 0x9c, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x00, // MSG_ACCEPTED
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
    0x00, 0x00, 0x00, 0x00, // SUCCESS
];

const PROG_UNAVAIL_REPLY: &[u8] = &[
    0x00, 0x00, 0x00, 0x42, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x00, // MSG_ACCEPTED
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
    0x00, 0x00, 0x00, 0x01, // PROG_UNAVAIL
];

const PROG_MISMATCH_REPLY: &[u8] = &[
    0x00, 0x00, 0x01, 0x00, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x00, // MSG_ACCEPTED
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
    0x00, 0x00, 0x00, 0x02, // PROG_MISMATCH
    0x00, 0x00, 0x00, 0x02, // low=2
    0x00, 0x00, 0x00, 0x03, // high=3
];

const PROC_UNAVAIL_REPLY: &[u8] = &[
    0x00, 0x00, 0x00, 0x55, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x00, // MSG_ACCEPTED
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
    0x00, 0x00, 0x00, 0x03, // PROC_UNAVAIL
];

const GARBAGE_ARGS_REPLY: &[u8] = &[
    0x00, 0x00, 0x00, 0x99, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x00, // MSG_ACCEPTED
    0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
    0x00, 0x00, 0x00, 0x00, // verf len
    0x00, 0x00, 0x00, 0x04, // GARBAGE_ARGS
];

const RPC_MISMATCH_REPLY: &[u8] = &[
    0x00, 0x00, 0x00, 0x77, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x01, // MSG_DENIED
    0x00, 0x00, 0x00, 0x00, // RPC_MISMATCH
    0x00, 0x00, 0x00, 0x02, // low=2
    0x00, 0x00, 0x00, 0x02, // high=2
];

const AUTH_BADCRED_REPLY: &[u8] = &[
    0x00, 0x00, 0x00, 0xaa, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x01, // MSG_DENIED
    0x00, 0x00, 0x00, 0x01, // AUTH_ERROR
    0x00, 0x00, 0x00, 0x01, // AUTH_BADCRED
];

const AUTH_TOOWEAK_REPLY: &[u8] = &[
    0x00, 0x00, 0x00, 0xbb, // XID
    0x00, 0x00, 0x00, 0x01, // REPLY
    0x00, 0x00, 0x00, 0x01, // MSG_DENIED
    0x00, 0x00, 0x00, 0x01, // AUTH_ERROR
    0x00, 0x00, 0x00, 0x05, // AUTH_TOOWEAK
];

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "NFS NULL CALL",
        bytes: NFS_NULL_CALL,
        kind: FixtureKind::Call {
            xid: 1,
            rpcvers: 2,
            prog: 100_003,
            vers: 3,
            proc_: 0,
            cred_flavor: RpcAuthFlavor::AuthNone,
            verf_flavor: RpcAuthFlavor::AuthNone,
            cred_body_len: 0,
            params_len: 0,
        },
    },
    Fixture {
        name: "PORTMAP GETPORT",
        bytes: PORTMAP_GETPORT,
        kind: FixtureKind::Call {
            xid: 156,
            rpcvers: 2,
            prog: 100_000,
            vers: 2,
            proc_: 3,
            cred_flavor: RpcAuthFlavor::AuthNone,
            verf_flavor: RpcAuthFlavor::AuthNone,
            cred_body_len: 0,
            params_len: 16,
        },
    },
    Fixture {
        name: "CALL with AUTH_UNIX",
        bytes: CALL_WITH_AUTH_UNIX,
        kind: FixtureKind::Call {
            xid: 1234,
            rpcvers: 2,
            prog: 100_003,
            vers: 3,
            proc_: 1,
            cred_flavor: RpcAuthFlavor::AuthSys,
            verf_flavor: RpcAuthFlavor::AuthNone,
            cred_body_len: 24,
            params_len: 0,
        },
    },
    Fixture {
        name: "SUCCESS REPLY",
        bytes: SUCCESS_REPLY,
        kind: FixtureKind::Accepted {
            xid: 156,
            stat: RpcAcceptStat::Success,
            mismatch: None,
        },
    },
    Fixture {
        name: "PROG_UNAVAIL REPLY",
        bytes: PROG_UNAVAIL_REPLY,
        kind: FixtureKind::Accepted {
            xid: 66,
            stat: RpcAcceptStat::ProgUnavail,
            mismatch: None,
        },
    },
    Fixture {
        name: "PROG_MISMATCH REPLY",
        bytes: PROG_MISMATCH_REPLY,
        kind: FixtureKind::Accepted {
            xid: 256,
            stat: RpcAcceptStat::ProgMismatch,
            mismatch: Some((2, 3)),
        },
    },
    Fixture {
        name: "PROC_UNAVAIL REPLY",
        bytes: PROC_UNAVAIL_REPLY,
        kind: FixtureKind::Accepted {
            xid: 85,
            stat: RpcAcceptStat::ProcUnavail,
            mismatch: None,
        },
    },
    Fixture {
        name: "GARBAGE_ARGS REPLY",
        bytes: GARBAGE_ARGS_REPLY,
        kind: FixtureKind::Accepted {
            xid: 153,
            stat: RpcAcceptStat::GarbageArgs,
            mismatch: None,
        },
    },
    Fixture {
        name: "RPC_MISMATCH REPLY",
        bytes: RPC_MISMATCH_REPLY,
        kind: FixtureKind::Rejected {
            xid: 119,
            stat: RpcRejectStat::RpcMismatch,
            mismatch: Some((2, 2)),
            auth_stat: None,
        },
    },
    Fixture {
        name: "AUTH_BADCRED REPLY",
        bytes: AUTH_BADCRED_REPLY,
        kind: FixtureKind::Rejected {
            xid: 170,
            stat: RpcRejectStat::AuthError,
            mismatch: None,
            auth_stat: Some(RpcAuthStat::AuthBadcred),
        },
    },
    Fixture {
        name: "AUTH_TOOWEAK REPLY",
        bytes: AUTH_TOOWEAK_REPLY,
        kind: FixtureKind::Rejected {
            xid: 187,
            stat: RpcRejectStat::AuthError,
            mismatch: None,
            auth_stat: Some(RpcAuthStat::AuthTooweak),
        },
    },
];

fn decode(bytes: &[u8]) -> RpcMessage {
    let decoder = RpcMessageDecoder::new();
    decoder
        .decode_message(bytes)
        .unwrap_or_else(|e| panic!("decode failed: {e}"))
        .unwrap_or_else(|| panic!("expected complete message"))
}

#[test]
fn rpc_decode_fixture_matrix() {
    for fixture in FIXTURES {
        let msg = decode(fixture.bytes);
        match (fixture.kind, msg) {
            (
                FixtureKind::Call {
                    xid,
                    rpcvers,
                    prog,
                    vers,
                    proc_,
                    cred_flavor,
                    verf_flavor,
                    cred_body_len,
                    params_len,
                },
                RpcMessage::Call(call),
            ) => {
                assert_eq!(call.xid, xid, "fixture={}", fixture.name);
                assert_eq!(call.rpcvers, rpcvers, "fixture={}", fixture.name);
                assert_eq!(call.prog, prog, "fixture={}", fixture.name);
                assert_eq!(call.vers, vers, "fixture={}", fixture.name);
                assert_eq!(call.proc_, proc_, "fixture={}", fixture.name);
                assert_eq!(call.cred.flavor, cred_flavor, "fixture={}", fixture.name);
                assert_eq!(call.verf.flavor, verf_flavor, "fixture={}", fixture.name);
                assert_eq!(
                    call.cred.body.len(),
                    cred_body_len,
                    "fixture={}",
                    fixture.name
                );
                assert_eq!(call.params.len(), params_len, "fixture={}", fixture.name);
            }
            (
                FixtureKind::Accepted {
                    xid,
                    stat,
                    mismatch,
                },
                RpcMessage::AcceptedReply(reply),
            ) => {
                assert_eq!(reply.xid, xid, "fixture={}", fixture.name);
                assert_eq!(reply.stat, stat, "fixture={}", fixture.name);
                match (mismatch, reply.mismatch_info) {
                    (Some((low, high)), Some(actual)) => {
                        assert_eq!(actual.low, low, "fixture={}", fixture.name);
                        assert_eq!(actual.high, high, "fixture={}", fixture.name);
                    }
                    (None, None) => {}
                    _ => panic!("mismatch_info mismatch for fixture={}", fixture.name),
                }
            }
            (
                FixtureKind::Rejected {
                    xid,
                    stat,
                    mismatch,
                    auth_stat,
                },
                RpcMessage::RejectedReply(reply),
            ) => {
                assert_eq!(reply.xid, xid, "fixture={}", fixture.name);
                assert_eq!(reply.stat, stat, "fixture={}", fixture.name);
                assert_eq!(reply.auth_stat, auth_stat, "fixture={}", fixture.name);
                match (mismatch, reply.mismatch_info) {
                    (Some((low, high)), Some(actual)) => {
                        assert_eq!(actual.low, low, "fixture={}", fixture.name);
                        assert_eq!(actual.high, high, "fixture={}", fixture.name);
                    }
                    (None, None) => {}
                    _ => panic!("mismatch_info mismatch for fixture={}", fixture.name),
                }
            }
            (expected, actual) => panic!(
                "message kind mismatch for fixture={}: expected {:?}, got {:?}",
                fixture.name,
                expected_discriminant(expected),
                actual
            ),
        }
    }
}

fn expected_discriminant(kind: FixtureKind) -> &'static str {
    match kind {
        FixtureKind::Call { .. } => "Call",
        FixtureKind::Accepted { .. } => "AcceptedReply",
        FixtureKind::Rejected { .. } => "RejectedReply",
    }
}

#[test]
fn rpc_fixture_roundtrip_matrix() {
    let mut encoder = RpcMessageEncoder::new();
    let decoder = RpcMessageDecoder::new();

    for fixture in FIXTURES {
        let first = decode(fixture.bytes);
        let encoded = encoder
            .encode_message(&first)
            .unwrap_or_else(|e| panic!("encode failed fixture={}: {e}", fixture.name));
        let second = decoder
            .decode_message(&encoded)
            .unwrap_or_else(|e| panic!("decode failed fixture={}: {e}", fixture.name))
            .unwrap_or_else(|| panic!("expected complete message fixture={}", fixture.name));
        assert_eq!(second, first, "fixture={}", fixture.name);
    }
}

#[test]
fn rpc_encoder_decoder_matrix() {
    let mut encoder = RpcMessageEncoder::new();
    let decoder = RpcMessageDecoder::new();

    let cred = RpcOpaqueAuth::none();
    let verf = RpcOpaqueAuth::none();

    let call = encoder
        .encode_call(1, 100, 1, 0, &cred, &verf, &[])
        .expect("encode call");
    let decoded = decoder.decode_message(&call).expect("decode call").unwrap();
    match decoded {
        RpcMessage::Call(msg) => {
            assert_eq!(msg.xid, 1);
            assert_eq!(msg.rpcvers, 2);
            assert_eq!(msg.prog, 100);
            assert_eq!(msg.vers, 1);
            assert_eq!(msg.proc_, 0);
            assert_eq!(msg.cred.flavor, RpcAuthFlavor::AuthNone);
        }
        other => panic!("expected call, got {other:?}"),
    }

    let cred = RpcOpaqueAuth {
        flavor: RpcAuthFlavor::AuthSys,
        body: vec![1, 2, 3, 4, 5],
    };
    let call = encoder
        .encode_call(10, 200, 2, 5, &cred, &verf, &[0, 0, 0, 42])
        .expect("encode call with auth");
    let decoded = decoder
        .decode_message(&call)
        .expect("decode call with auth")
        .unwrap();
    match decoded {
        RpcMessage::Call(msg) => {
            assert_eq!(msg.xid, 10);
            assert_eq!(msg.prog, 200);
            assert_eq!(msg.vers, 2);
            assert_eq!(msg.proc_, 5);
            assert_eq!(msg.cred.body, vec![1, 2, 3, 4, 5]);
            assert_eq!(msg.params, vec![0, 0, 0, 42]);
        }
        other => panic!("expected call, got {other:?}"),
    }

    for (xid, stat) in [
        (2, RpcAcceptStat::ProgUnavail),
        (4, RpcAcceptStat::ProcUnavail),
        (5, RpcAcceptStat::GarbageArgs),
    ] {
        let bytes = encoder
            .encode_accepted_reply(xid, &verf, stat as u32, None, &[])
            .expect("encode accepted reply");
        let decoded = decoder
            .decode_message(&bytes)
            .expect("decode accepted reply")
            .unwrap();
        match decoded {
            RpcMessage::AcceptedReply(msg) => {
                assert_eq!(msg.xid, xid);
                assert_eq!(msg.stat, stat);
            }
            other => panic!("expected accepted reply, got {other:?}"),
        }
    }

    let mismatch = json_joy_json_pack::rpc::RpcMismatchInfo { low: 1, high: 3 };
    let bytes = encoder
        .encode_accepted_reply(
            3,
            &verf,
            RpcAcceptStat::ProgMismatch as u32,
            Some(&mismatch),
            &[],
        )
        .expect("encode prog mismatch");
    let decoded = decoder
        .decode_message(&bytes)
        .expect("decode prog mismatch")
        .unwrap();
    match decoded {
        RpcMessage::AcceptedReply(msg) => {
            assert_eq!(msg.xid, 3);
            assert_eq!(msg.stat, RpcAcceptStat::ProgMismatch);
            let mismatch = msg.mismatch_info.expect("mismatch info");
            assert_eq!(mismatch.low, 1);
            assert_eq!(mismatch.high, 3);
        }
        other => panic!("expected accepted reply, got {other:?}"),
    }

    let mismatch = json_joy_json_pack::rpc::RpcMismatchInfo { low: 2, high: 2 };
    let bytes =
        encoder.encode_rejected_reply(6, RpcRejectStat::RpcMismatch as u32, Some(&mismatch), None);
    let decoded = decoder
        .decode_message(&bytes)
        .expect("decode rpc mismatch")
        .unwrap();
    match decoded {
        RpcMessage::RejectedReply(msg) => {
            assert_eq!(msg.xid, 6);
            assert_eq!(msg.stat, RpcRejectStat::RpcMismatch);
            let mismatch = msg.mismatch_info.expect("mismatch info");
            assert_eq!(mismatch.low, 2);
            assert_eq!(mismatch.high, 2);
        }
        other => panic!("expected rejected reply, got {other:?}"),
    }

    let bytes = encoder.encode_rejected_reply(
        7,
        RpcRejectStat::AuthError as u32,
        None,
        Some(RpcAuthStat::AuthBadcred as u32),
    );
    let decoded = decoder
        .decode_message(&bytes)
        .expect("decode auth error")
        .unwrap();
    match decoded {
        RpcMessage::RejectedReply(msg) => {
            assert_eq!(msg.xid, 7);
            assert_eq!(msg.stat, RpcRejectStat::AuthError);
            assert_eq!(msg.auth_stat, Some(RpcAuthStat::AuthBadcred));
        }
        other => panic!("expected rejected reply, got {other:?}"),
    }
}

#[test]
fn rpc_auth_padding_roundtrip_matrix() {
    let mut encoder = RpcMessageEncoder::new();
    let decoder = RpcMessageDecoder::new();
    let verf = RpcOpaqueAuth::none();

    for (xid, body) in [
        (1u32, vec![1u8]),
        (2u32, vec![1u8, 2]),
        (3u32, vec![1u8, 2, 3]),
        (4u32, vec![1u8, 2, 3, 4]),
    ] {
        let cred = RpcOpaqueAuth {
            flavor: RpcAuthFlavor::AuthSys,
            body: body.clone(),
        };
        let bytes = encoder
            .encode_call(xid, 100, 1, 0, &cred, &verf, &[])
            .expect("encode call with padded auth");
        let decoded = decoder
            .decode_message(&bytes)
            .expect("decode call with padded auth")
            .unwrap();
        match decoded {
            RpcMessage::Call(msg) => {
                assert_eq!(msg.xid, xid);
                assert_eq!(msg.cred.body, body);
            }
            other => panic!("expected call, got {other:?}"),
        }
    }
}

#[test]
fn rpc_partial_and_error_matrix() {
    let decoder = RpcMessageDecoder::new();

    let partial = &CALL_WITH_AUTH_UNIX[..20];
    assert_eq!(
        decoder.decode_message(partial).expect("partial decode"),
        None
    );

    let invalid_msg_type = [
        0x00, 0x00, 0x00, 0x01, // xid
        0x00, 0x00, 0x00, 0x99, // invalid msg_type
    ];
    let err = decoder
        .decode_message(&invalid_msg_type)
        .expect_err("invalid msg_type should error");
    assert_eq!(err, RpcDecodeError::InvalidMessageType(0x99));

    let invalid_reply_stat = [
        0x00, 0x00, 0x00, 0x01, // xid
        0x00, 0x00, 0x00, 0x01, // REPLY
        0x00, 0x00, 0x00, 0x99, // invalid reply_stat
    ];
    let err = decoder
        .decode_message(&invalid_reply_stat)
        .expect_err("invalid reply_stat should error");
    assert_eq!(err, RpcDecodeError::InvalidReplyStat(0x99));

    let oversized_auth = [
        0x00, 0x00, 0x00, 0x01, // xid
        0x00, 0x00, 0x00, 0x00, // CALL
        0x00, 0x00, 0x00, 0x02, // rpcvers
        0x00, 0x00, 0x00, 0x01, // prog
        0x00, 0x00, 0x00, 0x01, // vers
        0x00, 0x00, 0x00, 0x00, // proc
        0x00, 0x00, 0x00, 0x01, // cred flavor AUTH_UNIX
        0xff, 0xff, 0xff, 0xff, // oversize auth len
    ];
    let err = decoder
        .decode_message(&oversized_auth)
        .expect_err("oversized auth should error");
    assert_eq!(err, RpcDecodeError::AuthBodyTooLarge(u32::MAX));
}

#[test]
fn rpc_non_success_accepted_reply_keeps_results_when_present() {
    let decoder = RpcMessageDecoder::new();
    let bytes = [
        0x00, 0x00, 0x00, 0x02, // xid
        0x00, 0x00, 0x00, 0x01, // REPLY
        0x00, 0x00, 0x00, 0x00, // MSG_ACCEPTED
        0x00, 0x00, 0x00, 0x00, // verf flavor AUTH_NULL
        0x00, 0x00, 0x00, 0x00, // verf len
        0x00, 0x00, 0x00, 0x01, // PROG_UNAVAIL
        0x00, 0x00, 0x00, 0x2a, // trailing data
    ];

    let decoded = decoder
        .decode_message(&bytes)
        .expect("decode non-success accepted reply")
        .unwrap();
    match decoded {
        RpcMessage::AcceptedReply(msg) => {
            assert_eq!(msg.stat, RpcAcceptStat::ProgUnavail);
            assert_eq!(msg.results, Some(vec![0x00, 0x00, 0x00, 0x2a]));
        }
        other => panic!("expected accepted reply, got {other:?}"),
    }
}
