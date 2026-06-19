use std::fmt;
use thiserror::Error;

/// Structured POSIX errno codes for FFI error handling.
///
/// Replaces the old `Err(format!("FFI errno: {} (code {})", name, code))`
/// pattern with a typed enum that callers can match on.
///
/// # Example
///
/// ```rust
/// match errno {
///     Errno::ENOENT => { /* file not found */ }
///     Errno::EACCES => { /* permission denied */ }
///     _ => { /* other */ }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Errno {
    // === POSIX error codes 1–96 (Linux) ===
    #[error("EPERM (code 1): Operation not permitted")]
    EPERM,
    #[error("ENOENT (code 2): No such file or directory")]
    ENOENT,
    #[error("ESRCH (code 3): No such process")]
    ESRCH,
    #[error("EINTR (code 4): Interrupted system call")]
    EINTR,
    #[error("EIO (code 5): I/O error")]
    EIO,
    #[error("ENXIO (code 6): No such device or address")]
    ENXIO,
    #[error("E2BIG (code 7): Argument list too long")]
    E2BIG,
    #[error("ENOEXEC (code 8): Exec format error")]
    ENOEXEC,
    #[error("EBADF (code 9): Bad file number")]
    EBADF,
    #[error("ECHILD (code 10): No child processes")]
    ECHILD,
    #[error("EAGAIN (code 11): Try again")]
    EAGAIN,
    #[error("ENOMEM (code 12): Out of memory")]
    ENOMEM,
    #[error("EACCES (code 13): Permission denied")]
    EACCES,
    #[error("EFAULT (code 14): Bad address")]
    EFAULT,
    #[error("ENOTBLK (code 15): Block device required")]
    ENOTBLK,
    #[error("EBUSY (code 16): Device or resource busy")]
    EBUSY,
    #[error("EEXIST (code 17): File exists")]
    EEXIST,
    #[error("EXDEV (code 18): Cross-device link")]
    EXDEV,
    #[error("ENODEV (code 19): No such device")]
    ENODEV,
    #[error("ENOTDIR (code 20): Not a directory")]
    ENOTDIR,
    #[error("EISDIR (code 21): Is a directory")]
    EISDIR,
    #[error("EINVAL (code 22): Invalid argument")]
    EINVAL,
    #[error("ENFILE (code 23): File table overflow")]
    ENFILE,
    #[error("EMFILE (code 24): Too many open files")]
    EMFILE,
    #[error("ENOTTY (code 25): Not a typewriter")]
    ENOTTY,
    #[error("ETXTBSY (code 26): Text file busy")]
    ETXTBSY,
    #[error("EFBIG (code 27): File too large")]
    EFBIG,
    #[error("ENOSPC (code 28): No space left on device")]
    ENOSPC,
    #[error("ESPIPE (code 29): Illegal seek")]
    ESPIPE,
    #[error("EROFS (code 30): Read-only file system")]
    EROFS,
    #[error("EMLINK (code 31): Too many links")]
    EMLINK,
    #[error("EPIPE (code 32): Broken pipe")]
    EPIPE,
    #[error("EDOM (code 33): Math argument out of domain of func")]
    EDOM,
    #[error("ERANGE (code 34): Math result not representable")]
    ERANGE,
    #[error("ENODATA (code 35): No data available")]
    ENODATA,
    #[error("ETIME (code 36): Timer expired")]
    ETIME,
    #[error("ENOSR (code 37): Out of streams resources")]
    ENOSR,
    #[error("ENOSTR (code 38): Device not a stream")]
    ENOSTR,
    #[error("ENOSYS (code 39): Function not implemented")]
    ENOSYS,
    #[error("ELOOP (code 40): Too many symbolic links")]
    ELOOP,
    #[error("ECANCELED (code 41): Operation canceled")]
    ECANCELED,
    #[error("EIDRM (code 42): Identifier removed")]
    EIDRM,
    #[error("ENOTSOCK (code 47): Socket operation on non-socket")]
    ENOTSOCK,
    #[error("EDESTADDRREQ (code 48): Destination address required")]
    EDESTADDRREQ,
    #[error("EMSGSIZE (code 49): Message too long")]
    EMSGSIZE,
    #[error("EPROTOTYPE (code 50): Protocol wrong type for socket")]
    EPROTOTYPE,
    #[error("ENOPROTOOPT (code 51): Protocol not available")]
    ENOPROTOOPT,
    #[error("EPROTONOSUPPORT (code 52): Protocol not supported")]
    EPROTONOSUPPORT,
    #[error("ESOCKTNOSUPPORT (code 53): Socket type not supported")]
    ESOCKTNOSUPPORT,
    #[error("EOPNOTSUPP (code 54): Operation not supported on transport endpoint")]
    EOPNOTSUPP,
    #[error("ENOTSUP (code 55): Operation not supported")]
    ENOTSUP,
    #[error("EPFNOSUPPORT (code 56): Protocol family not supported")]
    EPFNOSUPPORT,
    #[error("EAFNOSUPPORT (code 57): Address family not supported by protocol")]
    EAFNOSUPPORT,
    #[error("EADDRINUSE (code 58): Address already in use")]
    EADDRINUSE,
    #[error("EADDRNOTAVAIL (code 59): Cannot assign requested address")]
    EADDRNOTAVAIL,
    #[error("ENETDOWN (code 60): Network is down")]
    ENETDOWN,
    #[error("ENETUNREACH (code 61): Network is unreachable")]
    ENETUNREACH,
    #[error("ENETRESET (code 62): Network dropped connection because of reset")]
    ENETRESET,
    #[error("ECONNABORTED (code 63): Software caused connection abort")]
    ECONNABORTED,
    #[error("ECONNRESET (code 64): Connection reset by peer")]
    ECONNRESET,
    #[error("ENOBUFS (code 65): No buffer space available")]
    ENOBUFS,
    #[error("EISCONN (code 66): Transport endpoint is already connected")]
    EISCONN,
    #[error("ENOTCONN (code 67): Transport endpoint is not connected")]
    ENOTCONN,
    #[error("ESHUTDOWN (code 68): Cannot send after transport endpoint shutdown")]
    ESHUTDOWN,
    #[error("ETOOMANYREFS (code 69): Too many references: cannot splice")]
    ETOOMANYREFS,
    #[error("ETIMEDOUT (code 70): Connection timed out")]
    ETIMEDOUT,
    #[error("ECONNREFUSED (code 71): Connection refused")]
    ECONNREFUSED,
    #[error("EHOSTDOWN (code 72): Host is down")]
    EHOSTDOWN,
    #[error("EHOSTUNREACH (code 73): No route to host")]
    EHOSTUNREACH,
    #[error("EALREADY (code 74): Operation already in progress")]
    EALREADY,
    #[error("EINPROGRESS (code 75): Operation now in progress")]
    EINPROGRESS,
    #[error("ESTALE (code 76): Stale file handle")]
    ESTALE,
    #[error("EDQUOT (code 77): Quota exceeded")]
    EDQUOT,
    #[error("ENOMEDIUM (code 78): No medium found")]
    ENOMEDIUM,
    #[error("EMEDIUMTYPE (code 79): Wrong medium type")]
    EMEDIUMTYPE,
    #[error("ENOKEY (code 81): Required key not available")]
    ENOKEY,
    #[error("EKEYEXPIRED (code 82): Key has expired")]
    EKEYEXPIRED,
    #[error("EKEYREVOKED (code 83): Key has been revoked")]
    EKEYREVOKED,
    #[error("EKEYREJECTED (code 84): Key was rejected by service")]
    EKEYREJECTED,
    #[error("EOWNERDEAD (code 85): Owner died")]
    EOWNERDEAD,
    #[error("ENOTRECOVERABLE (code 86): State not recoverable")]
    ENOTRECOVERABLE,
    #[error("ERFKILL (code 87): Operation not possible due to RF-kill")]
    ERFKILL,
    #[error("EHWPOISON (code 88): Memory page has hardware error")]
    EHWPOISON,
    #[error("EUCLEAN (code 89): Structure needs cleaning")]
    EUCLEAN,
    #[error("ENOTNAM (code 90): Not a XENIX named type file")]
    ENOTNAM,
    #[error("ENAVAIL (code 91): No XENIX semaphores available")]
    ENAVAIL,
    #[error("EISNAM (code 92): Is a named type file")]
    EISNAM,
    #[error("EREMOTEIO (code 93): Remote I/O error")]
    EREMOTEIO,
    #[error("EDEADLK (code 94): Resource deadlock would occur")]
    EDEADLK,
    #[error("ENOLCK (code 95): No record locks available")]
    ENOLCK,
    #[error("ENOTEMPTY (code 96): Directory not empty")]
    ENOTEMPTY,

    /// Unknown errno code (not in the POSIX 1–96 range).
    #[error("Unknown errno (code {0})")]
    Unknown(i32),

    /// Unknown errno code with a human-readable name from `strerror`.
    #[error("Unknown errno (code {0}): {1}")]
    UnknownWithName(i32, String),

    /// Non-errno FFI wrapper error (argument validation, library loading, etc.).
    #[error("FFI wrapper: {0}")]
    Generic(String),
}

