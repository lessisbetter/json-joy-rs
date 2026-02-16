use json_joy_core::model_api::{ApiOperation, ApiOperationKind, NativeModelApi, PathStep};
use json_joy_core::patch::{DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use serde_json::json;

fn patch_from_ops(sid: u64, time: u64, ops: &[DecodedOp]) -> Patch {
    let bytes = encode_patch_from_ops(sid, time, ops).expect("patch encode must succeed");
    Patch::from_binary(&bytes).expect("patch decode must succeed")
}

#[test]
fn upstream_port_model_api_from_patches_and_apply_batch() {
    // Upstream mapping:
    // - json-crdt/model/Model.ts `fromPatches`, `applyBatch`
    let sid = 97001;
    let root = Timestamp { sid, time: 1 };
    let hello = Timestamp { sid, time: 3 };
    let world = Timestamp { sid, time: 5 };

    let p1_ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewCon {
            id: hello,
            value: json_joy_core::patch::ConValue::Json(json!("hello")),
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 4 },
            obj: root,
            data: vec![("msg".into(), hello)],
        },
    ];
    let p2_ops = vec![
        DecodedOp::NewCon {
            id: world,
            value: json_joy_core::patch::ConValue::Json(json!("world")),
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 6 },
            obj: root,
            data: vec![("next".into(), world)],
        },
    ];
    let p1 = patch_from_ops(sid, 1, &p1_ops);
    let p2 = patch_from_ops(sid, 5, &p2_ops);

    let mut api =
        NativeModelApi::from_patches(std::slice::from_ref(&p1)).expect("from_patches must succeed");
    api.apply_batch(std::slice::from_ref(&p2))
        .expect("apply_batch must succeed");

    assert_eq!(api.view(), json!({"msg":"hello","next":"world"}));
}

#[test]
fn upstream_port_model_api_find_path_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/find.ts
    let sid = 97002;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 3 },
            },
            DecodedOp::NewArr {
                id: Timestamp { sid, time: 4 },
            },
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 5 },
                value: json_joy_core::patch::ConValue::Json(json!(1)),
            },
            DecodedOp::InsArr {
                id: Timestamp { sid, time: 6 },
                obj: Timestamp { sid, time: 4 },
                reference: Timestamp { sid, time: 4 },
                data: vec![Timestamp { sid, time: 5 }],
            },
            DecodedOp::InsObj {
                id: Timestamp { sid, time: 7 },
                obj: Timestamp { sid, time: 3 },
                data: vec![("items".into(), Timestamp { sid, time: 4 })],
            },
            DecodedOp::InsObj {
                id: Timestamp { sid, time: 8 },
                obj: Timestamp { sid, time: 1 },
                data: vec![("doc".into(), Timestamp { sid, time: 3 })],
            },
        ],
    )])
    .expect("from_patches must succeed");

    assert_eq!(
        api.find(&[
            PathStep::Key("doc".into()),
            PathStep::Key("items".into()),
            PathStep::Index(0),
        ]),
        Some(json!(1))
    );
    api.arr_push(
        &[PathStep::Key("doc".into()), PathStep::Key("items".into())],
        json!(2),
    )
    .expect("arr_push must succeed");
    assert_eq!(
        api.find(&[
            PathStep::Key("doc".into()),
            PathStep::Key("items".into()),
            PathStep::Index(1),
        ]),
        Some(json!(2))
    );
}

#[test]
fn upstream_port_model_api_mutator_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts (set/object/array/string style edits)
    let sid = 97003;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.obj_put(&[], "title", json!("hello"))
        .expect("obj_put must succeed");
    api.obj_put(&[], "list", json!([1]))
        .expect("obj_put must succeed");
    api.add(
        &[PathStep::Key("list".into()), PathStep::Index(1)],
        json!(9),
    )
    .expect("add must succeed");
    api.replace(
        &[PathStep::Key("list".into()), PathStep::Index(0)],
        json!(7),
    )
    .expect("replace must succeed");
    api.remove(&[PathStep::Key("list".into()), PathStep::Index(2)])
        .expect("remove must succeed");
    api.arr_push(&[PathStep::Key("list".into())], json!(2))
        .expect("arr_push must succeed");
    api.obj_put(&[], "name", json!("ab"))
        .expect("obj_put must succeed");
    api.str_ins(&[PathStep::Key("name".into())], 1, "Z")
        .expect("str_ins must succeed");
    api.add(&[PathStep::Key("subtitle".into())], json!("s"))
        .expect("add on object must succeed");
    api.replace(&[PathStep::Key("subtitle".into())], json!("S"))
        .expect("replace on object must succeed");
    api.remove(&[PathStep::Key("subtitle".into())])
        .expect("remove on object must succeed");
    api.set(&[PathStep::Key("title".into())], json!("world"))
        .expect("set must succeed");

    assert_eq!(
        api.view(),
        json!({"title":"world","list":[7,9,2],"name":"aZb"})
    );
}

