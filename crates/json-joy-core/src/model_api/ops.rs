impl NativeModelApi {
    pub fn from_model_binary(data: &[u8], sid_hint: Option<u64>) -> Result<Self, ModelApiError> {
        let mut runtime = RuntimeModel::from_model_binary(data)?;
        let sid = sid_hint.unwrap_or(65_536);
        // Match upstream `Model.load(binary, sid)` behavior for logical models:
        // adopt the caller-provided local session ID for subsequent local ops.
        if sid_hint.is_some() && data.first().is_some_and(|b| (b & 0x80) == 0) {
            runtime = runtime.fork_with_sid(sid);
        }
        Ok(Self {
            runtime,
            sid,
            next_listener_id: 1,
            listeners: BTreeMap::new(),
            next_batch_listener_id: 1,
            batch_listeners: BTreeMap::new(),
        })
    }

    pub fn from_patches(patches: &[Patch]) -> Result<Self, ModelApiError> {
        let first = patches.first().ok_or(ModelApiError::NoPatches)?;
        let (sid, _) = first.id().ok_or(ModelApiError::MissingPatchId)?;
        let mut runtime = RuntimeModel::new_logical_empty(sid);
        for patch in patches {
            runtime.apply_patch(patch)?;
        }
        Ok(Self {
            runtime,
            sid,
            next_listener_id: 1,
            listeners: BTreeMap::new(),
            next_batch_listener_id: 1,
            batch_listeners: BTreeMap::new(),
        })
    }

    pub fn on_change<F>(&mut self, listener: F) -> u64
    where
        F: FnMut(ChangeEvent) + Send + Sync + 'static,
    {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.saturating_add(1);
        self.listeners.insert(id, Box::new(listener));
        id
    }

    pub fn off_change(&mut self, listener_id: u64) -> bool {
        self.listeners.remove(&listener_id).is_some()
    }

    pub fn on_changes<F>(&mut self, listener: F) -> u64
    where
        F: FnMut(BatchChangeEvent) + Send + Sync + 'static,
    {
        let id = self.next_batch_listener_id;
        self.next_batch_listener_id = self.next_batch_listener_id.saturating_add(1);
        self.batch_listeners.insert(id, Box::new(listener));
        id
    }

    pub fn off_changes(&mut self, listener_id: u64) -> bool {
        self.batch_listeners.remove(&listener_id).is_some()
    }

    pub fn on_change_at<F>(&mut self, path: Vec<PathStep>, mut listener: F) -> u64
    where
        F: FnMut(ScopedChangeEvent) + Send + Sync + 'static,
    {
        self.on_change(move |ev| {
            let before = value_at_path(&ev.before, &path).cloned();
            let after = value_at_path(&ev.after, &path).cloned();
            if before != after {
                listener(ScopedChangeEvent {
                    path: path.clone(),
                    before,
                    after,
                    patch_id: ev.patch_id,
                    origin: ev.origin,
                });
            }
        })
    }

    pub fn apply_patch(&mut self, patch: &Patch) -> Result<(), ModelApiError> {
        let before = self.runtime.view_json();
        self.runtime.apply_patch(patch)?;
        let after = self.runtime.view_json();
        let origin = match patch.id() {
            Some((sid, _)) if sid == self.sid => ChangeEventOrigin::Local,
            Some(_) => ChangeEventOrigin::Remote,
            None => ChangeEventOrigin::Local,
        };
        self.emit_change(ChangeEvent {
            origin,
            patch_id: patch.id(),
            before,
            after,
        });
        if let Some((sid, _)) = patch.id() {
            self.sid = self.sid.max(sid);
        }
        Ok(())
    }

    pub fn apply_batch(&mut self, patches: &[Patch]) -> Result<(), ModelApiError> {
        let before = self.runtime.view_json();
        let mut patch_ids: Vec<(u64, u64)> = Vec::with_capacity(patches.len());
        for patch in patches {
            if let Some(id) = patch.id() {
                patch_ids.push(id);
            }
            self.apply_patch(patch)?;
        }
        let after = self.runtime.view_json();
        if before != after {
            self.emit_batch_change(BatchChangeEvent {
                patch_ids,
                before,
                after,
            });
        }
        Ok(())
    }

    pub fn view(&self) -> Value {
        self.runtime.view_json()
    }

    pub fn to_model_binary(&self) -> Result<Vec<u8>, ModelApiError> {
        Ok(self.runtime.to_model_binary_like()?)
    }

    pub fn find(&self, path: &[PathStep]) -> Option<Value> {
        let mut node = self.runtime.view_json();
        for step in path {
            node = match (step, node) {
                (PathStep::Key(k), Value::Object(map)) => map.get(k)?.clone(),
                (PathStep::Index(i), Value::Array(arr)) => arr.get(*i)?.clone(),
                (PathStep::Append, _) => return None,
                _ => return None,
            };
        }
        Some(node)
    }

    pub fn read(&self, path: Option<&[PathStep]>) -> Option<Value> {
        match path {
            None => Some(self.runtime.view_json()),
            Some(p) => self.find(p),
        }
    }

    pub fn read_ptr(&self, ptr: Option<&str>) -> Option<Value> {
        match ptr {
            None => self.read(None),
            Some(p) => {
                let steps = parse_json_pointer(p).ok()?;
                self.read(Some(&steps))
            }
        }
    }

    pub fn select(&self, path: Option<&[PathStep]>) -> Option<Value> {
        self.read(path)
    }

    pub fn select_ptr(&self, ptr: Option<&str>) -> Option<Value> {
        self.read_ptr(ptr)
    }

    pub fn find_ptr(&self, ptr: &str) -> Option<Value> {
        let steps = parse_json_pointer(ptr).ok()?;
        self.find(&steps)
    }

    pub fn set(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        let mut next = self.runtime.view_json();
        if path.is_empty() {
            next = value;
            return self.apply_target_view(next);
        }
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        *target = value;
        self.apply_target_view(next)
    }

    pub fn obj_put(
        &mut self,
        path: &[PathStep],
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), ModelApiError> {
        let key = key.into();
        if let Ok(target) = self.resolve_path_node_id(path) {
            if let Some(obj_id) = self.runtime.resolve_object_node(target) {
                let start = self.next_local_time();
                let mut emitter = LocalEmitter::new(self.sid, start);
                let child = emitter.emit_value(&value);
                let ins_id = emitter.next_id();
                emitter.push(DecodedOp::InsObj {
                    id: ins_id,
                    obj: obj_id,
                    data: vec![(key.clone(), child)],
                });
                return self.apply_local_ops(emitter.into_ops());
            }
        }
        let mut next = self.runtime.view_json();
        let target = if path.is_empty() {
            &mut next
        } else {
            get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?
        };
        let map = target.as_object_mut().ok_or(ModelApiError::NotObject)?;
        map.insert(key, value);
        self.apply_target_view(next)
    }

    pub fn arr_push(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        if let Ok(target) = self.resolve_path_node_id(path) {
            if let Some(arr_id) = self.runtime.resolve_array_node(target) {
                let reference = self
                    .runtime
                    .array_visible_slots(arr_id)
                    .and_then(|slots| slots.last().copied())
                    .unwrap_or(arr_id);
                let start = self.next_local_time();
                let mut emitter = LocalEmitter::new(self.sid, start);
                let child = emitter.emit_value(&value);
                let ins_id = emitter.next_id();
                emitter.push(DecodedOp::InsArr {
                    id: ins_id,
                    obj: arr_id,
                    reference,
                    data: vec![child],
                });
                return self.apply_local_ops(emitter.into_ops());
            }
        }
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        let arr = target.as_array_mut().ok_or(ModelApiError::NotArray)?;
        arr.push(value);
        self.apply_target_view(next)
    }

    pub fn str_ins(&mut self, path: &[PathStep], pos: usize, text: &str) -> Result<(), ModelApiError> {
        if text.is_empty() {
            return Ok(());
        }
        if let Ok(target) = self.resolve_path_node_id(path) {
            if let Some(str_id) = self.runtime.resolve_string_node(target) {
                let slots = self.runtime.string_visible_slots(str_id).unwrap_or_default();
                let clamped = pos.min(slots.len());
                let reference = if clamped == 0 {
                    str_id
                } else {
                    slots[clamped - 1]
                };
                let start = self.next_local_time();
                let op = DecodedOp::InsStr {
                    id: Timestamp {
                        sid: self.sid,
                        time: start,
                    },
                    obj: str_id,
                    reference,
                    data: text.to_owned(),
                };
                return self.apply_local_ops(vec![op]);
            }
        }
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        let s = target.as_str().ok_or(ModelApiError::NotString)?;
        let mut chars: Vec<char> = s.chars().collect();
        let p = pos.min(chars.len());
        for (offset, ch) in text.chars().enumerate() {
            chars.insert(p + offset, ch);
        }
        *target = Value::String(chars.into_iter().collect());
        self.apply_target_view(next)
    }

    pub fn bin_ins(
        &mut self,
        path: &[PathStep],
        pos: usize,
        bytes: &[u8],
    ) -> Result<(), ModelApiError> {
        if bytes.is_empty() {
            return Ok(());
        }
        if let Ok(target) = self.resolve_path_node_id(path) {
            if let Some(bin_id) = self.runtime.resolve_bin_node(target) {
                let slots = self.runtime.bin_visible_slots(bin_id).unwrap_or_default();
                let clamped = pos.min(slots.len());
                let reference = if clamped == 0 {
                    bin_id
                } else {
                    slots[clamped - 1]
                };
                let start = self.next_local_time();
                let op = DecodedOp::InsBin {
                    id: Timestamp {
                        sid: self.sid,
                        time: start,
                    },
                    obj: bin_id,
                    reference,
                    data: bytes.to_vec(),
                };
                return self.apply_local_ops(vec![op]);
            }
        }
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        let arr = target.as_array_mut().ok_or(ModelApiError::NotArray)?;
        let mut i = pos.min(arr.len());
        for b in bytes {
            arr.insert(i, Value::from(*b));
            i += 1;
        }
        self.apply_target_view(next)
    }

    pub fn bin_del(
        &mut self,
        path: &[PathStep],
        pos: usize,
        length: usize,
    ) -> Result<(), ModelApiError> {
        if length == 0 {
            return Ok(());
        }
        if let Ok(target) = self.resolve_path_node_id(path) {
            if let Some(bin_id) = self.runtime.resolve_bin_node(target) {
                let spans = self.runtime.bin_find_interval(bin_id, pos, length);
                if spans.is_empty() {
                    return Ok(());
                }
                let start = self.next_local_time();
                let op = DecodedOp::Del {
                    id: Timestamp {
                        sid: self.sid,
                        time: start,
                    },
                    obj: bin_id,
                    what: spans,
                };
                return self.apply_local_ops(vec![op]);
            }
        }
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        let arr = target.as_array_mut().ok_or(ModelApiError::NotArray)?;
        if pos < arr.len() {
            let end = (pos + length).min(arr.len());
            arr.drain(pos..end);
        }
        self.apply_target_view(next)
    }

    pub fn vec_set(
        &mut self,
        path: &[PathStep],
        index: usize,
        value: Option<Value>,
    ) -> Result<(), ModelApiError> {
        if let Ok(target) = self.resolve_path_node_id(path) {
            if let Some(vec_id) = self.runtime.resolve_vec_node(target) {
                let start = self.next_local_time();
                let mut emitter = LocalEmitter::new(self.sid, start);
                let child = match value {
                    Some(v) => emitter.emit_value(&v),
                    None => {
                        let undef = emitter.next_id();
                        emitter.push(DecodedOp::NewCon {
                            id: undef,
                            value: ConValue::Undef,
                        });
                        undef
                    }
                };
                let ins_id = emitter.next_id();
                emitter.push(DecodedOp::InsVec {
                    id: ins_id,
                    obj: vec_id,
                    data: vec![(index as u64, child)],
                });
                return self.apply_local_ops(emitter.into_ops());
            }
        }

        let mut current = self.read(Some(path)).ok_or(ModelApiError::PathNotFound)?;
        let arr = current.as_array_mut().ok_or(ModelApiError::NotArray)?;
        if index >= arr.len() {
            arr.resize(index + 1, Value::Null);
        }
        match value {
            Some(v) => arr[index] = v,
            None => arr[index] = Value::Null,
        }
        self.replace(path, current)
    }

    pub fn add(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        if path.is_empty() {
            return Err(ModelApiError::InvalidPathOp);
        }
        let (parent, leaf) = split_parent(path)?;

        if let Ok(parent_id) = self.resolve_path_node_id(parent) {
            match leaf {
                PathStep::Key(key) => {
                    if let Some(obj_id) = self.runtime.resolve_object_node(parent_id) {
                        let start = self.next_local_time();
                        let mut emitter = LocalEmitter::new(self.sid, start);
                        let child = emitter.emit_value(&value);
                        let ins_id = emitter.next_id();
                        emitter.push(DecodedOp::InsObj {
                            id: ins_id,
                            obj: obj_id,
                            data: vec![(key.clone(), child)],
                        });
                        return self.apply_local_ops(emitter.into_ops());
                    }
                }
                PathStep::Index(idx) => {
                    if let Some(arr_id) = self.runtime.resolve_array_node(parent_id) {
                        let slots = self.runtime.array_visible_slots(arr_id).unwrap_or_default();
                        let clamped = (*idx).min(slots.len());
                        let reference = if clamped == 0 {
                            arr_id
                        } else {
                            slots[clamped - 1]
                        };
                        let start = self.next_local_time();
                        let mut emitter = LocalEmitter::new(self.sid, start);
                        let mut ids = Vec::new();
                        match &value {
                            Value::Array(items) => {
                                ids.reserve(items.len());
                                for item in items {
                                    ids.push(emitter.emit_array_item(item));
                                }
                            }
                            other => ids.push(emitter.emit_array_item(other)),
                        }
                        if ids.is_empty() {
                            return Ok(());
                        }
                        let ins_id = emitter.next_id();
                        emitter.push(DecodedOp::InsArr {
                            id: ins_id,
                            obj: arr_id,
                            reference,
                            data: ids,
                        });
                        return self.apply_local_ops(emitter.into_ops());
                    }
                    if let Some(vec_id) = self.runtime.resolve_vec_node(parent_id) {
                        let index = *idx as u64;
                        let start = self.next_local_time();
                        let mut emitter = LocalEmitter::new(self.sid, start);
                        let child = emitter.emit_value(&value);
                        let ins_id = emitter.next_id();
                        emitter.push(DecodedOp::InsVec {
                            id: ins_id,
                            obj: vec_id,
                            data: vec![(index, child)],
                        });
                        return self.apply_local_ops(emitter.into_ops());
                    }
                    if let Some(str_id) = self.runtime.resolve_string_node(parent_id) {
                        let text = match value {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        if text.is_empty() {
                            return Ok(());
                        }
                        let slots = self.runtime.string_visible_slots(str_id).unwrap_or_default();
                        let clamped = (*idx).min(slots.len());
                        let reference = if clamped == 0 {
                            str_id
                        } else {
                            slots[clamped - 1]
                        };
                        let start = self.next_local_time();
                        return self.apply_local_ops(vec![DecodedOp::InsStr {
                            id: Timestamp {
                                sid: self.sid,
                                time: start,
                            },
                            obj: str_id,
                            reference,
                            data: text,
                        }]);
                    }
                    if let Some(bin_id) = self.runtime.resolve_bin_node(parent_id) {
                        let bytes = parse_bin_add_bytes(&value).ok_or(ModelApiError::InvalidPathOp)?;
                        if bytes.is_empty() {
                            return Ok(());
                        }
                        let slots = self.runtime.bin_visible_slots(bin_id).unwrap_or_default();
                        let clamped = (*idx).min(slots.len());
                        let reference = if clamped == 0 {
                            bin_id
                        } else {
                            slots[clamped - 1]
                        };
                        let start = self.next_local_time();
                        return self.apply_local_ops(vec![DecodedOp::InsBin {
                            id: Timestamp {
                                sid: self.sid,
                                time: start,
                            },
                            obj: bin_id,
                            reference,
                            data: bytes,
                        }]);
                    }
                }
                PathStep::Append => {
                    if let Some(arr_id) = self.runtime.resolve_array_node(parent_id) {
                        let reference = self
                            .runtime
                            .array_visible_slots(arr_id)
                            .and_then(|slots| slots.last().copied())
                            .unwrap_or(arr_id);
                        let start = self.next_local_time();
                        let mut emitter = LocalEmitter::new(self.sid, start);
                        let mut ids = Vec::new();
                        match &value {
                            Value::Array(items) => {
                                ids.reserve(items.len());
                                for item in items {
                                    ids.push(emitter.emit_array_item(item));
                                }
                            }
                            other => ids.push(emitter.emit_array_item(other)),
                        }
                        if ids.is_empty() {
                            return Ok(());
                        }
                        let ins_id = emitter.next_id();
                        emitter.push(DecodedOp::InsArr {
                            id: ins_id,
                            obj: arr_id,
                            reference,
                            data: ids,
                        });
                        return self.apply_local_ops(emitter.into_ops());
                    }
                }
            }
        }

        let mut next = self.runtime.view_json();
        let target = resolve_parent_target_mut(&mut next, parent)?;
        apply_add_to_json_target(target, leaf, value)?;
        self.apply_target_view(next)
    }

    pub fn replace(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        if path.is_empty() {
            return self.apply_target_view(value);
        }
        let (parent, leaf) = split_parent(path)?;
        if let Ok(parent_id) = self.resolve_path_node_id(parent) {
            match leaf {
                PathStep::Key(key) => {
                    if let Some(obj_id) = self.runtime.resolve_object_node(parent_id) {
                        if self.runtime.object_field(obj_id, key).is_none() {
                            return Err(ModelApiError::PathNotFound);
                        }
                        let start = self.next_local_time();
                        let mut emitter = LocalEmitter::new(self.sid, start);
                        let child = emitter.emit_value(&value);
                        let ins_id = emitter.next_id();
                        emitter.push(DecodedOp::InsObj {
                            id: ins_id,
                            obj: obj_id,
                            data: vec![(key.clone(), child)],
                        });
                        return self.apply_local_ops(emitter.into_ops());
                    }
                }
                PathStep::Index(idx) => {
                    if let Some(arr_id) = self.runtime.resolve_array_node(parent_id) {
                        let values = self.runtime.array_visible_values(arr_id).unwrap_or_default();
                        if *idx > values.len() {
                            return Err(ModelApiError::PathNotFound);
                        }
                        if *idx == values.len() {
                            let reference = self
                                .runtime
                                .array_visible_slots(arr_id)
                                .and_then(|slots| slots.last().copied())
                                .unwrap_or(arr_id);
                            let start = self.next_local_time();
                            let mut emitter = LocalEmitter::new(self.sid, start);
                            let child = emitter.emit_array_item(&value);
                            let ins_id = emitter.next_id();
                            emitter.push(DecodedOp::InsArr {
                                id: ins_id,
                                obj: arr_id,
                                reference,
                                data: vec![child],
                            });
                            return self.apply_local_ops(emitter.into_ops());
                        }
                        let reference = self
                            .runtime
                            .array_find(arr_id, *idx)
                            .ok_or(ModelApiError::PathNotFound)?;
                        let start = self.next_local_time();
                        let mut emitter = LocalEmitter::new(self.sid, start);
                        let child = emitter.emit_array_item(&value);
                        let upd_id = emitter.next_id();
                        emitter.push(DecodedOp::UpdArr {
                            id: upd_id,
                            obj: arr_id,
                            reference,
                            val: child,
                        });
                        return self.apply_local_ops(emitter.into_ops());
                    }
                    if let Some(vec_id) = self.runtime.resolve_vec_node(parent_id) {
                        let index = *idx as u64;
                        let start = self.next_local_time();
                        let mut emitter = LocalEmitter::new(self.sid, start);
                        let child = emitter.emit_value(&value);
                        let ins_id = emitter.next_id();
                        emitter.push(DecodedOp::InsVec {
                            id: ins_id,
                            obj: vec_id,
                            data: vec![(index, child)],
                        });
                        return self.apply_local_ops(emitter.into_ops());
                    }
                }
                PathStep::Append => return Err(ModelApiError::InvalidPathOp),
            }
        }
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        *target = value;
        self.apply_target_view(next)
    }

    pub fn remove(&mut self, path: &[PathStep]) -> Result<(), ModelApiError> {
        self.remove_with_length(path, 1)
    }

    pub fn remove_with_length(
        &mut self,
        path: &[PathStep],
        length: usize,
    ) -> Result<(), ModelApiError> {
        if path.is_empty() {
            return Err(ModelApiError::InvalidPathOp);
        }
        let mut next = self.runtime.view_json();
        let (parent, leaf) = split_parent(path)?;
        let target = resolve_parent_target_mut(&mut next, parent)?;
        if let PathStep::Index(idx) = leaf {
            if let Ok(parent_id) = self.resolve_path_node_id(parent) {
                if let Some(arr_id) = self.runtime.resolve_array_node(parent_id) {
                    let spans = self.runtime.array_find_interval(arr_id, *idx, length.max(1));
                    if !spans.is_empty() {
                        let start = self.next_local_time();
                        return self.apply_local_ops(vec![DecodedOp::Del {
                            id: Timestamp {
                                sid: self.sid,
                                time: start,
                            },
                            obj: arr_id,
                            what: spans,
                        }]);
                    }
                } else if let Some(str_id) = self.runtime.resolve_string_node(parent_id) {
                    let spans = self.runtime.string_find_interval(str_id, *idx, length.max(1));
                    if !spans.is_empty() {
                        let start = self.next_local_time();
                        return self.apply_local_ops(vec![DecodedOp::Del {
                            id: Timestamp {
                                sid: self.sid,
                                time: start,
                            },
                            obj: str_id,
                            what: spans,
                        }]);
                    }
                } else if let Some(bin_id) = self.runtime.resolve_bin_node(parent_id) {
                    let spans = self.runtime.bin_find_interval(bin_id, *idx, length.max(1));
                    if !spans.is_empty() {
                        let start = self.next_local_time();
                        return self.apply_local_ops(vec![DecodedOp::Del {
                            id: Timestamp {
                                sid: self.sid,
                                time: start,
                            },
                            obj: bin_id,
                            what: spans,
                        }]);
                    }
                } else if let Some(vec_id) = self.runtime.resolve_vec_node(parent_id) {
                    let index = *idx as u64;
                    let start = self.next_local_time();
                    let mut emitter = LocalEmitter::new(self.sid, start);
                    let undef = emitter.next_id();
                    emitter.push(DecodedOp::NewCon {
                        id: undef,
                        value: ConValue::Undef,
                    });
                    let ins_id = emitter.next_id();
                    emitter.push(DecodedOp::InsVec {
                        id: ins_id,
                        obj: vec_id,
                        data: vec![(index, undef)],
                    });
                    return self.apply_local_ops(emitter.into_ops());
                }
            }
        }
        if let PathStep::Key(key) = leaf {
            if let Ok(parent_id) = self.resolve_path_node_id(parent) {
                if let Some(obj_id) = self.runtime.resolve_object_node(parent_id) {
                    if self.runtime.object_field(obj_id, key).is_none() {
                        return Ok(());
                    }
                    let start = self.next_local_time();
                    let mut emitter = LocalEmitter::new(self.sid, start);
                    let undef = emitter.next_id();
                    emitter.push(DecodedOp::NewCon {
                        id: undef,
                        value: ConValue::Undef,
                    });
                    let ins_id = emitter.next_id();
                    emitter.push(DecodedOp::InsObj {
                        id: ins_id,
                        obj: obj_id,
                        data: vec![(key.clone(), undef)],
                    });
                    return self.apply_local_ops(emitter.into_ops());
                }
            }
        }
        apply_remove_to_json_target(target, leaf, length)?;
        self.apply_target_view(next)
    }

    // Upstream-compatible tolerant operation helpers: return false on invalid paths/types.
    pub fn try_add(&mut self, path: &[PathStep], value: Value) -> bool {
        self.add(path, value).is_ok()
    }

    pub fn try_add_ptr(&mut self, ptr: &str, value: Value) -> bool {
        let Ok(steps) = parse_json_pointer(ptr) else {
            return false;
        };
        self.try_add(&steps, value)
    }

    pub fn try_replace(&mut self, path: &[PathStep], value: Value) -> bool {
        self.replace(path, value).is_ok()
    }

    pub fn try_replace_ptr(&mut self, ptr: &str, value: Value) -> bool {
        let Ok(steps) = parse_json_pointer(ptr) else {
            return false;
        };
        self.try_replace(&steps, value)
    }

    pub fn try_remove(&mut self, path: &[PathStep]) -> bool {
        self.remove(path).is_ok()
    }

    pub fn try_remove_with_length(&mut self, path: &[PathStep], length: usize) -> bool {
        self.remove_with_length(path, length).is_ok()
    }

    pub fn try_remove_ptr(&mut self, ptr: &str) -> bool {
        let Ok(steps) = parse_json_pointer(ptr) else {
            return false;
        };
        self.try_remove(&steps)
    }

    pub fn merge_ptr(&mut self, ptr: Option<&str>, value: Value) -> bool {
        match ptr {
            None => self.merge(None, value),
            Some(p) => match parse_json_pointer(p) {
                Ok(steps) => self.merge(Some(&steps), value),
                Err(_) => false,
            },
        }
    }

    pub fn op(&mut self, operation: ApiOperation) -> bool {
        match operation {
            ApiOperation::Add { path, value } => self.try_add(&path, value),
            ApiOperation::Replace { path, value } => self.try_replace(&path, value),
            ApiOperation::Remove { path, length } => self.try_remove_with_length(&path, length),
            ApiOperation::Merge { path, value } => self.merge(Some(&path), value),
        }
    }

    pub fn op_tuple(
        &mut self,
        kind: ApiOperationKind,
        path: &[PathStep],
        value: Option<Value>,
        length: Option<usize>,
    ) -> bool {
        match kind {
            ApiOperationKind::Add => value.map(|v| self.try_add(path, v)).unwrap_or(false),
            ApiOperationKind::Replace => value.map(|v| self.try_replace(path, v)).unwrap_or(false),
            ApiOperationKind::Remove => self.try_remove_with_length(path, length.unwrap_or(1)),
            ApiOperationKind::Merge => value.map(|v| self.merge(Some(path), v)).unwrap_or(false),
        }
    }

    pub fn op_ptr_tuple(
        &mut self,
        kind: ApiOperationKind,
        ptr: &str,
        value: Option<Value>,
        length: Option<usize>,
    ) -> bool {
        let Ok(path) = parse_json_pointer(ptr) else {
            return false;
        };
        self.op_tuple(kind, &path, value, length)
    }

    pub fn diff(&self, next: &Value) -> Result<Option<Patch>, ModelApiError> {
        let base = self.runtime.to_model_binary_like()?;
        let patch = diff_model_to_patch_bytes(&base, next, self.sid)?;
        match patch {
            Some(bytes) => {
                let decoded = Patch::from_binary(&bytes)
                    .map_err(|e| ModelApiError::PatchDecode(e.to_string()))?;
                Ok(Some(decoded))
            }
            None => Ok(None),
        }
    }

    pub fn merge(&mut self, path: Option<&[PathStep]>, value: Value) -> bool {
        let mut next = self.runtime.view_json();
        match path {
            None => next = value,
            Some([]) => next = value,
            Some(p) => {
                let Some(target) = get_path_mut(&mut next, p) else {
                    return false;
                };
                *target = value;
            }
        }
        self.apply_target_view(next).is_ok()
    }

    pub fn node(&mut self) -> NodeHandle<'_> {
        NodeHandle {
            api: self,
            path: Vec::new(),
        }
    }

    pub fn node_ptr(&mut self, ptr: &str) -> Result<NodeHandle<'_>, ModelApiError> {
        let mut handle = self.node();
        for step in parse_json_pointer(ptr)? {
            handle.path.push(step);
        }
        Ok(handle)
    }

    pub fn s(&mut self) -> NodeHandle<'_> {
        self.node()
    }

    pub fn s_ptr(&mut self, ptr: &str) -> Result<NodeHandle<'_>, ModelApiError> {
        self.node_ptr(ptr)
    }

    fn apply_target_view(&mut self, next: Value) -> Result<(), ModelApiError> {
        let base = self.runtime.to_model_binary_like()?;
        let patch = diff_model_to_patch_bytes(&base, &next, self.sid)?;
        if let Some(bytes) = patch {
            let decoded =
                Patch::from_binary(&bytes).map_err(|e| ModelApiError::PatchDecode(e.to_string()))?;
            self.apply_patch(&decoded)?;
        }
        Ok(())
    }

    fn apply_local_ops(&mut self, ops: Vec<DecodedOp>) -> Result<(), ModelApiError> {
        if ops.is_empty() {
            return Ok(());
        }
        let first = ops[0].id();
        let bytes = encode_patch_from_ops(first.sid, first.time, &ops)
            .map_err(|e| ModelApiError::PatchDecode(format!("patch encode failed: {e}")))?;
        let patch = Patch::from_binary(&bytes)
            .map_err(|e| ModelApiError::PatchDecode(format!("patch decode failed: {e}")))?;
        self.apply_patch(&patch)
    }

    fn next_local_time(&self) -> u64 {
        let observed_next = self
            .runtime
            .clock
            .observed
            .get(&self.sid)
            .and_then(|ranges| ranges.last().map(|(_, end)| end.saturating_add(1)))
            .unwrap_or(1);
        let table_next = self
            .runtime
            .clock_table
            .iter()
            .find(|c| c.sid == self.sid)
            .map(|c| c.time.saturating_add(1))
            .unwrap_or(1);
        observed_next.max(table_next)
    }

    fn resolve_path_node_id(&self, path: &[PathStep]) -> Result<Timestamp, ModelApiError> {
        let mut current = self.runtime.root_id().ok_or(ModelApiError::PathNotFound)?;
        for step in path {
            current = match step {
                PathStep::Key(key) => self
                    .runtime
                    .object_field(current, key)
                    .ok_or(ModelApiError::PathNotFound)?,
                PathStep::Index(idx) => {
                    if let Some(arr_id) = self.runtime.resolve_array_node(current) {
                        self.runtime
                            .array_visible_values(arr_id)
                            .and_then(|v| v.get(*idx).copied())
                            .ok_or(ModelApiError::PathNotFound)?
                    } else if let Some(vec_id) = self.runtime.resolve_vec_node(current) {
                        self.runtime
                            .vec_index_value(vec_id, *idx as u64)
                            .ok_or(ModelApiError::PathNotFound)?
                    } else {
                        return Err(ModelApiError::InvalidPathOp);
                    }
                }
                PathStep::Append => return Err(ModelApiError::InvalidPathOp),
            };
        }
        Ok(current)
    }

    fn emit_change(&mut self, event: ChangeEvent) {
        for listener in self.listeners.values_mut() {
            listener(event.clone());
        }
    }

    fn emit_batch_change(&mut self, event: BatchChangeEvent) {
        for listener in self.batch_listeners.values_mut() {
            listener(event.clone());
        }
    }
}

