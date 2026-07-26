use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use fs2::FileExt;
use tempfile::NamedTempFile;
use thiserror::Error;
use time::OffsetDateTime;

use crate::domain::ChangeRequestKey;

use super::{SESSION_SCHEMA_VERSION, SessionSnapshot, SessionSummary, StatePaths};

#[derive(Debug, Error)]
pub enum StateError {
    #[error("state directory could not be determined")]
    StateDirectoryUnavailable,
    #[error("session schema {found} is not supported; expected {expected}")]
    SchemaMismatch { expected: u32, found: u32 },
    #[error("corrupt session {original} was moved to {quarantined}: {message}")]
    CorruptSession {
        original: PathBuf,
        quarantined: PathBuf,
        message: String,
    },
    #[error("session is already open for writing: {path}")]
    Locked { path: PathBuf },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("could not atomically persist session: {0}")]
    Persist(#[from] tempfile::PersistError),
}

pub trait SessionStore: Send + Sync {
    fn list(&self) -> Result<Vec<SessionSummary>, StateError>;
    fn load(&self, key: &ChangeRequestKey) -> Result<Option<SessionSnapshot>, StateError>;
    fn open(&self, key: &ChangeRequestKey) -> Result<SessionAccess, StateError>;
    fn open_writable(&self, key: &ChangeRequestKey) -> Result<SessionHandle, StateError>;
    fn delete(&self, key: &ChangeRequestKey) -> Result<(), StateError>;
}

#[allow(clippy::large_enum_variant)]
pub enum SessionAccess {
    ReadWrite(SessionHandle),
    ReadOnly {
        snapshot: Option<SessionSnapshot>,
        path: PathBuf,
    },
}

pub struct SessionHandle {
    path: PathBuf,
    lock: File,
}

impl SessionHandle {
    pub fn save(&mut self, snapshot: &SessionSnapshot) -> Result<(), StateError> {
        validate_schema(snapshot)?;
        write_atomic(&self.path, snapshot)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock);
    }
}

#[derive(Debug, Clone)]
pub struct JsonSessionStore {
    paths: StatePaths,
}

impl JsonSessionStore {
    pub fn new(paths: StatePaths) -> Result<Self, StateError> {
        fs::create_dir_all(paths.root())?;
        set_private_directory(paths.root())?;
        Ok(Self { paths })
    }

    pub fn discover() -> Result<Self, StateError> {
        Self::new(StatePaths::discover()?)
    }

    pub fn paths(&self) -> &StatePaths {
        &self.paths
    }

    fn read_path(&self, path: &Path) -> Result<Option<SessionSnapshot>, StateError> {
        let json = match fs::read(path) {
            Ok(json) => json,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        match serde_json::from_slice::<SessionSnapshot>(&json) {
            Ok(snapshot) => {
                validate_schema(&snapshot)?;
                Ok(Some(snapshot))
            }
            Err(error) => Err(quarantine(path, error)?),
        }
    }
}

impl SessionStore for JsonSessionStore {
    fn list(&self) -> Result<Vec<SessionSummary>, StateError> {
        let mut summaries = Vec::new();
        for entry in fs::read_dir(self.paths.root())? {
            let path = entry?.path();
            if !is_session_file(&path) {
                continue;
            }
            if let Some(snapshot) = self.read_path(&path)? {
                summaries.push(SessionSummary {
                    key: snapshot.key,
                    head: snapshot.head,
                    updated_at: snapshot.updated_at,
                    path,
                });
            }
        }
        summaries.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(summaries)
    }

    fn load(&self, key: &ChangeRequestKey) -> Result<Option<SessionSnapshot>, StateError> {
        self.read_path(&self.paths.session_path(key)?)
    }

    fn open(&self, key: &ChangeRequestKey) -> Result<SessionAccess, StateError> {
        match self.open_writable(key) {
            Ok(handle) => Ok(SessionAccess::ReadWrite(handle)),
            Err(StateError::Locked { path }) => Ok(SessionAccess::ReadOnly {
                snapshot: self.load(key)?,
                path,
            }),
            Err(error) => Err(error),
        }
    }

    fn open_writable(&self, key: &ChangeRequestKey) -> Result<SessionHandle, StateError> {
        let path = self.paths.session_path(key)?;
        let lock_path = self.paths.lock_path(key)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        set_private_file(&lock)?;
        match lock.try_lock_exclusive() {
            Ok(()) => Ok(SessionHandle { path, lock }),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Err(StateError::Locked { path })
            }
            Err(error) => Err(error.into()),
        }
    }

    fn delete(&self, key: &ChangeRequestKey) -> Result<(), StateError> {
        let handle = self.open_writable(key)?;
        match fs::remove_file(handle.path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn is_session_file(path: &Path) -> bool {
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return false;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.len() == 64 && stem.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn validate_schema(snapshot: &SessionSnapshot) -> Result<(), StateError> {
    if snapshot.schema_version != SESSION_SCHEMA_VERSION {
        return Err(StateError::SchemaMismatch {
            expected: SESSION_SCHEMA_VERSION,
            found: snapshot.schema_version,
        });
    }
    Ok(())
}

fn write_atomic(path: &Path, snapshot: &SessionSnapshot) -> Result<(), StateError> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "session path has no parent"))?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    set_private_file(temporary.as_file())?;
    serde_json::to_writer_pretty(&mut temporary, snapshot)?;
    temporary.write_all(b"\n")?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    sync_directory(parent)?;
    Ok(())
}

fn quarantine(path: &Path, error: serde_json::Error) -> Result<StateError, StateError> {
    let timestamp = OffsetDateTime::now_utc().unix_timestamp();
    let quarantined = PathBuf::from(format!("{}.corrupt-{timestamp}", path.display()));
    fs::rename(path, &quarantined)?;
    Ok(StateError::CorruptSession {
        original: path.to_path_buf(),
        quarantined,
        message: error.to_string(),
    })
}

#[cfg(unix)]
fn set_private_file(file: &File) -> Result<(), StateError> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file(_file: &File) -> Result<(), StateError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), StateError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory(_path: &Path) -> Result<(), StateError> {
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), StateError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), StateError> {
    Ok(())
}