#[test]
fn upstream_port_model_api_tolerant_ops_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts NodeApi.{add,replace,remove,op,read,select}
    let sid = 97004;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    assert_eq!(api.read(None), Some(json!({})));
    assert_eq!(api.select(Some(&[PathStep::Key("missing".into())])), None);

    assert!(api.try_add(&[PathStep::Key("items".into())], json!([1, 2])));
    assert!(api.try_add(&[PathStep::Key("items".into()), PathStep::Append], json!(3)));
    assert!(api.try_replace(
        &[PathStep::Key("items".into()), PathStep::Index(0)],
        json!(7)
    ));
    assert!(api.try_remove(&[PathStep::Key("items".into()), PathStep::Index(1)]));
    assert!(api.op(ApiOperation::Add {
        path: vec![PathStep::Key("title".into())],
        value: json!("ok"),
    }));
    assert!(api.op(ApiOperation::Replace {
        path: vec![PathStep::Key("title".into())],
        value: json!("ready"),
    }));
    assert!(api.op(ApiOperation::Remove {
        path: vec![PathStep::Key("title".into())],
        length: 1,
    }));

    assert_eq!(
        api.read(Some(&[PathStep::Key("items".into())])),
        Some(json!([7, 3]))
    );
    assert!(!api.try_add(&[], json!(1)));
    assert!(!api.try_replace(&[PathStep::Key("missing".into())], json!(1)));
    assert!(!api.try_remove(&[]));
}

#[test]
fn upstream_port_model_api_node_handle_proxy_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/proxy.ts path-bound ergonomic mutation surface.
    let sid = 97005;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.node()
        .obj_put("doc", json!({"title":"ab","list":[1]}))
        .expect("obj_put via handle must succeed");
    api.node()
        .at_key("doc")
        .at_key("list")
        .arr_push(json!(2))
        .expect("arr_push via handle must succeed");
    api.node()
        .at_key("doc")
        .at_key("title")
        .str_ins(1, "Z")
        .expect("str_ins via handle must succeed");
    api.node()
        .at_key("doc")
        .at_key("list")
        .at_append()
        .add(json!(3))
        .expect("append add via handle must succeed");
    api.node()
        .at_key("doc")
        .at_key("list")
        .at_index(0)
        .replace(json!(7))
        .expect("replace via handle must succeed");

    let doc = api.node().at_key("doc").read();
    assert_eq!(doc, Some(json!({"title":"aZb","list":[7,2,3]})));
}

#[test]
fn upstream_port_model_api_bin_handle_native_mutation_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts BinApi.ins / BinApi.del behavior surface.
    let sid = 97055;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
            DecodedOp::NewBin {
                id: Timestamp { sid, time: 3 },
            },
            DecodedOp::InsBin {
                id: Timestamp { sid, time: 4 },
                obj: Timestamp { sid, time: 3 },
                reference: Timestamp { sid, time: 3 },
                data: vec![1, 2, 3],
            },
            DecodedOp::InsObj {
                id: Timestamp { sid, time: 7 },
                obj: Timestamp { sid, time: 1 },
                data: vec![("bin".into(), Timestamp { sid, time: 3 })],
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.node()
        .at_key("bin")
        .as_bin()
        .expect("bin handle")
        .ins(1, &[9, 8])
        .expect("bin ins must succeed");
    api.node()
        .at_key("bin")
        .as_bin()
        .expect("bin handle")
        .del(0, 2)
        .expect("bin del must succeed");

    assert_eq!(api.view(), json!({"bin":{"0":8,"1":2,"2":3}}));
}