struct LocalEmitter {
    sid: u64,
    cursor: u64,
    ops: Vec<DecodedOp>,
}

impl LocalEmitter {
    fn new(sid: u64, start_time: u64) -> Self {
        Self {
            sid,
            cursor: start_time,
            ops: Vec::new(),
        }
    }

    fn next_id(&self) -> Timestamp {
        Timestamp {
            sid: self.sid,
            time: self.cursor,
        }
    }

    fn push(&mut self, op: DecodedOp) {
        self.cursor = self.cursor.saturating_add(op.span());
        self.ops.push(op);
    }

    fn emit_value(&mut self, value: &Value) -> Timestamp {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                let id = self.next_id();
                self.push(DecodedOp::NewCon {
                    id,
                    value: ConValue::Json(value.clone()),
                });
                id
            }
            Value::String(s) => {
                let str_id = self.next_id();
                self.push(DecodedOp::NewStr { id: str_id });
                if !s.is_empty() {
                    let ins_id = self.next_id();
                    self.push(DecodedOp::InsStr {
                        id: ins_id,
                        obj: str_id,
                        reference: str_id,
                        data: s.clone(),
                    });
                }
                str_id
            }
            Value::Array(items) => {
                let arr_id = self.next_id();
                self.push(DecodedOp::NewArr { id: arr_id });
                if !items.is_empty() {
                    let mut children = Vec::with_capacity(items.len());
                    for item in items {
                        if matches!(item, Value::Null | Value::Bool(_) | Value::Number(_)) {
                            let val_id = self.next_id();
                            self.push(DecodedOp::NewVal { id: val_id });
                            let con_id = self.emit_value(item);
                            let ins_id = self.next_id();
                            self.push(DecodedOp::InsVal {
                                id: ins_id,
                                obj: val_id,
                                val: con_id,
                            });
                            children.push(val_id);
                        } else {
                            children.push(self.emit_value(item));
                        }
                    }
                    let ins_id = self.next_id();
                    self.push(DecodedOp::InsArr {
                        id: ins_id,
                        obj: arr_id,
                        reference: arr_id,
                        data: children,
                    });
                }
                arr_id
            }
            Value::Object(map) => {
                let obj_id = self.next_id();
                self.push(DecodedOp::NewObj { id: obj_id });
                if !map.is_empty() {
                    let mut pairs = Vec::with_capacity(map.len());
                    for (k, v) in map {
                        let child = self.emit_value(v);
                        pairs.push((k.clone(), child));
                    }
                    let ins_id = self.next_id();
                    self.push(DecodedOp::InsObj {
                        id: ins_id,
                        obj: obj_id,
                        data: pairs,
                    });
                }
                obj_id
            }
        }
    }

    fn emit_array_item(&mut self, value: &Value) -> Timestamp {
        if matches!(value, Value::Null | Value::Bool(_) | Value::Number(_)) {
            let val_id = self.next_id();
            self.push(DecodedOp::NewVal { id: val_id });
            let con_id = self.emit_value(value);
            let ins_id = self.next_id();
            self.push(DecodedOp::InsVal {
                id: ins_id,
                obj: val_id,
                val: con_id,
            });
            val_id
        } else {
            self.emit_value(value)
        }
    }

    fn into_ops(self) -> Vec<DecodedOp> {
        self.ops
    }
}