impl Errno {
    /// Map a raw POSIX errno integer to the corresponding `Errno` variant.
    pub fn from_code(code: i32) -> Self {
        match code {
            1 => Self::EPERM,
            2 => Self::ENOENT,
            3 => Self::ESRCH,
            4 => Self::EINTR,
            5 => Self::EIO,
            6 => Self::ENXIO,
            7 => Self::E2BIG,
            8 => Self::ENOEXEC,
            9 => Self::EBADF,
            10 => Self::ECHILD,
            11 => Self::EAGAIN,
            12 => Self::ENOMEM,
            13 => Self::EACCES,
            14 => Self::EFAULT,
            15 => Self::ENOTBLK,
            16 => Self::EBUSY,
            17 => Self::EEXIST,
            18 => Self::EXDEV,
            19 => Self::ENODEV,
            20 => Self::ENOTDIR,
            21 => Self::EISDIR,
            22 => Self::EINVAL,
            23 => Self::ENFILE,
            24 => Self::EMFILE,
            25 => Self::ENOTTY,
            26 => Self::ETXTBSY,
            27 => Self::EFBIG,
            28 => Self::ENOSPC,
            29 => Self::ESPIPE,
            30 => Self::EROFS,
            31 => Self::EMLINK,
            32 => Self::EPIPE,
            33 => Self::EDOM,
            34 => Self::ERANGE,
            35 => Self::ENODATA,
            36 => Self::ETIME,
            37 => Self::ENOSR,
            38 => Self::ENOSTR,
            39 => Self::ENOSYS,
            40 => Self::ELOOP,
            41 => Self::ECANCELED,
            42 => Self::EIDRM,
            47 => Self::ENOTSOCK,
            48 => Self::EDESTADDRREQ,
            49 => Self::EMSGSIZE,
            50 => Self::EPROTOTYPE,
            51 => Self::ENOPROTOOPT,
            52 => Self::EPROTONOSUPPORT,
            53 => Self::ESOCKTNOSUPPORT,
            54 => Self::EOPNOTSUPP,
            55 => Self::ENOTSUP,
            56 => Self::EPFNOSUPPORT,
            57 => Self::EAFNOSUPPORT,
            58 => Self::EADDRINUSE,
            59 => Self::EADDRNOTAVAIL,
            60 => Self::ENETDOWN,
            61 => Self::ENETUNREACH,
            62 => Self::ENETRESET,
            63 => Self::ECONNABORTED,
            64 => Self::ECONNRESET,
            65 => Self::ENOBUFS,
            66 => Self::EISCONN,
            67 => Self::ENOTCONN,
            68 => Self::ESHUTDOWN,
            69 => Self::ETOOMANYREFS,
            70 => Self::ETIMEDOUT,
            71 => Self::ECONNREFUSED,
            72 => Self::EHOSTDOWN,
            73 => Self::EHOSTUNREACH,
            74 => Self::EALREADY,
            75 => Self::EINPROGRESS,
            76 => Self::ESTALE,
            77 => Self::EDQUOT,
            78 => Self::ENOMEDIUM,
            79 => Self::EMEDIUMTYPE,
            81 => Self::ENOKEY,
            82 => Self::EKEYEXPIRED,
            83 => Self::EKEYREVOKED,
            84 => Self::EKEYREJECTED,
            85 => Self::EOWNERDEAD,
            86 => Self::ENOTRECOVERABLE,
            87 => Self::ERFKILL,
            88 => Self::EHWPOISON,
            89 => Self::EUCLEAN,
            90 => Self::ENOTNAM,
            91 => Self::ENAVAIL,
            92 => Self::EISNAM,
            93 => Self::EREMOTEIO,
            94 => Self::EDEADLK,
            95 => Self::ENOLCK,
            96 => Self::ENOTEMPTY,
            _ => {
                let name = unsafe {
                    let c_str = libc::strerror(code);
                    if !c_str.is_null() {
                        std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned()
                    } else {
                        format!("Unknown (code {})", code)
                    }
                };
                Self::UnknownWithName(code, name)
            }
        }
    }

