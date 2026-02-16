#[derive(Debug, Clone, PartialEq)]
pub enum ApiOperation {
    Add { path: Vec<PathStep>, value: Value },
    Replace { path: Vec<PathStep>, value: Value },
    Remove { path: Vec<PathStep>, length: usize },
    Merge { path: Vec<PathStep>, value: Value },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiOperationKind {
    Add,
    Replace,
    Remove,
    Merge,
}

#[derive(Debug, Error)]
pub enum ModelApiError {
    #[error("no patches provided")]
    NoPatches,
    #[error("first patch missing id")]
    MissingPatchId,
    #[error("path not found")]
    PathNotFound,
    #[error("path does not point to object")]
    NotObject,
    #[error("path does not point to array")]
    NotArray,
    #[error("path does not point to string")]
    NotString,
    #[error("invalid path operation")]
    InvalidPathOp,
    #[error("model encode/decode failed: {0}")]
    Model(#[from] ModelError),
    #[error("patch apply failed: {0}")]
    Apply(#[from] ApplyError),
    #[error("diff failed: {0}")]
    Diff(#[from] DiffError),
    #[error("patch decode failed: {0}")]
    PatchDecode(String),
}

pub struct NativeModelApi {
    runtime: RuntimeModel,
    sid: u64,
    next_listener_id: u64,
    listeners: BTreeMap<u64, Box<dyn FnMut(ChangeEvent) + Send + Sync>>,
    next_batch_listener_id: u64,
    batch_listeners: BTreeMap<u64, Box<dyn FnMut(BatchChangeEvent) + Send + Sync>>,
}

pub struct NodeHandle<'a> {
    api: &'a mut NativeModelApi,
    path: Vec<PathStep>,
}

pub struct ObjHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct ArrHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct StrHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct ValHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct BinHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct VecHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct ConHandle<'a> {
    inner: NodeHandle<'a>,
}