#[test]
fn upstream_port_model_api_typed_node_wrappers_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts NodeApi.{asObj,asArr,asStr} behavior slice.
    let sid = 97006;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.node()
        .as_obj()
        .expect("root object wrapper must resolve")
        .set("doc", json!({"title":"ab","list":[1,2]}))
        .expect("set via object wrapper must succeed");

    let mut list = api
        .node()
        .at_key("doc")
        .at_key("list")
        .as_arr()
        .expect("array wrapper must resolve");
    assert_eq!(list.length(), 2);
    list.ins(1, json!(9)).expect("ins must succeed");
    list.upd(0, json!(7)).expect("upd must succeed");
    list.del(2).expect("del must succeed");

    let mut title = api
        .node()
        .at_key("doc")
        .at_key("title")
        .as_str()
        .expect("string wrapper must resolve");
    assert_eq!(title.length(), 2);
    title.ins(1, "Z").expect("str ins must succeed");
    title.del(2, 1).expect("str del must succeed");

    let mut doc = api
        .node()
        .at_key("doc")
        .as_obj()
        .expect("object wrapper must resolve");
    assert!(doc.has("title"));
    doc.del("title").expect("object del must succeed");

    assert_eq!(api.view(), json!({"doc":{"list":[7,9]}}));
}

#[test]
fn upstream_port_model_api_extended_typed_wrappers_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts typed wrappers (`asVal/asBin/asVec/asCon`) baseline.
    let sid = 97007;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.node()
        .as_obj()
        .expect("root object wrapper must resolve")
        .set("bin", json!([1, 2, 3]))
        .expect("bin seed set must succeed");
    api.node()
        .as_obj()
        .expect("root object wrapper must resolve")
        .set("vec", json!([1, 2]))
        .expect("vec seed set must succeed");
    api.node()
        .as_obj()
        .expect("root object wrapper must resolve")
        .set("con", json!("x"))
        .expect("con seed set must succeed");

    let mut bin = api
        .node()
        .at_key("bin")
        .as_bin()
        .expect("bin wrapper must resolve");
    assert_eq!(bin.length(), 3);
    bin.ins(1, &[9, 8]).expect("bin ins must succeed");
    bin.del(0, 1).expect("bin del must succeed");

    let mut vec = api
        .node()
        .at_key("vec")
        .as_vec()
        .expect("vec wrapper must resolve");
    vec.set(3, Some(json!(7))).expect("vec set must succeed");
    vec.set(1, None).expect("vec remove-style set must succeed");

    let mut con = api
        .node()
        .at_key("con")
        .as_con()
        .expect("con wrapper must resolve");
    assert_eq!(con.view(), Some(json!("x")));
    con.set(json!("y")).expect("con set must succeed");

    let mut val = api
        .node()
        .at_key("con")
        .as_val()
        .expect("val wrapper must resolve");
    assert_eq!(val.view(), Some(json!("y")));
    val.set(json!("z")).expect("val set must succeed");

    assert_eq!(
        api.view(),
        json!({"bin":[9,8,2,3],"vec":[1,null,null,7],"con":"z"})
    );
}

#[test]
fn upstream_port_model_api_diff_merge_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts NodeApi.{diff,merge,op('merge')} behavior slice.
    let sid = 97008;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.obj_put(&[], "doc", json!({"a":1,"b":[1]}))
        .expect("seed put must succeed");

    let diff = api
        .diff(&json!({"doc":{"a":2,"b":[1,2]}}))
        .expect("diff must succeed");
    assert!(diff.is_some());

    assert!(api.merge(
        Some(&[PathStep::Key("doc".into()), PathStep::Key("a".into())]),
        json!(2)
    ));
    assert!(api.op(ApiOperation::Merge {
        path: vec![PathStep::Key("doc".into()), PathStep::Key("b".into())],
        value: json!([1, 2]),
    }));

    assert_eq!(api.view(), json!({"doc":{"a":2,"b":[1,2]}}));
}

#[test]
fn upstream_port_model_api_json_pointer_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts path normalization (`toPath`) and "-" append behavior.
    let sid = 97009;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.obj_put(&[], "doc", json!({"a/b":{"~k":[1]}}))
        .expect("seed put must succeed");
    assert_eq!(api.read_ptr(Some("/doc/a~1b/~0k/0")), Some(json!(1)));
    assert_eq!(api.select_ptr(Some("doc/a~1b/~0k/0")), Some(json!(1)));

    assert!(api.try_add_ptr("/doc/a~1b/~0k/-", json!(2)));
    assert!(api.try_replace_ptr("/doc/a~1b/~0k/0", json!(7)));
    assert!(api.try_remove_ptr("/doc/a~1b/~0k/1"));

    assert_eq!(api.read_ptr(Some("/doc/a~1b/~0k")), Some(json!([7])));
    assert!(!api.try_add_ptr("/missing/-", json!(1)));
}