    /// Return the numeric POSIX errno code, or 0 for `Generic`.
    pub fn code(&self) -> i32 {
        match self {
            Self::EPERM => 1,
            Self::ENOENT => 2,
            Self::ESRCH => 3,
            Self::EINTR => 4,
            Self::EIO => 5,
            Self::ENXIO => 6,
            Self::E2BIG => 7,
            Self::ENOEXEC => 8,
            Self::EBADF => 9,
            Self::ECHILD => 10,
            Self::EAGAIN => 11,
            Self::ENOMEM => 12,
            Self::EACCES => 13,
            Self::EFAULT => 14,
            Self::ENOTBLK => 15,
            Self::EBUSY => 16,
            Self::EEXIST => 17,
            Self::EXDEV => 18,
            Self::ENODEV => 19,
            Self::ENOTDIR => 20,
            Self::EISDIR => 21,
            Self::EINVAL => 22,
            Self::ENFILE => 23,
            Self::EMFILE => 24,
            Self::ENOTTY => 25,
            Self::ETXTBSY => 26,
            Self::EFBIG => 27,
            Self::ENOSPC => 28,
            Self::ESPIPE => 29,
            Self::EROFS => 30,
            Self::EMLINK => 31,
            Self::EPIPE => 32,
            Self::EDOM => 33,
            Self::ERANGE => 34,
            Self::ENODATA => 35,
            Self::ETIME => 36,
            Self::ENOSR => 37,
            Self::ENOSTR => 38,
            Self::ENOSYS => 39,
            Self::ELOOP => 40,
            Self::ECANCELED => 41,
            Self::EIDRM => 42,
            Self::ENOTSOCK => 47,
            Self::EDESTADDRREQ => 48,
            Self::EMSGSIZE => 49,
            Self::EPROTOTYPE => 50,
            Self::ENOPROTOOPT => 51,
            Self::EPROTONOSUPPORT => 52,
            Self::ESOCKTNOSUPPORT => 53,
            Self::EOPNOTSUPP => 54,
            Self::ENOTSUP => 55,
            Self::EPFNOSUPPORT => 56,
            Self::EAFNOSUPPORT => 57,
            Self::EADDRINUSE => 58,
            Self::EADDRNOTAVAIL => 59,
            Self::ENETDOWN => 60,
            Self::ENETUNREACH => 61,
            Self::ENETRESET => 62,
            Self::ECONNABORTED => 63,
            Self::ECONNRESET => 64,
            Self::ENOBUFS => 65,
            Self::EISCONN => 66,
            Self::ENOTCONN => 67,
            Self::ESHUTDOWN => 68,
            Self::ETOOMANYREFS => 69,
            Self::ETIMEDOUT => 70,
            Self::ECONNREFUSED => 71,
            Self::EHOSTDOWN => 72,
            Self::EHOSTUNREACH => 73,
            Self::EALREADY => 74,
            Self::EINPROGRESS => 75,
            Self::ESTALE => 76,
            Self::EDQUOT => 77,
            Self::ENOMEDIUM => 78,
            Self::EMEDIUMTYPE => 79,
            Self::ENOKEY => 81,
            Self::EKEYEXPIRED => 82,
            Self::EKEYREVOKED => 83,
            Self::EKEYREJECTED => 84,
            Self::EOWNERDEAD => 85,
            Self::ENOTRECOVERABLE => 86,
            Self::ERFKILL => 87,
            Self::EHWPOISON => 88,
            Self::EUCLEAN => 89,
            Self::ENOTNAM => 90,
            Self::ENAVAIL => 91,
            Self::EISNAM => 92,
            Self::EREMOTEIO => 93,
            Self::EDEADLK => 94,
            Self::ENOLCK => 95,
            Self::ENOTEMPTY => 96,
            Self::Unknown(c) | Self::UnknownWithName(c, _) => *c,
            Self::Generic(_) => 0,
        }
    }

    /// Returns `true` if this is a POSIX errno variant (not `Generic` or `Unknown`).
    pub fn is_posix_errno(&self) -> bool {
        !matches!(self, Self::Generic(_) | Self::Unknown(_) | Self::UnknownWithName(_, _))
    }
}

impl From<String> for Errno {
    fn from(msg: String) -> Self {
        Self::Generic(msg)
    }
}

impl From<&str> for Errno {
    fn from(msg: &str) -> Self {
        Self::Generic(msg.to_string())
    }
}