fn parse_bin_add_bytes(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::Number(n) => n.as_u64().and_then(|v| u8::try_from(v).ok()).map(|b| vec![b]),
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                let b = v.as_u64().and_then(|n| u8::try_from(n).ok())?;
                out.push(b);
            }
            Some(out)
        }
        _ => None,
    }
}

fn resolve_parent_target_mut<'a>(
    next: &'a mut Value,
    parent: &[PathStep],
) -> Result<&'a mut Value, ModelApiError> {
    if parent.is_empty() {
        Ok(next)
    } else {
        get_path_mut(next, parent).ok_or(ModelApiError::PathNotFound)
    }
}

fn apply_add_to_json_target(
    target: &mut Value,
    leaf: &PathStep,
    value: Value,
) -> Result<(), ModelApiError> {
    match (target, leaf) {
        (Value::Object(map), PathStep::Key(key)) => {
            map.insert(key.clone(), value);
            Ok(())
        }
        (Value::Array(arr), PathStep::Index(idx)) => {
            let i = (*idx).min(arr.len());
            match value {
                Value::Array(items) => {
                    arr.splice(i..i, items);
                }
                other => arr.insert(i, other),
            }
            Ok(())
        }
        (Value::Array(arr), PathStep::Append) => {
            match value {
                Value::Array(items) => arr.extend(items),
                other => arr.push(other),
            }
            Ok(())
        }
        _ => Err(ModelApiError::InvalidPathOp),
    }
}

fn apply_remove_to_json_target(
    target: &mut Value,
    leaf: &PathStep,
    length: usize,
) -> Result<(), ModelApiError> {
    match (target, leaf) {
        (Value::Object(map), PathStep::Key(key)) => {
            map.remove(key);
            Ok(())
        }
        (Value::Array(arr), PathStep::Index(idx)) => {
            if *idx < arr.len() {
                let end = (*idx + length.max(1)).min(arr.len());
                arr.drain(*idx..end);
            }
            Ok(())
        }
        (Value::Array(arr), PathStep::Append) => {
            let _ = arr.pop();
            Ok(())
        }
        (Value::String(s), PathStep::Index(idx)) => {
            let mut chars: Vec<char> = s.chars().collect();
            if *idx < chars.len() {
                let end = (*idx + length.max(1)).min(chars.len());
                chars.drain(*idx..end);
                *s = chars.into_iter().collect();
            }
            Ok(())
        }
        _ => Err(ModelApiError::InvalidPathOp),
    }
}