#[test]
fn upstream_port_model_api_remove_length_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts remove(path, length) semantics across families.
    let sid = 97010;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.obj_put(&[], "arr", json!([1, 2, 3, 4]))
        .expect("seed arr");
    api.obj_put(&[], "str", json!("abcdef")).expect("seed str");
    api.obj_put(&[], "bin", json!([1, 2, 3, 4, 5]))
        .expect("seed bin");

    assert!(api.try_remove_with_length(&[PathStep::Key("arr".into()), PathStep::Index(1)], 2));
    assert!(api.try_remove_with_length(&[PathStep::Key("str".into()), PathStep::Index(2)], 3));
    assert!(api.op(ApiOperation::Remove {
        path: vec![PathStep::Key("bin".into()), PathStep::Index(1)],
        length: 3,
    }));

    assert_eq!(api.view(), json!({"arr":[1,4],"str":"abf","bin":[1,5]}));
}

#[test]
fn upstream_port_model_api_operation_tuple_dispatch_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts `op([type,path,value])` dispatch style.
    let sid = 97011;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
        ],
    )])
    .expect("from_patches must succeed");

    assert!(api.op_tuple(
        ApiOperationKind::Add,
        &[PathStep::Key("doc".into())],
        Some(json!({"x":[1,2,3]})),
        None
    ));
    assert!(api.op_ptr_tuple(ApiOperationKind::Replace, "/doc/x/0", Some(json!(7)), None));
    assert!(api.op_ptr_tuple(ApiOperationKind::Remove, "/doc/x/1", None, Some(2)));
    assert!(api.op_ptr_tuple(
        ApiOperationKind::Merge,
        "/doc",
        Some(json!({"x":[7],"ok":true})),
        None
    ));

    assert_eq!(api.view(), json!({"doc":{"x":[7],"ok":true}}));
    assert!(!api.op_ptr_tuple(ApiOperationKind::Add, "/doc/x/0", None, None));
}

#[test]
fn upstream_port_model_api_add_replace_remove_vec_and_array_semantics_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/nodes.ts NodeApi.{add,replace,remove} semantics:
    //   - array add inserts full array payload elements
    //   - array replace at index==length appends
    //   - vec replace/remove map to set(index, value|undefined)
    let sid = 97012;
    let mut api = NativeModelApi::from_patches(&[patch_from_ops(
        sid,
        1,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
            DecodedOp::NewVec {
                id: Timestamp { sid, time: 3 },
            },
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 4 },
                value: json_joy_core::patch::ConValue::Json(json!(10)),
            },
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 5 },
                value: json_joy_core::patch::ConValue::Json(json!(20)),
            },
            DecodedOp::InsVec {
                id: Timestamp { sid, time: 6 },
                obj: Timestamp { sid, time: 3 },
                data: vec![
                    (0, Timestamp { sid, time: 4 }),
                    (1, Timestamp { sid, time: 5 }),
                ],
            },
            DecodedOp::InsObj {
                id: Timestamp { sid, time: 7 },
                obj: Timestamp { sid, time: 1 },
                data: vec![("vec".into(), Timestamp { sid, time: 3 })],
            },
        ],
    )])
    .expect("from_patches must succeed");

    api.obj_put(&[], "arr", json!([1, 2])).expect("seed arr");

    assert!(api.try_add(
        &[PathStep::Key("arr".into()), PathStep::Index(1)],
        json!([7, 8])
    ));
    assert!(api.try_replace(&[PathStep::Key("arr".into()), PathStep::Index(4)], json!(9)));
    assert!(api.try_replace(
        &[PathStep::Key("vec".into()), PathStep::Index(3)],
        json!(30)
    ));
    assert!(api.try_remove(&[PathStep::Key("vec".into()), PathStep::Index(1)]));

    assert_eq!(
        api.view(),
        json!({"arr":[1,7,8,2,9],"vec":[10,null,null,30]})
    );
}
