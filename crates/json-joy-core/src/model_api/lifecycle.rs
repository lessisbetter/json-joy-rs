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
}
