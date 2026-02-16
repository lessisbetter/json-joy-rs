impl<'a> NodeHandle<'a> {
    pub fn at_ptr(mut self, ptr: &str) -> Result<Self, ModelApiError> {
        for step in parse_json_pointer(ptr)? {
            self.path.push(step);
        }
        Ok(self)
    }

    pub fn at_key(mut self, key: impl Into<String>) -> Self {
        self.path.push(PathStep::Key(key.into()));
        self
    }

    pub fn at_index(mut self, index: usize) -> Self {
        self.path.push(PathStep::Index(index));
        self
    }

    pub fn at_append(mut self) -> Self {
        self.path.push(PathStep::Append);
        self
    }

    pub fn path(&self) -> &[PathStep] {
        &self.path
    }

    pub fn read(&self) -> Option<Value> {
        self.api.read(Some(&self.path))
    }

    pub fn set(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.set(&self.path, value)
    }

    pub fn add(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.add(&self.path, value)
    }

    pub fn replace(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.replace(&self.path, value)
    }

    pub fn remove(&mut self) -> Result<(), ModelApiError> {
        self.api.remove(&self.path)
    }

    pub fn obj_put(
        &mut self,
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), ModelApiError> {
        self.api.obj_put(&self.path, key, value)
    }

    pub fn arr_push(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.arr_push(&self.path, value)
    }

    pub fn str_ins(&mut self, pos: usize, text: &str) -> Result<(), ModelApiError> {
        self.api.str_ins(&self.path, pos, text)
    }

    pub fn as_obj(self) -> Result<ObjHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Object(_)) => Ok(ObjHandle { inner: self }),
            _ => Err(ModelApiError::NotObject),
        }
    }

    pub fn as_arr(self) -> Result<ArrHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Array(_)) => Ok(ArrHandle { inner: self }),
            _ => Err(ModelApiError::NotArray),
        }
    }

    pub fn as_str(self) -> Result<StrHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::String(_)) => Ok(StrHandle { inner: self }),
            _ => Err(ModelApiError::NotString),
        }
    }

    pub fn as_val(self) -> Result<ValHandle<'a>, ModelApiError> {
        Ok(ValHandle { inner: self })
    }

    pub fn as_bin(self) -> Result<BinHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Array(arr))
                if arr
                    .iter()
                    .all(|v| v.as_u64().is_some_and(|n| n <= 255)) =>
            {
                Ok(BinHandle { inner: self })
            }
            Some(Value::Object(map))
                if map
                    .iter()
                    .all(|(k, v)| k.parse::<usize>().is_ok() && v.as_u64().is_some_and(|n| n <= 255)) =>
            {
                Ok(BinHandle { inner: self })
            }
            _ => Err(ModelApiError::NotArray),
        }
    }

    pub fn as_vec(self) -> Result<VecHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Array(_)) => Ok(VecHandle { inner: self }),
            _ => Err(ModelApiError::NotArray),
        }
    }

    pub fn as_con(self) -> Result<ConHandle<'a>, ModelApiError> {
        Ok(ConHandle { inner: self })
    }
}

impl<'a> ObjHandle<'a> {
    pub fn has(&self, key: &str) -> bool {
        self.inner
            .read()
            .and_then(|v| v.as_object().map(|m| m.contains_key(key)))
            .unwrap_or(false)
    }

    pub fn set(&mut self, key: impl Into<String>, value: Value) -> Result<(), ModelApiError> {
        self.inner.obj_put(key, value)
    }

    pub fn del(&mut self, key: &str) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Key(key.to_owned()));
        self.inner.api.remove(&path)
    }
}

impl<'a> ArrHandle<'a> {
    pub fn length(&self) -> usize {
        self.inner
            .read()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    }

    pub fn ins(&mut self, index: usize, value: Value) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Index(index));
        self.inner.api.add(&path, value)
    }

    pub fn upd(&mut self, index: usize, value: Value) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Index(index));
        self.inner.api.replace(&path, value)
    }

    pub fn del(&mut self, index: usize) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Index(index));
        self.inner.api.remove(&path)
    }
}

impl<'a> StrHandle<'a> {
    pub fn length(&self) -> usize {
        self.inner
            .read()
            .and_then(|v| v.as_str().map(|s| s.chars().count()))
            .unwrap_or(0)
    }

    pub fn ins(&mut self, index: usize, text: &str) -> Result<(), ModelApiError> {
        self.inner.str_ins(index, text)
    }

    pub fn del(&mut self, index: usize, length: usize) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Index(index));
        self.inner.api.remove_with_length(&path, length)
    }
}

impl<'a> ValHandle<'a> {
    pub fn view(&self) -> Option<Value> {
        self.inner.read()
    }

    pub fn set(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.inner.replace(value)
    }
}

impl<'a> BinHandle<'a> {
    pub fn length(&self) -> usize {
        self.inner
            .read()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    }

    pub fn ins(&mut self, index: usize, bytes: &[u8]) -> Result<(), ModelApiError> {
        self.inner.api.bin_ins(&self.inner.path, index, bytes)
    }

    pub fn del(&mut self, index: usize, length: usize) -> Result<(), ModelApiError> {
        self.inner.api.bin_del(&self.inner.path, index, length)
    }
}

impl<'a> VecHandle<'a> {
    pub fn set(&mut self, index: usize, value: Option<Value>) -> Result<(), ModelApiError> {
        let mut current = self.inner.read().ok_or(ModelApiError::PathNotFound)?;
        let arr = current.as_array_mut().ok_or(ModelApiError::NotArray)?;
        if index >= arr.len() {
            arr.resize(index + 1, Value::Null);
        }
        match value {
            Some(v) => arr[index] = v,
            None => arr[index] = Value::Null,
        }
        self.inner.api.replace(&self.inner.path, current)
    }
}

impl<'a> ConHandle<'a> {
    pub fn view(&self) -> Option<Value> {
        self.inner.read()
    }

    pub fn set(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.inner.replace(value)
    }
}
